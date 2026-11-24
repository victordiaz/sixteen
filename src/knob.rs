//! Drawing of the Midi Fighter Twister: black chassis with corner screws,
//! 4×4 push encoders (knurled black caps, RGB halo at the base, 11-LED white
//! indicator ring) and three edge-mounted side buttons per side.

use std::f32::consts::PI;

use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use crate::theme::{self, CAP, CHASSIS, CHASSIS_EDGE, LED, LED_OFF, SHIFT, with_alpha};

pub const RING_LEDS: usize = 11;
const START: f32 = 0.75 * PI; // bottom-left, sweeping clockwise (y points down)
const SWEEP: f32 = 1.5 * PI;

pub struct Knob {
    /// Ring value 0..=127; None draws an unlit ring.
    pub value: Option<u8>,
    pub shift: Option<u8>,
    pub pressed: bool,
    /// RGB halo color (configured inactive/active color).
    pub rgb: Option<Color32>,
    /// 0..1 recent-activity glow.
    pub flash: f32,
    pub selected: bool,
}

/// Geometry of the device for a given grid pitch; shared by the monitor and the editor.
pub struct Layout {
    pub body: Rect,
    pub pitch: f32,
    pub grid_origin: Pos2,
}

impl Layout {
    pub fn new(top_left: Pos2, pitch: f32) -> Self {
        let margin = pitch * 0.42;
        let size = Vec2::splat(pitch * 4.0 + margin * 2.0);
        Self {
            body: Rect::from_min_size(top_left, size),
            pitch,
            grid_origin: top_left + Vec2::splat(margin),
        }
    }

    pub fn size(pitch: f32) -> Vec2 {
        // Room for the side buttons that stand proud of the chassis edges.
        Vec2::new(pitch * (4.0 + 0.84) + pitch * 0.16, pitch * (4.0 + 0.84))
    }

    pub fn knob_center(&self, i: usize) -> Pos2 {
        self.grid_origin + Vec2::new(self.pitch * ((i % 4) as f32 + 0.5), self.pitch * ((i / 4) as f32 + 0.5))
    }

    pub fn knob_radius(&self) -> f32 {
        self.pitch * 0.40
    }

