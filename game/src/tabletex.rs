//! Textures for the pieces of card stock that lie on the board: action cards
//! in the Lead and Surpass/Copy/Pivot slots, the Court row, and the power
//! track's numbered strip. They are painted on the CPU (shapes plus the game's
//! own fonts through ab_glyph) once at start-up, then drawn as flat quads in
//! the 3D scene, so they sit on the table and move with the camera.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use dust_engine::mesh::{MeshData, Vertex};
use reach_rules::{Card, State, COURT_EFFECTS, COURT_ICONS, COURT_NAMES, RESOURCES, SUITS};

/// Card texture size (portrait, the printed 96:136 ratio).
pub const CW: u32 = 240;
pub const CH: u32 = 340;

/// What each suit's pips may do, as printed on the card.
pub const SUIT_LINES: [&str; 4] = ["TAX · REPAIR · INFLUENCE", "BATTLE · MOVE · SECURE", "BUILD · REPAIR", "MOVE · INFLUENCE"];

const HEAD: &[u8] = include_bytes!("../assets/fonts/ChakraPetch_600SemiBold.ttf");
const BODY: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans_500Medium.ttf");

type Rgb = [f32; 3];

fn rgb(c: [u8; 3]) -> Rgb {
    c.map(|v| v as f32 / 255.0)
}

fn suit_rgb(s: usize) -> Rgb {
    rgb([[146, 118, 188], [196, 86, 76], [206, 160, 76], [84, 142, 196]][s])
}

fn res_rgb(t: usize) -> Rgb {
    rgb([[176, 150, 112], [205, 125, 70], [176, 78, 72], [120, 176, 196], [150, 120, 196]][t])
}

const BRASS: Rgb = [214.0 / 255.0, 164.0 / 255.0, 86.0 / 255.0];
const INK: Rgb = [0.9, 0.9, 0.92];
const STOCK: Rgb = [0.10, 0.11, 0.15];

fn scale(c: Rgb, k: f32) -> Rgb {
    c.map(|v| v * k)
}

pub struct Canvas {
    pub w: u32,
    pub h: u32,
    px: Vec<f32>, // straight RGBA, 0..1
}

