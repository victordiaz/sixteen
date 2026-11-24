mod colors;
mod config;
mod config_ui;
mod demo;
mod knob;
mod protocol;
mod theme;
mod twister;
mod worker;

use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Sense, Stroke, Vec2};
use midir::{Ignore, MidiInput, MidiInputConnection};

use theme::icon;
use twister::{BANKS, ENCODERS, SIDE_BUTTONS, TwisterState};

const LOG_CAPACITY: usize = 500;
const FLASH: Duration = Duration::from_millis(600);

struct LogEntry {
    time: Instant,
    bytes: Vec<u8>,
    text: String,
}

#[derive(Default)]
struct Shared {
    state: TwisterState,
    log: VecDeque<LogEntry>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Monitor,
    Configure,
}

struct App {
    shared: Arc<Mutex<Shared>>,
    ports: Vec<String>,
    selected_port: Option<String>,
    connection: Option<MidiInputConnection<()>>,
    status: String,
    view_bank: usize,
    follow_device: bool,
    started: Instant,
    tab: Tab,
    show_log: bool,
    sysex_tx: Sender<Vec<u8>>,
    cfg_tx: Sender<worker::Command>,
    cfg_status: Arc<Mutex<worker::Status>>,
    cfg: config_ui::ConfigUi,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let (sysex_tx, sysex_rx) = mpsc::channel();
        let (cfg_tx, cfg_rx) = mpsc::channel();
        let cfg_status = Arc::new(Mutex::new(worker::Status::default()));
        worker::spawn(cfg_rx, sysex_rx, Arc::clone(&cfg_status), cc.egui_ctx.clone());
        let mut app = Self {
            shared: Arc::default(),
            ports: Vec::new(),
            selected_port: None,
            connection: None,
            status: String::new(),
            view_bank: 0,
            follow_device: true,
            started: Instant::now(),
            tab: Tab::Monitor,
            show_log: false,
            sysex_tx,
            cfg_tx,
            cfg_status,
            cfg: config_ui::ConfigUi::default(),
        };
        if let Ok(mode) = std::env::var("SIXTEEN_DEMO") {
            demo::apply(&mut app, &mode);
        }
        app.refresh_ports();
        app.selected_port = app
            .ports
            .iter()
            .find(|p| p.to_lowercase().contains("twister"))
            .or(app.ports.first())
            .cloned();
        if app.selected_port.is_some() {
            app.connect(&cc.egui_ctx);
        }
        app
    }

    fn refresh_ports(&mut self) {
        match MidiInput::new("sixteen-scan") {
            Ok(input) => {
                self.ports = input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect();
                if self.ports.is_empty() {
                    self.status = "No MIDI inputs found. Plug in the Twister and press refresh.".into();
                }
            }
            Err(e) => self.status = format!("MIDI is unavailable: {e}"),
        }
    }

    fn connect(&mut self, ctx: &egui::Context) {
        self.connection = None;
        let Some(name) = self.selected_port.clone() else {
            return;
        };
        let mut input = match MidiInput::new("sixteen") {
            Ok(i) => i,
            Err(e) => {
                self.status = format!("MIDI is unavailable: {e}");
                return;
            }
        };
        let Some(port) = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).ok().as_deref() == Some(name.as_str()))
        else {
            self.status = format!("{name} is gone. Press refresh.");
            return;
        };

        // Configuration replies arrive as SysEx, which midir drops by default.
        input.ignore(Ignore::None);
        let shared = Arc::clone(&self.shared);
        let sysex_tx = self.sysex_tx.clone();
        let ctx = ctx.clone();
        let result = input.connect(
            &port,
            "sixteen-in",
            move |_stamp, msg, _| {
                if msg.first() == Some(&0xF0) {
                    let _ = sysex_tx.send(msg.to_vec());
                }
                let event = twister::decode(msg);
                let now = Instant::now();
                if let Ok(mut s) = shared.lock() {
                    s.state.apply(event, now);
                    if s.log.len() >= LOG_CAPACITY {
                        s.log.pop_front();
                    }
                    s.log.push_back(LogEntry {
                        time: now,
                        bytes: msg.to_vec(),
                        text: twister::describe(msg, &event),
                    });
                }
                ctx.request_repaint();
            },
            (),
        );
        match result {
            Ok(conn) => {
                self.connection = Some(conn);
                self.status = format!("Listening to {name}");
            }
            Err(e) => self.status = format!("Could not open {name}: {e}"),
        }
    }

    fn disconnect(&mut self) {
        if let Some(conn) = self.connection.take() {
            conn.close();
        }
        self.status = "Disconnected".into();
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, glyph, tip) in [
                (Tab::Monitor, icon::MONITOR, "Monitor"),
                (Tab::Configure, icon::CONFIGURE, "Configure"),
            ] {
                if theme::icon_toggle(ui, self.tab == tab, glyph, tip).clicked() {
                    self.tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_toggle(ui, self.show_log, icon::LOG, "MIDI log").clicked() {
                    self.show_log = !self.show_log;
                }
                ui.add_space(6.0);
                let (glyph, tip) = if self.connection.is_some() {
                    (icon::CONNECTED, "Disconnect")
                } else {
                    (icon::PLUG, "Connect")
                };
                if theme::icon_button(ui, glyph, tip).clicked() {
                    if self.connection.is_some() {
                        self.disconnect();
                    } else {
                        self.connect(ui.ctx());
                    }
                }
                if theme::icon_button(ui, icon::REFRESH, "Rescan MIDI devices").clicked() {
                    self.refresh_ports();
                }
                let mut changed = false;
                egui::ComboBox::from_id_salt("port")
                    .selected_text(RichText::new(self.selected_port.clone().unwrap_or_else(|| "No input".into())).color(theme::TEXT_DIM))
                    .width(190.0)
                    .show_ui(ui, |ui| {
                        for p in &self.ports {
                            let sel = self.selected_port.as_deref() == Some(p.as_str());
                            if ui.selectable_label(sel, p).clicked() {
                                self.selected_port = Some(p.clone());
                                changed = true;
                            }
                        }
                    });
                if changed {
                    self.connect(ui.ctx());
                }
                let color = if self.connection.is_some() { theme::OK } else { theme::ERR };
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, color);
                resp.on_hover_text(&self.status);
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let (busy, message, progress, error) = {
            let s = self.cfg_status.lock().unwrap();
            (s.busy, s.message.clone(), s.progress, s.error)
        };
        ui.horizontal(|ui| {
            if busy {
                ui.add(egui::ProgressBar::new(progress.clamp(0.0, 1.0)).desired_width(120.0).desired_height(4.0));
            } else if !message.is_empty() {
                let (glyph, color) = if error { (icon::WARN, theme::ERR) } else { (icon::OK, theme::OK) };
                ui.colored_label(color, glyph);
            }
            ui.label(RichText::new(&message).small().color(if error { theme::ERR } else { theme::TEXT_DIM }));
        });
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let n = self.shared.lock().unwrap().log.len();
            ui.label(RichText::new(n.to_string()).small().color(theme::TEXT_FAINT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::icon_button(ui, icon::TRASH, "Clear").clicked() {
                    self.shared.lock().unwrap().log.clear();
                }
            });
        });
        let shared = self.shared.lock().unwrap();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for (row, entry) in shared.log.iter().enumerate() {
                    let t = entry.time.saturating_duration_since(self.started).as_secs_f32();
                    let mut hex: Vec<String> = entry.bytes.iter().take(6).map(|b| format!("{b:02X}")).collect();
                    if entry.bytes.len() > 6 {
                        hex.push("…".into());
                    }
                    let fill = if row % 2 == 0 { theme::PANEL } else { Color32::TRANSPARENT };
                    egui::Frame::new()
                        .fill(fill)
                        .inner_margin(Margin::symmetric(6, 2))
                        .corner_radius(CornerRadius::same(4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{t:6.1}")).monospace().color(theme::TEXT_FAINT));
                                ui.label(RichText::new(&entry.text).size(12.0).color(theme::TEXT));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(hex.join(" ")).monospace().size(10.5).color(theme::TEXT_FAINT));
                                });
                            });
                        });
                }
            });
    }

    /// Halo color of an encoder, from the configuration last read, if any.
    fn cap_color(&self, bank: usize, index: usize, pressed: bool) -> Option<Color32> {
        let cfg = self.cfg.device.as_ref()?;
        let rec = cfg.encoders.get(bank * 16 + index)?;
        let palette = colors::palette(cfg.globals.get(&33) == Some(&1));
        let tag = if pressed { 19 } else { 20 };
        rec.get(tag).map(|i| palette[(i & 0x7F) as usize])
    }

    fn monitor(&mut self, ui: &mut egui::Ui) {
        let state = self.shared.lock().unwrap().state.clone();
        if self.follow_device {
            self.view_bank = state.device_bank;
        }
        let bank = self.view_bank;
        let now = Instant::now();

        let avail = ui.available_size();
        let pitch = ((avail.y - 70.0) / 4.84).min(avail.x / 5.0).clamp(64.0, 130.0);
        let size = knob::Layout::size(pitch);
        ui.add_space(((avail.y - size.y - 50.0) / 2.0).max(4.0));
        let (response, painter) = ui.allocate_painter(size, Sense::hover());
        let layout = knob::Layout::new(response.rect.min + Vec2::new(pitch * 0.08, 0.0), pitch);
        knob::chassis(&painter, &layout);

        let mut animating = false;
        for i in 0..ENCODERS {
            let enc = state.encoders[bank][i];
            let flash = flash_amount(enc.last_activity, now);
            animating |= flash > 0.0;
            let center = layout.knob_center(i);
            knob::knob(
                &painter,
                center,
                layout.knob_radius(),
                &knob::Knob {
                    value: Some(enc.value),
                    shift: Some(enc.shift_value),
                    pressed: enc.pressed,
                    rgb: self.cap_color(bank, i, enc.pressed),
                    flash,
                    selected: false,
                },
            );
            if enc.touched {
                let color = theme::lerp(theme::TEXT_FAINT, theme::TEXT, flash);
                knob::value_label(&painter, center + Vec2::new(0.0, pitch * 0.47), &enc.value.to_string(), color);
            }
        }
        for i in 0..SIDE_BUTTONS {
            let btn = state.side[bank][i];
            let flash = flash_amount(btn.last_activity, now);
            animating |= flash > 0.0;
            knob::side_button(&painter, layout.side_button(i), btn.pressed, flash);
        }
        if animating {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }

        ui.add_space(14.0);
        self.bank_selector(ui, state.device_bank);
    }

    /// Bank dots under the device, plus the follow-device link toggle.
    fn bank_selector(&mut self, ui: &mut egui::Ui, device_bank: usize) {
        let dot = 9.0;
        let gap = 14.0;
        let width = BANKS as f32 * (dot + gap) + 30.0;
        ui.allocate_ui_with_layout(Vec2::new(width, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for b in 0..BANKS {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(dot + 6.0), Sense::click());
                let c = rect.center();
                let viewing = b == self.view_bank;
                let fill = if b == device_bank { theme::LED } else if resp.hovered() { theme::TEXT_DIM } else { theme::RAISED };
                ui.painter().circle_filled(c, dot / 2.0, fill);
                if viewing {
                    ui.painter().circle_stroke(c, dot / 2.0 + 3.0, Stroke::new(1.0, theme::TEXT_DIM));
                }
                if resp.on_hover_text(format!("Bank {}", b + 1)).clicked() {
                    self.view_bank = b;
                    self.follow_device = false;
                }
            }
            let (glyph, tip) = if self.follow_device {
                (icon::FOLLOW, "Following the device's bank")
            } else {
                (icon::UNFOLLOW, "Follow the device's bank")
            };
            if theme::icon_toggle(ui, self.follow_device, glyph, tip).clicked() {
                self.follow_device = !self.follow_device;
            }
        });
    }
}

fn flash_amount(last: Option<Instant>, now: Instant) -> f32 {
    match last {
        Some(t) => {
            let dt = now.saturating_duration_since(t);
            if dt >= FLASH { 0.0 } else { 1.0 - dt.as_secs_f32() / FLASH.as_secs_f32() }
        }
        None => 0.0,
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let bar = egui::Frame::new().fill(theme::BG).inner_margin(Margin::symmetric(12, 8));
        egui::Panel::top("top").frame(bar).show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status")
            .show_separator_line(false)
            .frame(bar.inner_margin(Margin::symmetric(12, 4)))
            .show(ui, |ui| self.status_bar(ui));
        if self.show_log {
            egui::Panel::right("log")
                .frame(egui::Frame::new().fill(theme::BG).inner_margin(Margin::same(10)))
                .default_size(300.0)
                .resizable(true)
                .show(ui, |ui| self.log_panel(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(Margin::same(16)))
            .show(ui, |ui| match self.tab {
                Tab::Configure => self.config_tab(ui),
                Tab::Monitor => {
                    ui.vertical_centered(|ui| self.monitor(ui));
                }
            });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("Sixteen"),
        ..Default::default()
    };
    eframe::run_native("Sixteen", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
