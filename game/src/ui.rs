//! egui screens: start menu, in-game HUD (scores, hand, actions, log), game over.

use egui::{self, Color32, FontFamily, FontId, RichText};
use reach_rules::{Action, Card, Kind, Phase, State, AMBITIONS, COURT_CARDS, LEADERS, LEADER_EFFECTS, RESOURCES, SUITS};

use crate::courtart;
use crate::fieldmanual;
use crate::guides;

pub const NAMES: [&str; 4] = ["Crimson", "Azure", "Amber", "Verdant"];
/// Player colours (sRGB): distinct but muted, so they sit on a dark board without glowing.
pub const COLORS_UI: [[u8; 3]; 4] = [[196, 72, 64], [76, 128, 212], [212, 164, 66], [92, 176, 128]];

// Palette: tinted neutrals, one brass accent, red only for alerts. See docs/STYLE.md.
pub const INK: Color32 = Color32::from_rgb(222, 224, 228);
pub const DIM: Color32 = Color32::from_rgb(138, 146, 160);
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(10, 13, 18, 246);
pub const HAIR: Color32 = Color32::from_rgb(54, 61, 76);
pub const BRASS: Color32 = Color32::from_rgb(214, 164, 86);
pub const CAPTIVE: Color32 = Color32::from_rgb(198, 108, 94);

pub fn ui_color(p: usize) -> Color32 {
    let c = COLORS_UI[p];
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn suit_color(s: usize) -> Color32 {
    match s {
        0 => Color32::from_rgb(146, 118, 188),
        1 => Color32::from_rgb(196, 86, 76),
        2 => Color32::from_rgb(206, 160, 76),
        _ => Color32::from_rgb(84, 142, 196),
    }
}

pub fn res_color(t: usize) -> Color32 {
    let c = [[176, 150, 112], [205, 125, 70], [176, 78, 72], [120, 176, 196], [150, 120, 196]][t];
    Color32::from_rgb(c[0], c[1], c[2])
}

/// A small cup for the trophies count: destroyed enemy ships and buildings.
fn paint_trophy(painter: &egui::Painter, r: egui::Rect, color: Color32) {
    let c = r.center();
    let cup = egui::Rect::from_center_size(egui::pos2(c.x, r.top() + 3.2), egui::vec2(7.2, 5.0));
    painter.rect_filled(cup, 1.4, color);
    painter.line_segment([egui::pos2(c.x, r.top() + 5.6), egui::pos2(c.x, r.bottom() - 1.8)], egui::Stroke::new(1.2, color));
    painter.line_segment([egui::pos2(c.x - 2.6, r.bottom() - 1.2), egui::pos2(c.x + 2.6, r.bottom() - 1.2)], egui::Stroke::new(1.4, color));
}

/// A shackle ring for the captives count: one per enemy city taken in battle.
fn paint_captive(painter: &egui::Painter, r: egui::Rect, color: Color32) {
    let c = r.center();
    painter.circle_stroke(c, 3.1, egui::Stroke::new(1.3, color));
    painter.line_segment([egui::pos2(c.x + 2.2, c.y - 2.2), egui::pos2(c.x + 4.0, c.y - 4.0)], egui::Stroke::new(1.2, color));
}

/// Heading text: Chakra Petch. Body text is IBM Plex Sans (both SIL OFL, see assets/fonts).
pub fn head(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(FontId::new(size, FontFamily::Name("head".into())))
}

// Draw a label and record it in the frame probe (text, rect, panel, layer).
macro_rules! plabel {
    ($ui:expr, $panel:expr, $z:expr, $role:expr, $text:expr) => {
        crate::probe::ptext($ui, $panel, $z, $role, ($text).into())
    };
}

/// One-time font and style setup.
pub fn theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("chakra".into(), egui::FontData::from_static(include_bytes!("../assets/fonts/ChakraPetch_600SemiBold.ttf")));
    fonts.font_data.insert("plex".into(), egui::FontData::from_static(include_bytes!("../assets/fonts/IBMPlexSans_400Regular.ttf")));
    fonts.font_data.insert("plex_med".into(), egui::FontData::from_static(include_bytes!("../assets/fonts/IBMPlexSans_500Medium.ttf")));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "plex".into());
    fonts.families.insert(FontFamily::Name("head".into()), vec!["chakra".into(), "plex_med".into()]);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(egui::TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(egui::TextStyle::Button, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(egui::TextStyle::Small, FontId::new(11.5, FontFamily::Proportional));
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    let v = &mut style.visuals;
    v.override_text_color = Some(INK);
    v.window_fill = PANEL;
    v.panel_fill = Color32::TRANSPARENT;
    v.window_rounding = 2.0.into();
    v.selection.bg_fill = BRASS.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, BRASS);
    let r = egui::Rounding::same(2.0);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, HAIR);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(26, 30, 40);
    v.widgets.inactive.bg_fill = Color32::from_rgb(26, 30, 40);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, HAIR);
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(38, 44, 58);
    v.widgets.hovered.bg_fill = Color32::from_rgb(38, 44, 58);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, BRASS);
    v.widgets.hovered.rounding = r;
    v.widgets.active.weak_bg_fill = BRASS.gamma_multiply(0.4);
    v.widgets.active.bg_fill = BRASS.gamma_multiply(0.4);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, BRASS);
    v.widgets.active.rounding = r;
    ctx.set_style(style);
}

/// A flat panel: near-black tinted fill, one hairline, square-ish corners. No glow.
pub fn panel() -> egui::Frame {
    egui::Frame::none().fill(PANEL).stroke(egui::Stroke::new(1.0, HAIR)).rounding(2.0).inner_margin(12.0)
}

pub fn card_label(c: Card) -> String {
    c.name()
}