impl Canvas {
    pub fn new(w: u32, h: u32, bg: Rgb, alpha: f32) -> Self {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&[bg[0], bg[1], bg[2], alpha]);
        }
        Canvas { w, h, px }
    }

    fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || a <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = a.min(1.0);
        for k in 0..3 {
            self.px[i + k] = self.px[i + k] * (1.0 - a) + c[k] * a;
        }
        self.px[i + 3] = self.px[i + 3] + a * (1.0 - self.px[i + 3]);
    }

    /// Fills everything where `inside(x, y)` holds, 4x4 supersampled, within a box.
    fn fill_by(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, inside: impl Fn(f32, f32) -> bool) {
        let (xa, ya) = (x0.floor().max(0.0) as i32, y0.floor().max(0.0) as i32);
        let (xb, yb) = (x1.ceil().min(self.w as f32) as i32, y1.ceil().min(self.h as f32) as i32);
        for y in ya..yb {
            for x in xa..xb {
                let mut n = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        if inside(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) {
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    self.blend(x, y, c, a * n as f32 / 16.0);
                }
            }
        }
    }

    pub fn rrect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, c: Rgb, a: f32) {
        self.fill_by(x0, y0, x1, y1, c, a, |x, y| {
            let dx = (x0 + r - x).max(x - (x1 - r)).max(0.0);
            let dy = (y0 + r - y).max(y - (y1 - r)).max(0.0);
            x >= x0 && x <= x1 && y >= y0 && y <= y1 && dx * dx + dy * dy <= r * r
        });
    }

    /// A rounded-rectangle outline of width `t`.
    pub fn rframe(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, t: f32, c: Rgb, a: f32) {
        let inside = move |x: f32, y: f32, i: f32| {
            let (a0, b0, a1, b1, rr) = (x0 + i, y0 + i, x1 - i, y1 - i, (r - i).max(0.0));
            let dx = (a0 + rr - x).max(x - (a1 - rr)).max(0.0);
            let dy = (b0 + rr - y).max(y - (b1 - rr)).max(0.0);
            x >= a0 && x <= a1 && y >= b0 && y <= b1 && dx * dx + dy * dy <= rr * rr
        };
        self.fill_by(x0, y0, x1, y1, c, a, |x, y| inside(x, y, 0.0) && !inside(x, y, t));
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
        self.fill_by(cx - r, cy - r, cx + r, cy + r, c, a, |x, y| (x - cx).powi(2) + (y - cy).powi(2) <= r * r);
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: Rgb, a: f32) {
        self.fill_by(cx - r, cy - r, cx + r, cy + r, c, a, |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            d <= r && d >= r - t
        });
    }

    pub fn poly(&mut self, pts: &[(f32, f32)], c: Rgb, a: f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pts {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let pts = pts.to_vec();
        self.fill_by(x0, y0, x1, y1, c, a, move |x, y| {
            let mut inside = false;
            let n = pts.len();
            let mut j = n - 1;
            for i in 0..n {
                let (xi, yi) = pts[i];
                let (xj, yj) = pts[j];
                if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            inside
        });
    }

    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, c: Rgb, al: f32) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (nx, ny) = (-dy / l * w * 0.5, dx / l * w * 0.5);
        self.poly(&[(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)], c, al);
    }

    /// A four-point star like the printed action pips.
    pub fn star(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        let k = 0.36;
        self.poly(&[(cx, cy - r), (cx + r * k, cy), (cx, cy + r), (cx - r * k, cy)], c, 1.0);
        self.poly(&[(cx - r, cy), (cx, cy - r * k), (cx + r, cy), (cx, cy + r * k)], c, 1.0);
    }

    /// Text width in pixels.
    pub fn measure(font: &[u8], size: f32, s: &str) -> f32 {
        let f = FontRef::try_from_slice(font).expect("font");
        let sf = f.as_scaled(PxScale::from(size));
        let mut w = 0.0;
        let mut prev = None;
        for ch in s.chars() {
            let g = sf.glyph_id(ch);
            if let Some(p) = prev {
                w += sf.kern(p, g);
            }
            w += sf.h_advance(g);
            prev = Some(g);
        }
        w
    }

    /// Draws `s` with its baseline at y. `align`: 0 left, 1 centre, 2 right of x.
    pub fn text(&mut self, font: &[u8], size: f32, x: f32, y: f32, align: u8, s: &str, c: Rgb) {
        let f = FontRef::try_from_slice(font).expect("font");
        let sf = f.as_scaled(PxScale::from(size));
        let w = Self::measure(font, size, s);
        let mut cx = match align {
            1 => x - w / 2.0,
            2 => x - w,
            _ => x,
        };
        let mut prev = None;
        for ch in s.chars() {
            let gid = sf.glyph_id(ch);
            if let Some(p) = prev {
                cx += sf.kern(p, gid);
            }
            let g = gid.with_scale_and_position(PxScale::from(size), point(cx, y));
            if let Some(o) = f.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| self.blend(b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, c, cov));
            }
            cx += sf.h_advance(gid);
            prev = Some(gid);
        }
    }

    /// Word-wrapped text centred in [x0, x1]; returns the y after the last line.
    pub fn wrap(&mut self, font: &[u8], size: f32, x0: f32, x1: f32, mut y: f32, s: &str, c: Rgb) -> f32 {
        let mut line = String::new();
        for word in s.split_whitespace() {
            let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if Self::measure(font, size, &trial) > x1 - x0 && !line.is_empty() {
                self.text(font, size, (x0 + x1) / 2.0, y, 1, &line, c);
                y += size * 1.22;
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        if !line.is_empty() {
            self.text(font, size, (x0 + x1) / 2.0, y, 1, &line, c);
            y += size * 1.22;
        }
        y
    }

    pub fn rgba8(&self) -> Vec<u8> {
        self.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8).collect()
    }
}

fn card_stock(c: &mut Canvas, edge: Rgb) {
    let (w, h) = (c.w as f32, c.h as f32);
    c.rrect(0.0, 0.0, w, h, 16.0, STOCK, 1.0);
    c.rframe(0.0, 0.0, w, h, 16.0, 4.0, edge, 1.0);
    c.rframe(9.0, 9.0, w - 9.0, h - 9.0, 10.0, 1.5, scale(edge, 0.55), 1.0);
}

