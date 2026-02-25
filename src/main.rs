mod display;
mod protocol;
mod usb;

use anyhow::Result;
use clap::{Parser, Subcommand};

use protocol::{ConfigMsgIn, ConfigMsgOut, Param, Value, APP_MAX_PARAMS, GLOBAL_CHANNELS};
use usb::FaderpunkDevice;

#[derive(Parser)]
#[command(name = "fp", about = "CLI tool for the Faderpunk controller")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if the Faderpunk is connected
    Ping,

    /// Show current device configuration
    Status,

    /// List available apps on the device
    Apps,

    /// View or modify the fader layout
    Layout {
        #[command(subcommand)]
        action: Option<LayoutAction>,
    },

    /// View or set app parameters
    Param {
        #[command(subcommand)]
        action: Option<ParamAction>,
    },

    /// Get or set global configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Save current device config to a JSON file
    Save {
        /// Output file path
        path: String,
    },

    /// Load a config from a JSON file and apply it to the device
    Load {
        /// Input file path
        path: String,
    },
}

#[derive(Subcommand)]
enum LayoutAction {
    /// Show the current layout (default)
    Show,

    /// Assign an app to a fader slot (1-16)
    Set {
        /// Fader slot number (1-16)
        slot: u8,
        /// App name or ID (use 'apps' command to see available)
        app: String,
    },

    /// Remove an app from a fader slot
    Remove {
        /// Fader slot number (1-16)
        slot: u8,
    },

    /// Clear the entire layout
    Clear,

    /// Fill all 16 faders with a single app
    Fill {
        /// App name or ID
        app: String,
    },
}

#[derive(Subcommand)]
enum ParamAction {
    /// Show parameters for all apps (default)
    Show {
        /// Optional: fader slot to show (1-16)
        slot: Option<u8>,
    },

    /// Set a parameter value
    Set {
        /// Fader slot number (1-16)
        slot: u8,
        /// Parameter name or index (0-based)
        param: String,
        /// Value to set
        value: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show full global config
    Show,

    /// Set the BPM
    Bpm {
        /// BPM value (e.g. 120.0)
        value: f32,
    },

    /// Set LED brightness (100-255)
    Brightness {
        /// Brightness value
        value: u8,
    },

    /// Set takeover mode (pickup, jump, scale)
    Takeover {
        /// Mode name
        mode: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ping => cmd_ping().await,
        Commands::Status => cmd_status().await,
        Commands::Apps => cmd_apps().await,
        Commands::Layout { action } => cmd_layout(action).await,
        Commands::Param { action } => cmd_param(action).await,
        Commands::Config { action } => cmd_config(action).await,
        Commands::Save { path } => cmd_save(&path).await,
        Commands::Load { path } => cmd_load(&path).await,
    }
}

async fn cmd_ping() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let response = dev.send_receive(&ConfigMsgIn::Ping).await?;

    match response {
        ConfigMsgOut::Pong => println!("Faderpunk is connected!"),
        other => println!("Unexpected response: {:?}", other),
    }
    Ok(())
}

async fn cmd_status() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;

    let config_resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
    if let ConfigMsgOut::GlobalConfig(config) = config_resp {
        display::print_global_config(&config);
    }

    println!();

    let app_info = fetch_app_info(&mut dev).await?;

    let layout_resp = dev.send_receive(&ConfigMsgIn::GetLayout).await?;
    if let ConfigMsgOut::Layout(layout) = layout_resp {
        display::print_layout(&layout, Some(&app_info));
    }

    Ok(())
}

// ── Helpers ──

/// Fetch app metadata from device.
async fn fetch_app_info(dev: &mut FaderpunkDevice) -> Result<Vec<display::AppInfo>> {
    let responses = dev.send_receive_batch(&ConfigMsgIn::GetAllApps).await?;
    let mut info = Vec::new();
    for resp in responses {
        if let ConfigMsgOut::AppConfig(app_id, channels, (_, name, _, color, icon, params)) = resp {
            info.push(display::AppInfo {
                app_id,
                channels,
                name,
                color,
                icon,
                params,
            });
        }
    }
    Ok(info)
}