thread_local! {
    /// The ledger overlay: score, resources, spoils and the ambition cards.
    static LEDGER: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Opens or closes the ledger (bound to L in `main`, where the game sees keys).
pub fn toggle_ledger() {
    let open = LEDGER.with(|l| l.get());
    if !open {
        fieldmanual::close();
        guides::close();
    }
    LEDGER.with(|l| l.set(!open));
}

pub fn close_ledger() -> bool {
    LEDGER.with(|l| {
        let was = l.get();
        l.set(false);
        was
    })
}

pub fn toggle_manual() {
    if !fieldmanual::is_open() {
        close_ledger();
        guides::close();
    }
    fieldmanual::toggle();
}

pub fn open_tutorial() {
    close_ledger();
    guides::close();
    fieldmanual::open_tutorial();
}

pub fn toggle_guide() {
    if !guides::is_open() {
        close_ledger();
        fieldmanual::close();
    }
    guides::toggle();
}

/// Escape closes the topmost overlay before it leaves the table.
pub fn close_overlay() -> bool {
    if fieldmanual::is_open() {
        fieldmanual::close();
        return true;
    }
    if guides::is_open() {
        guides::close();
        return true;
    }
    close_ledger()
}

pub fn sys_name(sys: usize) -> String {
    format!("{}{}", ["Spire ", "Crescent ", "Hexad ", "Gate "][sys % 4], sys / 4 + 1)
}

pub struct MenuState {
    pub players: usize,
    pub bots: [bool; 4],
    pub setup: usize,
}

impl Default for MenuState {
    fn default() -> Self {
        Self { players: 3, bots: [false, true, true, true], setup: 0 }
    }
}

/// Returns true when "Start" is pressed.
pub fn menu(ctx: &egui::Context, m: &mut MenuState) -> bool {
    let mut start = false;
    egui::Area::new(egui::Id::new("menu")).anchor(egui::Align2::LEFT_CENTER, [64.0, 0.0]).show(ctx, |ui| {
        ui.set_width(330.0);
        ui.label(head("EMBER REACH", 40.0));
        ui.label(RichText::new("A fading star cluster. Five chapters, one victor.").color(DIM));
        ui.add_space(22.0);
        panel().show(ui, |ui| {
            ui.set_width(306.0);
            ui.label(head("TABLE", 12.0).color(DIM));
            ui.horizontal(|ui| {
                ui.label("Players");
                for n in 2..=4 {
                    ui.selectable_value(&mut m.players, n, n.to_string());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Setup card");
                for k in 0..State::setup_count(m.players) {
                    ui.selectable_value(&mut m.setup, k, (k + 1).to_string());
                }
            });
            ui.add_space(4.0);
            ui.label(head("SEATS", 12.0).color(DIM));
            for p in 0..m.players {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(4.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0.0, ui_color(p));
                    ui.label(head(NAMES[p].to_uppercase(), 14.0));
                    ui.selectable_value(&mut m.bots[p], false, "Human");
                    ui.selectable_value(&mut m.bots[p], true, "Bot");
                });
            }
        });
        ui.add_space(14.0);
        if ui.add_sized([306.0, 38.0], egui::Button::new(head("BEGIN", 16.0).color(Color32::from_rgb(20, 16, 10))).fill(BRASS)).clicked() {
            start = true;
        }
        ui.add_space(8.0);
        if ui.add_sized([306.0, 26.0], egui::Button::new(head("HOW TO PLAY", 11.0))).clicked() {
            open_tutorial();
        }
        ui.add_space(14.0);
        ui.label(RichText::new("Right-drag orbit  ·  middle-drag or WASD pan  ·  wheel zoom\nQ/E turn  ·  Space overview  ·  C cinematic camera\nL ledger  ·  R rules  ·  G board guide").small().color(DIM));
    });
    if fieldmanual::is_open() {
        fieldmanual::overlay(ctx, State::power_goal(m.players));
    }
    start
}

/// Seat naming and control when the board is showing a site room.
pub struct OnlineHud {
    pub me: usize,
    pub who: Vec<String>,
    /// Seats the Worker plays.
    pub ai: [bool; 4],
}

/// Shown while the page has not yet handed over a room.
pub fn loading(ctx: &egui::Context, text: &str) {
    egui::Area::new(egui::Id::new("loading")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.label(head("EMBER REACH", 40.0));
        ui.label(RichText::new(text).color(DIM));
    });
}

pub struct HudInput<'a> {
    pub state: &'a State,
    pub bots: &'a [bool; 4],
    pub busy: bool,
    pub selected: Option<usize>,
    pub log: &'a [String],
    pub toast: Option<&'a str>,
    /// Hot-seat: a human seat that must confirm before its hand is shown.
    pub handoff: Option<usize>,
    pub cinematic: bool,
    /// Gate numbers: text and screen position in points.
    pub labels: &'a [(String, [f32; 2])],
    pub table: &'a crate::cards::Table,
    pub flies: &'a [crate::cards::Fly],
    pub slots: crate::cards::Slots,
    pub anchors: &'a guides::Anchors,
    pub online: Option<&'a OnlineHud>,
}

#[derive(Default)]
pub struct HudOutput {
    pub action: Option<Action>,
    pub hover_systems: Vec<usize>,
    pub new_game: bool,
    pub ready: bool,
    pub toggle_cinematic: bool,
    pub overview: bool,
    pub wants_pointer: bool,
}

