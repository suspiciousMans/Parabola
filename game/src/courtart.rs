//! Court card art: five small emblems drawn with the UI painter.
//!
//! The game ships no image files (the planets, ships and board are generated
//! too), so each Court card gets its own figure built from primitives. The
//! silhouettes are deliberately different at 40 px wide: a banner, a wax seal,
//! a scroll, linked rings and a three-way accord.

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use reach_rules::CourtCard;

use crate::ui::{BRASS, INK};

const FACE: Color32 = Color32::from_rgb(23, 26, 34);

fn shade(c: Color32, k: f32) -> Color32 {
    Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8)
}

fn line(p: &Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    p.line_segment([a, b], stroke);
}

fn poly(p: &Painter, pts: Vec<Pos2>, fill: Color32) {
    p.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
}

/// Paints a Court card face into `r`. `enabled` dims the emblem when the card
/// cannot be secured right now; `alpha` is used by the deal-in animation.
pub fn paint(p: &Painter, r: Rect, card: CourtCard, enabled: bool, alpha: f32) {
    let brass = if enabled { BRASS } else { shade(BRASS, 0.55) };
    let ink = if enabled { INK } else { shade(INK, 0.55) };
    let a = alpha.clamp(0.0, 1.0);
    let fill = Color32::from_rgba_unmultiplied(FACE.r(), FACE.g(), FACE.b(), (255.0 * a) as u8);
    let edge = Color32::from_rgba_unmultiplied(brass.r(), brass.g(), brass.b(), (150.0 * a) as u8);
    p.rect_filled(r, 2.0, fill);
    p.rect_stroke(r, 2.0, Stroke::new(1.0, edge));
    let inner = r.shrink(4.0);
    let col = Color32::from_rgba_unmultiplied(brass.r(), brass.g(), brass.b(), (255.0 * a) as u8);
    let pale = Color32::from_rgba_unmultiplied(ink.r(), ink.g(), ink.b(), (220.0 * a) as u8);
    let thin = Stroke::new(1.0, shade(col, 0.7));
    let strong = Stroke::new(1.4, col);
    let c = inner.center();

    match card.0 % 5 {
        // Envoy: a standing figure with a pennant.
        0 => {
            let head = pos2(c.x - inner.width() * 0.13, inner.top() + inner.height() * 0.25);
            p.circle_stroke(head, inner.width() * 0.10, strong);
            let body = head + vec2(0.0, inner.height() * 0.14);
            line(p, body, body + vec2(0.0, inner.height() * 0.42), strong);
            line(p, body + vec2(-inner.width() * 0.16, inner.height() * 0.20), body + vec2(inner.width() * 0.16, inner.height() * 0.20), thin);
            line(p, body + vec2(0.0, inner.height() * 0.42), body + vec2(-inner.width() * 0.13, inner.height() * 0.58), thin);
            line(p, body + vec2(0.0, inner.height() * 0.42), body + vec2(inner.width() * 0.13, inner.height() * 0.58), thin);
            let staff = pos2(inner.right() - inner.width() * 0.20, inner.top() + inner.height() * 0.12);
            line(p, staff, staff + vec2(0.0, inner.height() * 0.76), strong);
            poly(p, vec![staff + vec2(0.0, inner.height() * 0.06), staff + vec2(inner.width() * 0.30, inner.height() * 0.16), staff + vec2(0.0, inner.height() * 0.28)], col);
        }
        // Seal: a wax stamp with ribbons.
        1 => {
            let rad = inner.height().min(inner.width()) * 0.33;
            p.circle_stroke(c - vec2(0.0, inner.height() * 0.06), rad, strong);
            p.circle_stroke(c - vec2(0.0, inner.height() * 0.06), rad * 0.72, thin);
            for k in 0..8 {
                let ang = k as f32 / 8.0 * std::f32::consts::TAU;
                let (s, co) = ang.sin_cos();
                line(
                    p,
                    c - vec2(0.0, inner.height() * 0.06) + vec2(co, s) * rad * 0.30,
                    c - vec2(0.0, inner.height() * 0.06) + vec2(co, s) * rad * 0.66,
                    thin,
                );
            }
            p.circle_filled(c - vec2(0.0, inner.height() * 0.06), 2.0, pale);
            let tail = c + vec2(0.0, inner.height() * 0.30);
            poly(p, vec![tail, tail + vec2(-inner.width() * 0.20, inner.height() * 0.26), tail + vec2(-inner.width() * 0.04, inner.height() * 0.12)], col);
            poly(p, vec![tail + vec2(inner.width() * 0.06, 0.0), tail + vec2(inner.width() * 0.22, inner.height() * 0.24), tail + vec2(inner.width() * 0.02, inner.height() * 0.12)], shade(col, 0.8));
        }
        // Writ: a rolled scroll with a stamped corner.
        2 => {
            let s = Rect::from_center_size(c, vec2(inner.width() * 0.70, inner.height() * 0.78));
            p.rect_filled(s, 2.0, shade(FACE, 1.6));
            p.rect_stroke(s, 2.0, thin);
            for k in 0..3 {
                let y = s.top() + s.height() * (0.24 + k as f32 * 0.18);
                line(p, pos2(s.left() + s.width() * 0.16, y), pos2(s.right() - s.width() * (if k == 2 { 0.42 } else { 0.14 }), y), thin);
            }
            line(p, pos2(s.left() - 2.0, s.top() + 3.0), pos2(s.right() + 2.0, s.top() + 3.0), strong);
            line(p, pos2(s.left() - 2.0, s.bottom() - 3.0), pos2(s.right() + 2.0, s.bottom() - 3.0), strong);
            p.circle_filled(pos2(s.right() - s.width() * 0.22, s.bottom() - s.height() * 0.22), 3.0, col);
        }
        // Pact: two linked rings over a handshake bar.
        3 => {
            let r0 = inner.height() * 0.25;
            let a = c + vec2(-inner.width() * 0.15, -inner.height() * 0.04);
            let b = c + vec2(inner.width() * 0.15, inner.height() * 0.04);
            p.circle_stroke(a, r0, strong);
            p.circle_stroke(b, r0, strong);
            p.circle_stroke(a, r0 * 0.56, thin);
            p.circle_stroke(b, r0 * 0.56, thin);
            line(p, c + vec2(-inner.width() * 0.30, inner.height() * 0.32), c + vec2(inner.width() * 0.02, inner.height() * 0.20), thin);
            line(p, c + vec2(-inner.width() * 0.02, inner.height() * 0.20), c + vec2(inner.width() * 0.30, inner.height() * 0.32), thin);
            poly(p, vec![c + vec2(-4.0, inner.height() * 0.20), c + vec2(0.0, inner.height() * 0.14), c + vec2(4.0, inner.height() * 0.20), c + vec2(0.0, inner.height() * 0.26)], pale);
        }
        // Accord: three houses joined in a triangle.
        _ => {
            let r0 = inner.width() * 0.14;
            let top = c + vec2(0.0, -inner.height() * 0.26);
            let l = c + vec2(-inner.width() * 0.26, inner.height() * 0.22);
            let r = c + vec2(inner.width() * 0.26, inner.height() * 0.22);
            for (a, b) in [(top, l), (l, r), (r, top)] {
                line(p, a, b, thin);
            }
            for node in [top, l, r] {
                p.circle_stroke(node, r0, strong);
                p.circle_filled(node, r0 * 0.34, col);
            }
            p.circle_filled(c, 2.0, pale);
        }
    }
}
