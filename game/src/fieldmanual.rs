//! The Field Manual: a rulebook plus a short guided tutorial, shown over the
//! table. Rules text follows `rules/ox/reach.ox`; it deliberately avoids
//! effects the ruleset does not implement.

use egui::{Align, Color32, Layout, RichText};
use std::cell::Cell;

use crate::courtart;
use crate::ui::{head, panel, BRASS, DIM, HAIR, INK};

const TABS: [&str; 5] = ["TUTORIAL", "TURN", "ACTIONS", "COURT", "SCORING"];

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
    static TAB: Cell<usize> = const { Cell::new(0) };
    static STEP: Cell<usize> = const { Cell::new(0) };
}

const TUTORIAL: [(&str, &str, &[&str]); 6] = [
    (
        "1 · THE GOAL",
        "Power is the score. Reach the table's goal at the end of a chapter and you win; after chapter five the highest power wins.",
        &[
            "The goal is 33 power with two players, 30 with three and 27 with four.",
            "Each chapter deals a fresh hand and scores the declared ambitions.",
        ],
    ),
    (
        "2 · LEAD A TRICK",
        "The player holding the initiative leads one action card, or passes the initiative to the next player. The leader takes actions equal to the card's pips.",
        &[
            "Four suits: Influence, Aggression, Construction and Mobilization.",
            "Leading may declare an ambition: the lowest open marker goes on it and the lead number becomes zero.",
        ],
    ),
    (
        "3 · FOLLOW",
        "Everyone else must play a card in turn order. Surpass with a higher card of the lead suit for its full pips, copy with any card face down for one action of the lead suit, or pivot with another suit for one action of that suit.",
        &[
            "A follower may also seize the initiative by playing a second card face down.",
            "The initiative goes to whoever seized it, else the highest surpass, else stays with the leader.",
        ],
    ),
    (
        "4 · TAKE ACTIONS",
        "Actions come from the suit you played. The status line at the bottom shows what is legal now, and the board filters actions when you select a system.",
        &[
            "Administration: tax, repair, influence. Aggression: battle, move, secure. Construction: build, repair. Mobilization: move, influence.",
            "Click a system on the board to narrow the action list to that system.",
        ],
    ),
    (
        "5 · INFLUENCE AND SECURE THE COURT",
        "Influence places one of your agents on a Court card. Secure takes a card where you have the most agents: rival agents there become your captives and your own return to you.",
        &[
            "The row refills from the Court deck. Secured cards add their icons to ambitions.",
            "The Court row runs along the top edge of the board, with agent stacks at each card's foot.",
        ],
    ),
    (
        "6 · SCORE THE CHAPTER",
        "The chapter ends when hands are empty or every player passes in a row. Each declared ambition pays first and second place; first place also gains +2 with four cities built, or +5 with all five.",
        &[
            "Ties for first take second place value; ties for second score nothing. Markers flip to higher values each chapter.",
            "The board guide (G) labels the ring, Court, played cards, ambitions and power track.",
        ],
    ),
];

pub fn is_open() -> bool {
    OPEN.with(|o| o.get())
}

pub fn close() {
    OPEN.with(|o| o.set(false));
}

pub fn toggle() {
    OPEN.with(|o| o.set(!o.get()));
}

pub fn open_tutorial() {
    OPEN.with(|o| o.set(true));
    TAB.with(|t| t.set(0));
    STEP.with(|s| s.set(0));
}

fn rule(ui: &mut egui::Ui, title: &str, body: &str) {
    ui.add_space(6.0);
    ui.label(head(title, 11.0).color(BRASS));
    ui.label(RichText::new(body).color(INK));
}

fn bullets(ui: &mut egui::Ui, lines: &[&str]) {
    for line in lines {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let (dot, _) = ui.allocate_exact_size(egui::vec2(4.0, 4.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 1.6, BRASS);
            ui.add(egui::Label::new(RichText::new(*line).color(DIM)).wrap());
        });
    }
}