/// Build layout entries from a Layout for cross-referencing.
fn layout_entries(layout: &protocol::Layout) -> Vec<display::LayoutEntry> {
    layout
        .0
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| {
            slot.map(|(app_id, channels, layout_id)| display::LayoutEntry {
                start: i,
                size: channels,
                app_id,
                layout_id,
            })
        })
        .collect()
}

/// Resolve an app name or ID string to (app_id, channels).
fn resolve_app(input: &str, apps: &[display::AppInfo]) -> Result<(u8, usize)> {
    // Try as numeric ID first
    if let Ok(id) = input.parse::<u8>() {
        if let Some(app) = apps.iter().find(|a| a.app_id == id) {
            return Ok((app.app_id, app.channels));
        }
        anyhow::bail!("No app with ID {}. Use 'apps' to see available.", id);
    }

    // Try case-insensitive name match
    let lower = input.to_lowercase();
    let matches: Vec<_> = apps
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&lower))
        .collect();

    match matches.len() {
        0 => anyhow::bail!("No app matching '{}'. Use 'apps' to see available.", input),
        1 => Ok((matches[0].app_id, matches[0].channels)),
        _ => {
            let names: Vec<_> = matches.iter().map(|a| format!("{} [{}]", a.name, a.app_id)).collect();
            anyhow::bail!(
                "Ambiguous app name '{}'. Matches: {}. Use the app ID instead.",
                input,
                names.join(", ")
            );
        }
    }
}

/// Find the layout entry at a given fader slot (1-based).
fn find_entry_at_slot(entries: &[display::LayoutEntry], slot: u8) -> Option<&display::LayoutEntry> {
    let idx = slot as usize - 1;
    entries.iter().find(|e| idx >= e.start && idx < e.start + e.size)
}

/// Get the current layout from device.
async fn fetch_layout(dev: &mut FaderpunkDevice) -> Result<protocol::Layout> {
    let resp = dev.send_receive(&ConfigMsgIn::GetLayout).await?;
    match resp {
        ConfigMsgOut::Layout(layout) => Ok(layout),
        _ => anyhow::bail!("Unexpected response for Layout"),
    }
}

/// Send a layout to device and return the validated layout.
async fn send_layout(dev: &mut FaderpunkDevice, layout: protocol::Layout) -> Result<protocol::Layout> {
    let resp = dev.send_receive(&ConfigMsgIn::SetLayout(layout)).await?;
    match resp {
        ConfigMsgOut::Layout(validated) => Ok(validated),
        _ => anyhow::bail!("Unexpected response for SetLayout"),
    }
}

fn validate_slot(slot: u8) -> Result<()> {
    if slot < 1 || slot > 16 {
        anyhow::bail!("Slot must be 1-16, got {}", slot);
    }
    Ok(())
}

// ── Apps ──

async fn cmd_apps() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let responses = dev.send_receive_batch(&ConfigMsgIn::GetAllApps).await?;

    let mut apps = Vec::new();
    for resp in responses {
        if let ConfigMsgOut::AppConfig(app_id, channels, (_, name, desc, color, icon, _)) = resp {
            apps.push((app_id, channels, name, desc, color, icon));
        }
    }

    display::print_app_list(&apps);
    Ok(())
}

// ── Layout ──

async fn cmd_layout(action: Option<LayoutAction>) -> Result<()> {
    match action.unwrap_or(LayoutAction::Show) {
        LayoutAction::Show => layout_show().await,
        LayoutAction::Set { slot, app } => layout_set(slot, &app).await,
        LayoutAction::Remove { slot } => layout_remove(slot).await,
        LayoutAction::Clear => layout_clear().await,
        LayoutAction::Fill { app } => layout_fill(&app).await,
    }
}

async fn layout_show() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let layout = fetch_layout(&mut dev).await?;
    display::print_layout(&layout, Some(&app_info));
    Ok(())
}

