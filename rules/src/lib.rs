//! Ember Reach rules.
//!
//! The rules themselves live in `ox/reach.ox` (Oxidized) and are transpiled to
//! Rust by `build.rs`. This crate is the typed face of that code: `State` is
//! the flat integer list the rules operate on, `Action` is a four-integer move.
//! Both are plain data (serde) so they can be saved, hashed and sent over a
//! network; the same code path serves hot-seat, bots and LAN/web play.

use serde::{Deserialize, Serialize};

#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_parens,
    unused_assignments,
    non_snake_case,
    non_camel_case_types,
    clippy::all
)]
mod ox {
    include!(concat!(env!("OUT_DIR"), "/reach_gen.rs"));
}

use ox::DynamicValue as Dv;

pub const SYSTEMS: usize = 24;
pub const CLUSTERS: usize = 6;
pub const MAX_PLAYERS: usize = 4;

fn to_dv(v: &[i64]) -> Dv {
    Dv::list(v.iter().map(|x| Dv::Int(*x)).collect())
}

fn from_dv(d: Dv) -> Vec<i64> {
    match d {
        Dv::List(l) => l.into_vec().into_iter().map(|x| x.as_i64()).collect(),
        other => panic!("rules returned {other:?}, expected a list"),
    }
}

pub const RESOURCES: [&str; 5] = ["Material", "Fuel", "Weapons", "Relics", "Psionic"];
pub const SUITS: [&str; 4] = ["Administration", "Aggression", "Construction", "Mobilization"];
pub const AMBITIONS: [&str; 5] = ["Tycoon", "Tyrant", "Warlord", "Keeper", "Empath"];
/// The ten Court cards (our own designs). Card `id`'s effect is `COURT_EFFECTS[id]`.
pub const COURT_CARDS: [&str; 10] = ["Envoy", "Seal", "Writ", "Pact", "Accord", "Herald", "Warden", "Charter", "Oath", "Concord"];
pub const COURT_NAMES: [&str; 10] = COURT_CARDS;
/// The resource icon each Court card adds to ambitions (index into `RESOURCES`).
pub const COURT_ICONS: [usize; 10] = [0, 2, 3, 4, 1, 1, 3, 0, 2, 4];
/// Agents each player owns.
pub const AGENTS: i64 = 10;
/// What each Court card does for its holder.
pub const COURT_EFFECTS: [&str; 10] = [
    "Tax takes one more of that resource",
    "The first building hit in each battle against you is ignored",
    "Repair fixes every damaged piece of yours in the system",
    "The first self hit in each of your attacks is ignored",
    "+1 power at each chapter end",
    "+1 power each time you secure a Court card",
    "Rivals who tax your cities capture none of your agents",
    "Building a city also takes one of its planet's resource",
    "Influence places one more agent",
    "Gain a Lore at each chapter end",
];
pub const LEADERS: [&str; 6] = ["Broker", "Admiral", "Architect", "Seer", "Pilgrim", "Corsair"];
/// Each leader's rule bend, in `LEADERS` order.
pub const LEADER_EFFECTS: [&str; 6] = [
    "Build ships places one more ship",
    "One more ship hit when attacking with any hit",
    "Your builds are never damaged",
    "+1 power at chapter end while holding Psionic",
    "Influence places one more agent",
    "Each raid key takes two resources",
];
/// Lore tokens a player can hold.
pub const LORE_CAP: i64 = 3;

const P_BASE: usize = 40;
const P_STRIDE: usize = 20;
const SHIPS_BASE: usize = 120;
const SLOTS_BASE: usize = 312;
const SUPPLY_BASE: usize = 456;
const H_PASSES: usize = 461;
const H_DECK: usize = 462;
const H_FLIPS: usize = 463;
const H_X_SECURE: usize = 465;
const H_AMB_FIRST: usize = 466;
const H_AMB_SECOND: usize = 471;
const AGENTS_BASE: usize = 476;
const CAP_BASE: usize = 492;

/// A move: `[kind, a, b, c]`. See `ox/reach.ox` for the meaning of each kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action(pub [i64; 4]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Lead,
    Surpass,
    Copy,
    Pivot,
    Pass,
    Seize,
    Prelude,
    EndTurn,
    Tax,
    BuildCity,
    BuildPort,
    BuildShips,
    Move,
    Repair,
    Battle,
    Secure,
    Lore,
    Influence,
    Mulligan,
}

