mod display;
mod protocol;
mod usb;

use anyhow::Result;
use clap::{Parser, Subcommand};

use protocol::{ConfigMsgIn, ConfigMsgOut};
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

    /// Show the current layout (which app is on which fader)
    Layout,

    /// Show parameters for all running apps
    Params,

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
        Commands::Layout => cmd_layout().await,
        Commands::Params => cmd_params().await,
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

    // Get global config
    let config_resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
    if let ConfigMsgOut::GlobalConfig(config) = config_resp {
        println!("Global Config:");
        display::print_global_config(&config);
    }

    println!();

    // Get layout
    let layout_resp = dev.send_receive(&ConfigMsgIn::GetLayout).await?;
    if let ConfigMsgOut::Layout(layout) = layout_resp {
        println!("Layout:");
        display::print_layout(&layout);
    }

    Ok(())
}

async fn cmd_apps() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let responses = dev.send_receive_batch(&ConfigMsgIn::GetAllApps).await?;

    let mut apps = Vec::new();
    for resp in responses {
        if let ConfigMsgOut::AppConfig(app_id, channels, (_, name, desc, color, icon, _)) = resp {
            apps.push((app_id, channels, name, desc, color, icon));
        }
    }

    println!("Available apps ({}):", apps.len());
    display::print_app_list(&apps);
    Ok(())
}

async fn cmd_layout() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let resp = dev.send_receive(&ConfigMsgIn::GetLayout).await?;

    if let ConfigMsgOut::Layout(layout) = resp {
        println!("Current layout:");
        display::print_layout(&layout);
    }

    Ok(())
}

async fn cmd_params() -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;
    let responses = dev.send_receive_batch(&ConfigMsgIn::GetAllAppParams).await?;

    println!("App parameters:");
    for resp in responses {
        if let ConfigMsgOut::AppState(layout_id, values) = resp {
            display::print_app_params(layout_id, &values);
        }
    }

    Ok(())
}

async fn cmd_config(action: ConfigAction) -> Result<()> {
    let mut dev = FaderpunkDevice::open()?;

    match action {
        ConfigAction::Show => {
            let resp = dev.send_receive(&ConfigMsgIn::GetGlobalConfig).await?;
            if let ConfigMsgOut::GlobalConfig(config) = resp {
                println!("Global Config:");
                display::print_global_config(&config);
            }
        }
        ConfigAction::Bpm { value } => {
            // Read current config, modify BPM, write back
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
