//! "Configure" tab: read, edit and write the Twister configuration.

use std::collections::BTreeMap;

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::colors;
use crate::config::{self, ENCODER_SETTINGS, GLOBAL_SETTINGS, EncoderRecord, Kind, Setting, TwisterConfig};
use crate::knob;
use crate::theme::{self, CHANGED, icon};
use crate::worker::Command;

#[derive(Default, PartialEq, Clone, Copy)]
pub enum View {
    #[default]
    Encoders,
    Global,
}

#[derive(Default)]
pub struct ConfigUi {
    /// Configuration being edited.
    pub edit: Option<TwisterConfig>,
    /// Last configuration read from / written to the device.
    pub device: Option<TwisterConfig>,
    pub view: View,
    pub bank: usize,
    pub encoder: usize,
    pub color_picker: Option<u8>,
    /// Set once the automatic first read has been sent.
    pub auto_read_sent: bool,
}

fn count_changes(edit: &TwisterConfig, device: &TwisterConfig) -> usize {
    let globals = edit.globals.iter().filter(|(t, v)| device.globals.get(t) != Some(v)).count();
    let encoders: usize = edit
        .encoders
        .iter()
        .zip(&device.encoders)
        .map(|(a, b)| a.pairs.iter().zip(&b.pairs).filter(|(x, y)| x != y).count())
        .sum();
    globals + encoders
}

impl super::App {
    pub fn config_tab(&mut self, ui: &mut egui::Ui) {
        if let Some(cfg) = self.cfg_status.lock().unwrap().read_result.take() {
            self.cfg.bank = self.cfg.bank.min(cfg.banks().saturating_sub(1));
            self.cfg.device = Some(cfg.clone());
            self.cfg.edit = Some(cfg);
        }
        let busy = self.cfg_status.lock().unwrap().busy;

        if self.cfg.edit.is_none() {
            // Read automatically the first time the page is opened.
            if !self.cfg.auto_read_sent && !busy && self.connection.is_some() {
                self.cfg.auto_read_sent = true;
                self.read();
            }
            if empty_state(ui, busy, self.selected_port.is_some()) {
                self.read();
            }
            return;
        }

        self.config_header(ui, busy);
        ui.add_space(12.0);

        let ConfigUi { edit, device, view, bank, encoder, color_picker, .. } = &mut self.cfg;
        let edit = edit.as_mut().unwrap();
        let device = device.as_ref();
        let palette = colors::palette(edit.globals.get(&33) == Some(&1));

        match view {
            View::Global => {
                egui::ScrollArea::vertical().id_salt("globals").auto_shrink([false, false]).show(ui, |ui| {
                    globals_editor(ui, &mut edit.globals, device.map(|d| &d.globals));
                });
            }
            View::Encoders => {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        if let Some(sel) = device_picker(ui, edit, *bank, *encoder, palette) {
                            *encoder = sel;
                            *color_picker = None;
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            // Center the bank row under the device.
                            let device_w = knob::Layout::size(PICKER_PITCH).x;
                            let row_w = edit.banks() as f32 * (28.0 + 8.0) + 8.0 + 34.0;
                            ui.add_space(((device_w - row_w) / 2.0).max(0.0));
                            for b in 0..edit.banks() {
                                let text = RichText::new((b + 1).to_string()).color(if *bank == b { theme::LED } else { theme::TEXT_DIM });
                                if ui.add(egui::Button::selectable(*bank == b, text).min_size(Vec2::new(28.0, 24.0))).on_hover_text(format!("Bank {}", b + 1)).clicked() {
                                    *bank = b;
                                }
                            }
                            ui.add_space(8.0);
                            if theme::icon_button(ui, icon::COPY, "Copy to the whole bank (keeps MIDI numbers)").clicked() {
                                copy_to_bank(edit, *bank, *bank * 16 + *encoder);
                            }
                        });
                    });
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        let idx = *bank * 16 + *encoder;
                        let reference = device.and_then(|d| d.encoders.get(idx));
                        egui::ScrollArea::vertical().id_salt("encoder").auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(record) = edit.encoders.get_mut(idx) {
                                encoder_editor(ui, record, reference, palette, color_picker);
                            }
                        });
                    });
                });
            }
        }
    }

    fn read(&mut self) {
        if let Some(port) = self.selected_port.clone() {
            let _ = self.cfg_tx.send(Command::Read { port });
        }
    }

    fn config_header(&mut self, ui: &mut egui::Ui, busy: bool) {
        let changes = match (&self.cfg.edit, &self.cfg.device) {
            (Some(e), Some(d)) => count_changes(e, d),
            _ => 0,
        };
        ui.horizontal(|ui| {
            for (view, glyph, tip) in [(View::Encoders, icon::ENCODERS, "Encoders"), (View::Global, icon::GLOBAL, "Global")] {
                if theme::icon_toggle(ui, self.cfg.view == view, glyph, tip).clicked() {
                    self.cfg.view = view;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!busy && self.selected_port.is_some(), |ui| {
                    let (fill, text) = if changes > 0 { (CHANGED, theme::BG) } else { (theme::RAISED, theme::TEXT) };
                    let write = egui::Button::new(RichText::new(format!("{}  Write", icon::WRITE)).color(text).strong())
                        .fill(fill)
                        .min_size(Vec2::new(92.0, 30.0));
                    if ui.add(write).on_hover_text("Store these settings on the Twister").clicked() {
                        if let (Some(edit), Some(port)) = (self.cfg.edit.clone(), self.selected_port.clone()) {
                            self.cfg.device = Some(edit.clone());
                            let _ = self.cfg_tx.send(Command::Write { port, config: edit });
                        }
                    }
                    let read = egui::Button::new(RichText::new(format!("{}  Read", icon::READ)).color(theme::TEXT))
                        .frame_when_inactive(false)
                        .min_size(Vec2::new(80.0, 30.0));
                    if ui.add(read).on_hover_text("Load the settings stored on the Twister").clicked() {
                        self.read();
                    }
                    if changes > 0 {
                        if theme::icon_button(ui, icon::REVERT, "Discard changes").clicked() {
                            self.cfg.edit = self.cfg.device.clone();
                        }
                        ui.label(RichText::new(changes.to_string()).color(CHANGED)).on_hover_text("Unsaved changes");
                    }
                });
            });
        });
    }
}