fn tutorial(ui: &mut egui::Ui, goal: i64) {
    let mut step = STEP.with(|s| s.get()).min(TUTORIAL.len() - 1);
    let (title, body, notes) = TUTORIAL[step];
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(head(format!("STEP {} OF {}", step + 1, TUTORIAL.len()), 9.0).color(DIM));
        ui.add_space(8.0);
        for k in 0..TUTORIAL.len() {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(14.0, 3.0), egui::Sense::hover());
            ui.painter().rect_filled(dot, 0.0, if k <= step { BRASS } else { HAIR });
        }
    });
    ui.add_space(10.0);
    ui.label(head(title, 22.0));
    ui.add_space(4.0);
    let body = body.replace("the table's goal", &format!("the table's {goal}-power goal"));
    ui.add(egui::Label::new(RichText::new(body).size(14.5)).wrap());
    ui.add_space(8.0);
    bullets(ui, notes);
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        let back = ui.add_enabled(step > 0, egui::Button::new("Back"));
        if back.clicked() {
            step -= 1;
            STEP.with(|s| s.set(step));
        }
        let next = ui.add_sized([110.0, 26.0], egui::Button::new(head(if step + 1 == TUTORIAL.len() { "DONE" } else { "NEXT" }, 11.5)));
        if next.clicked() {
            if step + 1 == TUTORIAL.len() {
                close();
            } else {
                step += 1;
                STEP.with(|s| s.set(step));
            }
        }
        ui.label(RichText::new("The manual stays available with R.").small().color(DIM));
    });
}

fn turn(ui: &mut egui::Ui) {
    rule(ui, "ONE TRICK", "The initiative holder leads a card (actions equal to its pips) or passes the initiative. Every other seat must follow in turn order.");
    rule(ui, "FOLLOWING", "Surpass: higher card of the lead suit, full pips. Copy: any card face down, one action of the lead suit. Pivot: other suit, one action of that suit. Followers cannot pass.");
    rule(ui, "WINNING THE TRICK", "Initiative goes to a seizing player, else the highest surpass, else stays with the leader. The cards go to the discard pile.");
    rule(ui, "AFTER SIX TRICKS", "When every hand is empty, or all players pass in a row, the chapter scores and a new chapter deals fresh hands.");
}

fn actions(ui: &mut egui::Ui) {
    let rows = [
        ("Tax", "+1 resource of the system's planet type, where you have a city or control one."),
        ("Build city / starport", "On a non-gate system where you have a piece, while you have pieces left to build."),
        ("Build ships", "At your starport, taking ships from your reserve."),
        ("Move", "Move ships to adjacent systems. A starport lets you move along any path."),
        ("Repair", "Flip one damaged ship or building back to full strength."),
        ("Battle", "Split up to 18 ships into assault, skirmish and raid dice. Hits damage and destroy; destroyed pieces become trophies."),
        ("Prelude", "Spend a resource for an action of its kind: Material builds, Fuel moves, Weapons battle, Relics secure, Psionic takes a lead-suit action."),
        ("Influence", "Place one agent on a Court card."),
        ("Secure", "Take a Court card where you have the most agents. Rival agents there become captives; the row refills."),
        ("Lore", "Once per turn on your action step, spend a Lore token for one more action of the current suit."),
    ];
    for (name, body) in rows {
        ui.add_space(5.0);
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(150.0);
            ui.label(head(name, 11.5));
            ui.label(RichText::new(body).color(DIM));
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Damaged ships fight worse until repaired. Trophies feed Warlord and captives feed Tyrant. Taxing a rival's city captures one of their agents.").small().color(DIM));
}

fn court(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
        for slot in 0..10 {
            let card = reach_rules::CourtCard(slot as u8);
            ui.vertical(|ui| {
                ui.set_width(86.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(64.0, 72.0), egui::Sense::hover());
                courtart::paint(ui.painter(), r, card, true, 1.0);
                ui.add_space(3.0);
                ui.label(head(card.name().to_uppercase(), 10.0));
                ui.label(RichText::new(reach_rules::COURT_EFFECTS[slot % 5]).small().color(DIM));
            });
        }
    });
    rule(ui, "SECURING", "Influence puts your agents on a Court card. Secure takes it if you have more agents there than any rival. Rival agents become your captives, your own agents come home, and the effect is yours from then on.");
    rule(ui, "THE ROW", "Four cards (three with two players) sit along the top edge of the board. A secured card is replaced from the Court deck.");
    rule(ui, "NO PIP", "Each player has ten agents. Agents in the Court or held captive are not in your supply.");
    ui.add_space(8.0);
    ui.label(head("LEADERS", 11.5));
    ui.label(RichText::new("Each player is dealt one leader at setup. It adds one starting resource and bends one rule.").small().color(DIM));
    for (name, body) in reach_rules::LEADERS.iter().zip(reach_rules::LEADER_EFFECTS) {
        ui.horizontal_wrapped(|ui| {
            ui.label(head(name.to_uppercase(), 10.0));
            ui.label(RichText::new(body).small().color(DIM));
        });
    }
    rule(ui, "LORE", "After each chapter is scored, the player with the least power gains a Lore token (at most 3; nobody gains one when all are level). Once per turn on your action step, spend one for an extra action of the current suit.");
}

