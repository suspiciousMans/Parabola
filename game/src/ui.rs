//! egui screens: start menu, in-game HUD (scores, hand, actions, log), game over.

use egui::{self, Color32, RichText};
use reach_rules::{Action, Card, Phase, State, Suit};

pub const NAMES: [&str; 4] = ["Crimson", "Azure", "Amber", "Verdant"];
/// sRGB, for UI.
pub const COLORS_UI: [[u8; 3]; 4] = [[222, 58, 52], [58, 120, 245], [243, 190, 40], [70, 214, 140]];

pub fn ui_color(p: usize) -> Color32 {
    let c = COLORS_UI[p];
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn suit_color(s: Suit) -> Color32 {
    match s {
        Suit::Move => Color32::from_rgb(70, 160, 230),
        Suit::Fight => Color32::from_rgb(225, 80, 70),
        Suit::Build => Color32::from_rgb(230, 170, 60),
        Suit::Influence => Color32::from_rgb(160, 110, 230),
    }
}

pub fn card_label(c: Card) -> String {
    format!("{} {}", c.suit().name(), c.rank())
}

pub struct MenuState {
    pub players: usize,
    pub bots: [bool; 4],
}

impl Default for MenuState {
    fn default() -> Self {
        Self { players: 3, bots: [false, true, true, true] }
    }
}

/// Returns true when "Start" is pressed.
pub fn menu(ctx: &egui::Context, m: &mut MenuState) -> bool {
    let mut start = false;
    egui::Area::new(egui::Id::new("menu")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(24.0).show(ui, |ui| {
            ui.set_width(340.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("EMBER REACH").size(34.0).strong().color(Color32::from_rgb(255, 170, 70)));
                ui.label(RichText::new("a fading star cluster, three chapters, one victor").italics());
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label("Players");
                for n in 2..=4 {
                    ui.selectable_value(&mut m.players, n, n.to_string());
                }
            });
            ui.add_space(6.0);
            for p in 0..m.players {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:<8}", NAMES[p])).color(ui_color(p)).strong());
                    ui.selectable_value(&mut m.bots[p], false, "Human");
                    ui.selectable_value(&mut m.bots[p], true, "Bot");
                });
            }
            ui.add_space(14.0);
            ui.vertical_centered(|ui| {
                if ui.add_sized([180.0, 36.0], egui::Button::new(RichText::new("Start game").size(18.0))).clicked() {
                    start = true;
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(
                    "Right-drag orbit · middle-drag or WASD pan · wheel zoom\nQ/E turn · Space overview · double-click a system to focus",
                )
                .small()
                .weak(),
            );
        });
    });
    start
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
}

#[derive(Default)]
pub struct HudOutput {
    pub action: Option<Action>,
    pub hover_systems: Vec<usize>,
    pub new_game: bool,
    pub ready: bool,
    pub overview: bool,
    pub wants_pointer: bool,
}

pub fn action_text(a: Action) -> String {
    match a {
        Action::Play(c) => card_label(c),
        Action::Move { from, to } => format!("Move  {} → {}", from + 1, to + 1),
        Action::Fight { system, enemy } => format!("Fight at {} vs {}", system + 1, NAMES[enemy as usize]),
        Action::Build { system } => format!("Build city at {}  (2 power)", system + 1),
        Action::Levy { system } => format!("Levy ships at {}  (1 power)", system + 1),
        Action::Sway => "Sway  (+2 power)".into(),
        Action::Tax => "Tax  (+1 power per city)".into(),
        Action::EndTurn => "End turn".into(),
    }
}

pub fn action_systems(a: Action) -> Vec<usize> {
    match a {
        Action::Move { from, to } => vec![from as usize, to as usize],
        Action::Fight { system, .. } | Action::Build { system } | Action::Levy { system } => vec![system as usize],
        _ => vec![],
    }
}

