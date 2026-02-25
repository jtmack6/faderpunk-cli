// Pretty-printing helpers with color and visual fader layout.

use owo_colors::OwoColorize;
use owo_colors::Style;

use crate::protocol::*;

// ── Color mapping ──
// Maps Faderpunk LED colors to their actual RGB values (from libfp/src/colors.rs)

fn color_to_rgb(color: &Color) -> (u8, u8, u8) {
    match color {
        Color::White => (255, 255, 255),
        Color::Yellow => (255, 174, 0),
        Color::Orange => (255, 132, 0),
        Color::Red => (255, 0, 0),
        Color::Lime => (255, 240, 0),
        Color::Green => (132, 255, 0),
        Color::Cyan => (0, 255, 186),
        Color::SkyBlue => (0, 198, 255),
        Color::Blue => (6, 0, 255),
        Color::Violet => (246, 0, 255),
        Color::Pink => (255, 0, 150),
        Color::PaleGreen => (101, 255, 156),
        Color::Sand => (255, 216, 120),
        Color::Rose => (255, 120, 120),
        Color::Salmon => (255, 131, 131),
        Color::LightBlue => (115, 129, 255),
        Color::Custom(r, g, b) => (*r, *g, *b),
    }
}

fn style_for_color(color: &Color) -> Style {
    let (r, g, b) = color_to_rgb(color);
    Style::new().color(owo_colors::Rgb(r, g, b))
}

fn bg_style_for_color(color: &Color) -> Style {
    let (r, g, b) = color_to_rgb(color);
    // Use dark text on bright colors, light text on dark colors
    let luminance = (r as u16 * 299 + g as u16 * 587 + b as u16 * 114) / 1000;
    let (fr, fg_, fb) = if luminance > 140 { (0, 0, 0) } else { (255, 255, 255) };
    Style::new()
        .on_color(owo_colors::Rgb(r, g, b))
        .color(owo_colors::Rgb(fr, fg_, fb))
}

// ── Icon mapping ──

fn icon_char(icon: &AppIcon) -> &'static str {
    match icon {
        AppIcon::Fader => "\u{2195}",       // ↕ vertical arrows (fader)
        AppIcon::AdEnv => "\u{2571}",        // ╱ rising slope (envelope)
        AppIcon::Random => "\u{2248}",       // ≈ wavy (random)
        AppIcon::Euclid => "\u{25cb}",       // ○ circle (euclidean)
        AppIcon::Attenuate => "\u{25bf}",    // ▿ down triangle (attenuate)
        AppIcon::Die => "\u{2684}",          // ⚄ die face 5
        AppIcon::Quantize => "\u{266b}",     // ♫ notes (quantize)
        AppIcon::Sequence => "\u{25a0}",     // ■ filled square (sequence)
        AppIcon::Note => "\u{266a}",         // ♪ note
        AppIcon::EnvFollower => "\u{223f}",  // ∿ sine wave
        AppIcon::SoftRandom => "\u{224b}",   // ≋ triple tilde
        AppIcon::Sine => "\u{223f}",         // ∿ sine wave
        AppIcon::NoteBox => "\u{2669}",      // ♩ quarter note
        AppIcon::SequenceSquare => "\u{25a1}", // □ empty square
        AppIcon::NoteGrid => "\u{2637}",     // ☷ trigram (grid)
        AppIcon::KnobRound => "\u{25c9}",    // ◉ fisheye (knob)
        AppIcon::Stereo => "\u{29bf}",       // ⦿ circled bullet (stereo)
    }
}

// ── Section header ──

fn header(title: &str) {
    let bar = "─".repeat(title.len() + 2);
    println!("┌{}┐", bar);
    println!("│ {} │", title.bold());
    println!("└{}┘", bar);
}

fn sub_header(title: &str) {
    println!();
    println!("  {} {}", "▸".dimmed(), title.bold());
}

fn kv(key: &str, value: &str) {
    println!("    {:<16} {}", format!("{}:", key).dimmed(), value);
}

// ── Global config ──