/// Suit emblem in the art panel (our own line drawings).
fn suit_art(c: &mut Canvas, s: usize, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgb) {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let r = (y1 - y0).min(x1 - x0) * 0.42;
    match s {
        0 => {
            c.ring(cx, cy, r, 4.0, col, 0.9);
            c.ring(cx, cy, r * 0.68, 2.5, col, 0.6);
            c.ring(cx, cy, r * 0.36, 2.5, col, 0.6);
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                c.line((cx + a.cos() * r, cy + a.sin() * r), (cx + a.cos() * r * 1.18, cy + a.sin() * r * 1.18), 2.5, col, 0.7);
            }
            c.circle(cx, cy, 6.0, col, 1.0);
        }
        1 => {
            for d in [-1.0f32, 1.0] {
                c.line((cx - d * r, cy - r), (cx + d * r, cy + r), 9.0, scale(col, 0.7), 0.95);
                c.line((cx - d * r, cy - r), (cx + d * r, cy + r), 3.0, col, 1.0);
            }
            c.ring(cx, cy, r * 0.45, 3.0, col, 0.7);
        }
        2 => {
            let base = y1 - 10.0;
            for (k, bw) in [0.78f32, 0.56, 0.34].iter().enumerate() {
                let bh = (y1 - y0) * 0.17;
                let yb = base - k as f32 * bh * 1.15;
                c.rframe(cx - r * bw * 1.3, yb - bh, cx + r * bw * 1.3, yb, 2.0, 3.0, col, 0.9);
            }
            c.line((cx, y0 + 8.0), (cx, base - (y1 - y0) * 0.55), 3.0, col, 0.9);
            c.line((cx, y0 + 10.0), (cx + r * 0.7, y0 + 26.0), 2.0, col, 0.6);
        }
        _ => {
            for k in -1..=1 {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * 0.45;
                let (dx, dy) = (a.cos(), a.sin());
                let tail = (cx, y1 - 10.0);
                let tip = (tail.0 + dx * r * 1.9, tail.1 + dy * r * 1.9);
                c.line(tail, tip, 3.5, col, 0.9);
                let (nx, ny) = (-dy, dx);
                c.poly(&[(tip.0 + dx * 12.0, tip.1 + dy * 12.0), (tip.0 - dx * 6.0 + nx * 9.0, tip.1 - dy * 6.0 + ny * 9.0), (tip.0 - dx * 6.0 - nx * 9.0, tip.1 - dy * 6.0 - ny * 9.0)], col, 1.0);
            }
        }
    }
}

const AMBITIONS_SHORT: [&str; 5] = ["TYCOON", "TYRANT", "WARLORD", "KEEPER", "EMPATH"];

/// Face of an action card.
pub fn action_face(card: Card) -> Canvas {
    let s = card.suit();
    let col = suit_rgb(s);
    let mut c = Canvas::new(CW, CH, STOCK, 0.0);
    card_stock(&mut c, scale(col, 0.95));
    let w = CW as f32;
    // header band: suit name and what its pips do
    c.rrect(12.0, 12.0, w - 12.0, 74.0, 8.0, scale(col, 0.45), 1.0);
    c.text(HEAD, 25.0, 22.0, 42.0, 0, &SUITS[s].to_uppercase(), INK);
    c.text(BODY, 14.5, 22.0, 64.0, 0, SUIT_LINES[s], [0.82, 0.84, 0.88]);
    // big number and pips
    c.circle(50.0, 118.0, 34.0, scale(col, 0.3), 1.0);
    c.ring(50.0, 118.0, 34.0, 3.0, col, 1.0);
    c.text(HEAD, 52.0, 50.0, 136.0, 1, &card.number().to_string(), INK);
    let pips = State::card_pips(s, card.number());
    for k in 0..pips {
        c.star(108.0 + k as f32 * 30.0, 112.0, 13.0, BRASS);
    }
    let amb = match card.number() {
        1 => String::new(),
        7 => "DECLARE ANY".to_string(),
        n => format!("DECLARE {}", AMBITIONS_SHORT[n as usize - 2]),
    };
    if !amb.is_empty() {
        c.text(HEAD, 14.0, 96.0, 146.0, 0, &amb, scale(INK, 0.75));
    }
    // art panel
    let (x0, y0, x1, y1) = (18.0, 166.0, w - 18.0, CH as f32 - 22.0);
    c.rrect(x0, y0, x1, y1, 6.0, scale(col, 0.14), 1.0);
    c.rframe(x0, y0, x1, y1, 6.0, 2.0, scale(col, 0.55), 1.0);
    suit_art(&mut c, s, x0, y0, x1, y1, col);
    c
}