pub fn action_text(a: Action, st: &State) -> String {
    let v = a.0;
    match a.kind() {
        Kind::Lead => {
            if v[2] >= 0 {
                format!("Lead · declare {}", AMBITIONS[v[2] as usize])
            } else {
                "Lead".into()
            }
        }
        Kind::Surpass => "Surpass".into(),
        Kind::Copy => "Copy".into(),
        Kind::Pivot => "Pivot".into(),
        Kind::Pass => "Pass".into(),
        Kind::Seize => "Seize".into(),
        Kind::Prelude => match v[1] {
            0 => "Spend Material · build or repair".into(),
            1 => "Spend Fuel · move".into(),
            2 => "Spend Weapons · pips may battle".into(),
            3 => "Spend Relic · secure".into(),
            _ => "Spend Psionic · lead-card action".into(),
        },
        Kind::EndTurn => "End turn".into(),
        Kind::Tax => format!("Tax {}", sys_name(v[1] as usize)),
        Kind::Influence => format!("Influence {}", court_name(st, v[1])),
        Kind::BuildCity => format!("Build city · {}", sys_name(v[1] as usize)),
        Kind::BuildPort => format!("Build starport · {}", sys_name(v[1] as usize)),
        Kind::BuildShips => format!("Build ships · {}", sys_name(v[1] as usize)),
        Kind::Move => format!("Move {} · {} to {}", v[3], sys_name(v[1] as usize), sys_name(v[2] as usize)),
        Kind::Repair => format!("Repair · {}", sys_name(v[1] as usize)),
        Kind::Battle => {
            let (asl, sk, rd) = (v[3] / 49, v[3] / 7 % 7, v[3] % 7);
            format!("Battle · {} vs {} ({}A {}S {}R)", sys_name(v[1] as usize), NAMES[v[2] as usize], asl, sk, rd)
        }
        Kind::Secure => format!("Secure {}", court_name(st, v[1])),
        Kind::Lore => "Spend Lore · extra action".into(),
        Kind::Mulligan => if v[1] == 1 { "Mulligan · draw a new hand".into() } else { "Keep this hand".into() },
    }
}

pub fn action_systems(a: Action) -> Vec<usize> {
    let v = a.0;
    match a.kind() {
        Kind::Move => vec![v[1] as usize, v[2] as usize],
        Kind::Tax | Kind::BuildCity | Kind::BuildPort | Kind::BuildShips | Kind::Repair | Kind::Battle => vec![v[1] as usize],
        _ => vec![],
    }
}

/// Short seat label for tight tables (online names are chosen by players).
fn seat_short(h: &HudInput, p: usize) -> String {
    let name = match h.online {
        Some(o) if o.ai[p] => NAMES[p].to_string(),
        Some(o) => o.who[p].clone(),
        None => NAMES[p].to_string(),
    };
    name.chars().take(11).collect::<String>().to_uppercase()
}

