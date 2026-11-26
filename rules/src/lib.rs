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

const P_BASE: usize = 40;
const P_STRIDE: usize = 20;
const SHIPS_BASE: usize = 120;
const SLOTS_BASE: usize = 312;
const SUPPLY_BASE: usize = 456;

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
    pub fn extras(&self) -> [i64; 4] {
        [self.h(14), self.h(15), self.h(16), self.h(17)]
    }
    pub fn cluster_out(&self, c: usize) -> bool {
        (self.h(20) >> c) & 1 == 1
    }
    pub fn in_play(&self, sys: usize) -> bool {
        !self.cluster_out(sys / 4)
    }
    /// Declaration rank (1..3) of each ambition, 0 if not declared.
    pub fn ambition_ranks(&self) -> [i64; 5] {
        [self.h(22), self.h(23), self.h(24), self.h(25), self.h(26)]
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
    pub fn hand(&self, p: usize) -> Vec<Card> {
        let m = self.pf(p, 11);
        (0..28u8).filter(|c| (m >> c) & 1 == 1).map(Card).collect()
    }
    pub fn last_card(&self, p: usize) -> Option<Card> {
        let c = self.pf(p, 14);
        (c >= 0).then_some(Card(c as u8))
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
}
