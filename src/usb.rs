// USB transport layer for communicating with the Faderpunk.
//
// Protocol: postcard-serialized messages, framed with COBS encoding.
// Wire format: [2-byte big-endian payload length] [postcard payload] → COBS encode → [0x00 delimiter]

use anyhow::{Context, Result, bail};
use nusb::Interface;
use nusb::transfer::RequestBuffer;

use crate::protocol::{ConfigMsgIn, ConfigMsgOut};

const FADERPUNK_VID: u16 = 0xf569;
const FADERPUNK_PID: u16 = 0x0001;
const USB_CLASS_VENDOR: u8 = 0xff;
const USB_TRANSFER_SIZE: usize = 512;
const FRAME_DELIMITER: u8 = 0x00;

/// Represents a connected Faderpunk device.
pub struct FaderpunkDevice {
    iface: Interface,
    recv_buf: Vec<u8>,
}

impl FaderpunkDevice {
    /// Find and connect to a Faderpunk device.
    pub fn open() -> Result<Self> {
        let device_info = nusb::list_devices()?
            .find(|d| d.vendor_id() == FADERPUNK_VID && d.product_id() == FADERPUNK_PID)
            .context("Faderpunk not found — is it connected via USB?")?;

        let device = device_info.open()?;

        // Find the vendor-class interface (0xff)
        let config = device.active_configuration()?;
        let iface_num = config
            .interfaces()
            .find(|i| {
                i.alt_settings()
                    .any(|a| a.class() == USB_CLASS_VENDOR)
            })
            .context("No WebUSB interface found on device")?
            .interface_number();

        let iface = device.claim_interface(iface_num)?;

        Ok(FaderpunkDevice {
            iface,
            recv_buf: Vec::new(),
        })
    }

    /// Send a message to the device.
    pub async fn send(&self, msg: &ConfigMsgIn) -> Result<()> {
        let serialized =
            postcard::to_allocvec(msg).context("Failed to serialize message")?;

        // Prepend 2-byte big-endian length
        let payload_len = serialized.len();
        let mut with_len = Vec::with_capacity(payload_len + 2);
        with_len.push(((payload_len >> 8) & 0xFF) as u8);
        with_len.push((payload_len & 0xFF) as u8);
        with_len.extend_from_slice(&serialized);

        // COBS encode
        let mut cobs_buf = vec![0u8; with_len.len() + with_len.len() / 254 + 2];
        let cobs_len = cobs::try_encode(&with_len, &mut cobs_buf)
            .map_err(|_| anyhow::anyhow!("COBS encoding failed"))?;

        // Append frame delimiter
        let mut frame = Vec::with_capacity(cobs_len + 1);
        frame.extend_from_slice(&cobs_buf[..cobs_len]);
        frame.push(FRAME_DELIMITER);

        // Find the bulk OUT endpoint
        let ep_out = self
            .iface
            .descriptors()
            .next()
            .context("No alt setting")?
            .endpoints()
            .find(|e| e.direction() == nusb::transfer::Direction::Out)
            .context("No OUT endpoint found")?
            .address();

        // Send in 64-byte chunks (USB max packet size)
        for chunk in frame.chunks(64) {
            self.iface.bulk_out(ep_out, chunk.to_vec()).await.into_result()?;
        }

        Ok(())
    }

    /// Receive a single message from the device.
    pub async fn receive(&mut self) -> Result<ConfigMsgOut> {
        let ep_in = self
            .iface
            .descriptors()
            .next()
            .context("No alt setting")?
            .endpoints()
            .find(|e| e.direction() == nusb::transfer::Direction::In)
            .context("No IN endpoint found")?
            .address();

        loop {
            // Check if we already have a complete frame in the buffer
            if let Some(delim_pos) = self.recv_buf.iter().position(|&b| b == FRAME_DELIMITER) {
                let packet: Vec<u8> = self.recv_buf.drain(..=delim_pos).collect();
                let frame = &packet[..packet.len() - 1]; // strip delimiter

                if frame.is_empty() {
                    continue;
                }

                // COBS decode
                let mut decode_buf = frame.to_vec();
                let decoded_len = cobs::decode_in_place(&mut decode_buf)
                    .map_err(|_| anyhow::anyhow!("COBS decode failed"))?;

                if decoded_len < 2 {
                    bail!("Corrupted message (too short after COBS decode)");
                }

                // Skip the 2-byte length prefix, deserialize the rest
                let msg: ConfigMsgOut = postcard::from_bytes(&decode_buf[2..decoded_len])
                    .context("Failed to deserialize device response")?;

                return Ok(msg);
            }

            // Need more data from USB
            let data = self.iface.bulk_in(ep_in, RequestBuffer::new(USB_TRANSFER_SIZE)).await.into_result()?;
            self.recv_buf.extend_from_slice(&data);
        }
    }

    /// Send a message and receive the response.
    pub async fn send_receive(&mut self, msg: &ConfigMsgIn) -> Result<ConfigMsgOut> {
        self.send(msg).await?;
        self.receive().await
    }

    /// Send a message that triggers a batch response, collect all messages.
    pub async fn send_receive_batch(&mut self, msg: &ConfigMsgIn) -> Result<Vec<ConfigMsgOut>> {
        self.send(msg).await?;

        // First response should be BatchMsgStart(count)
        let start = self.receive().await?;
        let count = match start {
            ConfigMsgOut::BatchMsgStart(n) => n,
            other => bail!("Expected BatchMsgStart, got: {:?}", other),
        };

        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            results.push(self.receive().await?);
        }

        // Expect BatchMsgEnd
        let end = self.receive().await?;
        if !matches!(end, ConfigMsgOut::BatchMsgEnd) {
            bail!("Expected BatchMsgEnd, got: {:?}", end);
        }

        Ok(results)
    }
}
