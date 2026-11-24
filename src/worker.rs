//! Background thread that reads/writes the Twister configuration over SysEx.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use midir::{MidiOutput, MidiOutputConnection};

use crate::config::{EncoderRecord, RECORD_PAIRS, TwisterConfig};
use crate::protocol::{self, Reply};

const GLOBALS_TIMEOUT: Duration = Duration::from_millis(1500);
const RECORD_TIMEOUT: Duration = Duration::from_millis(1000);
const SEND_GAP: Duration = Duration::from_millis(3);
const MAX_RECORDS: u8 = 128; // 8-bank firmware
const MIN_RECORDS: u8 = 64; // 4-bank firmware

pub enum Command {
    Read { port: String },
    Write { port: String, config: TwisterConfig },
}

#[derive(Default)]
pub struct Status {
    pub busy: bool,
    pub message: String,
    pub progress: f32,
    /// Set when a read finishes; the UI takes it.
    pub read_result: Option<TwisterConfig>,
    pub error: bool,
}

pub fn spawn(
    commands: Receiver<Command>,
    sysex: Receiver<Vec<u8>>,
    status: Arc<Mutex<Status>>,
    ctx: egui::Context,
) {
    thread::spawn(move || {
        let set = |f: &dyn Fn(&mut Status)| {
            if let Ok(mut s) = status.lock() {
                f(&mut s);
            }
            ctx.request_repaint();
        };
        for cmd in commands {
            set(&|s| {
                s.busy = true;
                s.error = false;
                s.progress = 0.0;
            });
            let progress = |p: f32, msg: String| {
                set(&|s| {
                    s.progress = p;
                    s.message = msg.clone();
                })
            };
            let result = match cmd {
                Command::Read { port } => read(&port, &sysex, &progress).map(|cfg| {
                    let msg = format!(
                        "Read {} global settings and {} encoders ({} banks)",
                        cfg.globals.len(),
                        cfg.encoders.len(),
                        cfg.banks()
                    );
                    set(&|s| s.read_result = Some(cfg.clone()));
                    msg
                }),
                Command::Write { port, config } => write(&port, &config, &progress),
            };
            set(&|s| {
                s.busy = false;
                s.progress = 1.0;
                match &result {
                    Ok(msg) => s.message = msg.clone(),
                    Err(e) => {
                        s.message = e.clone();
                        s.error = true;
                    }
                }
            });
        }
    });
}

fn open_output(port_name: &str) -> Result<MidiOutputConnection, String> {
    let out = MidiOutput::new("sixteen-out").map_err(|e| e.to_string())?;
    let ports = out.ports();
    let name_of = |p| out.port_name(p).unwrap_or_default();
    let port = ports
        .iter()
        .find(|p| name_of(p) == port_name)
        .or_else(|| ports.iter().find(|p| name_of(p).to_lowercase().contains("twister")))
        .ok_or_else(|| format!("No MIDI output named '{port_name}'"))?
        .clone();
    out.connect(&port, "sixteen-config").map_err(|e| e.to_string())
}

fn send(conn: &mut MidiOutputConnection, msg: &[u8]) -> Result<(), String> {
    conn.send(msg).map_err(|e| format!("MIDI send failed: {e}"))?;
    thread::sleep(SEND_GAP);
    Ok(())
}

fn wait_for<T>(
    sysex: &Receiver<Vec<u8>>,
    timeout: Duration,
    mut accept: impl FnMut(Reply) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        match sysex.recv_timeout(left) {
            Ok(msg) => {
                if let Some(done) = protocol::parse(&msg).and_then(&mut accept) {
                    return Some(done);
                }
            }
            Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

fn read(
    port: &str,
    sysex: &Receiver<Vec<u8>>,
    progress: &dyn Fn(f32, String),
) -> Result<TwisterConfig, String> {
    let mut conn = open_output(port)?;
    while sysex.try_recv().is_ok() {} // drop stale replies

    progress(0.0, "Reading global settings…".into());
    send(&mut conn, &protocol::pull_globals())?;
    let globals = wait_for(sysex, GLOBALS_TIMEOUT, |r| match r {
        Reply::Globals(pairs) => Some(pairs),
        _ => None,
    })
    .ok_or("No reply to the global config request. Is the Twister connected and selected?")?;

    let mut config = TwisterConfig { globals: globals.into_iter().collect(), encoders: Vec::new() };
    for record in 1..=MAX_RECORDS {
        progress(
            record as f32 / MIN_RECORDS as f32,
            format!("Reading encoder {record}…"),
        );
        send(&mut conn, &protocol::pull_encoder(record))?;
        let mut data = Vec::new();
        let complete = wait_for(sysex, RECORD_TIMEOUT, |r| match r {
            Reply::EncoderChunk { record: got, part, total, data: chunk } if got == record => {
                data.extend(chunk);
                (part >= total).then_some(())
            }
            _ => None,
        });
        match complete {
            Some(()) if data.len() >= RECORD_PAIRS => {
                config.encoders.push(EncoderRecord::from_bytes(&data));
            }
            _ if record > MIN_RECORDS && (record - 1) % 16 == 0 => break, // no 5th bank
            _ => return Err(format!("Encoder {record} did not answer")),
        }
    }
    Ok(config)
}

fn write(
    port: &str,
    config: &TwisterConfig,
    progress: &dyn Fn(f32, String),
) -> Result<String, String> {
    let mut conn = open_output(port)?;
    let n = config.encoders.len();
    // Same order as the Utility: encoder records first, then the global settings.
    for (i, record) in config.encoders.iter().enumerate() {
        progress(i as f32 / (n + 1) as f32, format!("Writing encoder {}…", i + 1));
        for msg in protocol::push_encoder((i + 1) as u8, &record.to_bytes()) {
            send(&mut conn, &msg)?;
        }
    }
    progress(n as f32 / (n + 1) as f32, "Writing global settings…".into());
    let pairs: Vec<(u8, u8)> = config.globals.iter().map(|(&t, &v)| (t, v)).collect();
    send(&mut conn, &protocol::push_globals(&pairs))?;
    Ok(format!("Wrote {} encoders and {} global settings", n, pairs.len()))
}