pub fn print_global_config(config: &GlobalConfig) {
    header("Global Config");

    sub_header("Clock");
    kv("Source", &format!("{:?}", config.clock.clock_src));
    kv("BPM", &format!("{}", config.clock.internal_bpm));
    kv("Ext PPQN", &format!("{}", config.clock.ext_ppqn));
    kv("Reset source", &format!("{:?}", config.clock.reset_src));

    sub_header("Control");
    kv("Takeover mode", &format!("{:?}", config.takeover_mode));
    kv("LED brightness", &format!("{}", config.led_brightness));
    kv("I2C mode", &format!("{:?}", config.i2c_mode));

    sub_header("Quantizer");
    kv("Key", &format!("{:?}", config.quantizer.key));
    kv("Tonic", &format!("{:?}", config.quantizer.tonic));

    sub_header("Aux Jacks");
    for (i, aux) in config.aux.iter().enumerate() {
        kv(&format!("Aux {}", i + 1), &format_aux(aux));
    }

    sub_header("MIDI Outputs");
    let labels = ["USB", "Out 1", "Out 2"];
    for (i, out) in config.midi.outs.iter().enumerate() {
        let clock_icon = if out.send_clock { "●" } else { "○" };
        let transport_icon = if out.send_transport { "●" } else { "○" };
        kv(
            labels[i],
            &format!(
                "{} clk  {} transport  {:?}",
                clock_icon, transport_icon, out.mode
            ),
        );
    }
}

fn format_aux(aux: &AuxJackMode) -> String {
    match aux {
        AuxJackMode::None => "─".dimmed().to_string(),
        AuxJackMode::ClockOut(div) => format!("Clock ÷{}", clock_div_value(div)),
        AuxJackMode::ResetOut => "Reset".to_string(),
    }
}

fn clock_div_value(div: &ClockDivision) -> &'static str {
    match div {
        ClockDivision::_1 => "1",
        ClockDivision::_2 => "2",
        ClockDivision::_4 => "4",
        ClockDivision::_6 => "6",
        ClockDivision::_8 => "8",
        ClockDivision::_12 => "12",
        ClockDivision::_24 => "24",
        ClockDivision::_96 => "96",
        ClockDivision::_192 => "192",
        ClockDivision::_384 => "384",
    }
}

// ── Layout (visual fader strip) ──

/// App info needed to render the layout visually
pub struct AppInfo {
    pub app_id: u8,
    pub name: String,
    pub color: Color,
    pub icon: AppIcon,
}

/// Print the layout as a visual fader strip.
/// If `apps` is provided, renders with colors and names.
/// Falls back to a plain table if no app info is available.
pub fn print_layout(layout: &Layout, apps: Option<&[AppInfo]>) {
    header("Layout");

    // Collect occupied slot ranges: (start, size, app_id, layout_id)
    let mut entries: Vec<(usize, usize, u8, u8)> = Vec::new();
    for (i, slot) in layout.0.iter().enumerate() {
        if let Some((app_id, channels, layout_id)) = slot {
            entries.push((i, *channels, *app_id, *layout_id));
        }
    }

    if entries.is_empty() {
        println!("  {}", "(empty layout)".dimmed());
        return;
    }

    // Print the visual fader strip
    println!();

    // Top border
    print!("  ");
    for (i, entry) in entries.iter().enumerate() {
        let width = entry.1 * 5;
        if i == 0 {
            print!("┌{}┐", "─".repeat(width - 1));
        } else {
            print!("┌{}┐", "─".repeat(width - 1));
        }
    }
    println!();

    // App names row (colored)
    print!("  ");
    for entry in &entries {
        let (_, size, app_id, _) = entry;
        let width = size * 5;
        let inner = width - 1;

        let (name, color, icon) = if let Some(apps) = apps {
            if let Some(info) = apps.iter().find(|a| a.app_id == *app_id) {
                (info.name.clone(), info.color, info.icon)
            } else {
                (format!("App {}", app_id), Color::White, AppIcon::Fader)
            }
        } else {
            (format!("App {}", app_id), Color::White, AppIcon::Fader)
        };

        let style = bg_style_for_color(&color);
        let label = format!("{} {}", icon_char(&icon), name);
        let label = if label.len() > inner {
            label[..inner].to_string()
        } else {
            format!("{:^width$}", label, width = inner)
        };
        print!("│{}│", format!("{}", label).style(style));
    }
    println!();

    // Fader number row
    print!("  ");
    for entry in &entries {
        let (start, size, _, _) = entry;
        let width = size * 5;
        let inner = width - 1;

        let range = if *size == 1 {
            format!("{}", start + 1)
        } else {
            format!("{}-{}", start + 1, start + size)
        };
        print!("│{:^width$}│", range.dimmed(), width = inner);
    }
    println!();

    // Bottom border
    print!("  ");
    for entry in &entries {
        let width = entry.1 * 5;
        print!("└{}┘", "─".repeat(width - 1));
    }
    println!();
    println!();

    // Legend table
    println!(
        "  {:>4}  {:>8}  {:>6}  {}",
        "Slot".dimmed(),
        "Layout ID".dimmed(),
        "App ID".dimmed(),
        "App".dimmed()
    );
    for (start, size, app_id, layout_id) in &entries {
        let (name, color) = if let Some(apps) = apps {
            if let Some(info) = apps.iter().find(|a| a.app_id == *app_id) {
                (info.name.clone(), info.color)
            } else {
                (format!("App {}", app_id), Color::White)
            }
        } else {
            (format!("App {}", app_id), Color::White)
        };

        let style = style_for_color(&color);
        let range = if *size == 1 {
            format!("{}", start + 1)
        } else {
            format!("{}-{}", start + 1, start + size)
        };
        let dot = "●".style(style);
        println!("  {:>4}  {:>8}  {:>6}  {} {}", range, layout_id, app_id, dot, name);
    }
}

