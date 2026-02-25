// Protocol types that mirror libfp's ConfigMsgIn/ConfigMsgOut.
//
// These must stay in sync with the firmware's libfp crate — same field order,
// same enum variant order, same types. Postcard serialization is positional,
// so even renaming a field is fine, but reordering breaks compatibility.
//
// Source of truth: faderpunk/libfp/src/lib.rs

use serde::{Deserialize, Serialize};

// ── Constants ──

pub const GLOBAL_CHANNELS: usize = 16;
pub const APP_MAX_PARAMS: usize = 16;

// ── Enums (must match libfp variant order exactly) ──

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClockSrc {
    None,
    Atom,
    Meteor,
    Cube,
    Internal,
    MidiIn,
    MidiUsb,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResetSrc {
    None,
    Atom,
    Meteor,
    Cube,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum I2cMode {
    Calibration,
    Leader,
    Follower,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TakeoverMode {
    Pickup,
    Jump,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClockDivision {
    _1 = 1,
    _2 = 2,
    _4 = 4,
    _6 = 6,
    _8 = 8,
    _12 = 12,
    _24 = 24,
    _96 = 96,
    _192 = 192,
    _384 = 384,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AuxJackMode {
    None,
    ClockOut(ClockDivision),
    ResetOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Note {
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Key {
    Chromatic,
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    BluesMaj,
    BluesMin,
    PentatonicMaj,
    PentatonicMin,
    Folk,
    Japanese,
    Gamelan,
    HungarianMin,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    Linear,
    Logarithmic,
    Exponential,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Waveform {
    Triangle,
    Saw,
    SawInv,
    Square,
    Sine,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Color {
    White,
    Yellow,
    Orange,
    Red,
    Lime,
    Green,
    Cyan,
    SkyBlue,
    Blue,
    Violet,
    Pink,
    PaleGreen,
    Sand,
    Rose,
    Salmon,
    LightBlue,
    Custom(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AppIcon {
    Fader,
    AdEnv,
    Random,
    Euclid,
    Attenuate,
    Die,
    Quantize,
    Sequence,
    Note,
    EnvFollower,
    SoftRandom,
    Sine,
    NoteBox,
    SequenceSquare,
    NoteGrid,
    KnobRound,
    Stereo,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Range {
    _0_10V,
    _0_5V,
    _Neg5_5V,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MidiOutMode {
    None,
    Local,
    MidiThru { sources: MidiIn },
    MidiMerge { sources: MidiIn },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MidiMode {
    Note,
    Cc,
}

// ── Newtype wrappers ──

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiCc(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiChannel(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiIn(pub [bool; 2]); // [usb, din]

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiNote(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiOut(pub [bool; 3]); // [usb, out1, out2]

// ── Config structs ──

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiOutConfig {
    pub send_clock: bool,
    pub send_transport: bool,
    pub mode: MidiOutMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiConfig {
    pub outs: [MidiOutConfig; 3], // [usb, out1, out2]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClockConfig {
    pub clock_src: ClockSrc,
    pub ext_ppqn: u8,
    pub reset_src: ResetSrc,
    pub internal_bpm: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantizerConfig {
    pub key: Key,
    pub tonic: Note,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub aux: [AuxJackMode; 3],
    pub clock: ClockConfig,
    pub i2c_mode: I2cMode,
    pub led_brightness: u8,
    pub midi: MidiConfig,
    pub quantizer: QuantizerConfig,
    pub takeover_mode: TakeoverMode,
}

// Layout: array of 16 slots, each optionally (app_id, channels, layout_id)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layout(pub [Option<(u8, usize, u8)>; GLOBAL_CHANNELS]);

// ── Parameter types (for app config) ──

// Param describes the metadata — only received from device, never sent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Param {
    None,
    #[serde(rename = "i32")]
    Int { name: String, min: i32, max: i32 },
    #[serde(rename = "f32")]
    Float { name: String, min: f32, max: f32 },
    #[serde(rename = "bool")]
    Bool { name: String },
    Enum { name: String, variants: Vec<String> },
    Curve { name: String, variants: Vec<Curve> },
    Waveform { name: String, variants: Vec<Waveform> },
    Color { name: String, variants: Vec<Color> },
    Range { name: String, variants: Vec<Range> },
    Note { name: String, variants: Vec<Note> },
    MidiCc { name: String },
    MidiChannel { name: String },
    MidiIn,
    MidiMode,
    MidiNote { name: String },
    MidiOut,
}

// Value is the actual parameter value — sent and received
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    #[serde(rename = "i32")]
    Int(i32),
    #[serde(rename = "f32")]
    Float(f32),
    #[serde(rename = "bool")]
    Bool(bool),
    Enum(usize),
    Curve(Curve),
    Waveform(Waveform),
    Color(Color),
    Range(Range),
    Note(Note),
    MidiCc(MidiCc),
    MidiChannel(MidiChannel),
    MidiIn(MidiIn),
    MidiMode(MidiMode),
    MidiNote(MidiNote),
    MidiOut(MidiOut),
}

// ── Wire messages ──

// Host → Device
#[derive(Debug, Serialize, Deserialize)]
pub enum ConfigMsgIn {
    Ping,
    GetAllApps,
    GetGlobalConfig,
    SetGlobalConfig(GlobalConfig),
    GetLayout,
    SetLayout(Layout),
    GetAllAppParams,
    GetAppParams { layout_id: u8 },
    SetAppParams {
        layout_id: u8,
        values: [Option<Value>; APP_MAX_PARAMS],
    },
    FactoryReset,
}

// Device → Host
// Note: the firmware uses ConfigMsgOut<'a> with borrowed data, but for
// deserialization on the host side we own all data (String, Vec).
#[derive(Debug, Serialize, Deserialize)]
pub enum ConfigMsgOut {
    Pong,
    BatchMsgStart(usize),
    BatchMsgEnd,
    GlobalConfig(GlobalConfig),
    Layout(Layout),
    // (app_id, channels, (param_count, name, description, color, icon, params))
    AppConfig(u8, usize, (usize, String, String, Color, AppIcon, Vec<Param>)),
    // (layout_id, values)
    AppState(u8, Vec<Value>),
}