async fn layout_set(slot: u8, app_name: &str) -> Result<()> {
    validate_slot(slot)?;
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let (app_id, channels) = resolve_app(app_name, &app_info)?;

    let idx = slot as usize - 1;
    let end = idx + channels;
    if end > GLOBAL_CHANNELS {
        anyhow::bail!(
            "App '{}' needs {} fader(s), won't fit at slot {} (only {} slots remaining)",
            app_name,
            channels,
            slot,
            GLOBAL_CHANNELS - idx
        );
    }

    let mut layout = fetch_layout(&mut dev).await?;

    // Clear any existing apps that overlap with the new placement
    for i in 0..GLOBAL_CHANNELS {
        if let Some((_, ch, _)) = layout.0[i] {
            let app_end = i + ch;
            // If this existing app overlaps with our target range, remove it
            if i < end && app_end > idx {
                layout.0[i] = None;
            }
        }
    }

    // Find next available layout_id
    let used_ids: Vec<u8> = layout
        .0
        .iter()
        .filter_map(|s| s.map(|(_, _, lid)| lid))
        .collect();
    let layout_id = (0..GLOBAL_CHANNELS as u8)
        .find(|id| !used_ids.contains(id))
        .unwrap_or(0);

    // Place the app
    layout.0[idx] = Some((app_id, channels, layout_id));

    let validated = send_layout(&mut dev, layout).await?;

    let app = app_info.iter().find(|a| a.app_id == app_id).unwrap();
    println!(
        "Placed {} at fader{} {}",
        app.name,
        if channels > 1 { "s" } else { "" },
        if channels > 1 {
            format!("{}-{}", slot, slot as usize + channels - 1)
        } else {
            format!("{}", slot)
        }
    );
    println!();
    display::print_layout(&validated, Some(&app_info));

    Ok(())
}

async fn layout_remove(slot: u8) -> Result<()> {
    validate_slot(slot)?;
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let mut layout = fetch_layout(&mut dev).await?;
    let entries = layout_entries(&layout);

    if let Some(entry) = find_entry_at_slot(&entries, slot) {
        let name = app_info
            .iter()
            .find(|a| a.app_id == entry.app_id)
            .map(|a| a.name.as_str())
            .unwrap_or("unknown");
        layout.0[entry.start] = None;
        let validated = send_layout(&mut dev, layout).await?;
        println!("Removed {} from fader {}", name, slot);
        println!();
        display::print_layout(&validated, Some(&app_info));
    } else {
        println!("Fader {} is already empty", slot);
    }

    Ok(())
}

async fn layout_clear() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let layout = protocol::Layout([None; GLOBAL_CHANNELS]);
    send_layout(&mut dev, layout).await?;
    println!("Layout cleared — all faders empty");
    Ok(())
}

async fn layout_fill(app_name: &str) -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let (app_id, channels) = resolve_app(app_name, &app_info)?;

    let mut layout = protocol::Layout([None; GLOBAL_CHANNELS]);
    let mut pos = 0usize;
    let mut layout_id = 0u8;

    while pos + channels <= GLOBAL_CHANNELS {
        layout.0[pos] = Some((app_id, channels, layout_id));
        pos += channels;
        layout_id += 1;
    }

    let validated = send_layout(&mut dev, layout).await?;

    let app = app_info.iter().find(|a| a.app_id == app_id).unwrap();
    let count = GLOBAL_CHANNELS / channels;
    println!(
        "Filled layout with {} x {} ({} ch each)",
        count, app.name, channels
    );
    println!();
    display::print_layout(&validated, Some(&app_info));

    Ok(())
}

// ── Params ──

async fn cmd_param(action: Option<ParamAction>) -> Result<()> {
    match action.unwrap_or(ParamAction::Show { slot: None }) {
        ParamAction::Show { slot } => param_show(slot).await,
        ParamAction::Set { slot, param, value } => param_set(slot, &param, &value).await,
    }
}

