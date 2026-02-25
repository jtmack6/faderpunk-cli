# faderpunk-cli

CLI tool for configuring the [Faderpunk](https://faderpunk.com) controller over USB.

This is an alternative to the [web configurator](https://github.com/ATOVproject/faderpunk) — same protocol, same capabilities, from the terminal. Read device state, change settings, swap app layouts, and save/load configuration presets as JSON files.

## Install

Requires [Rust](https://rustup.rs/) (stable).

```bash
cargo install --path .
```

Or build and run directly:

```bash
cargo run -- <command>
```

## Usage

Connect your Faderpunk via USB, then:

### Check connection

```bash
faderpunk-cli ping
# Faderpunk is connected!
```

### View device state

```bash
faderpunk-cli status      # global config + layout summary
faderpunk-cli apps        # list all available apps on the device
faderpunk-cli layout      # show which app is assigned to each fader
faderpunk-cli params      # show current parameters for all running apps
```

### Change settings

```bash
faderpunk-cli config show                # view full global config
faderpunk-cli config bpm 140             # set internal clock BPM
faderpunk-cli config brightness 200      # set LED brightness (100-255)
faderpunk-cli config takeover jump       # set fader takeover mode (pickup, jump, scale)
```

### Save and load presets

```bash
# Save current config to a JSON file
faderpunk-cli save my-preset.json

# Load and apply a saved config
faderpunk-cli load my-preset.json
```

Preset files contain the global config and layout in human-readable JSON, so you can edit them by hand or keep them in version control.

## How it works

The Faderpunk exposes a vendor-class USB interface that speaks the same protocol as the web configurator:

- **Serialization**: [postcard](https://docs.rs/postcard) (compact binary format)
- **Framing**: [COBS](https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing) encoding with `0x00` delimiter
- **Wire format**: `[2-byte big-endian payload length][postcard payload]` -> COBS encode -> `[0x00]`

The protocol types in `src/protocol.rs` mirror the firmware's `libfp` crate. They must stay in sync with the firmware — same enum variant order, same field order, same types.

## Terminal output

The CLI uses truecolor output matching the Faderpunk's actual LED colors, with:

- Colored fader strip visualization showing app layout with box drawing
- Unicode icons for each app type (↕ Fader, ∿ Sine, ♪ Note, ⚄ Die, ≈ Random, etc.)
- Formatted values for MIDI (CC/channel/ports), voltage ranges, and booleans
- Section headers and dimmed labels for clean, scannable output

Requires a terminal with truecolor support (iTerm2, kitty, WezTerm, Windows Terminal, etc.).

## Project structure

```
src/
├── main.rs       # CLI entry point (clap commands)
├── protocol.rs   # Protocol types mirroring libfp
├── usb.rs        # USB transport (nusb + COBS framing)
└── display.rs    # Colored terminal output and fader visualization
```

## Requirements

- Faderpunk firmware v1.8.0+
- macOS, Linux, or Windows
- USB connection (not Bluetooth)

## Related

- [Faderpunk firmware + web configurator](https://github.com/ATOVproject/faderpunk)
- [faderpunk.com](https://faderpunk.com)

## License

MIT