fn scoring(ui: &mut egui::Ui) {
    rule(ui, "AMBITIONS", "A leader can declare an ambition by placing a marker. At chapter end each ambition pays the sum of its markers to first and second place. Ties for first score second place; ties for second score nothing.");
    rule(ui, "CITIES", "Cities let you Tax their planet's resource. First place on an ambition gains +2 with four cities built, or +5 with all five.");
    rule(ui, "FIVE CHAPTERS", "The game runs up to five chapters. If a player reaches the power goal at a chapter end, the game ends there; otherwise the highest power after chapter five wins.");
    rule(ui, "TROPHIES & CAPTIVES", "Trophies are destroyed enemy pieces. Captives are rival agents taken by Secure or by taxing a rival city. Both return to their owners once scored.");
}

/// Draws the manual if it is open. The caller passes the table's power goal.
pub fn overlay(ctx: &egui::Context, goal: i64) {
    if !is_open() {
        return;
    }
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("manual_veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(4, 6, 10, 238));
            if resp.clicked() {
                close();
            }
        });
    egui::Area::new(egui::Id::new("manual"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let width = (screen.width() - 48.0).clamp(330.0, 760.0);
            let resp = panel().inner_margin(18.0).show(ui, |ui| {
                ui.set_width(width);
                let mut tab = TAB.with(|t| t.get()).min(TABS.len() - 1);
                ui.horizontal(|ui| {
                    ui.label(head("FIELD MANUAL", 22.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close  [Esc]").clicked() {
                            close();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, HAIR));
                ui.add_space(8.0);
                // A stable body height keeps the section bar under the cursor
                // when the reader switches between short and long sections.
                let body_h = (screen.height() - 500.0).clamp(300.0, 420.0);
                ui.allocate_ui_with_layout(egui::vec2(width, body_h), Layout::top_down(Align::Min), |ui| {
                    ui.set_min_size(egui::vec2(width, body_h));
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(body_h)
                        .show(ui, |ui| match tab {
                            0 => tutorial(ui, goal),
                            1 => turn(ui),
                            2 => actions(ui),
                            3 => court(ui),
                            _ => scoring(ui),
                        });
                });
                ui.add_space(6.0);
                ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), egui::Stroke::new(1.0, HAIR));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (k, name) in TABS.iter().enumerate() {
                        let selected = tab == k;
                        let b = egui::Button::new(head(*name, 9.0).color(if selected { Color32::from_rgb(20, 16, 10) } else { DIM }))
                            .min_size(egui::vec2(54.0, 20.0))
                            .fill(if selected { BRASS } else { Color32::from_rgb(26, 30, 40) });
                        if ui.push_id(("manual_tab", k), |ui| ui.add(b)).inner.clicked() {
                            tab = k;
                            TAB.with(|t| t.set(k));
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("G board guide   ·   R manual   ·   L ledger   ·   C cinematic camera").small().color(DIM));
            });
            crate::probe::panel("manual", 4, resp.response.rect);
        });
}
