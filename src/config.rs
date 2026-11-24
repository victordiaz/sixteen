//! Configuration model and setting schema, taken from the Utility's
//! `device.mf_twister.js` plugin (tags, ranges, option lists, stored values).

use std::collections::BTreeMap;

pub const RECORD_PAIRS: usize = 16;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// Stored as the shown number.
    Int { min: u8, max: u8 },
    /// 0 = off, 1 = on.
    Bool,
    /// Stored value = option value.
    Enum(&'static [(&'static str, u8)]),
    /// Stored value = option index + 1 (0 = "not sent" on older firmware).
    EnumOffset(&'static [&'static str]),
    /// 1 = off, 2 = on (0 = not sent).
    BoolOffset,
    /// LED palette index 0..127.
    Color,
}

#[derive(Debug, Clone, Copy)]
pub struct Setting {
    pub tag: u8,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: Kind,
}

const fn s(tag: u8, label: &'static str, group: &'static str, kind: Kind) -> Setting {
    Setting { tag, label, group, kind }
}

macro_rules! opts {
    ($($name:expr),* $(,)?) => {{
        const N: &[&str] = &[$($name),*];
        N
    }};
}

const fn indexed<const N: usize>(names: [&'static str; N]) -> [(&'static str, u8); N] {
    let mut out = [("", 0u8); N];
    let mut i = 0;
    while i < N {
        out[i] = (names[i], i as u8);
        i += 1;
    }
    out
}

const SIDE_ACTIONS: [(&str, u8); 20] = indexed([
    "CC Hold", "CC Toggle", "Note Hold", "Note Toggle", "Shift Page A", "Shift Page B",
    "Shift Page A (Toggle)", "Shift Page B (Toggle)", "Next Bank", "Previous Bank",
    "Bank Select", "Bank 1", "Bank 2", "Bank 3", "Bank 4", "Bank 5", "Bank 6", "Bank 7",
    "Bank 8", "Cycle Bank",
]);
/// Firmware older than 2023-12-11 uses this shorter list.
const SIDE_ACTIONS_LEGACY: [(&str, u8); 13] = indexed([
    "CC Hold", "CC Toggle", "Note Hold", "Note Toggle", "Shift Page A", "Shift Page B",
    "Next Bank", "Previous Bank", "Bank 1", "Bank 2", "Bank 3", "Bank 4", "Cycle Bank",
]);
const SLEEP_TIMER: [(&str, u8); 8] = indexed([
    "Off", "1 minute", "3 minutes", "5 minutes", "10 minutes", "20 minutes", "30 minutes",
    "60 minutes",
]);
const SLEEP_ANIM: [(&str, u8); 2] = indexed(["Turn LEDs Off", "Rainbow Wave"]);
const COLOR_MAP: [(&str, u8); 2] = indexed(["Classic", "Expanded"]);
const DETENT_SIZES: &[&str] = opts!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];

const SENSITIVITY: [(&str, u8); 3] = indexed(["360", "Responsive", "Velocity Sensitive"]);
const INDICATOR: [(&str, u8); 4] = indexed(["Dot", "Bar", "Blended Bar", "Spread"]);
const ENCODER_TYPES: [(&str, u8); 10] = indexed([
    "Note", "CC", "Relative (ENC 3FH/41H)", "Button Velocity Control", "Mouse Move",
    "Mouse Scroll", "Program Change", "MCU Fader", "MCU V-Pot", "Disabled",
]);
const SWITCH_ACTIONS: [(&str, u8); 12] = indexed([
    "CC Hold", "CC Toggle", "Note Hold", "Note Toggle", "Reset Encoder Value (0)",
    "Reset Encoder Value (127)", "Encoder Fine Adjust", "Shift Encoder (Hold)",
    "Shift Encoder (Toggle)", "Re-send Program", "Disabled", "MCU Button",
]);

