//! Ember Reach rules.
//!
//! The rules themselves live in `ox/reach.ox` (Oxidized) and are transpiled to
//! Rust by `build.rs`. This crate is the typed face of that code: `State` is
//! the flat integer list the rules operate on, `Action` is a decoded move.
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

pub const SYSTEMS: usize = 7;
pub const MAX_PLAYERS: usize = 4;
pub const ROUNDS: i64 = 3;

fn to_dv(v: &[i64]) -> Dv {
    Dv::list(v.iter().map(|x| Dv::Int(*x)).collect())
}

fn from_dv(d: Dv) -> Vec<i64> {
    match d {
        Dv::List(l) => l.into_vec().into_iter().map(|x| x.as_i64()).collect(),
        other => panic!("rules returned {other:?}, expected a list"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Move,
    Fight,
    Build,
    Influence,
}

impl Suit {
    pub fn from_index(i: i64) -> Suit {
        match i {
            0 => Suit::Move,
            1 => Suit::Fight,
            2 => Suit::Build,
            _ => Suit::Influence,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Suit::Move => "Move",
            Suit::Fight => "Fight",
            Suit::Build => "Build",
            Suit::Influence => "Influence",
        }
    }
}

/// An action card: suit and rank 1..=7 (card id = suit * 7 + rank - 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card(pub u8);

impl Card {
    pub fn new(suit: Suit, rank: u8) -> Card {
        Card(suit as u8 * 7 + rank - 1)
    }
    pub fn suit(self) -> Suit {
        Suit::from_index((self.0 / 7) as i64)
    }
    pub fn rank(self) -> u8 {
        self.0 % 7 + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Play(Card),
    Move { from: u8, to: u8 },
    Fight { system: u8, enemy: u8 },
    Build { system: u8 },
    Levy { system: u8 },
    Sway,
    Tax,
    EndTurn,
}

impl Action {
    pub fn encode(self) -> [i64; 3] {
        match self {
            Action::Play(c) => [0, c.0 as i64, 0],
            Action::Move { from, to } => [1, from as i64, to as i64],
            Action::Fight { system, enemy } => [2, system as i64, enemy as i64],
            Action::Build { system } => [3, system as i64, 0],
            Action::Levy { system } => [4, system as i64, 0],
            Action::Sway => [5, 0, 0],
            Action::EndTurn => [6, 0, 0],
            Action::Tax => [7, 0, 0],
        }
    }

    pub fn decode(a: &[i64]) -> Option<Action> {
        let (k, x, y) = (*a.first()?, *a.get(1)?, *a.get(2)?);
        Some(match k {
            0 => Action::Play(Card(x as u8)),
            1 => Action::Move { from: x as u8, to: y as u8 },
            2 => Action::Fight { system: x as u8, enemy: y as u8 },
            3 => Action::Build { system: x as u8 },
            4 => Action::Levy { system: x as u8 },
            5 => Action::Sway,
            6 => Action::EndTurn,
            7 => Action::Tax,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Lead,
    Follow,
    Act,
    Over,
}

/// What the last action did, for the log and animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    CardPlayed { player: u8, card: Card },
    Moved { from: u8, to: u8, ships: u8, player: u8 },
    Fought { system: u8, enemy_lost: u8, attacker_lost: u8, enemy: u8 },
    Built { system: u8, player: u8 },
    Levied { system: u8, player: u8, ships: u8 },
    Swayed { player: u8, power: u8 },
    EndedTurn { player: u8 },
    Taxed { player: u8, power: u8 },
    TrickWon { player: u8 },
    RoundScored { round: u8, ambition: Ambition },
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ambition {
    MostShips,
    MostCities,
    MostSystems,
}

impl Ambition {
    pub fn name(self) -> &'static str {
        match self {
            Ambition::MostShips => "Most ships on the map",
            Ambition::MostCities => "Most cities",
            Ambition::MostSystems => "Most systems held",
        }
    }
}

/// Whole game state: the flat list the Oxidized rules work on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State(pub Vec<i64>);

impl State {
    pub fn new(players: usize, seed: u64) -> State {
        assert!((2..=MAX_PLAYERS).contains(&players));
        State(from_dv(ox::new_game(Dv::Int(players as i64), Dv::Int((seed % 2_147_483_647) as i64))))
    }

    fn call(&self, f: impl FnOnce(Dv) -> Dv) -> Dv {
        f(to_dv(&self.0))
    }

    pub fn players(&self) -> usize {
        self.0[0] as usize
    }
    pub fn round(&self) -> i64 {
        self.0[1]
    }
    pub fn phase(&self) -> Phase {
        match self.0[2] {
            0 => Phase::Lead,
            1 => Phase::Follow,
            2 => Phase::Act,
            _ => Phase::Over,
        }
    }
    pub fn is_over(&self) -> bool {
        self.phase() == Phase::Over
    }
    pub fn current(&self) -> usize {
        self.0[3] as usize
    }
    pub fn lead_suit(&self) -> Option<Suit> {
        (self.0[5] >= 0 && self.phase() != Phase::Lead).then(|| Suit::from_index(self.0[5]))
    }
    pub fn actions_left(&self) -> i64 {
        self.0[6]
    }
    /// Suit of the actions the current player is taking (valid in `Phase::Act`).
    pub fn action_suit(&self) -> Suit {
        Suit::from_index(self.0[7])
    }
    pub fn initiative(&self) -> usize {
        self.0[12] as usize
    }
    pub fn ambition(&self) -> Ambition {
        match (self.round() - 1) % 3 {
            0 => Ambition::MostShips,
            1 => Ambition::MostCities,
            _ => Ambition::MostSystems,
        }
    }

    fn pf(&self, p: usize, f: usize) -> i64 {
        self.0[20 + p * 6 + f]
    }
    pub fn vp(&self, p: usize) -> i64 {
        self.pf(p, 0)
    }
    pub fn power(&self, p: usize) -> i64 {
        self.pf(p, 1)
    }
    pub fn pool(&self, p: usize) -> i64 {
        self.pf(p, 2)
    }
    /// Card the player most recently played this round, if any.
    pub fn last_card(&self, p: usize) -> Option<Card> {
        let c = self.pf(p, 4);
        (c >= 0).then_some(Card(c as u8))
    }
    pub fn hand(&self, p: usize) -> Vec<Card> {
        let mask = self.pf(p, 3);
        (0..28u8).filter(|c| (mask >> c) & 1 == 1).map(Card).collect()
    }
    pub fn ships(&self, system: usize, p: usize) -> i64 {
        self.0[44 + system * 4 + p]
    }
    pub fn city(&self, system: usize) -> Option<usize> {
        let c = self.0[72 + system];
        (c >= 0).then_some(c as usize)
    }
    pub fn controller(&self, system: usize) -> Option<usize> {
        let c = from_dv(Dv::list(vec![self.call(|s| ox::controller(s, Dv::Int(system as i64)))]));
        let c = c[0];
        (c >= 0).then_some(c as usize)
    }
    pub fn event(&self) -> Event {
        let e = &self.0[14..19];
        let u = |i: usize| e[i] as u8;
        match e[0] {
            0 => Event::CardPlayed { player: u(1), card: Card(u(2)) },
            1 => Event::Moved { from: u(1), to: u(2), ships: u(3), player: u(4) },
            2 => Event::Fought { system: u(1), enemy_lost: u(2), attacker_lost: u(3), enemy: u(4) },
            3 => Event::Built { system: u(1), player: u(2) },
            4 => Event::Levied { system: u(1), player: u(2), ships: u(3) },
            5 => Event::Swayed { player: u(1), power: u(2) },
            6 => Event::EndedTurn { player: u(1) },
            7 => Event::Taxed { player: u(1), power: u(2) },
            8 => Event::TrickWon { player: u(1) },
            9 => Event::RoundScored {
                round: u(1),
                ambition: match e[2] {
                    0 => Ambition::MostShips,
                    1 => Ambition::MostCities,
                    _ => Ambition::MostSystems,
                },
            },
            _ => Event::None,
        }
    }

    pub fn legal_actions(&self) -> Vec<Action> {
        let list = self.call(ox::legal_actions);
        match list {
            Dv::List(l) => l
                .into_vec()
                .into_iter()
                .filter_map(|a| Action::decode(&from_dv(a)))
                .collect(),
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

    /// The bot's pick for the current player.
    pub fn bot_choose(&self) -> Action {
        let a = from_dv(self.call(ox::bot_choose));
        Action::decode(&a).expect("bot returned a valid action")
    }

    pub fn winner(&self) -> usize {
        let w = from_dv(Dv::list(vec![self.call(ox::winner)]));
        w[0] as usize
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
        let s = State::new(3, 42);
        assert_eq!(s.players(), 3);
        assert_eq!(s.phase(), Phase::Lead);
        assert_eq!(s.hand(0).len(), 6);
        assert_eq!(s.city(1), Some(0));
        assert_eq!(s.ships(1, 0), 3);
        assert_eq!(s.power(0), 3);
    }

    #[test]
    fn same_seed_same_game() {
        assert_eq!(State::new(4, 9), State::new(4, 9));
        assert_ne!(State::new(4, 9), State::new(4, 10));
    }

    #[test]
    fn action_roundtrip() {
        for a in [
            Action::Play(Card(13)),
            Action::Move { from: 1, to: 0 },
            Action::Fight { system: 3, enemy: 1 },
            Action::Build { system: 2 },
            Action::Levy { system: 4 },
            Action::Sway,
            Action::Tax,
            Action::EndTurn,
        ] {
            assert_eq!(Action::decode(&a.encode()), Some(a));
        }
    }

    #[test]
    fn bots_finish_games() {
        for n in 2..=4 {
            let mut s = State::new(n, 77);
            let mut steps = 0;
            while !s.is_over() {
                s = s.apply(s.bot_choose());
                steps += 1;
                assert!(steps < 2000);
            }
            assert!(s.winner() < n);
            assert!((0..n).map(|p| s.vp(p)).sum::<i64>() > 0);
        }
    }

    #[test]
    fn validate_rejects_wrong_player_and_illegal() {
        let s = State::new(2, 1);
        let card = s.hand(0)[0];
        assert!(validate(&s, 0, Action::Play(card)).is_ok());
        assert!(validate(&s, 1, Action::Play(card)).is_err());
        assert!(validate(&s, 0, Action::Sway).is_err());
    }

    #[test]
    fn state_serialises() {
        let s = State::new(2, 5);
        let json = serde_json_like(&s);
        assert!(json.len() > 10);
    }

    fn serde_json_like(s: &State) -> String {
        format!("{:?}", s)
    }
}
