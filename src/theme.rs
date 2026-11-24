//! Visual identity: deep graphite chrome that recedes, so the device's own
//! light (white LED rings, RGB halos) is the only thing that glows.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle,
};

pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1B, 0x20);
pub const RAISED: Color32 = Color32::from_rgb(0x22, 0x26, 0x2D);
pub const HOVER: Color32 = Color32::from_rgb(0x2B, 0x30, 0x38);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x2B, 0x32);
pub const TEXT: Color32 = Color32::from_rgb(0xE4, 0xE7, 0xEC);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x7E, 0x87, 0x94);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x4E, 0x55, 0x60);
pub const LED: Color32 = Color32::from_rgb(0xF6, 0xF8, 0xFF);
pub const CHANGED: Color32 = Color32::from_rgb(0xE8, 0xA8, 0x4A);
pub const OK: Color32 = Color32::from_rgb(0x5F, 0xC2, 0x8A);
pub const ERR: Color32 = Color32::from_rgb(0xE0, 0x6A, 0x6A);

// Hardware.
pub const CHASSIS: Color32 = Color32::from_rgb(0x0B, 0x0C, 0x0E);
pub const CHASSIS_EDGE: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x34);
pub const CAP: Color32 = Color32::from_rgb(0x15, 0x17, 0x1A);
pub const LED_OFF: Color32 = Color32::from_rgb(0x1F, 0x22, 0x27);
pub const SHIFT: Color32 = Color32::from_rgb(0xB9, 0x9C, 0xF0);

/// Phosphor (regular) icon glyphs, bundled in `assets/fonts/Phosphor.ttf` (MIT).
pub mod icon {
    pub const MONITOR: &str = "\u{e628}"; // gauge
    pub const CONFIGURE: &str = "\u{e434}"; // sliders-horizontal
    pub const READ: &str = "\u{e20c}"; // download-simple
    pub const WRITE: &str = "\u{e4c0}"; // upload-simple
    pub const REVERT: &str = "\u{e038}"; // arrow-counter-clockwise
    pub const REFRESH: &str = "\u{e094}"; // arrows-clockwise
    pub const CONNECTED: &str = "\u{eb5a}"; // plugs-connected
    pub const PLUG: &str = "\u{e946}"; // plug
    pub const COPY: &str = "\u{e1ca}"; // copy
    pub const TRASH: &str = "\u{e4a6}"; // trash
    pub const LOG: &str = "\u{e2f4}"; // list-dashes
    pub const ENCODERS: &str = "\u{e464}"; // squares-four
    pub const GLOBAL: &str = "\u{e272}"; // gear-six
    pub const OK: &str = "\u{e184}"; // check-circle
    pub const WARN: &str = "\u{e4e2}"; // warning-circle
    pub const FOLLOW: &str = "\u{e2e6}"; // link-simple
    pub const UNFOLLOW: &str = "\u{e2e8}"; // link-simple-break
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Frame used for every settings group.
pub fn group_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
}

/// Icon-only button with a tooltip.
pub fn icon_button(ui: &mut egui::Ui, glyph: &str, tip: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(glyph).size(17.0)).frame_when_inactive(false))
        .on_hover_text(tip)
}

/// Icon toggle (selected = highlighted) with a tooltip.
pub fn icon_toggle(ui: &mut egui::Ui, selected: bool, glyph: &str, tip: &str) -> egui::Response {
    let text = egui::RichText::new(glyph).size(17.0).color(if selected { LED } else { TEXT_DIM });
    ui.add(egui::Button::selectable(selected, text)).on_hover_text(tip)
}

fn load_fonts(fonts: &mut FontDefinitions) {
    let mut add_front = |name: &str, path: &str, family: FontFamily| {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fonts.families.entry(family).or_default().insert(0, name.into());
        }
    };
    add_front("sf", "/System/Library/Fonts/SFNS.ttf", FontFamily::Proportional);
    add_front("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", FontFamily::Monospace);

    fonts.font_data.insert(
        "phosphor".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/Phosphor.ttf"))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("phosphor".into());
    }
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    load_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Dark);

    ctx.all_styles_mut(|style| {
        use FontFamily::{Monospace, Proportional};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(17.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, Proportional)),
            (TextStyle::Small, FontId::new(10.5, Proportional)),
            (TextStyle::Monospace, FontId::new(11.5, Monospace)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 7.0);
        s.button_padding = egui::vec2(8.0, 5.0);
        s.interact_size.y = 26.0;
        s.combo_width = 180.0;

        let v = &mut style.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = CHASSIS;
        v.faint_bg_color = PANEL;
        v.window_corner_radius = CornerRadius::same(10);
        v.window_stroke = Stroke::new(1.0, LINE);
        v.popup_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) };
        v.selection.bg_fill = RAISED;
        v.selection.stroke = Stroke::new(1.0, LED);
        v.hyperlink_color = LED;
        v.override_text_color = None;

        let radius = CornerRadius::same(6);
        let w = &mut v.widgets;
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        for (state, fill, stroke) in [
            (&mut w.inactive, RAISED, Color32::TRANSPARENT),
            (&mut w.hovered, HOVER, Color32::TRANSPARENT),
            (&mut w.active, HOVER, LINE),
            (&mut w.open, HOVER, Color32::TRANSPARENT),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.corner_radius = radius;
            state.expansion = 0.0;
        }
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.hovered.fg_stroke = Stroke::new(1.0, LED);
        w.active.fg_stroke = Stroke::new(1.0, LED);
    });
}