/// The ledger: every number the rules track, plus the ambition cards.
/// Returns true when the player asks to close it.
fn ledger(ctx: &egui::Context, h: &HudInput, s: &State, goal: i64) -> bool {
    let mut close = false;
    let n = s.players();
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("ledger_veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(4, 6, 10, 242));
            if resp.clicked() {
                close = true;
            }
        });
    crate::probe::panel("veil", 2, screen);
    egui::Area::new(egui::Id::new("ledger"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let resp = panel().inner_margin(18.0).show(ui, |ui| {
                ui.set_width((screen.width() - 56.0).clamp(300.0, 760.0));
                ui.horizontal(|ui| {
                    plabel!(ui, "ledger", 3, "title", head("LEDGER", 22.0));
                    plabel!(ui, "ledger", 3, "chapter", RichText::new(format!("chapter {}  ·  first to {goal} power", s.chapter())).small().color(DIM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("Close  [L]")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(6.0);
                let wide = ui.available_width() >= 540.0;

                plabel!(ui, "ledger", 3, "title.standings", head("STANDINGS", 10.0).color(DIM));
                if wide {
                    egui::Grid::new("ledger_players").num_columns(12).spacing([9.0, 3.0]).show(ui, |ui| {
                        for (hdr, full) in [
                            ("PLAYER", "player"),
                            ("POWER", "power / goal"),
                            ("MAT", "Material"),
                            ("FUEL", "Fuel"),
                            ("WPN", "Weapons"),
                            ("REL", "Relics"),
                            ("PSI", "Psionic"),
                            ("TRO", "Trophies: destroyed enemy pieces"),
                            ("CAP", "Captives: one per enemy city destroyed in battle"),
                            ("CITY", "Cities left to build"),
                            ("PORT", "Starports left to build"),
                            ("RES", "Ships in reserve"),
                        ] {
                            plabel!(ui, "ledger", 3, &format!("hdr.{hdr}"), head(hdr, 9.0).color(DIM)).on_hover_text(full);
                        }
                        ui.end_row();
                        for p in 0..n {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(3.0, 11.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 0.0, ui_color(p));
                                plabel!(ui, "ledger", 3, &format!("name.{p}"), head(seat_short(h, p), 10.0).color(if p == s.current() { INK } else { DIM }));
                            });
                            plabel!(ui, "ledger", 3, &format!("power.{p}"), head(format!("{}/{}", s.power(p), goal), 10.5));
                            for t in 0..5 {
                                plabel!(ui, "ledger", 3, &format!("res.{p}.{t}"), RichText::new(s.resource(p, t).to_string()).small());
                            }
                            plabel!(ui, "ledger", 3, &format!("tro.{p}"), RichText::new(s.trophies(p).to_string()).small());
                            plabel!(ui, "ledger", 3, &format!("cap.{p}"), RichText::new(s.captives(p).to_string()).small());
                            plabel!(ui, "ledger", 3, &format!("cities.{p}"), RichText::new(s.cities_left(p).to_string()).small());
                            plabel!(ui, "ledger", 3, &format!("ports.{p}"), RichText::new(s.ports_left(p).to_string()).small());
                            plabel!(ui, "ledger", 3, &format!("reserve.{p}"), RichText::new(s.reserve(p).to_string()).small());
                            ui.end_row();
                        }
                    });
                } else {
                    for p in 0..n {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(3.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 0.0, ui_color(p));
                            plabel!(ui, "ledger", 3, &format!("name.{p}"), head(seat_short(h, p), 11.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                plabel!(ui, "ledger", 3, &format!("power.{p}"), head(format!("{} / {}", s.power(p), goal), 12.0));
                            });
                        });
                        ui.horizontal_wrapped(|ui| {
                            for t in 0..5 {
                                let sq = ui.allocate_exact_size(egui::vec2(5.0, 5.0), egui::Sense::hover()).1;
                                ui.painter().rect_filled(sq.rect, 1.0, res_color(t));
                                plabel!(ui, "ledger", 3, &format!("res.{p}.{t}"), RichText::new(format!("{} {}", RESOURCES[t], s.resource(p, t))).small().color(DIM));
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            for (label, value) in [
                                ("trophies", s.trophies(p)),
                                ("captives", s.captives(p)),
                                ("cities left", s.cities_left(p)),
                                ("ports left", s.ports_left(p)),
                                ("reserve", s.reserve(p)),
                            ] {
                                plabel!(ui, "ledger", 3, &format!("spoils.{p}.{label}"), RichText::new(format!("{value} {label}")).small().color(DIM));
                            }
                        });
                        ui.add_space(4.0);
                    }
                }

                ui.add_space(8.0);
                plabel!(ui, "ledger", 3, "title.cards", head("SCORING CARDS · THE FIVE AMBITIONS", 10.0).color(DIM));
                let ranks = s.ambition_ranks();
                if wide {
                    egui::Grid::new("ledger_ambitions").num_columns(3 + n).spacing([10.0, 3.0]).show(ui, |ui| {
                        plabel!(ui, "ledger", 3, "hdr.CARD", head("CARD", 9.0).color(DIM));
                        plabel!(ui, "ledger", 3, "hdr.DECLARED", head("DECLARED", 9.0).color(DIM));
                        plabel!(ui, "ledger", 3, "hdr.MARKERS", head("1ST / 2ND", 9.0).color(DIM)).on_hover_text("Power for a clear first and second place");
                        for p in 0..n {
                            plabel!(ui, "ledger", 3, &format!("hdr.seat.{p}"), head(seat_short(h, p), 9.0).color(DIM));
                        }
                        ui.end_row();
                        for a in 0..5 {
                            let rank = ranks[a];
                            let vals = s.ambition_values(a);
                            let top = (0..n).map(|p| s.ambition_tally(p, a)).max().unwrap_or(0);
                            plabel!(ui, "ledger", 3, &format!("amb.{a}"), head(AMBITIONS[a].to_uppercase(), 10.0).color(if rank > 0 { INK } else { DIM }));
                            plabel!(ui, "ledger", 3, &format!("rank.{a}"), RichText::new(if rank > 0 { format!("×{rank}") } else { "—".into() }).small().color(if rank > 0 { BRASS } else { DIM }));
                            plabel!(ui, "ledger", 3, &format!("markers.{a}"), RichText::new(if rank > 0 { format!("{}/{}", vals[0], vals[1]) } else { "—".into() }).small().color(DIM));
                            for p in 0..n {
                                let tally = s.ambition_tally(p, a);
                                let leads = rank > 0 && tally > 0 && tally == top;
                                plabel!(ui, "ledger", 3, &format!("tally.{a}.{p}"), RichText::new(tally.to_string()).small().color(if leads { BRASS } else { DIM }));
                            }
                            ui.end_row();
                        }
                    });
                } else {
                    for a in 0..5 {
                        let rank = ranks[a];
                        ui.horizontal(|ui| {
                            plabel!(ui, "ledger", 3, &format!("amb.{a}"), head(AMBITIONS[a].to_uppercase(), 10.5).color(if rank > 0 { INK } else { DIM }));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let tag = if rank > 0 { let v = s.ambition_values(a); format!("{}/{}", v[0], v[1]) } else { "—".into() };
                                plabel!(ui, "ledger", 3, &format!("rank.{a}"), head(tag, 10.0).color(if rank > 0 { BRASS } else { DIM }));
                            });
                        });
                        let tallies: Vec<String> = (0..n).map(|p| format!("{} {}", seat_short(h, p), s.ambition_tally(p, a))).collect();
                        plabel!(ui, "ledger", 3, &format!("tally.{a}"), RichText::new(tallies.join("  ·  ")).small().color(DIM));
                    }
                }
                ui.add_space(8.0);
                let bank: Vec<String> = (0..5).map(|t| format!("{} {}", RESOURCES[t], s.supply(t))).collect();
                plabel!(ui, "ledger", 3, "bank", RichText::new(format!("BANK  {}", bank.join("  ·  "))).small().color(DIM));
                plabel!(ui, "ledger", 3, "note.trophies", RichText::new("Trophies are destroyed enemy pieces. Captives are rival agents taken by taxing their cities or securing Court cards. Each declared ambition pays the first and second place power of all its markers; Court card icons count with resources.").small().color(DIM));
                ui.add_space(8.0);
                plabel!(ui, "ledger", 3, "title.court", head("COURT · INFLUENCE WITH AGENTS, SECURE WITH THE MOST", 10.0).color(DIM));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
                    for slot in 0..s.court_slots() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            match s.court_card(slot) {
                                Some(card) => {
                                    let (art, _) = ui.allocate_exact_size(egui::vec2(24.0, 30.0), egui::Sense::hover());
                                    courtart::paint(ui.painter(), art, card, true, 1.0);
                                    plabel!(ui, "ledger", 3, &format!("court.{slot}"), RichText::new(format!("{} · {}", card.name().to_uppercase(), (0..n).map(|p| format!("{} {}", seat_short(h, p), s.agents_on(slot, p))).collect::<Vec<_>>().join(" "))).small().color(INK))
                                }
                                None => plabel!(ui, "ledger", 3, &format!("court.{slot}"), RichText::new("—").small().color(DIM)),
                            };
                        });
                    }
                });
                for p in 0..n {
                    let secured = s.secured(p);
                    let names: Vec<String> = secured.iter().map(|c| c.name().to_uppercase()).collect();
                    let text = if names.is_empty() {
                        format!("{}: none secured", seat_short(h, p))
                    } else {
                        format!("{}: {}", seat_short(h, p), names.join(" · "))
                    };
                    plabel!(ui, "ledger", 3, &format!("secured.{p}"), RichText::new(text).small().color(DIM));
                }
                plabel!(ui, "ledger", 3, "court.note", RichText::new(format!("A secured card stays in its player's panel: a resource icon for ambitions and a lasting effect. The row is refilled from the deck ({} left).", s.court_deck())).small().color(DIM));
            });
            crate::probe::panel("ledger", 3, resp.response.rect);
        });
    close
}