    /// Side buttons: 0..3 left (top to bottom), 3..6 right.
    pub fn side_button(&self, i: usize) -> Rect {
        let h = self.pitch * 0.55;
        let w = self.pitch * 0.09;
        let y = self.grid_origin.y + self.pitch * (0.5 + (i % 3) as f32 * 1.5);
        let x = if i < 3 { self.body.left() - w * 0.55 } else { self.body.right() + w * 0.55 };
        Rect::from_center_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    pub fn hit_knob(&self, p: Pos2) -> Option<usize> {
        (0..16).find(|&i| self.knob_center(i).distance(p) <= self.pitch * 0.48)
    }
}

pub fn chassis(painter: &Painter, l: &Layout) {
    let r = CornerRadius::same((l.pitch * 0.22) as u8);
    let body = l.body;
    painter.rect_filled(body.translate(Vec2::new(0.0, l.pitch * 0.06)).expand(1.0), r, Color32::from_black_alpha(110));
    painter.rect_filled(body, r, CHASSIS);
    painter.rect_stroke(body, r, Stroke::new(1.0, CHASSIS_EDGE), StrokeKind::Inside);
    // Faint top-edge sheen of the anodized plate.
    painter.rect_stroke(body.shrink(1.5), r, Stroke::new(1.0, Color32::from_white_alpha(6)), StrokeKind::Inside);
    // Corner screws.
    let inset = l.pitch * 0.2;
    let sr = (l.pitch * 0.045).max(2.0);
    for c in [body.left_top(), body.right_top(), body.left_bottom(), body.right_bottom()] {
        let p = c + Vec2::new(
            if c.x < body.center().x { inset } else { -inset },
            if c.y < body.center().y { inset } else { -inset },
        );
        painter.circle_filled(p, sr, Color32::from_rgb(0x1C, 0x1F, 0x23));
        painter.circle_stroke(p, sr, Stroke::new(0.8, Color32::from_rgb(0x30, 0x34, 0x3A)));
        painter.line_segment([p - Vec2::new(sr * 0.55, 0.0), p + Vec2::new(sr * 0.55, 0.0)], Stroke::new(0.8, Color32::from_rgb(0x08, 0x09, 0x0A)));
    }
}

fn led_pos(center: Pos2, radius: f32, i: usize) -> Pos2 {
    let a = START + SWEEP * i as f32 / (RING_LEDS - 1) as f32;
    center + Vec2::new(a.cos(), a.sin()) * radius
}

pub fn knob(painter: &Painter, center: Pos2, r: f32, k: &Knob) {
    let led_r = (r * 0.06).max(1.4);
    let ring_r = r * 0.98;

    // 11 white indicator LEDs: lit up to the value, the leading one blended.
    let level = k.value.filter(|&v| v > 0).map(|v| v as f32 / 127.0 * (RING_LEDS - 1) as f32);
    for i in 0..RING_LEDS {
        let at = led_pos(center, ring_r, i);
        let lit = level.map_or(0.0, |l| (l - i as f32 + 1.0).clamp(0.0, 1.0));
        if lit > 0.0 {
            painter.circle_filled(at, led_r * 3.2, with_alpha(LED, (18.0 * lit) as u8));
            painter.circle_filled(at, led_r * 1.9, with_alpha(LED, (40.0 * lit) as u8));
        }
        painter.circle_filled(at, led_r, theme::lerp(LED_OFF, LED, lit));
    }

    // Shift layer value: a fine inner arc.
    if let Some(s) = k.shift.filter(|&s| s > 0) {
        let end = START + SWEEP * s as f32 / 127.0;
        let pts: Vec<Pos2> = (0..=32)
            .map(|j| {
                let a = START + (end - START) * j as f32 / 32.0;
                center + Vec2::new(a.cos(), a.sin()) * (r * 0.84)
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(1.5, SHIFT)));
    }

    // RGB light spilling around the base of the cap.
    let cap_r = r * 0.66;
    if let Some(rgb) = k.rgb.filter(|c| *c != Color32::BLACK) {
        let s = if k.pressed { 1.0 } else { 0.7 };
        painter.circle_filled(center, cap_r * 1.22, with_alpha(rgb, (40.0 * s) as u8));
        painter.circle_filled(center, cap_r * 1.10, with_alpha(rgb, (90.0 * s) as u8));
    }

    // Knurled black cap.
    let fill = theme::lerp(CAP, Color32::from_rgb(0x24, 0x28, 0x2E), k.flash * 0.7 + if k.pressed { 0.5 } else { 0.0 });
    painter.circle_filled(center, cap_r, fill);
    let ridges = 28;
    for j in 0..ridges {
        let a = 2.0 * PI * j as f32 / ridges as f32;
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment([center + dir * (cap_r * 0.86), center + dir * (cap_r - 0.5)], Stroke::new(0.8, Color32::from_black_alpha(160)));
    }
    painter.circle_stroke(center, cap_r * 0.84, Stroke::new(0.8, Color32::from_white_alpha(10)));
    // Top-left catch light.
    let hl: Vec<Pos2> = (0..=12)
        .map(|j| {
            let a = 1.05 * PI + 0.55 * PI * j as f32 / 12.0;
            center + Vec2::new(a.cos(), a.sin()) * (cap_r - 1.0)
        })
        .collect();
    painter.add(Shape::line(hl, Stroke::new(1.0, Color32::from_white_alpha(26))));
    // Pointer notch.
    if let Some(v) = k.value {
        let a = START + SWEEP * v as f32 / 127.0;
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment([center + dir * (cap_r * 0.35), center + dir * (cap_r * 0.75)], Stroke::new(1.6, with_alpha(LED, 150)));
    }

    if k.selected {
        painter.circle_stroke(center, r * 1.08, Stroke::new(1.2, with_alpha(LED, 200)));
    }
}

pub fn side_button(painter: &Painter, rect: Rect, pressed: bool, flash: f32) {
    let radius = CornerRadius::same((rect.width() / 2.0) as u8);
    let fill = if pressed {
        LED
    } else {
        theme::lerp(Color32::from_rgb(0x1E, 0x21, 0x26), Color32::from_rgb(0x3A, 0x40, 0x49), flash)
    };
    if pressed {
        painter.rect_filled(rect.expand(4.0), CornerRadius::same(radius.nw + 4), with_alpha(LED, 28));
    }
    painter.rect_filled(rect, radius, fill);
}

pub fn value_label(painter: &Painter, at: Pos2, text: &str, color: Color32) {
    painter.text(at, Align2::CENTER_CENTER, text, FontId::proportional(10.0), color);
}