async fn param_show(slot: Option<u8>) -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let layout = fetch_layout(&mut dev).await?;
    let entries = layout_entries(&layout);

    if let Some(slot) = slot {
        validate_slot(slot)?;
        let entry = find_entry_at_slot(&entries, slot)
            .ok_or_else(|| anyhow::anyhow!("No app at fader {}", slot))?;

        let resp = dev
            .send_receive(&ConfigMsgIn::GetAppParams {
                layout_id: entry.layout_id,
            })
            .await?;
        if let ConfigMsgOut::AppState(layout_id, values) = resp {
            display::print_app_params(layout_id, &values, Some(&entries), Some(&app_info));
        }
    } else {
        let responses = dev.send_receive_batch(&ConfigMsgIn::GetAllAppParams).await?;
        for resp in responses {
            if let ConfigMsgOut::AppState(layout_id, values) = resp {
                display::print_app_params(layout_id, &values, Some(&entries), Some(&app_info));
            }
        }
    }

    Ok(())
}

async fn param_set(slot: u8, param_ref: &str, value_str: &str) -> Result<()> {
    validate_slot(slot)?;
    let mut dev = FaderpunkDevice::open()?;
    let app_info = fetch_app_info(&mut dev).await?;
    let layout = fetch_layout(&mut dev).await?;
    let entries = layout_entries(&layout);

    let entry = find_entry_at_slot(&entries, slot)
        .ok_or_else(|| anyhow::anyhow!("No app at fader {}", slot))?;

    // Get current params to know the types
    let resp = dev
        .send_receive(&ConfigMsgIn::GetAppParams {
            layout_id: entry.layout_id,
        })
        .await?;
    let current_values = match resp {
        ConfigMsgOut::AppState(_, values) => values,
        _ => anyhow::bail!("Unexpected response"),
    };

    // Get param metadata for this app
    let app = app_info
        .iter()
        .find(|a| a.app_id == entry.app_id)
        .ok_or_else(|| anyhow::anyhow!("App metadata not found"))?;

    // Resolve param reference — by index or by name
    let param_idx = if let Ok(idx) = param_ref.parse::<usize>() {
        if idx >= current_values.len() {
            anyhow::bail!(
                "Param index {} out of range (app has {} params)",
                idx,
                current_values.len()
            );
        }
        idx
    } else {
        // Search by name (case-insensitive)
        let lower = param_ref.to_lowercase();
        let found: Vec<(usize, &Param)> = app
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let name = display::get_param_name(p);
                !name.is_empty() && name.to_lowercase().contains(&lower)
            })
            .collect();

        match found.len() {
            0 => anyhow::bail!(
                "No param matching '{}'. Use 'param show {}' to see available.",
                param_ref,
                slot
            ),
            1 => found[0].0,
            _ => {
                let names: Vec<_> = found
                    .iter()
                    .map(|(i, p)| format!("{} [{}]", display::get_param_name(p), i))
                    .collect();
                anyhow::bail!(
                    "Ambiguous param '{}'. Matches: {}. Use the index instead.",
                    param_ref,
                    names.join(", ")
                );
            }
        }
    };

    let param_meta = app.params.get(param_idx);
    let new_value = parse_value(value_str, param_meta, &current_values[param_idx])?;

    // Build the SetAppParams message — None for all params except the one we're changing
    let mut values: [Option<Value>; APP_MAX_PARAMS] = [None; APP_MAX_PARAMS];
    // Send all current values (firmware replaces all at once)
    for (i, v) in current_values.iter().enumerate() {
        if i < APP_MAX_PARAMS {
            values[i] = Some(*v);
        }
    }
    values[param_idx] = Some(new_value);

    let resp = dev
        .send_receive(&ConfigMsgIn::SetAppParams {
            layout_id: entry.layout_id,
            values,
        })
        .await?;

    let param_name = param_meta
        .map(|p| display::get_param_name(p))
        .unwrap_or_default();
    let label = if param_name.is_empty() {
        format!("param {}", param_idx)
    } else {
        param_name
    };

    println!("Set {} = {}", label, value_str);

    // Show updated params
    if let ConfigMsgOut::AppState(layout_id, values) = resp {
        println!();
        display::print_app_params(layout_id, &values, Some(&entries), Some(&app_info));
    }

    Ok(())
}