pub fn hud(ctx: &egui::Context, h: &HudInput) -> HudOutput {
    let mut out = HudOutput::default();
    let s = h.state;
    let n = s.players();
    let cur = s.current();
    let goal = State::power_goal(n);

    // ---- gate numbers over the board
    {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("gates")));
        for (t, p) in h.labels {
            let c = egui::pos2(p[0], p[1]);
            painter.circle_filled(c, 10.0, Color32::from_rgba_unmultiplied(10, 12, 18, 215));
            painter.circle_stroke(c, 10.0, egui::Stroke::new(1.0, BRASS.gamma_multiply(0.7)));
            painter.text(c, egui::Align2::CENTER_CENTER, t, FontId::new(12.5, FontFamily::Name("head".into())), BRASS);
        }
    }

    // The ledger is a modal over the table; hide the floating table cards so
    // they cannot paint over the standings or cover the scrollable content.
    let ledger_open = LEDGER.with(|l| l.get());
    crate::probe::begin(ctx, ledger_open, !ledger_open && !s.is_over());
    if !ledger_open && !s.is_over() {
        crate::cards::draw_table(ctx, h.table, h.flies, &h.slots);
    }

    // ---- players (left): score, resources and spoils at a glance
    egui::Area::new(egui::Id::new("players")).anchor(egui::Align2::LEFT_TOP, [14.0, 14.0]).show(ctx, |ui| {
        // Leave room for the 220-point chapter panel on phones; a secured line
        // must not widen the standings enough to cover it.
        let left_w = if ctx.screen_rect().width() < 620.0 { 203.0 } else { 248.0 };
        ui.set_width(left_w);
        ui.set_max_width(left_w);
        let resp = panel().inner_margin(egui::Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
            ui.set_max_width(left_w - 24.0);
            for p in 0..n {
                let active = p == cur && !s.is_over();
                let top = ui.cursor().min;
                crate::cards::note_row(p, egui::pos2(top.x + 120.0, top.y + 17.0));
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(if active { 5.0 } else { 3.0 }, 54.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0.0, ui_color(p).gamma_multiply(if active { 1.0 } else { 0.6 }));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let nm = match h.online {
                                Some(o) if o.ai[p] => format!("{}  BOT", NAMES[p].to_uppercase()),
                                Some(o) => format!("{}{}", o.who[p].to_uppercase(), if p == o.me { "  YOU" } else { "" }),
                                None => format!("{}{}", NAMES[p].to_uppercase(), if h.bots[p] { "  BOT" } else { "" }),
                            };
                            plabel!(ui, "standings", 1, &format!("name.{p}"), head(nm, 13.0).color(if active { INK } else { DIM }));
                            if s.initiative() == p {
                                plabel!(ui, "standings", 1, &format!("lead.{p}"), head("LEAD", 10.5).color(BRASS));
                            }
                            // Phone widths have no room beside the power; the
                            // leader moves to the secured line there.
                            let l = s.leader(p);
                            if ctx.screen_rect().width() >= 620.0 {
                                plabel!(ui, "standings", 1, &format!("leader.{p}"), head(LEADERS[l].to_uppercase(), 9.0).color(DIM))
                                    .on_hover_text(format!("Leader: {}. {}", LEADERS[l], LEADER_EFFECTS[l]));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                plabel!(ui, "standings", 1, &format!("goal.{p}"), RichText::new(format!("/{goal}")).small().color(DIM));
                                plabel!(ui, "standings", 1, &format!("power.{p}"), head(s.power(p).to_string(), 20.0))
                                    .on_hover_text(format!("Power — the score. First to {goal} ends the game after a chapter."));
                            });
                        });
                        // score track towards the power goal
                        let w = ui.available_width().max(40.0);
                        let (track, _) = ui.allocate_exact_size(egui::vec2(w, 3.0), egui::Sense::hover());
                        ui.painter().rect_filled(track, 0.0, HAIR);
                        let done = (s.power(p) as f32 / goal.max(1) as f32).clamp(0.0, 1.0);
                        if done > 0.0 {
                            let fill = egui::Rect::from_min_size(track.min, egui::vec2(track.width() * done, track.height()));
                            ui.painter().rect_filled(fill, 0.0, ui_color(p));
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            for t in 0..5 {
                                let sq = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover()).1;
                                ui.painter().rect_filled(sq.rect, 1.0, res_color(t));
                                plabel!(ui, "standings", 1, &format!("res.{p}.{t}"), RichText::new(s.resource(p, t).to_string()).small().color(DIM))
                                    .on_hover_text(format!("{} {}  ·  bank {}", RESOURCES[t], s.resource(p, t), s.supply(t)));
                                ui.add_space(2.0);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let tro = s.trophies(p);
                            let (tr, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            paint_trophy(ui.painter(), tr, if tro > 0 { BRASS } else { DIM });
                            plabel!(ui, "standings", 1, &format!("tro.{p}"), RichText::new(format!("TROPHIES {}", tro)).small().color(if tro > 0 { INK } else { DIM }))
                                .on_hover_text("Destroyed enemy pieces");
                            ui.add_space(4.0);
                            let cap = s.captives(p);
                            let (cr, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            paint_captive(ui.painter(), cr, if cap > 0 { CAPTIVE } else { DIM });
                            plabel!(ui, "standings", 1, &format!("cap.{p}"), RichText::new(format!("CAPTIVES {}", cap)).small().color(if cap > 0 { INK } else { DIM }))
                                .on_hover_text("Rival agents taken by taxing their cities or securing Court cards");
                            ui.add_space(4.0);
                            plabel!(ui, "standings", 1, &format!("agents.{p}"), RichText::new(format!("AGENTS {}", s.agents_left(p))).small().color(DIM))
                                .on_hover_text("Agents in supply for Influence");
                            let lore = s.lore(p);
                            if lore > 0 {
                                ui.add_space(4.0);
                                plabel!(ui, "standings", 1, &format!("lore.{p}"), RichText::new(format!("LORE {}", lore)).small().color(BRASS))
                                    .on_hover_text("Lore: once per turn on your action step, spend one for an extra action of the current suit. The player furthest behind gains one after each chapter.");
                            }
                        });
                        let secured = s.secured(p);
                        let names: Vec<&str> = secured.iter().map(|c| c.name()).collect();
                        let mut text = if names.is_empty() {
                            "SECURED —".to_string()
                        } else {
                            format!("SECURED  {}", names.join(" · "))
                        };
                        if ctx.screen_rect().width() < 620.0 {
                            text = format!("{}  ·  {}", LEADERS[s.leader(p)].to_uppercase(), text);
                        }
                        crate::probe::ptext_wrap(ui, "standings", 1, &format!("secure.{p}"), RichText::new(text).small().color(if names.is_empty() { DIM } else { BRASS }))
                            .on_hover_text({
                                let mut t = String::from("Court cards: a resource icon for ambitions and a lasting effect.");
                                for c in &secured {
                                    t.push_str(&format!("\n{} ({}): {}", c.name(), RESOURCES[c.icon()], c.effect()));
                                }
                                t
                            });
                    });
                });
                if p + 1 < n {
                    ui.add_space(4.0);
                }
            }

        });
        crate::probe::panel("standings", 1, resp.response.rect);
    });

    // ---- chapter and ambition cards (right)
    let panel_id = if s.is_over() { "ambitions_final" } else { "ambitions" };
    egui::Area::new(egui::Id::new("chapter")).anchor(egui::Align2::RIGHT_TOP, [-14.0, 14.0]).show(ctx, |ui| {
        ui.set_width(220.0);
        let resp = panel().inner_margin(egui::Margin::symmetric(12.0, 10.0)).show(ui, |ui| {
            ui.set_width(196.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if s.is_over() {
                    plabel!(ui, "ambitions_final", 1, "title.cards", head("SCORING CARDS · THE FIVE AMBITIONS", 9.0).color(DIM));
                    ui.add_space(4.0);
                    plabel!(ui, "ambitions_final", 1, "final", head("FINAL CHAPTER SCORED", 14.0));
                    plabel!(ui, "ambitions_final", 1, "final_note", RichText::new("Ambition markers are cleared after scoring.").small().color(DIM));
                    ui.add_space(3.0);
                    plabel!(ui, "ambitions_final", 1, "scoring_note", RichText::new("Tyrant counts captives  ·  Warlord counts trophies").small().color(DIM));
                    ui.add_space(4.0);
                    plabel!(ui, "ambitions_final", 1, "court", RichText::new("COURT · secured cards kept").small().color(DIM));
                    return;
                }
                plabel!(ui, panel_id, 1, "chapter", head(format!("CHAPTER {} OF 5", s.chapter()), 14.0));
                plabel!(ui, panel_id, 1, "goal", RichText::new(format!("first to {goal} power")).small().color(DIM));
                ui.add_space(3.0);
                plabel!(ui, panel_id, 1, "title.cards", head("SCORING CARDS · THE FIVE AMBITIONS", 9.0).color(DIM));
                ui.add_space(2.0);
                let ranks = s.ambition_ranks();
                // Two cards per row, so the strip stays above the table.
                for row in 0..3 {
                    ui.horizontal(|ui| {
                        for a in row * 2..(row * 2 + 2).min(5) {
                            let rank = ranks[a];
                            let tally: Vec<i64> = (0..n).map(|p| s.ambition_tally(p, a)).collect();
                            let top = tally.iter().copied().max().unwrap_or(0);
                            let leaders: Vec<usize> = (0..n).filter(|&p| top > 0 && tally[p] == top).collect();
                            let note = if rank == 0 {
                                "not declared".to_string()
                            } else if top == 0 {
                                "no tally yet".to_string()
                            } else if leaders.len() == 1 {
                                format!("{}  {}", seat_short(h, leaders[0]), top)
                            } else {
                                format!("TIE  {top}")
                            };
                            ui.vertical(|ui| {
                                ui.set_width(92.0);
                                ui.horizontal(|ui| {
                                    plabel!(ui, panel_id, 1, &format!("amb.{a}"), head(AMBITIONS[a].to_uppercase(), 10.0).color(if rank > 0 { INK } else { DIM }));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let tag = if rank > 0 { let v = s.ambition_values(a); format!("{}/{}", v[0], v[1]) } else { "—".into() };
                                        plabel!(ui, panel_id, 1, &format!("rank.{a}"), head(tag, 9.0).color(if rank > 0 { BRASS } else { DIM }));
                                    });
                                });
                                plabel!(ui, panel_id, 1, &format!("leader.{a}"), RichText::new(note).small().color(DIM));
                            });
                        }
                    });
                }
                plabel!(ui, panel_id, 1, "court", RichText::new(format!("COURT · {} in the row · {} in the deck", s.court_available(), s.court_deck())).small().color(DIM));
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if ui.add_sized([94.0, 22.0], egui::Button::new(head(if guides::is_open() { "GUIDE ON [G]" } else { "GUIDE [G]" }, 9.0))).clicked() {
                        toggle_guide();
                    }
                    if ui.add_sized([94.0, 22.0], egui::Button::new(head("RULES [R]", 9.0))).clicked() {
                        open_tutorial();
                    }
                });
                if ui.add_sized([192.0, 22.0], egui::Button::new(head("LEDGER  [L]", 10.0))).clicked() {
                    LEDGER.with(|l| l.set(true));
                }
            });
        });
        crate::probe::panel(panel_id, 1, resp.response.rect);
    });

    // ---- log (above the hand), quiet. It sits below the HUD panels so a
    // played card or a long event line can never cover the hand's buttons.
    egui::Area::new(egui::Id::new("log"))
        .order(egui::Order::Background)
        .anchor(egui::Align2::LEFT_BOTTOM, [14.0, -218.0])
        .show(ctx, |ui| {
        ui.set_max_width(280.0);
        for (i, line) in h.log.iter().rev().take(4).rev().enumerate() {
            let fade = 90 + i as u8 * 30;
            ui.label(RichText::new(line).small().color(Color32::from_gray(fade.min(170))));
        }
    });

    if let Some(t) = h.toast {
        egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_TOP, [0.0, 22.0]).show(ctx, |ui| {
            panel().inner_margin(egui::Margin::symmetric(16.0, 8.0)).show(ui, |ui| {
                ui.add(egui::Label::new(head(t, 15.0)).wrap_mode(egui::TextWrapMode::Extend));
            });
        });
    }

    // ---- game over
    if s.is_over() {
        LEDGER.with(|l| l.set(false));
        // Sit below the HUD's court row and final-scoring panel so neither is
        // hidden by the result card on a short screen.
        egui::Area::new(egui::Id::new("over")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -104.0]).show(ctx, |ui| {
            let resp = panel().inner_margin(24.0).show(ui, |ui| {
                let w = s.winner();
                plabel!(ui, "game_over", 3, "winner", head(format!("{} WINS", seat_short(h, w)), 30.0).color(ui_color(w)));
                plabel!(ui, "game_over", 3, "chapter", RichText::new(format!("chapter {}  ·  first to {goal} power", s.chapter())).small().color(DIM));
                ui.add_space(10.0);
                egui::Grid::new("over_standings").num_columns(4).spacing([18.0, 4.0]).show(ui, |ui| {
                    for hdr in ["PLAYER", "POWER", "RESOURCES", "SPOILS"] {
                        plabel!(ui, "game_over", 3, &format!("hdr.{hdr}"), head(hdr, 9.5).color(DIM));
                    }
                    ui.end_row();
                    for p in 0..n {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(3.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 0.0, ui_color(p));
                            plabel!(ui, "game_over", 3, &format!("name.{p}"), head(seat_short(h, p), 12.0).color(if p == w { INK } else { DIM }));
                        });
                        plabel!(ui, "game_over", 3, &format!("power.{p}"), head(format!("{} / {}", s.power(p), goal), 12.0));
                        let res: i64 = (0..5).map(|t| s.resource(p, t)).sum();
                        plabel!(ui, "game_over", 3, &format!("resources.{p}"), RichText::new(res.to_string()).small());
                        plabel!(ui, "game_over", 3, &format!("spoils.{p}"), RichText::new(format!("{} trophies  ·  {} captives", s.trophies(p), s.captives(p))).small());
                        ui.end_row();
                    }
                });
                ui.add_space(12.0);
                if ui.button("New game").clicked() {
                    out.new_game = true;
                }
            });
            crate::probe::panel("game_over", 3, resp.response.rect);
        });
        out.wants_pointer = ctx.wants_pointer_input();
        return out;
    }

    // ---- hot-seat handoff
    if let Some(seat) = h.handoff {
        egui::Area::new(egui::Id::new("handoff")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            panel().inner_margin(24.0).show(ui, |ui| {
                ui.label(head(format!("PASS TO {}", NAMES[seat].to_uppercase()), 26.0).color(ui_color(seat)));
                ui.add_space(8.0);
                if ui.button("Ready").clicked() {
                    out.ready = true;
                }
            });
        });
        out.wants_pointer = ctx.wants_pointer_input();
        return out;
    }

    // ---- bottom: what to do now, then the hand and its plays
    // The guide is a reading mode: the hand and action strip step aside so the
    // labels can stay clear of the controls they describe.
    if !guides::is_open() {
    let human = !h.bots[cur];
    egui::Area::new(egui::Id::new("hand")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -14.0]).show(ctx, |ui| {
        // On a narrow window the hand uses almost the full width; on a wide one
        // it stays centred and clear of the players panel.
        let hand_w = (ctx.screen_rect().width() - 28.0).clamp(300.0, 900.0);
        ui.set_width(hand_w);
        ui.set_max_width(hand_w);
        // Narrow screens stack the card actions under each card; tightening the
        // rows keeps the hand from rising over the table.
        if ctx.screen_rect().width() < 900.0 {
            ui.spacing_mut().button_padding = egui::vec2(4.0, 3.0);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.y = 4.0;
        }
        let status = match (s.phase(), human) {
            (_, false) => match h.online {
                Some(o) if !o.ai[cur] => format!("Waiting for {}", o.who[cur]),
                _ => format!("{} is thinking", NAMES[cur]),
            },
            (Phase::Lead, _) => format!("{}: lead a card, or pass to end the chapter", NAMES[cur]),
            (Phase::Follow, _) => format!("{}: follow {}. Surpass, copy or pivot, seize, or pass", NAMES[cur], s.lead_suit().map(|x| SUITS[x]).unwrap_or("?")),
            (Phase::Act, _) => format!(
                "{}: {} {} left ({})",
                NAMES[cur],
                s.actions_left(),
                if s.actions_left() == 1 { "action" } else { "actions" },
                SUITS[s.action_suit()]
            ),
            _ => String::new(),
        };
        panel().inner_margin(egui::Margin::symmetric(12.0, 6.0)).show(ui, |ui| {
            ui.add(egui::Label::new(head(status, 15.0).color(if human { BRASS } else { DIM })).wrap());
        });
        ui.add_space(6.0);
        if human && !h.busy {
            let acts = s.legal_actions();
            let show = |ui: &mut egui::Ui, a: Action, out: &mut HudOutput| {
                let sys = action_systems(a);
                let b = ui.add(egui::Button::new(action_text(a, s)).wrap());
                if b.hovered() {
                    out.hover_systems = sys;
                }
                if b.clicked() {
                    out.action = Some(a);
                }
            };
            if s.phase() == Phase::Act {
                let many = acts.len() > 36;
                if many && h.selected.is_none() {
                    ui.label(RichText::new("Select a system on the board to see its actions").small().color(DIM));
                }
                ui.horizontal_wrapped(|ui| {
                    for a in acts.iter().copied() {
                        let sys = action_systems(a);
                        match h.selected {
                            Some(sel) if !sys.is_empty() && !sys.contains(&sel) => continue,
                            None if many && !sys.is_empty() => continue,
                            _ => {}
                        }
                        show(ui, a, &mut out);
                    }
                });
            } else {
                let now = ctx.input(|i| i.time);
                let mut seen = Vec::new();
                let cards = s.hand(cur);
                let narrow = ctx.screen_rect().width() < 620.0;
                let mut render = |ui: &mut egui::Ui, centre: bool| {
                    if centre {
                        // A one-card hand would otherwise sit in the far-left
                        // corner, under the Administration panel.
                        let row_w = cards.len().min(6) as f32 * (crate::cards::CARD_W + 4.0);
                        ui.add_space(((ui.available_width() - row_w) * 0.5).max(0.0));
                    }
                    for (i, c) in cards.iter().copied().enumerate() {
                        ui.vertical(|ui| {
                            // Actions wrap inside a card's column instead of
                            // pushing the whole hand off a narrow screen.
                            ui.set_max_width(crate::cards::CARD_W);
                            let r = crate::cards::hand_card(ui, c, i, now, &mut seen);
                            let mine: Vec<Action> = acts.iter().copied().filter(|a| a.card() == Some(c)).collect();
                            if r.clicked() && mine.len() == 1 {
                                out.action = Some(mine[0]);
                            }
                            for a in mine {
                                show(ui, a, &mut out);
                            }
                        });
                    }
                };
                if narrow {
                    // Six 96-point cards cannot fit a phone width; keep one row
                    // and let the hand scroll rather than spill off screen.
                    egui::ScrollArea::horizontal()
                        .id_salt("hand_scroll")
                        .auto_shrink([false, true])
                        .show(ui, |ui| ui.horizontal(|ui| render(ui, false)));
                } else {
                    ui.horizontal_wrapped(|ui| render(ui, true));
                }
                crate::cards::end_hand_frame(&seen);
                ui.horizontal_wrapped(|ui| {
                    for a in acts.iter().copied().filter(|a| a.card().is_none()) {
                        show(ui, a, &mut out);
                    }
                });
            }
        }
    });
    }
    if ledger_open {
        if ledger(ctx, h, s, goal) {
            LEDGER.with(|l| l.set(false));
        }
        out.wants_pointer = true;
    } else {
        out.wants_pointer = ctx.wants_pointer_input();
    }
    if guides::is_open() {
        guides::draw(ctx, h.anchors, goal);
        out.wants_pointer = true;
    }
    if fieldmanual::is_open() {
        fieldmanual::overlay(ctx, goal);
        out.wants_pointer = true;
    }
    out
}

