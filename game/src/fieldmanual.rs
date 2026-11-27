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
        "The player holding the initiative leads one action card. That player then takes two actions of the card's suit.",
        &[
            "Four suits: Influence, Aggression, Construction and Mobilization.",
            "The number on the card is its strength when the trick is compared.",
        ],
    ),
    (
        "3 · FOLLOW",
        "Everyone else plays one card in turn order and takes one action. Follow the lead suit if you can, otherwise pivot with any card.",
        &[
            "Surpass or copy to stay in the lead suit; pivot takes an action of the card you played.",
            "The highest lead-suit card wins the trick: +1 power and the initiative.",
        ],
    ),
    (
        "4 · TAKE ACTIONS",
        "Actions come from the suit you played. The status line at the bottom shows what is legal now, and the board filters actions when you select a system.",
        &[
            "Tax, build, move, repair, fight, or spend a resource for an extra action.",
            "Click a system on the board to narrow the action list to that system.",
        ],
    ),
    (
        "5 · SECURE THE COURT",
        "On your action step, spend 1 Relic to secure one face-up Court card. It scores 1 power immediately and stays in your panel.",
        &[
            "Securing does not spend an action pip and the Court row is not refilled.",
            "Click a Court card in the left panel, or open the Ledger to see the whole row.",
        ],
    ),
    (
        "6 · SCORE THE CHAPTER",
        "When the hands are empty the chapter ends. Declared ambitions pay their marker values; the Accord and the Seer add +1, and whoever is furthest behind gains Lore.",
        &[
            "After scoring, the next chapter deals new hands. The Court row is not refilled.",
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
    rule(ui, "ONE TRICK", "The initiative holder leads. They take two actions of the led suit; every other seat follows in turn order and takes one action.");
    rule(ui, "FOLLOWING", "Follow the lead suit when you can. Otherwise pivot with any card and take an action of that card's suit. Surpass and copy keep you in the lead suit.");
    rule(ui, "WINNING THE TRICK", "The highest card of the lead suit takes it: +1 power and the initiative. The cards go to the discard pile.");
    rule(ui, "AFTER SIX TRICKS", "When every hand is empty the chapter scores and a new chapter deals fresh hands. The Court row is not refilled.");
}

fn actions(ui: &mut egui::Ui) {
    let rows = [
        ("Tax", "+1 resource of the system's planet type, where you have a city or control one."),
        ("Build city / starport", "On a non-gate system where you have a piece, while you have pieces left to build."),
        ("Build ships", "At your starport, taking ships from your reserve."),
        ("Move", "Move ships to adjacent systems. A starport lets you move along any path."),
        ("Repair", "Flip one damaged ship or building back to full strength."),
        ("Battle", "Split up to 18 ships into assault, skirmish and raid dice. Hits damage and destroy; a destroyed city yields a trophy and a captive."),
        ("Prelude", "Spend one resource for an extra action, once per action step. Relics are for the Court."),
        ("Secure", "Spend 1 Relic on a Court card: +1 power and its lasting effect, no action pip, no refill."),
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
    ui.label(RichText::new("Damaged ships fight worse until repaired. Trophies and captives feed the Warlord and Tyrant ambitions.").small().color(DIM));
}

fn court(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
        for slot in 0..5 {
            let card = reach_rules::CourtCard(slot as u8);
            ui.vertical(|ui| {
                ui.set_width(86.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(64.0, 72.0), egui::Sense::hover());
                courtart::paint(ui.painter(), r, card, true, 1.0);
                ui.add_space(3.0);
                ui.label(head(card.name().to_uppercase(), 10.0));
                ui.label(RichText::new(reach_rules::COURT_EFFECTS[slot]).small().color(DIM));
            });
        }
    });
    rule(ui, "SECURING", "On your action step, spend 1 Relic to take one face-up Court card. It scores 1 power at once, stays in your panel for the rest of the game, and its effect is yours from then on.");
    rule(ui, "THE ROW", "Five cards are laid out at the start of the game. Securing removes a card and leaves an empty slot; the row is never refilled.");
    rule(ui, "NO PIP", "Securing is separate from your normal actions: it does not spend an action pip.");
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
    rule(ui, "AMBITIONS", "Five scoring cards sit at the right. A seat can declare an ambition; when the chapter ends, the highest tallies for the declared cards receive their marker values.");
    rule(ui, "CITIES", "Cities score no power on their own: they let you Tax their planet's resource, and a destroyed city gives the attacker a trophy and a captive.");
    rule(ui, "FIVE CHAPTERS", "The game runs up to five chapters. If a player reaches the power goal at a chapter end, the game ends there; otherwise the highest power after chapter five wins.");
    rule(ui, "TROPHIES & CAPTIVES", "Trophies are destroyed enemy pieces. Captives are taken when an enemy city is destroyed in battle. They are public and feed Tyrant and Warlord.");
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