// ── Apps list ──

pub fn print_app_list(apps: &[(u8, usize, String, String, Color, AppIcon)]) {
    header(&format!("Apps ({})", apps.len()));
    println!();

    for (app_id, channels, name, description, color, icon) in apps {
        let style = style_for_color(color);
        let dot = "●".style(style);
        let icon_str = icon_char(icon);
        let ch_label = if *channels == 1 {
            "1 ch".to_string()
        } else {
            format!("{} ch", channels)
        };
        println!(
            "  {} {} {:>2}  {} {}  {}",
            dot,
            icon_str,
            format!("[{}]", app_id).dimmed(),
            name.bold(),
            format!("({})", ch_label).dimmed(),
            description.dimmed(),
        );
    }
}

// ── App params ──

pub fn print_app_params(layout_id: u8, values: &[Value]) {
    println!(
        "  {} App {}",
        "▸".dimmed(),
        format!("(layout_id={})", layout_id).dimmed()
    );
    for (i, val) in values.iter().enumerate() {
        let formatted = format_value(val);
        println!("    {:>2}  {}", format!("{}.", i).dimmed(), formatted);
    }
    println!();
}

fn format_value(val: &Value) -> String {
    match val {
        Value::Int(v) => format!("{}", v),
        Value::Float(v) => format!("{:.1}", v),
        Value::Bool(v) => {
            if *v {
                "●".green().to_string()
            } else {
                "○".dimmed().to_string()
            }
        }
        Value::Enum(v) => format!("option {}", v),
        Value::Curve(c) => format!("{:?}", c),
        Value::Waveform(w) => format!("{:?}", w),
        Value::Color(c) => {
            let style = style_for_color(c);
            format!("{} {:?}", "●".style(style), c)
        }
        Value::Range(r) => match r {
            Range::_0_10V => "0–10V".to_string(),
            Range::_0_5V => "0–5V".to_string(),
            Range::_Neg5_5V => "±5V".to_string(),
        },
        Value::Note(n) => format!("{:?}", n),
        Value::MidiCc(MidiCc(cc)) => format!("CC {}", cc),
        Value::MidiChannel(MidiChannel(ch)) => format!("Ch {}", ch),
        Value::MidiIn(MidiIn(ports)) => {
            let usb = if ports[0] { "USB" } else { "" };
            let din = if ports[1] { "DIN" } else { "" };
            [usb, din]
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join("+")
        }
        Value::MidiMode(m) => format!("{:?}", m),
        Value::MidiNote(MidiNote(n)) => format!("Note {}", n),
        Value::MidiOut(MidiOut(ports)) => {
            let labels = ["USB", "Out1", "Out2"];
            ports
                .iter()
                .enumerate()
                .filter(|(_, on)| **on)
                .map(|(i, _)| labels[i])
                .collect::<Vec<_>>()
                .join("+")
        }
    }
}
