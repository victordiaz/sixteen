//! Decoding of the Midi Fighter Twister's factory-default MIDI map.
//!
//! Default map (MIDI channels are 1-based here, 0-based in the status byte):
//! - Ch 1, CC 0-63  : encoder rotation (bank = cc / 16, encoder = cc % 16)
//! - Ch 2, CC 0-63  : encoder push switch (127 = pressed, 0 = released)
//! - Ch 4, CC 0-3   : bank changed (value 127)
//! - Ch 4, CC 8-31  : side buttons, 6 per bank (3 left, then 3 right)
//! - Ch 5, CC 0-63  : shifted encoder rotation (hold a side "shift" button)

use std::time::Instant;

pub const BANKS: usize = 4;
pub const ENCODERS: usize = 16;
pub const SIDE_BUTTONS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Encoder { bank: usize, index: usize, value: u8 },
    EncoderSwitch { bank: usize, index: usize, pressed: bool },
    ShiftEncoder { bank: usize, index: usize, value: u8 },
    BankSelect { bank: usize },
    SideButton { bank: usize, index: usize, pressed: bool },
    Other,
}

pub fn decode(msg: &[u8]) -> Event {
    let [status, cc, value] = match msg {
        [s, c, v] => [*s, *c, *v],
        _ => return Event::Other,
    };
    if status & 0xF0 != 0xB0 {
        return Event::Other;
    }
    let channel = status & 0x0F;
    let cc = cc as usize;
    match (channel, cc) {
        (0, 0..=63) => Event::Encoder { bank: cc / ENCODERS, index: cc % ENCODERS, value },
        (1, 0..=63) => Event::EncoderSwitch {
            bank: cc / ENCODERS,
            index: cc % ENCODERS,
            pressed: value > 0,
        },
        (3, 0..=3) if value > 0 => Event::BankSelect { bank: cc },
        (3, 8..=31) => Event::SideButton {
            bank: (cc - 8) / SIDE_BUTTONS,
            index: (cc - 8) % SIDE_BUTTONS,
            pressed: value > 0,
        },
        (4, 0..=63) => Event::ShiftEncoder { bank: cc / ENCODERS, index: cc % ENCODERS, value },
        _ => Event::Other,
    }
}

pub fn describe(msg: &[u8], event: &Event) -> String {
    match *event {
        Event::Encoder { bank, index, value } => {
            format!("Bank {} Enc {:>2}  turn   {value}", bank + 1, index + 1)
        }
        Event::EncoderSwitch { bank, index, pressed } => format!(
            "Bank {} Enc {:>2}  {}",
            bank + 1,
            index + 1,
            if pressed { "press" } else { "release" }
        ),
        Event::ShiftEncoder { bank, index, value } => {
            format!("Bank {} Enc {:>2}  shift  {value}", bank + 1, index + 1)
        }
        Event::BankSelect { bank } => format!("Bank -> {}", bank + 1),
        Event::SideButton { bank, index, pressed } => format!(
            "Bank {} Side {} {}  {}",
            bank + 1,
            if index < 3 { "L" } else { "R" },
            index % 3 + 1,
            if pressed { "press" } else { "release" }
        ),
        Event::Other => describe_raw(msg),
    }
}

fn describe_raw(msg: &[u8]) -> String {
    let Some(&status) = msg.first() else {
        return "empty".into();
    };
    let ch = (status & 0x0F) + 1;
    let d = |i: usize| msg.get(i).copied().unwrap_or(0);
    match status & 0xF0 {
        0x80 => format!("ch{ch} NoteOff {} {}", d(1), d(2)),
        0x90 => format!("ch{ch} NoteOn {} {}", d(1), d(2)),
        0xB0 => format!("ch{ch} CC {} = {}", d(1), d(2)),
        0xC0 => format!("ch{ch} Program {}", d(1)),
        0xE0 => format!("ch{ch} PitchBend {}", (d(2) as u16) << 7 | d(1) as u16),
        0xF0 if status == 0xF0 => format!("SysEx ({} bytes)", msg.len()),
        _ => "other".into(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EncoderState {
    pub value: u8,
    pub shift_value: u8,
    pub pressed: bool,
    pub touched: bool,
    pub last_activity: Option<Instant>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SideState {
    pub pressed: bool,
    pub last_activity: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
pub struct TwisterState {
    pub encoders: [[EncoderState; ENCODERS]; BANKS],
    pub side: [[SideState; SIDE_BUTTONS]; BANKS],
    pub device_bank: usize,
}

impl TwisterState {
    pub fn apply(&mut self, event: Event, now: Instant) {
        match event {
            Event::Encoder { bank, index, value } => {
                let e = &mut self.encoders[bank][index];
                e.value = value;
                e.touched = true;
                e.last_activity = Some(now);
            }
            Event::EncoderSwitch { bank, index, pressed } => {
                let e = &mut self.encoders[bank][index];
                e.pressed = pressed;
                e.last_activity = Some(now);
            }
            Event::ShiftEncoder { bank, index, value } => {
                let e = &mut self.encoders[bank][index];
                e.shift_value = value;
                e.last_activity = Some(now);
            }
            Event::BankSelect { bank } => self.device_bank = bank,
            Event::SideButton { bank, index, pressed } => {
                let s = &mut self.side[bank][index];
                s.pressed = pressed;
                s.last_activity = Some(now);
            }
            Event::Other => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_encoder_turn_in_bank_two() {
        assert_eq!(
            decode(&[0xB0, 21, 100]),
            Event::Encoder { bank: 1, index: 5, value: 100 }
        );
    }

    #[test]
    fn decodes_encoder_switch() {
        assert_eq!(
            decode(&[0xB1, 63, 127]),
            Event::EncoderSwitch { bank: 3, index: 15, pressed: true }
        );
        assert_eq!(
            decode(&[0xB1, 0, 0]),
            Event::EncoderSwitch { bank: 0, index: 0, pressed: false }
        );
    }

    #[test]
    fn decodes_bank_select_and_side_buttons() {
        assert_eq!(decode(&[0xB3, 2, 127]), Event::BankSelect { bank: 2 });
        assert_eq!(
            decode(&[0xB3, 8, 127]),
            Event::SideButton { bank: 0, index: 0, pressed: true }
        );
        assert_eq!(
            decode(&[0xB3, 31, 0]),
            Event::SideButton { bank: 3, index: 5, pressed: false }
        );
    }

    #[test]
    fn decodes_shift_encoder() {
        assert_eq!(
            decode(&[0xB4, 17, 5]),
            Event::ShiftEncoder { bank: 1, index: 1, value: 5 }
        );
    }

    #[test]
    fn non_cc_is_other() {
        assert_eq!(decode(&[0x90, 60, 100]), Event::Other);
        assert_eq!(decode(&[0xF8]), Event::Other);
    }
}
