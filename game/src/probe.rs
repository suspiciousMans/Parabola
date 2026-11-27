//! Frame-level UI probe for the screenshot assertions.
//!
//! egui paints into a canvas, so there is no DOM text to scrape. Each frame
//! records the labels the HUD actually lays out (the RichText passed to egui,
//! the Response rect it returned, and the panel/layer it belongs to) plus the
//! rules state at that frame. The browser build mirrors the JSON into a hidden
//! DOM node and a JS global; the screenshot harness freezes the game so the
//! probe and the pixels describe the same frame.

use egui::{Context, Rect, Response, RichText, Ui};
use reach_rules::{State, AMBITIONS, COURT_CARDS};
use serde_json::json;
use std::cell::RefCell;

#[derive(Clone)]
struct Entry {
    panel: &'static str,
    z: i32,
    role: String,
    text: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone)]
struct Panel {
    id: &'static str,
    z: i32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Default, Clone)]
struct Probe {
    width: f32,
    height: f32,
    ledger: bool,
    table_cards: bool,
    panels: Vec<Panel>,
    texts: Vec<Entry>,
}

thread_local! {
    static PROBE: RefCell<Probe> = RefCell::new(Probe::default());
}

// Record the screen and the modal state at the start of a HUD frame.
pub fn begin(ctx: &Context, ledger: bool, table_cards: bool) {
    let screen = ctx.screen_rect();
    PROBE.with(|p| {
        *p.borrow_mut() = Probe {
            width: screen.width(),
            height: screen.height(),
            ledger,
            table_cards,
            ..Probe::default()
        }
    });
}

pub fn panel(id: &'static str, z: i32, rect: Rect) {
    PROBE.with(|p| {
        p.borrow_mut().panels.push(Panel {
            id,
            z,
            x: rect.min.x,
            y: rect.min.y,
            w: rect.width(),
            h: rect.height(),
        })
    });
}

fn record(panel: &'static str, z: i32, role: &str, text: &str, rect: Rect) {
    PROBE.with(|p| {
        p.borrow_mut().texts.push(Entry {
            panel,
            z,
            role: role.to_string(),
            text: text.to_string(),
            x: rect.min.x,
            y: rect.min.y,
            w: rect.width(),
            h: rect.height(),
        })
    });
}

/// Draw a label and record the exact text and the rectangle egui gave it.
pub fn ptext(ui: &mut Ui, panel: &'static str, z: i32, role: &str, text: RichText) -> Response {
    let raw = text.text().to_string();
    let response = ui.label(text);
    record(panel, z, role, &raw, response.rect);
    response
}

/// A wrapping label that still records its exact text for the HUD probe.
pub fn ptext_wrap(ui: &mut Ui, panel: &'static str, z: i32, role: &str, text: RichText) -> Response {
    let raw = text.text().to_string();
    let response = ui.add(egui::Label::new(text).wrap());
    record(panel, z, role, &raw, response.rect);
    response
}

fn rect_json(x: f32, y: f32, w: f32, h: f32) -> serde_json::Value {
    json!([x, y, w, h])
}

/// Snapshot the live rules values plus the current frame's layout.
pub fn json(s: &State, held: bool) -> String {
    let n = s.players();
    let probe = PROBE.with(|p| p.borrow().clone());
    let ranks = s.ambition_ranks();
    let power: Vec<i64> = (0..n).map(|p| s.power(p)).collect();
    let resources: Vec<Vec<i64>> = (0..n)
        .map(|p| (0..5).map(|t| s.resource(p, t)).collect())
        .collect();
    let trophies: Vec<i64> = (0..n).map(|p| s.trophies(p)).collect();
    let captives: Vec<i64> = (0..n).map(|p| s.captives(p)).collect();
    let court: Vec<i64> = (0..5).map(|i| s.court_card(i).map(|c| c.0 as i64).unwrap_or(-1)).collect();
    let secured: Vec<Vec<u8>> = (0..n).map(|p| s.secured(p).into_iter().map(|c| c.0).collect()).collect();
    let tallies: Vec<Vec<i64>> = (0..n)
        .map(|p| (0..5).map(|a| s.ambition_tally(p, a)).collect())
        .collect();
    let panels: Vec<_> = probe
        .panels
        .iter()
        .map(|p| json!({ "id": p.id, "z": p.z, "rect": rect_json(p.x, p.y, p.w, p.h) }))
        .collect();
    let texts: Vec<_> = probe
        .texts
        .iter()
        .map(|t| {
            json!({
                "panel": t.panel,
                "z": t.z,
                "role": t.role,
                "text": t.text,
                "rect": rect_json(t.x, t.y, t.w, t.h),
            })
        })
        .collect();
    json!({
        "width": probe.width,
        "height": probe.height,
        "screen": if s.is_over() { "game_over" } else if probe.ledger { "ledger" } else { "board" },
        "held": held,
        "ledger": probe.ledger,
        "over": s.is_over(),
        "table_cards": probe.table_cards,
        "rules": {
            "players": n,
            "chapter": s.chapter(),
            "goal": State::power_goal(n),
            "current": s.current(),
            "winner": s.winner(),
            "power": power,
            "resources": resources,
            "trophies": trophies,
            "captives": captives,
            "court": court,
            "court_names": COURT_CARDS,
            "court_available": s.court_available(),
            "secured": secured,
            "ambitions": AMBITIONS,
            "ranks": ranks,
            "tallies": tallies,
            "seats": crate::ui::NAMES,
        },
        "panels": panels,
        "texts": texts,
    })
    .to_string()
}