/// Shared card back (face-down copies, seized cards, the decks).
pub fn card_back() -> Canvas {
    let mut c = Canvas::new(CW, CH, STOCK, 0.0);
    card_stock(&mut c, scale(BRASS, 0.8));
    let (cx, cy) = (CW as f32 / 2.0, CH as f32 / 2.0);
    for k in 0..5 {
        let r = CW as f32 * (0.12 + k as f32 * 0.08);
        let a = if k % 2 == 0 { 0.8 } else { 0.4 };
        let pts = [(cx, cy - r * 1.5), (cx + r, cy), (cx, cy + r * 1.5), (cx - r, cy)];
        for i in 0..4 {
            c.line(pts[i], pts[(i + 1) % 4], 2.0, BRASS, a);
        }
    }
    c.star(cx, cy, 20.0, BRASS);
    c
}

/// Face of a Court card: name band in its resource colour, the resource icon
/// it adds to ambitions, and its lasting effect.
pub fn court_face(id: usize) -> Canvas {
    let ty = COURT_ICONS[id];
    let col = res_rgb(ty);
    let mut c = Canvas::new(CW, CH, STOCK, 0.0);
    card_stock(&mut c, scale(BRASS, 0.85));
    let w = CW as f32;
    c.rrect(12.0, 12.0, w - 12.0, 66.0, 8.0, scale(col, 0.5), 1.0);
    c.text(HEAD, 30.0, w / 2.0, 50.0, 1, &COURT_NAMES[id].to_uppercase(), INK);
    // icon medallion
    c.circle(w / 2.0, 128.0, 42.0, scale(col, 0.32), 1.0);
    c.ring(w / 2.0, 128.0, 42.0, 4.0, col, 1.0);
    c.circle(w / 2.0, 128.0, 16.0, col, 1.0);
    c.text(HEAD, 16.0, w / 2.0, 194.0, 1, &RESOURCES[ty].to_uppercase(), col);
    c.wrap(BODY, 19.0, 24.0, w - 24.0, 232.0, COURT_EFFECTS[id % 5], INK);
    c.text(HEAD, 13.0, w / 2.0, CH as f32 - 22.0, 1, "COURT", scale(BRASS, 0.9));
    c
}

/// The "ambition declared" zero marker that covers a lead card's number.
pub fn zero_marker() -> Canvas {
    let mut c = Canvas::new(96, 96, [0.0; 3], 0.0);
    c.circle(48.0, 48.0, 46.0, [0.03, 0.03, 0.04], 1.0);
    c.ring(48.0, 48.0, 46.0, 4.0, BRASS, 1.0);
    c.text(HEAD, 58.0, 48.0, 68.0, 1, "0", INK);
    c
}

/// Numbered strip for the power track: cells 0..=cells-1, goal marks drawn by the caller.
pub fn track_strip(cells: usize, cell_px: u32) -> Canvas {
    let (w, h) = (cells as u32 * cell_px, cell_px + cell_px / 3);
    let mut c = Canvas::new(w, h, [0.06, 0.055, 0.05], 1.0);
    for i in 0..cells {
        let x0 = (i as u32 * cell_px) as f32;
        let major = i % 5 == 0;
        let fill = if major { scale(BRASS, 0.5) } else { scale(BRASS, 0.18) };
        c.rrect(x0 + 2.0, 2.0, x0 + cell_px as f32 - 2.0, h as f32 - 2.0, 5.0, fill, 1.0);
        let ink = if major { [0.98, 0.94, 0.86] } else { scale(BRASS, 1.0) };
        c.text(HEAD, cell_px as f32 * 0.55, x0 + cell_px as f32 / 2.0, h as f32 * 0.68, 1, &i.to_string(), ink);
    }
    c
}

/// A flat quad on the XZ plane, 1x1, facing +Y; texture top (v=0) at -Z.
pub fn quad() -> MeshData {
    let v = |x: f32, z: f32, u: f32, vv: f32| Vertex { position: [x, 0.0, z], normal: [0.0, 1.0, 0.0], uv: [u, vv], color: [0.0; 3] };
    MeshData { vertices: vec![v(-0.5, -0.5, 0.0, 0.0), v(0.5, -0.5, 1.0, 0.0), v(0.5, 0.5, 1.0, 1.0), v(-0.5, 0.5, 0.0, 1.0)], indices: vec![0, 2, 1, 0, 3, 2] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_paint_without_panicking_and_are_opaque_in_the_middle() {
        for id in 0..28u8 {
            let c = action_face(Card(id));
            let px = c.rgba8();
            let mid = ((CH / 2 * CW + CW / 2) * 4 + 3) as usize;
            assert_eq!(px[mid], 255);
        }
        for id in 0..COURT_NAMES.len() {
            let _ = court_face(id);
        }
        let t = track_strip(41, 48);
        assert_eq!(t.w, 41 * 48);
    }
}
