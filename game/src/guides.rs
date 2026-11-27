//! Board guide labels: a toggleable overlay that names the key areas of the
//! table and says what they are for. Anchors come from the projected board, so
//! the labels follow the camera.

use egui::{pos2, vec2, Color32, FontId, Painter, Pos2, Rect, Stroke};
use std::cell::Cell;

use crate::ui::{BRASS, DIM, HAIR, INK, PANEL};

/// Projected screen points for the board's key areas, in points.
#[derive(Clone, Copy, Debug, Default)]
pub struct Anchors {
    pub ring: [f32; 2],
    pub gates: [f32; 2],
    pub court: [f32; 2],
    pub ambitions: [f32; 2],
    pub power: [f32; 2],
    pub played: [f32; 2],
}

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

pub fn is_open() -> bool {
    OPEN.with(|o| o.get())
}

pub fn close() {
    OPEN.with(|o| o.set(false));
}

pub fn toggle() {
    OPEN.with(|o| o.set(!o.get()));
}

struct Chip {
    title: &'static str,
    line: &'static str,
    line2: String,
    anchor: Pos2,
    at: Pos2,
}

fn chip(p: &Painter, c: &Chip, compact: bool) {
    let size = if compact { vec2(122.0, 22.0) } else { vec2(180.0, 52.0) };
    let rect = Rect::from_min_size(c.at, size);
    p.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(10, 13, 18, 244));
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, HAIR));
    p.line_segment([c.anchor, rect.center()], Stroke::new(1.0, BRASS.gamma_multiply(0.55)));
    p.circle_filled(c.anchor, 3.0, BRASS);
    p.circle_stroke(c.anchor, 5.0, Stroke::new(1.0, BRASS.gamma_multiply(0.45)));
    p.text(rect.min + vec2(9.0, if compact { 6.0 } else { 7.0 }), egui::Align2::LEFT_TOP, c.title, FontId::new(10.0, egui::FontFamily::Name("head".into())), BRASS);
    if !compact {
        p.text(rect.min + vec2(9.0, 24.0), egui::Align2::LEFT_TOP, c.line, FontId::new(10.0, egui::FontFamily::Proportional), INK);
        p.text(rect.min + vec2(9.0, 37.0), egui::Align2::LEFT_TOP, &c.line2, FontId::new(9.0, egui::FontFamily::Proportional), DIM);
    }
}

/// Paints the guide when it is open. `goal` is the current table's power goal.
pub fn draw(ctx: &egui::Context, a: &Anchors, goal: i64) {
    if !is_open() {
        return;
    }
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("board_guide")));
    let at = |x: f32, y: f32| pos2(x.clamp(8.0, (w - 188.0).max(8.0)), y.clamp(8.0, (h - 60.0).max(8.0)));
    let goal_line = format!("First to {goal} power ends it after a chapter.");
    let narrow = w < 620.0;
    let mut chips = [
        Chip {
            title: "COURT",
            line: "Spend 1 Relic in your action step.",
            line2: "The card leaves the row: +1 power.".into(),
            anchor: pos2(a.court[0], a.court[1]),
            at: at(w * 0.5 - 90.0, h * 0.12),
        },
        Chip {
            title: "GATES",
            line: "Gate systems join the clusters.",
            line2: "Ships travel along the lanes.".into(),
            anchor: pos2(a.gates[0], a.gates[1]),
            at: at(w - 420.0, h * 0.40),
        },
        Chip {
            title: "AMBITIONS",
            line: "Declared scoring cards.",
            line2: "They pay out every chapter end.".into(),
            anchor: pos2(a.ambitions[0], a.ambitions[1]),
            at: at(w - 200.0, h * 0.36 + 110.0),
        },
        Chip {
            title: "PLAYED CARDS",
            line: "The lead and follow cards decide",
            line2: "the trick. Highest lead suit wins.".into(),
            anchor: pos2(a.played[0], a.played[1]),
            at: at(20.0, h * 0.60),
        },
        Chip {
            title: "THE RING",
            line: "Move, tax, build and fight here.",
            line2: "Click a system to filter actions.".into(),
            anchor: pos2(a.ring[0], a.ring[1]),
            at: at(w * 0.5 - 90.0, (h * 0.60).min(h - 330.0)),
        },
        Chip {
            title: "POWER TRACK",
            line: "Each piece marks a player's power.",
            line2: goal_line,
            anchor: pos2(a.power[0], a.power[1]),
            at: at(w * 0.5 - 90.0, (h * 0.73).min(h - 250.0)),
        },
    ];
    if narrow {
        // No room for two-line chips beside the table: label the areas with a
        // compact, collision-free row instead. The manual carries the detail.
        let place = |x: f32, y: f32| pos2(x.clamp(8.0, (w - 130.0).max(8.0)), y.clamp(8.0, (h - 30.0).max(8.0)));
        chips[0].at = place(w * 0.5 - 61.0, 96.0);
        chips[1].at = place(10.0, h * 0.40);
        chips[2].at = place(w - 132.0, h * 0.42);
        chips[3].at = place(10.0, h * 0.52);
        chips[4].at = place(w * 0.5 - 61.0, h * 0.53);
        chips[5].at = place(w * 0.5 - 61.0, h * 0.67);
    }
    for c in &chips {
        chip(&p, c, narrow);
    }
    let hint_y = if narrow { h - 22.0 } else { 64.0 };
    let hint = Rect::from_center_size(pos2(w * 0.5, hint_y), vec2(222.0, 22.0));
    p.rect_filled(hint, 2.0, PANEL);
    p.rect_stroke(hint, 2.0, Stroke::new(1.0, HAIR));
    p.text(hint.center(), egui::Align2::CENTER_CENTER, "BOARD GUIDE  ·  G TO CLOSE", FontId::new(10.0, egui::FontFamily::Name("head".into())), BRASS);
}
