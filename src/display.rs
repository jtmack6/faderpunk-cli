// Pretty-printing helpers for displaying device state.

use crate::protocol::*;

pub fn print_global_config(config: &GlobalConfig) {
    println!("  Clock:");
    println!("    Source:       {:?}", config.clock.clock_src);
    println!("    BPM:          {}", config.clock.internal_bpm);
    println!("    Ext PPQN:     {}", config.clock.ext_ppqn);
    println!("    Reset source: {:?}", config.clock.reset_src);

    println!("  Takeover mode:  {:?}", config.takeover_mode);
    println!("  LED brightness: {}", config.led_brightness);
    println!("  I2C mode:       {:?}", config.i2c_mode);

    println!("  Quantizer:");
    println!("    Key:   {:?}", config.quantizer.key);
    println!("    Tonic: {:?}", config.quantizer.tonic);

    println!("  Aux jacks:");
    for (i, aux) in config.aux.iter().enumerate() {
        println!("    Aux {}: {:?}", i + 1, aux);
    }

    println!("  MIDI outputs:");
    let labels = ["USB", "Out 1", "Out 2"];
    for (i, out) in config.midi.outs.iter().enumerate() {
        println!("    {}:", labels[i]);
        println!("      Clock:     {}", out.send_clock);
        println!("      Transport: {}", out.send_transport);
        println!("      Mode:      {:?}", out.mode);
    }
}

pub fn print_layout(layout: &Layout) {
    println!("  {:>4}  {:>6}  {:>4}  {:>9}", "Slot", "App ID", "Size", "Layout ID");
    println!("  {}  {}  {}  {}", "----", "------", "----", "---------");

    for (i, slot) in layout.0.iter().enumerate() {
        match slot {
            Some((app_id, channels, layout_id)) => {
                println!("  {:>4}  {:>6}  {:>4}  {:>9}", i + 1, app_id, channels, layout_id);
            }
            None => {} // skip empty slots for cleaner output
        }
    }
}

pub fn print_app_list(apps: &[(u8, usize, String, String, Color, AppIcon)]) {
    for (app_id, channels, name, description, color, icon) in apps {
        println!(
            "  [{}] {} ({} ch) — {} [{:?}, {:?}]",
            app_id, name, channels, description, color, icon
        );
    }
}

pub fn print_app_params(layout_id: u8, values: &[Value]) {
    println!("  App (layout_id={}):", layout_id);
    for (i, val) in values.iter().enumerate() {
        println!("    Param {:>2}: {:?}", i, val);
    }
}
