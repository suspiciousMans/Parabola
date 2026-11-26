//! Action cards: how they look, and the little animations around playing them.
//!
//! Face anatomy follows the printed cards: suit name and action line at the
//! top, a big number with its action pips (four-point stars) beside it, an
//! art panel, and a scalloped bottom edge. The art is our own: one line
//! drawing per suit, drawn with the UI painter.

use egui::{pos2, vec2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke};
use reach_rules::{Card, State};
use std::cell::RefCell;
use std::collections::HashMap;

pub const CARD_W: f32 = 96.0;
pub const CARD_H: f32 = 136.0;

const LINES: [&str; 4] = ["TAX · REPAIR", "BATTLE · MOVE", "BUILD · REPAIR", "MOVE"];

fn suit_col(s: usize) -> Color32 {
    crate::ui::suit_color(s)
}

fn head(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("head".into()))
}

fn mul(c: Color32, k: f32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8, (a * 255.0) as u8)
}

pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A four-point star made of two thin diamonds.
fn star(p: &Painter, c: Pos2, r: f32, col: Color32) {
    let d = |a: f32, b: f32| vec![pos2(c.x, c.y - a), pos2(c.x + b, c.y), pos2(c.x, c.y + a), pos2(c.x - b, c.y)];
    p.add(Shape::convex_polygon(d(r, r * 0.38), col, Stroke::NONE));
    p.add(Shape::convex_polygon(d(r * 0.38, r), col, Stroke::NONE));
}

fn art(p: &Painter, r: Rect, suit: usize, col: Color32, a: f32) {
    p.rect_filled(r, 1.0, mul(col, 0.16, a));
    let line = Stroke::new(1.3, mul(col, 1.0, a * 0.85));
    let thin = Stroke::new(1.0, mul(col, 0.8, a * 0.5));
    let c = r.center();
    match suit {
        // Administration: a seal
        0 => {
            for (k, rad) in [0.42, 0.3, 0.18].iter().enumerate() {
                p.circle_stroke(c, r.height() * rad, if k == 0 { line } else { thin });
            }
            for k in 0..12 {
                let ang = k as f32 / 12.0 * std::f32::consts::TAU;
                let (s, co) = ang.sin_cos();
                p.line_segment([c + vec2(co, s) * r.height() * 0.42, c + vec2(co, s) * r.height() * 0.5], thin);
            }
            p.circle_filled(c, 2.5, mul(col, 1.0, a));
        }
        // Aggression: crossed blades
        1 => {
            for dir in [-1.0f32, 1.0] {
                let (a0, a1) = (c + vec2(-dir * r.width() * 0.4, -r.height() * 0.42), c + vec2(dir * r.width() * 0.4, r.height() * 0.42));
                let m = (a1 - a0).normalized();
                let n = vec2(-m.y, m.x) * 3.2;
                p.add(Shape::convex_polygon(vec![a0, c + n * 0.5, a1, c - n * 0.5], mul(col, 0.55, a * 0.9), line));
            }
            p.circle_stroke(c, r.height() * 0.2, thin);
        }
        // Construction: stacked blocks and a mast
        2 => {
            let w = r.width();
            let base = r.bottom() - 5.0;
            for (k, (bw, bh)) in [(0.7, 0.2), (0.5, 0.2), (0.3, 0.2)].iter().enumerate() {
                let y = base - (k as f32 + 1.0) * r.height() * 0.22;
                let br = Rect::from_center_size(pos2(c.x, y + r.height() * 0.1), vec2(w * bw, r.height() * bh));
                p.rect_stroke(br, 0.0, line);
                p.line_segment([br.left_center(), br.right_center()], thin);
            }
            p.line_segment([pos2(c.x, r.top() + 4.0), pos2(c.x, base - r.height() * 0.62)], line);
            p.line_segment([pos2(c.x, r.top() + 6.0), pos2(c.x + w * 0.28, r.top() + 14.0)], thin);
        }
        // Mobilization: a fan of arrows
        _ => {
            for k in -1..=1 {
                let ang = -std::f32::consts::FRAC_PI_2 + k as f32 * 0.45;
                let dir = vec2(ang.cos(), ang.sin());
                let tail = pos2(c.x, r.bottom() - 6.0);
                let tip = tail + dir * r.height() * 0.8;
                p.line_segment([tail, tip], line);
                let n = vec2(-dir.y, dir.x);
                p.add(Shape::convex_polygon(vec![tip + dir * 5.0, tip - dir * 4.0 + n * 4.0, tip - dir * 4.0 - n * 4.0], mul(col, 1.0, a * 0.9), Stroke::NONE));
            }
        }
    }
}