/// Name of the Court card in row slot `slot`.
fn court_name(s: &State, slot: i64) -> &'static str {
    s.court_card(slot.max(0) as usize).map(|c| c.name()).unwrap_or("—")
}

pub fn describe(e: [i64; 5], names: &[&str; 4]) -> Option<String> {
    let nm = |p: i64| names[p as usize];
    Some(match e[0] {
        0 => format!("{} leads {}", nm(e[1]), card_label(Card(e[2] as u8))),
        1 => format!("{} surpasses with {}", nm(e[1]), card_label(Card(e[2] as u8))),
        2 => format!("{} copies with {}", nm(e[1]), card_label(Card(e[2] as u8))),
        3 => format!("{} pivots with {}", nm(e[1]), card_label(Card(e[2] as u8))),
        4 => format!("{} passes", nm(e[1])),
        5 => format!("{} seizes the initiative", nm(e[1])),
        6 => format!("{} spends {}", nm(e[1]), RESOURCES[e[2] as usize]),
        8 => format!("{} taxes {}", nm(e[1]), sys_name(e[2] as usize)),
        9 => format!("{} builds a {} at {}", nm(e[1]), if e[3] == 1 { "city" } else { "starport" }, sys_name(e[2] as usize)),
        11 => format!("{} builds ships at {}", nm(e[1]), sys_name(e[2] as usize)),
        12 => format!("{} moves {} ships {} to {}", nm(e[1]), e[4], sys_name(e[2] as usize), sys_name(e[3] as usize)),
        13 => format!("{} repairs at {}", nm(e[1]), sys_name(e[2] as usize)),
        14 => format!("{} attacks {} at {}", nm(e[1]), nm(e[3]), sys_name(e[2] as usize)),
        15 => format!("{} secures {}{}", nm(e[1]), COURT_CARDS[e[3] as usize], if e[4] > 0 { format!(", taking {} captive", e[4]) } else { String::new() }),
        17 => format!("{} influences {}", nm(e[1]), COURT_CARDS[e[3] as usize]),
        16 => format!("{} spends Lore for an extra action", nm(e[1])),
        20 => format!("Round over, {} holds the initiative", nm(e[1])),
        21 => format!("Chapter {} scored", e[1]),
        _ => return None,
    })
}

/// The current frame's rendered text, geometry, layer order and rules values.
/// The browser build mirrors this JSON into the page for automated checks.
pub fn probe_json(s: &State, held: bool) -> String {
    crate::probe::json(s, held)
}