impl Action {
    pub fn kind(self) -> Kind {
        match self.0[0] {
            0 => Kind::Lead,
            1 => Kind::Surpass,
            2 => Kind::Copy,
            3 => Kind::Pivot,
            4 => Kind::Pass,
            5 => Kind::Seize,
            6 => Kind::Prelude,
            7 => Kind::EndTurn,
            8 => Kind::Tax,
            9 => Kind::BuildCity,
            10 => Kind::BuildPort,
            11 => Kind::BuildShips,
            12 => Kind::Move,
            13 => Kind::Repair,
            14 => Kind::Battle,
            15 => Kind::Secure,
            16 => Kind::Lore,
            17 => Kind::Influence,
            18 => Kind::Mulligan,
            _ => Kind::Battle,
        }
    }
    pub fn encode(self) -> [i64; 4] {
        self.0
    }
    pub fn decode(a: &[i64]) -> Option<Action> {
        (a.len() == 4).then(|| Action([a[0], a[1], a[2], a[3]]))
    }
    /// The card this action plays, if it plays one.
    pub fn card(self) -> Option<Card> {
        matches!(self.kind(), Kind::Lead | Kind::Surpass | Kind::Copy | Kind::Pivot | Kind::Seize)
            .then(|| Card(self.0[1] as u8))
    }
    /// For Move: (from, to, ships). For Battle: (system, defender).
    pub fn systems(self) -> (usize, usize) {
        match self.kind() {
            Kind::Move => (self.0[1] as usize, self.0[2] as usize),
            Kind::Battle => (self.0[1] as usize, self.0[2] as usize),
            _ => (self.0[1].max(0) as usize, 0),
        }
    }
}

/// Card ids 0..28: suit = id / 7, number = id % 7 + 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card(pub u8);

impl Card {
    pub fn suit(self) -> usize {
        (self.0 / 7) as usize
    }
    pub fn number(self) -> u8 {
        self.0 % 7 + 1
    }
    pub fn name(self) -> String {
        format!("{} {}", SUITS[self.suit()], self.number())
    }
}

/// Court card ids 0..10 (see `COURT_CARDS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CourtCard(pub u8);