/// Parse a string value into the appropriate Value type based on param metadata.
fn parse_value(s: &str, param: Option<&Param>, current: &Value) -> Result<Value> {
    // Use param metadata if available, otherwise infer from current value type
    match param {
        Some(Param::Int { min, max, .. }) => {
            let v: i32 = s.parse().map_err(|_| anyhow::anyhow!("Expected integer"))?;
            if v < *min || v > *max {
                anyhow::bail!("Value {} out of range ({}-{})", v, min, max);
            }
            Ok(Value::Int(v))
        }
        Some(Param::Float { min, max, .. }) => {
            let v: f32 = s.parse().map_err(|_| anyhow::anyhow!("Expected number"))?;
            if v < *min || v > *max {
                anyhow::bail!("Value {} out of range ({}-{})", v, min, max);
            }
            Ok(Value::Float(v))
        }
        Some(Param::Bool { .. }) => {
            let v = match s.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => anyhow::bail!("Expected bool (true/false, on/off, 1/0)"),
            };
            Ok(Value::Bool(v))
        }
        Some(Param::Enum { variants, .. }) => {
            // Try by index first
            if let Ok(idx) = s.parse::<usize>() {
                if idx >= variants.len() {
                    anyhow::bail!("Index {} out of range (0-{})", idx, variants.len() - 1);
                }
                return Ok(Value::Enum(idx));
            }
            // Try by name
            let lower = s.to_lowercase();
            let found: Vec<(usize, _)> = variants
                .iter()
                .enumerate()
                .filter(|(_, v)| v.to_lowercase().contains(&lower))
                .collect();
            match found.len() {
                0 => anyhow::bail!("No variant matching '{}'. Options: {}", s, variants.join(", ")),
                1 => Ok(Value::Enum(found[0].0)),
                _ => {
                    let names: Vec<_> = found.iter().map(|(i, v)| format!("{} [{}]", v, i)).collect();
                    anyhow::bail!("Ambiguous '{}'. Matches: {}", s, names.join(", "));
                }
            }
        }
        Some(Param::Curve { variants, .. }) => {
            let lower = s.to_lowercase();
            for v in variants {
                if format!("{:?}", v).to_lowercase() == lower {
                    return Ok(Value::Curve(*v));
                }
            }
            let options: Vec<_> = variants.iter().map(|v| format!("{:?}", v)).collect();
            anyhow::bail!("Unknown curve '{}'. Options: {}", s, options.join(", "))
        }
        Some(Param::Waveform { variants, .. }) => {
            let lower = s.to_lowercase();
            for v in variants {
                if format!("{:?}", v).to_lowercase() == lower {
                    return Ok(Value::Waveform(*v));
                }
            }
            let options: Vec<_> = variants.iter().map(|v| format!("{:?}", v)).collect();
            anyhow::bail!("Unknown waveform '{}'. Options: {}", s, options.join(", "))
        }
        Some(Param::Range { variants, .. }) => {
            let v = parse_range(s, variants)?;
            Ok(Value::Range(v))
        }
        Some(Param::MidiCc { .. }) => {
            let v: u8 = s.parse().map_err(|_| anyhow::anyhow!("Expected 0-127"))?;
            if v > 127 {
                anyhow::bail!("CC must be 0-127");
            }
            Ok(Value::MidiCc(protocol::MidiCc(v)))
        }
        Some(Param::MidiChannel { .. }) => {
            let v: u8 = s.parse().map_err(|_| anyhow::anyhow!("Expected 1-16"))?;
            if v < 1 || v > 16 {
                anyhow::bail!("Channel must be 1-16");
            }
            Ok(Value::MidiChannel(protocol::MidiChannel(v)))
        }
        Some(Param::MidiNote { .. }) => {
            let v: u8 = s.parse().map_err(|_| anyhow::anyhow!("Expected 0-127"))?;
            if v > 127 {
                anyhow::bail!("Note must be 0-127");
            }
            Ok(Value::MidiNote(protocol::MidiNote(v)))
        }
        Some(Param::MidiMode) => {
            let v = match s.to_lowercase().as_str() {
                "note" => protocol::MidiMode::Note,
                "cc" => protocol::MidiMode::Cc,
                _ => anyhow::bail!("Expected 'note' or 'cc'"),
            };
            Ok(Value::MidiMode(v))
        }
        Some(Param::MidiIn) => {
            let (usb, din) = parse_midi_ports_in(s)?;
            Ok(Value::MidiIn(protocol::MidiIn([usb, din])))
        }
        Some(Param::MidiOut) => {
            let (usb, out1, out2) = parse_midi_ports_out(s)?;
            Ok(Value::MidiOut(protocol::MidiOut([usb, out1, out2])))
        }
        Some(Param::Color { variants, .. }) => {
            let lower = s.to_lowercase();
            for v in variants {
                if format!("{:?}", v).to_lowercase() == lower {
                    return Ok(Value::Color(*v));
                }
            }
            let options: Vec<_> = variants.iter().map(|v| format!("{:?}", v)).collect();
            anyhow::bail!("Unknown color '{}'. Options: {}", s, options.join(", "))
        }
        Some(Param::Note { variants, .. }) => {
            let lower = s.to_lowercase();
            for v in variants {
                if format!("{:?}", v).to_lowercase() == lower {
                    return Ok(Value::Note(*v));
                }
            }
            let options: Vec<_> = variants.iter().map(|v| format!("{:?}", v)).collect();
            anyhow::bail!("Unknown note '{}'. Options: {}", s, options.join(", "))
        }
        Some(Param::None) | None => {
            // Infer from current value type
            match current {
                Value::Int(_) => Ok(Value::Int(s.parse()?)),
                Value::Float(_) => Ok(Value::Float(s.parse()?)),
                Value::Bool(_) => {
                    let v = matches!(s.to_lowercase().as_str(), "true" | "on" | "1" | "yes");
                    Ok(Value::Bool(v))
                }
                Value::Enum(_) => Ok(Value::Enum(s.parse()?)),
                Value::MidiCc(_) => Ok(Value::MidiCc(protocol::MidiCc(s.parse()?))),
                Value::MidiChannel(_) => Ok(Value::MidiChannel(protocol::MidiChannel(s.parse()?))),
                _ => anyhow::bail!("Can't infer type for this parameter. Specify by index."),
            }
        }
    }
}