/// Paints one card. `flip` 0 = face up, 1 = face down (the card narrows through
/// edge-on to turn over). `alpha` fades the whole card.
pub fn paint_card(p: &Painter, rect: Rect, card: Option<Card>, flip: f32, alpha: f32) {
    let s = (std::f32::consts::PI * flip).cos().abs().max(0.02);
    let rect = Rect::from_center_size(rect.center(), vec2(rect.width() * s, rect.height()));
    let back = flip > 0.5 || card.is_none();
    let body_h = rect.height() - 6.0;
    let body = Rect::from_min_size(rect.min, vec2(rect.width(), body_h));
    // shadow
    p.rect_filled(body.translate(vec2(0.0, 3.0)).expand(1.0), 4.0, Color32::from_rgba_unmultiplied(0, 0, 0, (70.0 * alpha) as u8));

    let (face, edge) = match card {
        Some(c) if !back => (mul(Color32::from_rgb(26, 29, 38), 1.0, alpha), mul(suit_col(c.suit()), 0.9, alpha)),
        _ => (mul(Color32::from_rgb(22, 25, 33), 1.0, alpha), mul(crate::ui::BRASS, 0.7, alpha)),
    };
    // scalloped bottom edge: a row of discs under the body
    let n = 9;
    let rr = body.width() / (n as f32 * 2.0);
    for k in 0..n {
        let cx = body.left() + rr * (2.0 * k as f32 + 1.0);
        p.circle_filled(pos2(cx, body.bottom()), rr * 0.95, face);
    }
    p.rect_filled(body, 3.0, face);
    p.rect_stroke(body, 3.0, Stroke::new(1.0, edge));

    match card {
        Some(c) if !back => {
            let col = suit_col(c.suit());
            let hdr = Rect::from_min_size(body.min, vec2(body.width(), 26.0));
            p.rect_filled(hdr, 3.0, mul(col, 0.42, alpha));
            let name = reach_rules::SUITS[c.suit()].to_uppercase();
            p.text(hdr.left_top() + vec2(6.0, 4.0), egui::Align2::LEFT_TOP, name, head(8.5), mul(Color32::from_rgb(235, 235, 238), 1.0, alpha));
            p.text(hdr.left_top() + vec2(6.0, 15.0), egui::Align2::LEFT_TOP, LINES[c.suit()], FontId::new(7.5, FontFamily::Proportional), mul(Color32::from_rgb(200, 204, 212), 1.0, alpha));
            // number and pips
            p.text(pos2(body.right() - 8.0, body.top() + 28.0), egui::Align2::RIGHT_TOP, c.number().to_string(), head(30.0), mul(Color32::from_rgb(232, 233, 236), 1.0, alpha));
            let pips = State::card_pips(c.suit(), c.number());
            for k in 0..pips {
                star(p, pos2(body.left() + 9.0 + k as f32 * 8.5, body.top() + 40.0), 3.8, mul(crate::ui::BRASS, 1.0, alpha));
            }
            let ar = Rect::from_min_max(pos2(body.left() + 6.0, body.top() + 62.0), pos2(body.right() - 6.0, body.bottom() - 8.0));
            art(p, ar, c.suit(), col, alpha);
            p.rect_stroke(ar, 1.0, Stroke::new(1.0, mul(col, 0.5, alpha * 0.6)));
        }
        _ => {
            // card back: nested diamonds
            let c = body.center();
            for k in 0..5 {
                let r = body.width() * (0.12 + k as f32 * 0.08);
                let pts = vec![pos2(c.x, c.y - r * 1.5), pos2(c.x + r, c.y), pos2(c.x, c.y + r * 1.5), pos2(c.x - r, c.y)];
                p.add(Shape::closed_line(pts, Stroke::new(1.0, mul(crate::ui::BRASS, if k % 2 == 0 { 0.7 } else { 0.35 }, alpha))));
            }
            star(p, c, 6.0, mul(crate::ui::BRASS, 0.9, alpha));
        }
    }
}

// ---- table state and flights --------------------------------------------

/// Where a flying card ends up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Land {
    Lead,
    Follow,
    Discard,
}

#[derive(Clone, Debug)]
pub struct Fly {
    pub card: Option<Card>,
    pub from: Pos2,
    pub to: Pos2,
    pub t: f32,
    pub dur: f32,
    pub flip_from: f32,
    pub flip_to: f32,
    pub land: Land,
}

/// Cards currently lying in the map-edge slots.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub lead: Option<Card>,
    /// Surpass / copy / pivot cards in order; `None` is a face-down seize.
    pub follows: Vec<Option<Card>>,
}

/// Screen geometry of the three map-edge slots (lead, surpass-copy-pivot, discard).
#[derive(Clone, Copy, Debug, Default)]
pub struct Slots {
    pub lead: [f32; 2],
    pub follow: [f32; 2],
    pub discard: [f32; 2],
    /// Width of a card lying in a slot, in points.
    pub card_w: f32,
}

