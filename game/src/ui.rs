//! egui screens: start menu, in-game HUD (scores, hand, actions, log), game over.

use egui::{self, Color32, FontFamily, FontId, RichText};
use reach_rules::{Action, Card, Kind, Phase, Slot, State, AMBITIONS, RESOURCES, SUITS, SYSTEMS};

pub const NAMES: [&str; 4] = ["Crimson", "Azure", "Amber", "Verdant"];
/// Player colours (sRGB): distinct but muted, so they sit on a dark board without glowing.
pub const COLORS_UI: [[u8; 3]; 4] = [[196, 72, 64], [76, 128, 212], [212, 164, 66], [92, 176, 128]];

// Palette: tinted neutrals, one brass accent, red only for alerts. See docs/STYLE.md.
pub const INK: Color32 = Color32::from_rgb(222, 224, 228);
pub const DIM: Color32 = Color32::from_rgb(138, 146, 160);
pub const PANEL: Color32 = Color32::from_rgba_premultiplied(12, 15, 21, 226);
pub const HAIR: Color32 = Color32::from_rgb(54, 61, 76);
pub const BRASS: Color32 = Color32::from_rgb(214, 164, 86);

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

/// Heading text: Chakra Petch. Body text is IBM Plex Sans (both SIL OFL, see assets/fonts).
pub fn head(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(FontId::new(size, FontFamily::Name("head".into())))
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
        ui.add_space(14.0);
        ui.label(RichText::new("Right-drag orbit  ·  middle-drag or WASD pan  ·  wheel zoom\nQ/E turn  ·  Space overview  ·  C cinematic camera").small().color(DIM));
    });
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
        Kind::Seize => "Seize".into(),
        Kind::Prelude => format!("Spend {} · extra action", RESOURCES[v[1] as usize]),
        Kind::EndTurn => "End turn".into(),
        Kind::Tax => format!("Tax {}", sys_name(v[1] as usize)),
        Kind::BuildCity => format!("Build city · {}", sys_name(v[1] as usize)),
        Kind::BuildPort => format!("Build starport · {}", sys_name(v[1] as usize)),
        Kind::BuildShips => format!("Build ships · {}", sys_name(v[1] as usize)),
        Kind::Move => format!("Move {} · {} to {}", v[3], sys_name(v[1] as usize), sys_name(v[2] as usize)),
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

    crate::cards::draw_table(ctx, h.table, h.flies, &h.slots);

    // ---- players (left): who is ahead, what they hold
    egui::Area::new(egui::Id::new("players")).anchor(egui::Align2::LEFT_TOP, [14.0, 14.0]).show(ctx, |ui| {
        ui.set_width(236.0);
        panel().inner_margin(egui::Margin::symmetric(12.0, 8.0)).show(ui, |ui| {
            for p in 0..n {
                let active = p == cur && !s.is_over();
                let top = ui.cursor().min;
                crate::cards::note_row(p, egui::pos2(top.x + 120.0, top.y + 17.0));
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(if active { 5.0 } else { 3.0 }, 34.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 0.0, ui_color(p).gamma_multiply(if active { 1.0 } else { 0.6 }));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let nm = match h.online {
                                Some(o) if o.ai[p] => format!("{}  BOT", NAMES[p].to_uppercase()),
                                Some(o) => format!("{}{}", o.who[p].to_uppercase(), if p == o.me { "  YOU" } else { "" }),
                                None => format!("{}{}", NAMES[p].to_uppercase(), if h.bots[p] { "  BOT" } else { "" }),
                            };
                            ui.label(head(nm, 13.0).color(if active { INK } else { DIM }));
                            if s.initiative() == p {
                                ui.label(head("LEAD", 10.5).color(BRASS));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(head(s.power(p).to_string(), 20.0));
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for t in 0..5 {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                                ui.painter().rect_filled(r, 1.0, res_color(t));
                                ui.label(RichText::new(s.resource(p, t).to_string()).small().color(DIM));
                                ui.add_space(3.0);
                            }
                        });
                    });
                });
                if p + 1 < n {
                    ui.add_space(2.0);
                }
            }
        });
    });

    // ---- chapter and ambitions (right)
    egui::Area::new(egui::Id::new("chapter")).anchor(egui::Align2::RIGHT_TOP, [-14.0, 14.0]).show(ctx, |ui| {
        ui.set_width(170.0);
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            ui.label(head(format!("CHAPTER {} OF 5", s.chapter()), 14.0));
            let ranks = s.ambition_ranks();
            let mut declared: Vec<(i64, usize)> = (0..5).filter(|&a| ranks[a] > 0).map(|a| (ranks[a], a)).collect();
            declared.sort();
            if declared.is_empty() {
                ui.label(RichText::new("no ambitions declared").small().color(DIM));
            }
            for (r, a) in declared {
                ui.label(RichText::new(format!("{}  {}", ["FIRST", "SECOND", "THIRD"][r as usize - 1], AMBITIONS[a])).small().color(BRASS));
            }
        });
    });

    // ---- log (bottom-left), quiet
    egui::Area::new(egui::Id::new("log")).anchor(egui::Align2::LEFT_BOTTOM, [14.0, -12.0]).show(ctx, |ui| {
        ui.set_max_width(300.0);
        for (i, line) in h.log.iter().rev().take(5).rev().enumerate() {
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
        egui::Area::new(egui::Id::new("over")).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            panel().inner_margin(24.0).show(ui, |ui| {
                let w = s.winner();
                ui.label(head(format!("{} WINS", NAMES[w].to_uppercase()), 30.0).color(ui_color(w)));
                ui.add_space(8.0);
                for p in 0..n {
                    ui.label(RichText::new(format!("{}   {} power", NAMES[p], s.power(p))).color(if p == w { INK } else { DIM }));
                }
                ui.add_space(12.0);
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
    let human = !h.bots[cur];
    egui::Area::new(egui::Id::new("hand")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -14.0]).show(ctx, |ui| {
        ui.set_width((ctx.screen_rect().width() - 560.0).clamp(520.0, 900.0));
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
            ui.label(head(status, 15.0).color(if human { BRASS } else { DIM }));
        });
        ui.add_space(6.0);
        if human && !h.busy {
            let acts = s.legal_actions();
            let show = |ui: &mut egui::Ui, a: Action, out: &mut HudOutput| {
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
                ui.horizontal(|ui| {
                    let now = ctx.input(|i| i.time);
                    let mut seen = Vec::new();
                    for (i, c) in s.hand(cur).into_iter().enumerate() {
                        ui.vertical(|ui| {
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
                    crate::cards::end_hand_frame(&seen);
                });
                ui.horizontal(|ui| {
                    for a in acts.iter().copied().filter(|a| a.card().is_none()) {
                        show(ui, a, &mut out);
                    }
                });
            }
        }
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
        12 => format!("{} moves {} ships {} to {}", nm(e[1]), e[4], sys_name(e[2] as usize), sys_name(e[3] as usize)),
        13 => format!("{} repairs at {}", nm(e[1]), sys_name(e[2] as usize)),
        14 => format!("{} attacks {} at {}", nm(e[1]), nm(e[3]), sys_name(e[2] as usize)),
        20 => format!("Round over, {} holds the initiative", nm(e[1])),
        21 => format!("Chapter {} scored", e[1]),
        _ => return None,
    })
}