/// Returns true when the read button was pressed.
fn empty_state(ui: &mut egui::Ui, busy: bool, has_port: bool) -> bool {
    let mut read = false;
    ui.add_space(ui.available_height() * 0.36);
    ui.vertical_centered(|ui| {
        let text = RichText::new(icon::READ).size(30.0).color(if busy { theme::TEXT_FAINT } else { theme::LED });
        let button = egui::Button::new(text).min_size(Vec2::splat(64.0)).corner_radius(CornerRadius::same(32));
        let tip = if has_port { "Read from Twister" } else { "Choose the Twister as MIDI input" };
        read = ui.add_enabled(!busy && has_port, button).on_hover_text(tip).on_disabled_hover_text(tip).clicked();
    });
    read
}

fn copy_to_bank(edit: &mut TwisterConfig, bank: usize, source: usize) {
    const KEEP: [u8; 2] = [14, 17]; // button / rotary MIDI number
    let src = edit.encoders[source].clone();
    for i in bank * 16..(bank * 16 + 16).min(edit.encoders.len()) {
        if i == source {
            continue;
        }
        for &(tag, value) in &src.pairs {
            if !KEEP.contains(&tag) {
                edit.encoders[i].set(tag, value);
            }
        }
    }
}

/// Lays out setting groups as cards in two independent columns (masonry), so a
/// short card never leaves a gap next to a tall one. Each card goes to the
/// currently shorter column.
fn group_cards<T>(ui: &mut egui::Ui, groups: &[(&'static str, Vec<T>)], mut row: impl FnMut(&mut egui::Ui, &T)) {
    let gap = 12.0;
    let columns = if ui.available_width() > 640.0 { 2 } else { 1 };
    let width = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;

    let mut assigned: Vec<Vec<usize>> = vec![Vec::new(); columns];
    let mut heights = vec![0usize; columns];
    for (i, (_, items)) in groups.iter().enumerate() {
        let c = (0..columns).min_by_key(|&c| heights[c]).unwrap();
        assigned[c].push(i);
        heights[c] += items.len() + 2; // rows + title/padding
    }

    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for column in &assigned {
            ui.vertical(|ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = gap;
                for &i in column {
                    let (title, items) = &groups[i];
                    theme::group_frame().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(width - 28.0);
                            ui.label(RichText::new(*title).size(12.0).strong().color(theme::TEXT_DIM));
                            ui.add_space(6.0);
                            egui::Grid::new(("grp", *title))
                                .num_columns(2)
                                .min_col_width(((width - 28.0) * 0.42).min(170.0))
                                .spacing([12.0, 8.0])
                                .show(ui, |ui| {
                                    for item in items {
                                        row(ui, item);
                                    }
                                });
                        });
                    });
                }
            });
        }
    });
}