fn lerp(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    pos2(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn slot_rect(c: [f32; 2], w: f32, dy: f32) -> Rect {
    Rect::from_center_size(pos2(c[0], c[1] + dy), vec2(w, w * CARD_H / CARD_W))
}

pub fn draw_table(ctx: &egui::Context, t: &Table, flies: &[Fly], s: &Slots) {
    if s.card_w <= 1.0 {
        return;
    }
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("cardfx")));
    if let Some(c) = t.lead {
        paint_card(&p, slot_rect(s.lead, s.card_w, 0.0), Some(c), 0.0, 1.0);
    }
    for (i, c) in t.follows.iter().enumerate() {
        let dy = (i as f32 - (t.follows.len() as f32 - 1.0) / 2.0) * s.card_w * 0.55;
        paint_card(&p, slot_rect(s.follow, s.card_w, dy), *c, if c.is_some() { 0.0 } else { 1.0 }, 1.0);
    }
    for f in flies {
        let k = ease_in_out(f.t / f.dur);
        let arc = (std::f32::consts::PI * k).sin() * 46.0;
        let c = lerp(f.from, f.to, k) - vec2(0.0, arc);
        let w = CARD_W + (s.card_w - CARD_W) * k;
        let flip = f.flip_from + (f.flip_to - f.flip_from) * ease_in_out((k - 0.1) / 0.5);
        let alpha = if f.land == Land::Discard { 1.0 - 0.6 * k } else { 1.0 };
        paint_card(&p, Rect::from_center_size(c, vec2(w, w * CARD_H / CARD_W)), f.card, flip, alpha);
    }
}

// ---- hand animation state ------------------------------------------------

#[derive(Default)]
struct HandState {
    seen: HashMap<u8, f64>,
    rows: [Option<Pos2>; 4],
    origin: Option<(u8, Pos2)>,
}

thread_local! {
    static HAND: RefCell<HandState> = RefCell::new(HandState::default());
}

/// Records where each player's row sits, so bots' cards can fly out of it.
pub fn note_row(p: usize, c: Pos2) {
    HAND.with(|h| h.borrow_mut().rows[p] = Some(c));
}

pub fn row_of(p: usize) -> Pos2 {
    HAND.with(|h| h.borrow().rows[p]).unwrap_or(pos2(120.0, 80.0))
}

/// Where the human's card was when they played it (falls back to the hand's centre).
pub fn origin_of(card: Option<Card>, fallback: Pos2) -> Pos2 {
    HAND.with(|h| match (&h.borrow().origin, card) {
        (Some((id, p)), Some(c)) if *id == c.0 => *p,
        (Some((_, p)), None) => *p,
        _ => fallback,
    })
}

/// A hand card: deals in (staggered rise and fade), lifts on hover. Returns the
/// click response and records the card's position as the flight origin on click.
pub fn hand_card(ui: &mut egui::Ui, card: Card, index: usize, now: f64, seen_now: &mut Vec<u8>) -> egui::Response {
    seen_now.push(card.0);
    let t0 = HAND.with(|h| *h.borrow_mut().seen.entry(card.0).or_insert(now));
    let k = ease_out(((now - t0) as f32 - index as f32 * 0.08) / 0.4);
    let (slot, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H + 18.0), egui::Sense::click());
    let lift = ui.ctx().animate_bool_with_time(egui::Id::new(("lift", card.0)), resp.hovered() && k >= 1.0, 0.12);
    let rect = Rect::from_min_size(slot.min + vec2(0.0, 18.0 - 14.0 * lift + 70.0 * (1.0 - k)), vec2(CARD_W, CARD_H)).expand(lift * 3.0);
    let p = ui.painter().clone();
    paint_card(&p, rect, Some(card), 0.0, k);
    if resp.clicked() {
        HAND.with(|h| h.borrow_mut().origin = Some((card.0, rect.center())));
    }
    resp
}

/// Forgets cards that left the hand so a re-deal animates again.
pub fn end_hand_frame(seen_now: &[u8]) {
    HAND.with(|h| h.borrow_mut().seen.retain(|c, _| seen_now.contains(c)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_is_bounded_and_monotone() {
        let mut last = -1.0;
        for i in 0..=20 {
            let v = ease_out(i as f32 / 20.0);
            assert!((0.0..=1.0).contains(&v) && v >= last);
            last = v;
        }
        assert_eq!(ease_in_out(0.0), 0.0);
        assert_eq!(ease_in_out(1.0), 1.0);
    }

    #[test]
    fn origin_falls_back_when_unknown() {
        let p = origin_of(Some(Card(3)), pos2(5.0, 6.0));
        assert_eq!(p, pos2(5.0, 6.0));
    }
}
