//! egui screens: start menu, in-game HUD (scores, hand, actions, log), game over.

use egui::{self, Color32, RichText};
use reach_rules::{Action, Card, Kind, Phase, Slot, State, AMBITIONS, RESOURCES, SUITS, SYSTEMS};

pub const NAMES: [&str; 4] = ["Crimson", "Azure", "Amber", "Verdant"];
/// sRGB, for UI.
pub const COLORS_UI: [[u8; 3]; 4] = [[222, 58, 52], [58, 120, 245], [243, 190, 40], [70, 214, 140]];

pub fn ui_color(p: usize) -> Color32 {
    let c = COLORS_UI[p];
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn suit_color(s: usize) -> Color32 {
    match s {
        0 => Color32::from_rgb(160, 110, 230),
        1 => Color32::from_rgb(225, 80, 70),
        2 => Color32::from_rgb(230, 170, 60),
        _ => Color32::from_rgb(70, 160, 230),
    }
}

pub fn res_color(t: usize) -> Color32 {
    let c = [[200, 160, 100], [240, 120, 60], [210, 80, 80], [110, 200, 230], [190, 120, 240]][t];
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn card_label(c: Card) -> String {
    c.name()
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
            ui.horizontal(|ui| {
                ui.label("Setup card");
                for k in 0..State::setup_count(m.players) {
                    ui.selectable_value(&mut m.setup, k, (k + 1).to_string());
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
        Kind::Seize => "Seize the initiative".into(),
        Kind::Prelude => format!("Spend {} · extra action", RESOURCES[v[1] as usize]),
        Kind::EndTurn => "End turn".into(),
        Kind::Tax => format!("Tax {}", sys_name(v[1] as usize)),
        Kind::BuildCity => format!("Build city · {}", sys_name(v[1] as usize)),
        Kind::BuildPort => format!("Build starport · {}", sys_name(v[1] as usize)),
        Kind::BuildShips => format!("Build ships · {}", sys_name(v[1] as usize)),
        Kind::Move => format!("Move {} · {} → {}", v[3], sys_name(v[1] as usize), sys_name(v[2] as usize)),
        Kind::Repair => format!("Repair · {}", sys_name(v[1] as usize)),
        Kind::Battle => {
            let (asl, sk, rd) = (v[3] / 49, v[3] / 7 % 7, v[3] % 7);
            format!("Battle · {} vs {} ({}A {}S {}R)", sys_name(v[1] as usize), NAMES[v[2] as usize], asl, sk, rd)
        }
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

pub fn hud(ctx: &egui::Context, h: &HudInput) -> HudOutput {
    let mut out = HudOutput::default();
    let s = h.state;
    let n = s.players();
    let cur = s.current();

    // ---- scoreboard (top-left)
    egui::Area::new(egui::Id::new("scores")).anchor(egui::Align2::LEFT_TOP, [10.0, 10.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.label(RichText::new(format!("Chapter {} of 5", s.chapter())).strong().size(16.0));
            let ranks = s.ambition_ranks();
            let mut declared: Vec<(i64, usize)> = (0..5).filter(|&a| ranks[a] > 0).map(|a| (ranks[a], a)).collect();
            declared.sort();
            let txt = if declared.is_empty() {
                "No ambitions declared".to_string()
            } else {
                declared.iter().map(|(r, a)| format!("{}: {}", ["1st", "2nd", "3rd"][*r as usize - 1], AMBITIONS[*a])).collect::<Vec<_>>().join("   ")
            };
            ui.label(RichText::new(txt).color(Color32::from_rgb(255, 200, 120)));
            ui.add_space(4.0);
            egui::Grid::new("sb").num_columns(7).spacing([10.0, 3.0]).show(ui, |ui| {
                for h in ["", "Power", "Resources", "Ships", "Cities", "Trophies", ""] {
                    ui.label(RichText::new(h).small());
                }
                ui.end_row();
                for p in 0..n {
                    let mark = if p == cur && !s.is_over() { "▶ " } else { "   " };
                    let mut name = format!("{mark}{}", NAMES[p]);
                    if h.bots[p] {
                        name.push_str(" (bot)");
                    }
                    ui.label(RichText::new(name).color(ui_color(p)).strong());
                    ui.label(RichText::new(s.power(p).to_string()).strong());
                    ui.horizontal(|ui| {
                        for t in 0..5 {
                            ui.label(RichText::new(s.resource(p, t).to_string()).color(res_color(t)).strong());
                        }
                    });
                    let on_board: i64 = (0..SYSTEMS).map(|y| s.ship_count(y, p)).sum();
                    ui.label(format!("{on_board}+{}", s.reserve(p)));
                    let cities = (0..SYSTEMS).flat_map(|y| (0..2).map(move |sl| (y, sl))).filter(|&(y, sl)| matches!(s.slot(y, sl), Slot::City { owner, .. } if owner == p)).count();
                    ui.label(cities.to_string());
                    ui.label(format!("{}/{}", s.trophies(p), s.captives(p)));
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
                    ui.label(RichText::new(format!("{}: {} power", NAMES[p], s.power(p))).color(ui_color(p)));
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
                (Phase::Lead, _) => format!("{}: lead a card, or pass to end the chapter", NAMES[cur]),
                (Phase::Follow, _) => format!(
                    "{}: follow {} (surpass, copy or pivot), seize, or pass",
                    NAMES[cur],
                    s.lead_suit().map(|x| SUITS[x]).unwrap_or("?")
                ),
                (Phase::Act, _) => format!(
                    "{}: {} action{} left ({})",
                    NAMES[cur],
                    s.actions_left(),
                    if s.actions_left() == 1 { "" } else { "s" },
                    SUITS[s.action_suit()]
                ),
                _ => String::new(),
            };
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(status).strong().color(ui_color(cur)));
            });
            if human && !h.busy {
                let acts = s.legal_actions();
                let mut show = |ui: &mut egui::Ui, a: Action, out: &mut HudOutput| {
                    let sys = action_systems(a);
                    let b = ui.button(action_text(a));
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
                        ui.label(RichText::new("Click a system on the board to see its actions").small());
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
                    ui.horizontal(|ui| {
                        for c in s.hand(cur) {
                            ui.vertical(|ui| {
                                let col = suit_color(c.suit());
                                ui.label(RichText::new(format!("{}\n{}", SUITS[c.suit()], c.number())).size(16.0).strong().color(Color32::WHITE).background_color(col.gamma_multiply(0.6)));
                                for a in acts.iter().copied().filter(|a| a.card() == Some(c)) {
                                    show(ui, a, &mut out);
                                }
                            });
                        }
                    });
                    ui.horizontal(|ui| {
                        for a in acts.iter().copied().filter(|a| a.card().is_none()) {
                            show(ui, a, &mut out);
                        }
                    });
                }
            }
        });
    });
    out.wants_pointer = ctx.wants_pointer_input();
    out
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
        12 => format!("{} moves {} ships {} → {}", nm(e[1]), e[4], sys_name(e[2] as usize), sys_name(e[3] as usize)),
        13 => format!("{} repairs at {}", nm(e[1]), sys_name(e[2] as usize)),
        14 => format!("{} attacks {} at {}", nm(e[1]), nm(e[3]), sys_name(e[2] as usize)),
        20 => format!("Round over, {} holds the initiative", nm(e[1])),
        21 => format!("Chapter {} scored", e[1]),
        _ => return None,
    })
}