fn grouped(settings: Vec<Setting>) -> Vec<(&'static str, Vec<Setting>)> {
    let mut groups: Vec<(&'static str, Vec<Setting>)> = Vec::new();
    for s in settings {
        match groups.iter_mut().find(|g| g.0 == s.group) {
            Some(g) => g.1.push(s),
            None => groups.push((s.group, vec![s])),
        }
    }
    groups
}

fn label_for(ui: &mut egui::Ui, setting: &Setting, changed: bool) {
    let text = if setting.label.is_empty() { format!("#{}", setting.tag) } else { setting.label.to_string() };
    let color = if changed { CHANGED } else { theme::TEXT_DIM };
    ui.label(RichText::new(text).color(color));
}

fn globals_editor(ui: &mut egui::Ui, globals: &mut BTreeMap<u8, u8>, reference: Option<&BTreeMap<u8, u8>>) {
    let snapshot = globals.clone();
    let known: Vec<Setting> = GLOBAL_SETTINGS
        .iter()
        .filter(|s| snapshot.contains_key(&s.tag))
        .map(|s| config::global_setting(s, &snapshot))
        .collect();
    // Tags without a known meaning are kept and written back, just not shown.
    let groups = grouped(known);
    group_cards(ui, &groups, |ui, setting| {
        let value = globals.get_mut(&setting.tag).unwrap();
        let changed = reference.and_then(|r| r.get(&setting.tag)) != Some(value);
        label_for(ui, setting, changed);
        setting_widget(ui, setting, value, colors::palette(false), &mut None);
        ui.end_row();
    });
}

fn encoder_editor(
    ui: &mut egui::Ui,
    record: &mut EncoderRecord,
    reference: Option<&EncoderRecord>,
    palette: &[Color32; 128],
    color_picker: &mut Option<u8>,
) {
    let known: Vec<Setting> = ENCODER_SETTINGS.iter().filter(|s| record.get(s.tag).is_some()).copied().collect();
    let groups = grouped(known);
    group_cards(ui, &groups, |ui, setting| {
        let mut value = record.get(setting.tag).unwrap();
        let changed = reference.and_then(|r| r.get(setting.tag)) != Some(value);
        label_for(ui, setting, changed);
        if setting_widget(ui, setting, &mut value, palette, color_picker) {
            record.set(setting.tag, value);
        }
        ui.end_row();
        if matches!(setting.kind, Kind::Color) && *color_picker == Some(setting.tag) {
            ui.label("");
            if let Some(picked) = palette_grid(ui, palette, value) {
                record.set(setting.tag, picked);
                *color_picker = None;
            }
            ui.end_row();
        }
    });
}

/// Returns true when the value changed.
fn setting_widget(
    ui: &mut egui::Ui,
    setting: &Setting,
    value: &mut u8,
    palette: &[Color32; 128],
    color_picker: &mut Option<u8>,
) -> bool {
    let before = *value;
    let id = ("setting", setting.tag, setting.group);
    match setting.kind {
        Kind::Int { min, max } => {
            ui.add(egui::DragValue::new(value).range(min..=max).speed(0.3));
        }
        Kind::Bool => {
            let mut on = *value != 0;
            if toggle(ui, &mut on).changed() {
                *value = on as u8;
            }
        }
        Kind::BoolOffset => {
            let mut on = *value == 2;
            if toggle(ui, &mut on).changed() {
                *value = if on { 2 } else { 1 };
            }
        }
        Kind::Enum(options) => {
            let current = options
                .iter()
                .find(|o| o.1 == *value)
                .map(|o| o.0.to_string())
                .unwrap_or_else(|| format!("#{value}"));
            egui::ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
                for &(name, v) in options {
                    ui.selectable_value(value, v, name);
                }
            });
        }
        Kind::EnumOffset(names) => {
            let current = match *value {
                0 => "—".to_string(),
                v => names.get(v as usize - 1).map(|s| s.to_string()).unwrap_or_else(|| format!("#{v}")),
            };
            egui::ComboBox::from_id_salt(id).selected_text(current).show_ui(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    ui.selectable_value(value, i as u8 + 1, *name);
                }
            });
        }
        Kind::Color => {
            let color = palette[(*value & 0x7F) as usize];
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(48.0, 20.0), Sense::click());
            let open = *color_picker == Some(setting.tag);
            ui.painter().rect_filled(rect, CornerRadius::same(10), color);
            let stroke = if open || resp.hovered() { theme::LED } else { theme::LINE };
            ui.painter().rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, stroke), StrokeKind::Outside);
            if resp.on_hover_text(format!("Color {}", *value)).clicked() {
                *color_picker = if open { None } else { Some(setting.tag) };
            }
        }
    }
    *value != before
}