pub fn hud(ctx: &egui::Context, h: &HudInput) -> HudOutput {
    let mut out = HudOutput::default();
    let s = h.state;
    let n = s.players();
    let cur = s.current();

    // ---- scoreboard (top-left)
    egui::Area::new(egui::Id::new("scores")).anchor(egui::Align2::LEFT_TOP, [10.0, 10.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.label(
                RichText::new(format!("Chapter {} of {}", s.round(), reach_rules::ROUNDS)).strong().size(16.0),
            );
            ui.label(RichText::new(format!("Ambition: {}", s.ambition().name())).color(Color32::from_rgb(255, 200, 120)));
            ui.add_space(4.0);
            egui::Grid::new("sb").num_columns(6).spacing([10.0, 3.0]).show(ui, |ui| {
                ui.label("");
                ui.label(RichText::new("VP").small());
                ui.label(RichText::new("Power").small());
                ui.label(RichText::new("Ships").small());
                ui.label(RichText::new("Cities").small());
                ui.label("");
                ui.end_row();
                for p in 0..n {
                    let mark = if p == cur && !s.is_over() { "▶ " } else { "   " };
                    let mut name = format!("{mark}{}", NAMES[p]);
                    if h.bots[p] {
                        name.push_str(" (bot)");
                    }
                    ui.label(RichText::new(name).color(ui_color(p)).strong());
                    ui.label(RichText::new(s.vp(p).to_string()).strong());
                    ui.label(s.power(p).to_string());
                    let on_board: i64 = (0..reach_rules::SYSTEMS).map(|y| s.ships(y, p)).sum();
                    ui.label(format!("{on_board}+{}", s.pool(p)));
                    ui.label((0..reach_rules::SYSTEMS).filter(|&y| s.city(y) == Some(p)).count().to_string());
                    ui.label(if s.initiative() == p { "★ lead" } else { "" });
                    ui.end_row();
                }
            });
        });
    });

    // ---- log (bottom-left)
    egui::Area::new(egui::Id::new("log")).anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0]).show(ctx, |ui| {
        ui.set_max_width(300.0);
        for line in h.log.iter().rev().take(6).rev() {
            ui.label(RichText::new(line).small().color(Color32::from_gray(190)));
        }
    });

    if let Some(t) = h.toast {
        egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_TOP, [0.0, 18.0]).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(t).size(18.0).strong());
            });
        });
    }

    // ---- game over
    if s.is_over() {
        egui::Area::new(egui::Id::new("over")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(22.0).show(ui, |ui| {
                let w = s.winner();
                ui.label(RichText::new(format!("{} wins!", NAMES[w])).size(30.0).strong().color(ui_color(w)));
                ui.add_space(8.0);
                for p in 0..n {
                    ui.label(RichText::new(format!("{}: {} VP, {} power", NAMES[p], s.vp(p), s.power(p))).color(ui_color(p)));
                }
                ui.add_space(10.0);
                if ui.button("New game").clicked() {
                    out.new_game = true;
                }
            });
        });
        out.wants_pointer = ctx.wants_pointer_input();
        return out;
    }

    // ---- hot-seat handoff
    if let Some(seat) = h.handoff {
        egui::Area::new(egui::Id::new("handoff")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(22.0).show(ui, |ui| {
                ui.label(RichText::new(format!("Pass to {}", NAMES[seat])).size(28.0).strong().color(ui_color(seat)));
                if ui.button("Ready").clicked() {
                    out.ready = true;
                }
            });
        });
        out.wants_pointer = ctx.wants_pointer_input();
        return out;
    }

    // ---- bottom: status line, actions, hand
    let human = !h.bots[cur];
    egui::TopBottomPanel::bottom("hand").frame(egui::Frame::none()).show_separator_line(false).show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            let status = match (s.phase(), human) {
                (_, false) => format!("{} (bot) is thinking…", NAMES[cur]),
                (Phase::Lead, _) => format!("{}: lead a card — {} actions of its suit", NAMES[cur], 2),
                (Phase::Follow, _) => format!(
                    "{}: follow {} (or pivot if you hold none)",
                    NAMES[cur],
                    s.lead_suit().map(|x| x.name()).unwrap_or("?")
                ),
                (Phase::Act, _) => format!(
                    "{}: {} action{} left ({})",
                    NAMES[cur],
                    s.actions_left(),
                    if s.actions_left() == 1 { "" } else { "s" },
                    s.action_suit().name()
                ),
                _ => String::new(),
            };
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(status).strong().color(ui_color(cur)));
            });
            if human && !h.busy {
                if s.phase() == Phase::Act {
                    let acts = s.legal_actions();
                    ui.horizontal_wrapped(|ui| {
                        for a in acts {
                            let sys = action_systems(a);
                            if let Some(sel) = h.selected {
                                if !sys.is_empty() && !sys.contains(&sel) {
                                    continue;
                                }
                            }
                            let b = ui.button(action_text(a));
                            if b.hovered() {
                                out.hover_systems = sys;
                            }
                            if b.clicked() {
                                out.action = Some(a);
                            }
                        }
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let legal = if matches!(s.phase(), Phase::Lead | Phase::Follow) { s.legal_actions() } else { vec![] };
                    for c in s.hand(cur) {
                        let playable = legal.contains(&Action::Play(c));
                        let col = suit_color(c.suit());
                        let txt = RichText::new(format!("{}\n{}", c.suit().name(), c.rank())).size(17.0).strong().color(
                            if playable { Color32::WHITE } else { Color32::from_gray(120) },
                        );
                        let b = egui::Button::new(txt)
                            .fill(if playable { col.gamma_multiply(0.75) } else { Color32::from_gray(40) })
                            .min_size(egui::vec2(78.0, 84.0));
                        if ui.add_enabled(playable, b).clicked() {
                            out.action = Some(Action::Play(c));
                        }
                    }
                });
            }
        });
    });
    out.wants_pointer = ctx.wants_pointer_input();
    out
}

pub fn describe(e: reach_rules::Event, lead_names: &[&str; 4]) -> Option<String> {
    use reach_rules::Event::*;
    let nm = |p: u8| lead_names[p as usize];
    Some(match e {
        CardPlayed { player, card } => format!("{} plays {}", nm(player), card_label(card)),
        Moved { from, to, ships, player } => format!("{} moves {ships} from {} to {}", nm(player), from + 1, to + 1),
        Fought { system, enemy_lost, attacker_lost, enemy } => {
            format!("Battle at {} vs {}: they lose {enemy_lost}, attacker loses {attacker_lost}", system + 1, nm(enemy))
        }
        Built { system, player } => format!("{} builds a city at {}", nm(player), system + 1),
        Levied { system, player, ships } => format!("{} levies {ships} ships at {}", nm(player), system + 1),
        Swayed { player, power } => format!("{} sways +{power} power", nm(player)),
        Taxed { player, power } => format!("{} taxes +{power} power", nm(player)),
        TrickWon { player } => format!("{} takes the lead (+1 power)", nm(player)),
        RoundScored { round, ambition } => format!("Chapter {round} scored: {}", ambition.name()),
        EndedTurn { .. } | None => return Option::None,
    })
}