impl CourtCard {
    pub fn name(self) -> &'static str {
        COURT_CARDS[self.0 as usize]
    }
    pub fn effect(self) -> &'static str {
        COURT_EFFECTS[self.0 as usize]
    }
    pub fn icon(self) -> usize {
        COURT_ICONS[self.0 as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Lead,
    Follow,
    Act,
    Over,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    City { owner: usize, damaged: bool },
    Starport { owner: usize, damaged: bool },
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct State(pub Vec<i64>);

impl State {
    /// `setup` picks one of the setup cards for this player count.
    pub fn new(players: usize, seed: u64, setup: usize) -> State {
        let v = ox::new_game(Dv::Int(players as i64), Dv::Int((seed % 2_147_483_647) as i64), Dv::Int(setup as i64));
        State(from_dv(v))
    }
    pub fn setup_count(players: usize) -> usize {
        ox::setup_count(Dv::Int(players as i64)).as_i64() as usize
    }
    fn call(&self, f: fn(Dv) -> Dv) -> Dv {
        f(to_dv(&self.0))
    }
    fn h(&self, i: usize) -> i64 {
        self.0[i]
    }
    pub fn players(&self) -> usize {
        self.h(0) as usize
    }
    pub fn chapter(&self) -> i64 {
        self.h(1)
    }
    pub fn phase(&self) -> Phase {
        match self.h(2) {
            0 => Phase::Lead,
            1 => Phase::Follow,
            2 => Phase::Act,
            _ => Phase::Over,
        }
    }
    pub fn is_over(&self) -> bool {
        self.h(2) == 4
    }
    pub fn current(&self) -> usize {
        self.h(3) as usize
    }
    pub fn initiative(&self) -> usize {
        self.h(4) as usize
    }
    pub fn lead_suit(&self) -> Option<usize> {
        (self.h(6) >= 0).then_some(self.h(6) as usize)
    }
    /// Suit of the card whose actions are being taken, and actions left.
    pub fn action_suit(&self) -> usize {
        self.h(12).max(0) as usize
    }
    pub fn actions_left(&self) -> i64 {
        self.h(11)
    }
    /// Prelude extras: [build/repair, move, weapon flag, lead-suit, secure].
    pub fn extras(&self) -> [i64; 5] {
        [self.h(14), self.h(15), self.h(16), self.h(17), self.h(H_X_SECURE)]
    }
    /// The lead card's number for Surpassing (0 once an ambition was declared).
    pub fn lead_number(&self) -> i64 {
        self.h(28)
    }
    /// Who has surpassed highest so far this round, if anyone.
    pub fn best_surpass(&self) -> Option<usize> {
        (self.h(8) >= 0).then_some(self.h(8) as usize)
    }
    /// Who seized the initiative this round, if anyone.
    pub fn seized(&self) -> Option<usize> {
        (self.h(9) >= 0).then_some(self.h(9) as usize)
    }
    pub fn passes(&self) -> i64 {
        self.h(H_PASSES)
    }
    pub fn cluster_out(&self, c: usize) -> bool {
        (self.h(20) >> c) & 1 == 1
    }
    pub fn in_play(&self, sys: usize) -> bool {
        !self.cluster_out(sys / 4)
    }
    /// Markers on each ambition this chapter (0 = not declared).
    pub fn ambition_ranks(&self) -> [i64; 5] {
        [self.h(22), self.h(23), self.h(24), self.h(25), self.h(26)]
    }
    /// [first, second] place power each ambition would pay now (all its markers).
    pub fn ambition_values(&self, a: usize) -> [i64; 2] {
        [self.h(H_AMB_FIRST + a), self.h(H_AMB_SECOND + a)]
    }
    /// Ambition markers placed this chapter (0..3).
    pub fn markers_placed(&self) -> i64 {
        self.h(27)
    }
    /// Ambition markers flipped to their high side so far.
    pub fn marker_flips(&self) -> i64 {
        self.h(H_FLIPS)
    }
    fn pf(&self, p: usize, off: usize) -> i64 {
        self.0[P_BASE + P_STRIDE * p + off]
    }
    pub fn power(&self, p: usize) -> i64 {
        self.pf(p, 0)
    }
    pub fn resource(&self, p: usize, t: usize) -> i64 {
        self.pf(p, 1 + t)
    }
    pub fn cities_left(&self, p: usize) -> i64 {
        self.pf(p, 6)
    }
    pub fn ports_left(&self, p: usize) -> i64 {
        self.pf(p, 7)
    }
    pub fn reserve(&self, p: usize) -> i64 {
        self.pf(p, 8)
    }
    pub fn trophies(&self, p: usize) -> i64 {
        self.pf(p, 9)
    }
    pub fn captives(&self, p: usize) -> i64 {
        self.pf(p, 10)
    }
    /// First and second place values of a marker on its low side (0 = the 5/3 marker).
    pub fn marker_values(rank: usize) -> [i64; 2] {
        let v = from_dv(ox::marker_values(Dv::Int(rank as i64)));
        [v[0], v[1]]
    }
    /// Cards still in the Court deck.
    pub fn court_deck(&self) -> usize {
        (self.h(H_DECK) as u64).count_ones() as usize
    }
    /// Face-up Court slots this player count uses: 3 with two players, else 4.
    pub fn court_slots(&self) -> usize {
        if self.players() == 2 { 3 } else { 4 }
    }
    /// Agents `p` has on Court slot `i`.
    pub fn agents_on(&self, slot: usize, p: usize) -> i64 {
        self.0[AGENTS_BASE + slot * 4 + p]
    }
    /// Captives `holder` keeps of `owner`'s agents.
    pub fn captives_of(&self, holder: usize, owner: usize) -> i64 {
        self.0[CAP_BASE + holder * 4 + owner]
    }
    /// Agents in `p`'s supply.
    pub fn agents_left(&self, p: usize) -> i64 {
        let court: i64 = (0..4).map(|i| self.agents_on(i, p)).sum();
        let held: i64 = (0..self.players()).map(|h| self.captives_of(h, p)).sum();
        AGENTS - court - held
    }
    /// Court icons of resource `t` that `p` holds.
    pub fn court_icons(&self, p: usize, t: usize) -> i64 {
        self.secured(p).iter().filter(|c| c.icon() == t).count() as i64
    }
    /// Power that ends the game after a chapter: 33 for 2 players, 30 for 3, 27 for 4.
    pub fn power_goal(players: usize) -> i64 {
        ox::power_goal(Dv::Int(players as i64)).as_i64()
    }
    /// A player's tally for ambition `a` (0 Tycoon .. 4 Empath), scored as the rules do.
    /// Read straight from the fields (the UI asks every frame); the test below
    /// keeps these formulas equal to `amb_tally` in the rules.
    pub fn ambition_tally(&self, p: usize, a: usize) -> i64 {
        match a {
            0 => self.resource(p, 0) + self.resource(p, 1) + self.court_icons(p, 0) + self.court_icons(p, 1),
            1 => self.captives(p),
            2 => self.trophies(p),
            3 => self.resource(p, 3) + self.court_icons(p, 3),
            _ => self.resource(p, 4) + self.court_icons(p, 4),
        }
    }
    pub fn hand(&self, p: usize) -> Vec<Card> {
        let m = self.pf(p, 11);
        (0..28u8).filter(|c| (m >> c) & 1 == 1).map(Card).collect()
    }
    /// How `p` played this round: 0 lead, 1 surpass, 2 copy, 3 pivot.
    pub fn play_mode(&self, p: usize) -> Option<usize> {
        let m = self.pf(p, 18);
        (m >= 0).then_some(m as usize)
    }
    /// The extra card `p` laid face down to seize the initiative this round.
    pub fn seize_card(&self, p: usize) -> Option<Card> {
        let c = self.pf(p, 19);
        (c >= 0).then_some(Card(c as u8))
    }
    /// Cards on the table this round in play order from the lead:
    /// (player, card, mode, seize card).
    pub fn table(&self) -> Vec<(usize, Card, usize, Option<Card>)> {
        let n = self.players();
        (0..n)
            .map(|k| (self.initiative() + k) % n)
            .filter_map(|p| Some((p, self.last_card(p)?, self.play_mode(p)?, self.seize_card(p))))
            .collect()
    }
    pub fn last_card(&self, p: usize) -> Option<Card> {
        let c = self.pf(p, 14);
        (c >= 0).then_some(Card(c as u8))
    }
    /// The face-up Court row (`court_slots()` long), `None` for an empty slot.
    pub fn court_row(&self) -> Vec<Option<CourtCard>> {
        (0..self.court_slots()).map(|i| self.court_card(i)).collect()
    }
    pub fn court_card(&self, slot: usize) -> Option<CourtCard> {
        if slot >= self.court_slots() {
            return None;
        }
        let c = self.h(35 + slot);
        (c >= 0).then_some(CourtCard(c as u8))
    }
    pub fn court_available(&self) -> usize {
        self.court_row().iter().flatten().count()
    }
    /// Court cards a player has secured; the mask is public, like trophies.
    pub fn secured(&self, p: usize) -> Vec<CourtCard> {
        let mask = self.pf(p, 15);
        (0..COURT_CARDS.len() as u8)
            .filter(|&c| (mask >> c) & 1 == 1)
            .map(CourtCard)
            .collect()
    }
    /// The leader dealt to `p` at setup (index into `LEADERS`).
    pub fn leader(&self, p: usize) -> usize {
        self.pf(p, 16).clamp(0, LEADERS.len() as i64 - 1) as usize
    }
    /// Lore tokens `p` holds; one buys an extra action once per turn.
    pub fn lore(&self, p: usize) -> i64 {
        self.pf(p, 17)
    }
    pub fn has_secured(&self, p: usize, c: CourtCard) -> bool {
        (self.pf(p, 15) >> c.0) & 1 == 1
    }
    pub fn supply(&self, t: usize) -> i64 {
        self.0[SUPPLY_BASE + t]
    }
    /// (fresh, damaged) ships of `p` in `sys`.
    pub fn ships(&self, sys: usize, p: usize) -> (i64, i64) {
        let i = SHIPS_BASE + sys * 8 + p * 2;
        (self.0[i], self.0[i + 1])
    }
    pub fn ship_count(&self, sys: usize, p: usize) -> i64 {
        let (a, b) = self.ships(sys, p);
        a + b
    }
    pub fn slot(&self, sys: usize, i: usize) -> Slot {
        let b = SLOTS_BASE + sys * 6 + i * 3;
        let (o, k, d) = (self.0[b], self.0[b + 1], self.0[b + 2] != 0);
        match k {
            1 => Slot::City { owner: o as usize, damaged: d },
            2 => Slot::Starport { owner: o as usize, damaged: d },
            _ => Slot::Empty,
        }
    }
    pub fn controller(&self, sys: usize) -> Option<usize> {
        let c = ox::controller(to_dv(&self.0), Dv::Int(sys as i64)).as_i64();
        (c >= 0).then_some(c as usize)
    }
    pub fn adjacent(&self, a: usize, b: usize) -> bool {
        ox::adjacent(to_dv(&self.0), Dv::Int(a as i64), Dv::Int(b as i64)).as_bool()
    }
    pub fn neighbors(&self, a: usize) -> Vec<usize> {
        from_dv(ox::neighbors(to_dv(&self.0), Dv::Int(a as i64))).into_iter().map(|x| x as usize).collect()
    }
    pub fn planet_type(sys: usize) -> Option<usize> {
        let t = ox::planet_type(Dv::Int(sys as i64)).as_i64();
        (t >= 0).then_some(t as usize)
    }
    /// Action pips on a card (suit 0..4, number 1..=7).
    pub fn card_pips(suit: usize, number: u8) -> i64 {
        ox::card_pips(Dv::Int(suit as i64), Dv::Int(number as i64)).as_i64()
    }
    pub fn planet_slots(sys: usize) -> usize {
        ox::planet_slots(Dv::Int(sys as i64)).as_i64() as usize
    }
    /// Last event as `[kind, a, b, c, d]` (see `set_event` in the rules).
    pub fn event(&self) -> [i64; 5] {
        [self.h(30), self.h(31), self.h(32), self.h(33), self.h(34)]
    }

    pub fn legal_actions(&self) -> Vec<Action> {
        match self.call(ox::legal_actions) {
            Dv::List(l) => l.into_vec().into_iter().filter_map(|a| Action::decode(&from_dv(a))).collect(),
            _ => Vec::new(),
        }
    }
    pub fn is_legal(&self, a: Action) -> bool {
        self.legal_actions().contains(&a)
    }
    /// The next state after `a`. Panics if the action is not legal; check
    /// `is_legal` first for untrusted input.
    pub fn apply(&self, a: Action) -> State {
        assert!(self.is_legal(a), "illegal action {a:?}");
        State(from_dv(ox::apply(to_dv(&self.0), to_dv(&a.encode()))))
    }
    pub fn bot_choose(&self) -> Action {
        Action::decode(&from_dv(self.call(ox::bot_choose))).expect("bot returned a valid action")
    }
    pub fn winner(&self) -> usize {
        self.call(ox::winner).as_i64() as usize
    }
}

/// Network/hot-seat validation helper: `Ok` if `player` may take `a` now.
pub fn validate(state: &State, player: usize, a: Action) -> Result<(), String> {
    if state.is_over() {
        return Err("the game is over".into());
    }
    if state.current() != player {
        return Err("not your turn".into());
    }
    if !state.is_legal(a) {
        return Err("illegal action".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_matches_rules() {
        let s = State::new(3, 42, 0);
        assert_eq!(s.players(), 3);
        assert_eq!(s.phase(), Phase::Lead);
        for p in 0..3 {
            assert_eq!(s.hand(p).len(), 6);
            let total: i64 = s.reserve(p) + (0..SYSTEMS).map(|x| s.ship_count(x, p)).sum::<i64>();
            assert_eq!(total, 15);
        }
    }

    #[test]
    fn score_values_match_the_rules() {
        assert_eq!(State::power_goal(2), 33);
        assert_eq!(State::power_goal(3), 30);
        assert_eq!(State::power_goal(4), 27);
        assert_eq!(State::marker_values(0), [5, 3]);
        assert_eq!(State::marker_values(2), [2, 0]);
        let s = State::new(3, 42, 0);
        for p in 0..3 {
            for a in 0..5 {
                let rules = ox::amb_tally(to_dv(&s.0), Dv::Int(p as i64), Dv::Int(a as i64)).as_i64();
                assert_eq!(s.ambition_tally(p, a), rules, "player {p}, ambition {a}");
            }
        }
    }

    #[test]
    fn same_seed_same_game() {
        assert_eq!(State::new(4, 9, 1), State::new(4, 9, 1));
        assert_ne!(State::new(4, 9, 1), State::new(4, 10, 1));
    }

    #[test]
    fn map_is_symmetric() {
        let s = State::new(4, 1, 0);
        for a in 0..SYSTEMS {
            for b in s.neighbors(a) {
                assert!(s.adjacent(b, a));
            }
        }
    }

    #[test]
    fn bots_finish_games() {
        for n in 2..=4 {
            let mut s = State::new(n, 77, 0);
            let mut steps = 0;
            while !s.is_over() {
                s = s.apply(s.bot_choose());
                steps += 1;
                assert!(steps < 6000);
            }
            assert!(s.winner() < n);
        }
    }

    #[test]
    fn validate_rejects_wrong_player() {
        let s = State::new(2, 1, 0);
        let a = s.legal_actions()[0];
        assert!(validate(&s, s.current(), a).is_ok());
        assert!(validate(&s, 1 - s.current(), a).is_err());
        assert!(validate(&s, s.current(), Action([99, 0, 0, 0])).is_err());
    }

    #[test]
    fn court_row_influence_and_secure_match_the_rules() {
        let mut s = State::new(3, 11, 0);
        assert_eq!(s.court_row().len(), 4);
        assert!(s.court_row().iter().all(Option::is_some));
        assert_eq!(s.court_deck(), 6);
        assert!(s.secured(0).is_empty());
        assert_eq!(State::new(2, 11, 0).court_row().len(), 3);
        // Player 0 acts with two Mobilization pips: Influence twice.
        s.0[2] = 2;
        s.0[3] = 0;
        s.0[11] = 2;
        s.0[12] = 3;
        s.0[P_BASE + 16] = 0;
        let card = s.court_card(1).unwrap();
        s = s.apply(Action([17, 1, 0, 0]));
        assert_eq!(s.agents_on(1, 0), 1);
        assert_eq!(s.agents_left(0), AGENTS - 1);
        assert!(!s.legal_actions().contains(&Action([15, 1, 0, 0])), "Mobilization cannot secure");
        // Switch to an Aggression pip: secure.
        s.0[2] = 2;
        s.0[3] = 0;
        s.0[11] = 1;
        s.0[12] = 1;
        assert!(s.legal_actions().contains(&Action([15, 1, 0, 0])));
        let t = s.apply(Action([15, 1, 0, 0]));
        assert!(t.has_secured(0, card));
        assert_ne!(t.court_card(1), Some(card));
        assert_eq!(t.agents_left(0), AGENTS);
        assert_eq!(t.court_deck(), 5);
    }

    #[test]
    fn leaders_and_lore_are_readable() {
        let s = State::new(4, 5, 0);
        let mut seen: Vec<usize> = (0..4).map(|p| s.leader(p)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 4);
        assert!((0..4).all(|p| s.lore(p) == 0));
        let mut t = s.clone();
        t.0[2] = 2;
        t.0[3] = 1;
        t.0[11] = 1;
        t.0[12] = 0;
        t.0[P_BASE + P_STRIDE + 17] = 1;
        let a = Action([16, 0, 0, 0]);
        assert_eq!(a.kind(), Kind::Lore);
        assert!(t.legal_actions().contains(&a));
        let u = t.apply(a);
        assert_eq!(u.lore(1), 0);
        assert_eq!(u.actions_left(), 2);
    }

    #[test]
    fn bot_games_produce_spoils() {
        let mut captives = 0;
        let mut trophies = 0;
        let mut games_with_captives = 0;
        for n in 2..=4 {
            for seed in 1..=6 {
                let mut s = State::new(n, seed, seed as usize);
                while !s.is_over() {
                    s = s.apply(s.bot_choose());
                }
                let c: i64 = (0..n).map(|p| s.captives(p)).sum();
                let t: i64 = (0..n).map(|p| s.trophies(p)).sum();
                for p in 0..n {
                    assert_eq!(
                        s.ambition_tally(p, 1),
                        s.captives(p),
                        "Tyrant must count the captives actually taken"
                    );
                }
                captives += c;
                trophies += t;
                if c > 0 {
                    games_with_captives += 1;
                }
            }
        }
        assert!(trophies > 0, "bot games fought no battles");
        assert!(
            captives > 0,
            "no bot game destroyed a city ({games_with_captives} of 18)"
        );
    }

}

#[cfg(test)]
mod table_probe {
    use super::*;
    #[test]
    fn table_lists_followers() {
        let mut s = State::new(4, 4, 0);
        let mut most = 0;
        while !s.is_over() {
            s = s.apply(s.bot_choose());
            most = most.max(s.table().len());
        }
        assert!(most >= 3, "most {most}");
    }
}