fn parse_range(s: &str, variants: &[protocol::Range]) -> Result<protocol::Range> {
    let lower = s.to_lowercase().replace(' ', "");
    for v in variants {
        let label = match v {
            protocol::Range::_0_10V => "0-10v",
            protocol::Range::_0_5V => "0-5v",
            protocol::Range::_Neg5_5V => "-5-5v",
        };
        if lower == label || lower == format!("{:?}", v).to_lowercase() {
            return Ok(*v);
        }
    }
    // Also accept common aliases
    match lower.as_str() {
        "10v" | "0-10" | "0-10v" => Ok(protocol::Range::_0_10V),
        "5v" | "0-5" | "0-5v" => Ok(protocol::Range::_0_5V),
        "bipolar" | "+-5v" | "+/-5v" | "-5-5v" | "-5v-5v" => Ok(protocol::Range::_Neg5_5V),
        _ => {
            let options: Vec<_> = variants.iter().map(|v| format!("{:?}", v)).collect();
            anyhow::bail!("Unknown range '{}'. Options: {}", s, options.join(", "))
        }
    }
}

fn parse_midi_ports_in(s: &str) -> Result<(bool, bool)> {
    let lower = s.to_lowercase();
    if lower == "none" || lower == "off" {
        return Ok((false, false));
    }
    if lower == "all" || lower == "both" {
        return Ok((true, true));
    }
    let usb = lower.contains("usb");
    let din = lower.contains("din");
    if !usb && !din {
        anyhow::bail!("Expected MIDI input ports: 'usb', 'din', 'usb+din', 'all', or 'none'");
    }
    Ok((usb, din))
}