pub const GLOBAL_SETTINGS: &[Setting] = &[
    s(0, "System channel", "MIDI", Kind::Int { min: 1, max: 16 }),
    s(34, "Ring animation channel", "MIDI", Kind::Int { min: 1, max: 16 }),
    s(35, "RGB animation channel", "MIDI", Kind::Int { min: 1, max: 16 }),
    s(8, "Super knob start", "MIDI", Kind::Int { min: 0, max: 127 }),
    s(9, "Super knob end", "MIDI", Kind::Int { min: 0, max: 127 }),
    s(32, "Ring brightness", "Lights", Kind::Int { min: 0, max: 127 }),
    s(31, "RGB brightness", "Lights", Kind::Int { min: 0, max: 127 }),
    s(33, "Palette", "Lights", Kind::Enum(&COLOR_MAP)),
    s(38, "Bank animation", "Lights", Kind::Bool),
    s(36, "Sleep after", "Lights", Kind::Enum(&SLEEP_TIMER)),
    s(37, "Sleep look", "Lights", Kind::Enum(&SLEEP_ANIM)),
    s(1, "Per-bank messages", "Side buttons", Kind::Bool),
    s(2, "Left top", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(3, "Left middle", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(4, "Left bottom", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(5, "Right top", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(6, "Right middle", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(7, "Right bottom", "Side buttons", Kind::Enum(&SIDE_ACTIONS)),
    s(39, "Detent size", "All encoders", Kind::EnumOffset(DETENT_SIZES)),
    s(40, "Dead zone", "All encoders", Kind::EnumOffset(DETENT_SIZES)),
    s(41, "Mute shift switch", "All encoders", Kind::BoolOffset),
];

pub const ENCODER_SETTINGS: &[Setting] = &[
    s(20, "Color", "Light", Kind::Color),
    s(19, "Pressed", "Light", Kind::Color),
    s(22, "Ring", "Light", Kind::Enum(&INDICATOR)),
    s(18, "Type", "Turn", Kind::Enum(&ENCODER_TYPES)),
    s(16, "Channel", "Turn", Kind::Int { min: 1, max: 16 }),
    s(17, "Number", "Turn", Kind::Int { min: 0, max: 127 }),
    s(11, "Feel", "Turn", Kind::Enum(&SENSITIVITY)),
    s(24, "Shift channel", "Turn", Kind::Int { min: 1, max: 16 }),
    s(12, "Action", "Press", Kind::Enum(&SWITCH_ACTIONS)),
    s(13, "Channel", "Press", Kind::Int { min: 1, max: 16 }),
    s(14, "Number", "Press", Kind::Int { min: 0, max: 127 }),
    s(10, "Center detent", "Extras", Kind::Bool),
    s(21, "Detent color", "Extras", Kind::Int { min: 0, max: 127 }),
    s(23, "Super knob", "Extras", Kind::Bool),
];

/// The side-button option list depends on firmware age; the newer firmware is
/// the one that reports the newer global tags (33+).
pub fn global_setting(setting: &Setting, globals: &BTreeMap<u8, u8>) -> Setting {
    let legacy = !globals.keys().any(|&t| t >= 33);
    let side_button = (2..=7).contains(&setting.tag);
    if legacy && side_button {
        Setting { kind: Kind::Enum(&SIDE_ACTIONS_LEGACY), ..*setting }
    } else {
        *setting
    }
}

/// One encoder record exactly as the device stores it: (tag, value) pairs in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncoderRecord {
    pub pairs: Vec<(u8, u8)>,
}

impl EncoderRecord {
    pub fn from_bytes(data: &[u8]) -> Self {
        Self { pairs: data.chunks_exact(2).map(|p| (p[0], p[1])).collect() }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.pairs.iter().flat_map(|&(t, v)| [t, v]).collect()
    }

    pub fn get(&self, tag: u8) -> Option<u8> {
        self.pairs.iter().find(|p| p.0 == tag).map(|p| p.1)
    }

    pub fn set(&mut self, tag: u8, value: u8) {
        if let Some(p) = self.pairs.iter_mut().find(|p| p.0 == tag) {
            p.1 = value;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TwisterConfig {
    pub globals: BTreeMap<u8, u8>,
    /// Index = record number - 1 (bank * 16 + encoder).
    pub encoders: Vec<EncoderRecord>,
}

impl TwisterConfig {
    pub fn banks(&self) -> usize {
        self.encoders.len() / 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_roundtrip_and_set() {
        let bytes = [10, 0, 11, 1, 19, 50];
        let mut r = EncoderRecord::from_bytes(&bytes);
        assert_eq!(r.get(11), Some(1));
        r.set(19, 64);
        assert_eq!(r.to_bytes(), vec![10, 0, 11, 1, 19, 64]);
        r.set(99, 1); // unknown tags are never added
        assert_eq!(r.pairs.len(), 3);
    }

    #[test]
    fn legacy_side_actions_when_no_new_tags() {
        let mut globals = BTreeMap::new();
        globals.insert(2, 0);
        let left_top = GLOBAL_SETTINGS.iter().find(|s| s.tag == 2).unwrap();
        match global_setting(left_top, &globals).kind {
            Kind::Enum(o) => assert_eq!(o.len(), 13),
            _ => panic!(),
        }
        globals.insert(33, 0);
        match global_setting(left_top, &globals).kind {
            Kind::Enum(o) => assert_eq!(o.len(), 20),
            _ => panic!(),
        }
    }
}
