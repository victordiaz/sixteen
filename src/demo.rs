//! Demo state for screenshots: `SIXTEEN_DEMO=monitor` (Monitor with a mocked log,
//! a few knobs turned and cap colors) or `SIXTEEN_DEMO=configure`.

use std::time::{Duration, Instant};

use crate::config::{EncoderRecord, TwisterConfig};
use crate::{App, LogEntry, Tab, twister};

/// Classic-palette color index per row of the grid (yellow, green, cyan, blue).
const ROW_COLORS: [u8; 4] = [62, 45, 20, 1];
const PRESSED_COLOR: u8 = 85;

fn mock_config() -> TwisterConfig {
    let encoders = (0..64u8)
        .map(|i| {
            let n = i % 16;
            let inactive = ROW_COLORS[(n / 4) as usize];
            EncoderRecord {
                pairs: vec![
                    (10, 0), (11, 1), (12, 0), (13, 2), (14, n), (15, 0), (16, 1), (17, n),
                    (18, 1), (19, PRESSED_COLOR), (20, inactive), (21, 63), (22, 2), (23, 0), (24, 4),
                ],
            }
        })
        .collect();
    TwisterConfig { globals: [(0, 4), (1, 1), (33, 0)].into_iter().collect(), encoders }
}

pub fn apply(app: &mut App, mode: &str) {
    if mode == "configure" {
        app.tab = Tab::Configure;
        app.cfg.device = Some(mock_config());
        app.cfg.edit = app.cfg.device.clone();
        app.cfg.auto_read_sent = true;
        app.cfg.encoder = 5;
        let mut s = app.cfg_status.lock().unwrap();
        s.message = "Read 12 global settings and 64 encoders (4 banks)".into();
        return;
    }
    app.tab = Tab::Monitor;
    app.show_log = true;
    app.follow_device = false;
    app.view_bank = 0;
    app.cfg.device = Some(mock_config());
    app.cfg.edit = app.cfg.device.clone();
    app.cfg.auto_read_sent = true;

    let now = Instant::now();
    let start = now.checked_sub(Duration::from_secs(12)).unwrap_or(now);
    app.started = start;

    // (seconds after start, raw MIDI message)
    let mut script: Vec<(f32, Vec<u8>)> = Vec::new();
    let mut t = 0.4;
    let turn = |script: &mut Vec<(f32, Vec<u8>)>, t: &mut f32, cc: u8, from: u8, to: u8, step: u8| {
        let mut v = from;
        while v != to {
            v = if to > v { (v + step).min(to) } else { v.saturating_sub(step).max(to) };
            script.push((*t, vec![0xB0, cc, v]));
            *t += 0.06;
        }
        *t += 0.5;
    };
    turn(&mut script, &mut t, 0, 0, 96, 8);
    turn(&mut script, &mut t, 5, 0, 64, 8);
    script.push((t, vec![0xB1, 5, 127]));
    script.push((t + 0.25, vec![0xB1, 5, 0]));
    t += 0.8;
    turn(&mut script, &mut t, 10, 0, 127, 16);
    script.push((t, vec![0xB3, 8, 127]));
    script.push((t + 0.2, vec![0xB3, 8, 0]));
    t += 0.6;
    for (i, v) in [(4u8, 40u8), (4, 52)] {
        script.push((t, vec![0xB4, i, v]));
        t += 0.1;
    }
    t += 0.4;
    turn(&mut script, &mut t, 15, 0, 30, 6);
    script.push((t, vec![0xB1, 12, 127]));

    let mut shared = app.shared.lock().unwrap();
    for (secs, msg) in &script {
        let time = start + Duration::from_secs_f32(*secs);
        let event = twister::decode(msg);
        // Only the last few events should still be glowing.
        shared.state.apply(event, start);
        shared.log.push_back(LogEntry { time, bytes: msg.clone(), text: twister::describe(msg, &event) });
    }
    // Leave encoder 13 visibly pressed and a couple of rings at rest values.
    let bank = &mut shared.state.encoders[0];
    bank[12].pressed = true;
    bank[12].last_activity = Some(now);
    bank[15].last_activity = Some(now);
    for (i, v) in [(2usize, 22u8), (7, 110), (9, 48)] {
        bank[i].value = v;
        bank[i].touched = true;
    }
}