fn parse_midi_ports_out(s: &str) -> Result<(bool, bool, bool)> {
    let lower = s.to_lowercase();
    if lower == "none" || lower == "off" {
        return Ok((false, false, false));
    }
    if lower == "all" {
        return Ok((true, true, true));
    }
    let usb = lower.contains("usb");
    let out1 = lower.contains("out1") || lower.contains("1");
    let out2 = lower.contains("out2") || lower.contains("2");
    if !usb && !out1 && !out2 {
        anyhow::bail!("Expected MIDI output ports: 'usb', 'out1', 'out2', 'all', or 'none'");
    }
    Ok((usb, out1, out2))
}

// ── Config ──

async fn cmd_config(action: ConfigAction) -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;

    match action {
        ConfigAction::Show => {
            let resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
            if let ConfigMsgOut::GlobalConfig(config) = resp {
                display::print_global_config(&config);
            }
        }
        ConfigAction::Bpm { value } => {
            let resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
            if let ConfigMsgOut::GlobalConfig(mut config) = resp {
                config.clock.internal_bpm = value;
                dev.send(&ConfigMsgIn::SetGlobalConfig(config)).await?;
                println!("BPM set to {}", value);
            }
        }
        ConfigAction::Brightness { value } => {
            if !(100..=255).contains(&value) {
                anyhow::bail!("Brightness must be 100-255");
            }
            let resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
            if let ConfigMsgOut::GlobalConfig(mut config) = resp {
                config.led_brightness = value;
                dev.send(&ConfigMsgIn::SetGlobalConfig(config)).await?;
                println!("LED brightness set to {}", value);
            }
        }
        ConfigAction::Takeover { mode } => {
            let takeover = match mode.to_lowercase().as_str() {
                "pickup" => protocol::TakeoverMode::Pickup,
                "jump" => protocol::TakeoverMode::Jump,
                "scale" => protocol::TakeoverMode::Scale,
                _ => anyhow::bail!("Unknown takeover mode: {} (use: pickup, jump, scale)", mode),
            };
            let resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
            if let ConfigMsgOut::GlobalConfig(mut config) = resp {
                config.takeover_mode = takeover;
                dev.send(&ConfigMsgIn::SetGlobalConfig(config)).await?;
                println!("Takeover mode set to {:?}", takeover);
            }
        }
    }

    Ok(())
}

// ── Save / Load ──

async fn cmd_save(path: &str) -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;

    let config_resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
    let layout_resp = dev.send_receive(&ConfigMsgIn::GetLayout).await?;

    let config = match config_resp {
        ConfigMsgOut::GlobalConfig(c) => c,
        _ => anyhow::bail!("Unexpected response for GlobalConfig"),
    };
    let layout = match layout_resp {
        ConfigMsgOut::Layout(l) => l,
        _ => anyhow::bail!("Unexpected response for Layout"),
    };

    let snapshot = serde_json::json!({
        "global_config": config,
        "layout": layout,
    });

    std::fs::write(path, serde_json::to_string_pretty(&snapshot)?)?;
    println!("Config saved to {}", path);
    Ok(())
}

async fn cmd_load(path: &str) -> Result<()> {
    let data = std::fs::read_to_string(path)?;
    let snapshot: serde_json::Value = serde_json::from_str(&data)?;

    let mut dev = FaderpunkDevice::open()?;

    if let Some(config_val) = snapshot.get("global_config") {
        let config: protocol::GlobalConfig = serde_json::from_value(config_val.clone())?;
        dev.send(&ConfigMsgIn::SetGlobalConfig(config)).await?;
        println!("Global config applied.");
    }

    if let Some(layout_val) = snapshot.get("layout") {
        let layout: protocol::Layout = serde_json::from_value(layout_val.clone())?;
        let resp = dev.send_receive(&ConfigMsgIn::SetLayout(layout)).await?;
        if let ConfigMsgOut::Layout(_) = resp {
            println!("Layout applied.");
        }
    }

    println!("Config loaded from {}", path);
    Ok(())
}