/// Compact switch.
fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = Vec2::new(32.0, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let fill = theme::lerp(theme::RAISED, Color32::from_rgb(0xC8, 0xCE, 0xD8), t);
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t);
    let knob = theme::lerp(theme::TEXT_DIM, theme::BG, t);
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 6.5, knob);
    resp
}

fn palette_grid(ui: &mut egui::Ui, palette: &[Color32; 128], current: u8) -> Option<u8> {
    const COLS: usize = 16;
    const CELL: f32 = 16.0;
    let size = Vec2::new(COLS as f32 * CELL, (128 / COLS) as f32 * CELL);
    let (resp, painter) = ui.allocate_painter(size, Sense::click());
    let hovered = resp.hover_pos().map(|p| {
        let d = p - resp.rect.min;
        (d.y / CELL) as usize * COLS + (d.x / CELL) as usize
    });
    for (i, &c) in palette.iter().enumerate() {
        let min = resp.rect.min + Vec2::new((i % COLS) as f32 * CELL, (i / COLS) as f32 * CELL);
        let r = Rect::from_min_size(min, Vec2::splat(CELL)).shrink(1.5);
        painter.rect_filled(r, CornerRadius::same(3), c);
        if i == current as usize || hovered == Some(i) {
            painter.rect_stroke(r, CornerRadius::same(3), Stroke::new(1.5, theme::LED), StrokeKind::Outside);
        }
    }
    if let Some(i) = hovered.filter(|&i| i < 128) {
        resp.clone().on_hover_text(i.to_string());
        if resp.clicked() {
            return Some(i as u8);
        }
    }
    None
}

fn short_kind(rec: &EncoderRecord) -> String {
    let kind = match rec.get(18) {
        Some(0) => "N",
        Some(1) => "CC",
        Some(2) => "R",
        Some(3) => "V",
        Some(4 | 5) => "M",
        Some(6) => "PC",
        Some(7) => "F",
        Some(8) => "VP",
        Some(9) => return "–".into(),
        _ => "",
    };
    format!("{kind}{}", rec.get(17).unwrap_or(0))
}

/// The device drawn as in Monitor, used to pick an encoder; returns the clicked one.
const PICKER_PITCH: f32 = 74.0;

fn device_picker(ui: &mut egui::Ui, edit: &TwisterConfig, bank: usize, selected: usize, palette: &[Color32; 128]) -> Option<usize> {
    let pitch = PICKER_PITCH;
    let (resp, painter) = ui.allocate_painter(knob::Layout::size(pitch), Sense::click());
    let layout = knob::Layout::new(resp.rect.min + Vec2::new(pitch * 0.08, 0.0), pitch);
    knob::chassis(&painter, &layout);
    for i in 0..16 {
        let Some(rec) = edit.encoders.get(bank * 16 + i) else { continue };
        let center = layout.knob_center(i);
        knob::knob(
            &painter,
            center,
            layout.knob_radius(),
            &knob::Knob {
                value: None,
                shift: None,
                pressed: false,
                rgb: rec.get(20).map(|c| palette[(c & 0x7F) as usize]),
                flash: 0.0,
                selected: i == selected,
            },
        );
        let color = if i == selected { theme::LED } else { theme::TEXT_FAINT };
        knob::value_label(&painter, center + Vec2::new(0.0, pitch * 0.47), &short_kind(rec), color);
    }
    let hovered = resp.hover_pos().and_then(|p| layout.hit_knob(p));
    if let Some(i) = hovered {
        if let Some(rec) = edit.encoders.get(bank * 16 + i) {
            let kind = ENCODER_SETTINGS.iter().find(|s| s.tag == 18).and_then(|s| match s.kind {
                Kind::Enum(o) => o.iter().find(|x| Some(x.1) == rec.get(18)).map(|x| x.0),
                _ => None,
            });
            resp.clone().on_hover_text(format!(
                "{} {}, ch {}",
                kind.unwrap_or("?"),
                rec.get(17).unwrap_or(0),
                rec.get(16).unwrap_or(0)
            ));
        }
    }
    if resp.clicked() { resp.interact_pointer_pos().and_then(|p| layout.hit_knob(p)) } else { None }
}
