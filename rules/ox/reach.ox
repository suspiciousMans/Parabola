// Ember Reach: rules.
//
// Pure, deterministic game logic. The host (Rust) holds the state as a flat
// List of Ints and an action as a short List of Ints, so state and actions are
// trivially serialisable (save files, network) and identical on every peer.
// Every random roll draws from an LCG stored in the state itself.
//
// Round: each player is dealt 6 action cards. A trick is led by whoever holds
// the initiative: the lead plays a card and takes 2 actions of that suit;
// each following player must play the lead suit if they can (1 action), or,
// holding none, may pivot with any card (1 action of the card's own suit).
// Highest card of the lead suit wins the trick: +1 power and the lead next.
// After 6 tricks the round's ambition is scored, cities pay income, repeat.
// Three rounds; most victory points wins.
//
// Suits: 0 Move, 1 Fight, 2 Build, 3 Influence.
// Actions ([kind, a, b]):
//   0 play card [0, card, 0]        1 move   [1, from, to]  (up to 2 ships)
//   2 fight [2, system, enemy]      3 build  [3, system, 0] (city, 2 power)
//   4 levy  [4, system, 0] (1 power: 2 ships from pool to your city)
//   5 sway  [5, 0, 0] (+2 power)    6 end turn [6, 0, 0]
//   7 tax   [7, 0, 0] (+1 power per city you own)

// ---- layout -------------------------------------------------------------

fn h_n() -> Int { return 0 }
fn h_round() -> Int { return 1 }
fn h_phase() -> Int { return 2 }      // 0 lead a card, 1 follow, 2 act, 3 game over
fn h_cur() -> Int { return 3 }
fn h_lead_suit() -> Int { return 5 }
fn h_acts() -> Int { return 6 }
fn h_asuit() -> Int { return 7 }
fn h_rng() -> Int { return 8 }
fn h_played() -> Int { return 9 }
fn h_best_rank() -> Int { return 10 }
fn h_best_player() -> Int { return 11 }
fn h_init() -> Int { return 12 }
fn h_ev() -> Int { return 14 }        // last event: kind, a, b, c, d (14..18)
fn p_base() -> Int { return 20 }
fn p_stride() -> Int { return 6 }
fn ships_base() -> Int { return 44 }
fn city_base() -> Int { return 72 }
fn state_len() -> Int { return 79 }
fn systems() -> Int { return 7 }
fn max_cities() -> Int { return 4 }
fn total_rounds() -> Int { return 3 }

fn pf(s, p, f) { return s[p_base() + p * p_stride() + f] }
fn vp(s, p) { return pf(s, p, 0) }
fn power(s, p) { return pf(s, p, 1) }
fn pool(s, p) { return pf(s, p, 2) }
fn hand(s, p) { return pf(s, p, 3) }
fn ships(s, sys, p) { return s[ships_base() + sys * 4 + p] }
fn city(s, sys) { return s[city_base() + sys] }
fn phase(s) { return s[h_phase()] }
fn cur(s) { return s[h_cur()] }
fn nplayers(s) { return s[h_n()] }
fn round(s) { return s[h_round()] }
fn acts(s) { return s[h_acts()] }

fn card_suit(c) { return c / 7 }
fn card_rank(c) { return c % 7 + 1 }
fn has_card(mask, c) { return (mask / pow2(c)) % 2 == 1 }

fn pow2(n) {
    let r = 1
    let i = 0
    while i < n {
        r = r * 2
        i = i + 1
    }
    return r
}

// ---- map ----------------------------------------------------------------

// System 0 is the hub, 1..6 a ring around it.
fn adjacent(a, b) {
    if a == b { return false }
    if a == 0 || b == 0 { return true }
    let d = (a - b + 6) % 6
    return d == 1 || d == 5
}

fn start_system(n, p) {
    if n == 2 { return [1, 4][p] }
    if n == 3 { return [1, 3, 5][p] }
    return [1, 3, 4, 6][p]
}

fn count_cities(s, p) {
    let c = 0
    for sys in range(0, systems()) {
        if city(s, sys) == p { c = c + 1 }
    }
    return c
}

fn total_ships(s, p) {
    let c = 0
    for sys in range(0, systems()) { c = c + ships(s, sys, p) }
    return c
}

// The player with strictly the most ships (a city counts as one), or -1.
fn controller(s, sys) {
    let best = -1
    let best_v = 0
    let tie = false
    for p in range(0, nplayers(s)) {
        let v = ships(s, sys, p)
        if city(s, sys) == p { v = v + 1 }
        if v > best_v {
            best_v = v
            best = p
            tie = false
        } else if v == best_v && v > 0 {
            tie = true
        }
    }
    if tie { return -1 }
    return best
}

fn count_controlled(s, p) {
    let c = 0
    for sys in range(0, systems()) {
        if controller(s, sys) == p { c = c + 1 }
    }
    return c
}

// ---- randomness ---------------------------------------------------------

fn rng_step(r) { return (r * 1103515245 + 12345) % 2147483648 }
fn rng_val(r) { return r / 65536 }

// Returns the state with the generator advanced; read the draw back with rng_val(s[h_rng()]).
fn draw(s) {
    let t = s
    t[h_rng()] = rng_step(s[h_rng()])
    return t
}

fn roll_d6(s) { return rng_val(s[h_rng()]) % 6 + 1 }

// ---- setup --------------------------------------------------------------

fn new_game(n, seed) {
    let s = []
    for i in range(0, state_len()) { s = push(s, 0) }
    s[h_n()] = n
    s[h_round()] = 1
    s[h_rng()] = seed % 2147483648
    for sys in range(0, systems()) { s[city_base() + sys] = -1 }
    for p in range(0, n) {
        let b = p_base() + p * p_stride()
        s[b + 1] = 3     // power
        s[b + 2] = 9     // pool
        s[b + 4] = -1
        let home = start_system(n, p)
        s[ships_base() + home * 4 + p] = 3
        s[city_base() + home] = p
    }
    s[h_init()] = 0
    return deal(s)
}

fn deal(s) {
    let t = s
    let deck = range(0, 28)
    let i = 27
    while i > 0 {
        t = draw(t)
        let j = rng_val(t[h_rng()]) % (i + 1)
        let tmp = deck[i]
        deck[i] = deck[j]
        deck[j] = tmp
        i = i - 1
    }
    let k = 0
    for p in range(0, nplayers(t)) {
        let mask = 0
        for m in range(0, 6) {
            mask = mask + pow2(deck[k])
            k = k + 1
        }
        t[p_base() + p * p_stride() + 3] = mask
        t[p_base() + p * p_stride() + 4] = -1
    }
    t[h_phase()] = 0
    t[h_cur()] = t[h_init()]
    t[h_lead_suit()] = -1
    t[h_played()] = 0
    t[h_best_rank()] = 0
    t[h_acts()] = 0
    return t
}

// ---- legal actions ------------------------------------------------------

fn cards_in_hand(s, p) {
    let out = []
    let mask = hand(s, p)
    for c in range(0, 28) {
        if has_card(mask, c) { out = push(out, c) }
    }
    return out
}

fn action_actions(s, suit) {
    let p = cur(s)
    let out = []
    if suit == 0 {
        for from in range(0, systems()) {
            if ships(s, from, p) > 0 {
                for to in range(0, systems()) {
                    if adjacent(from, to) { out = push(out, [1, from, to]) }
                }
            }
        }
    } else if suit == 1 {
        for sys in range(0, systems()) {
            if ships(s, sys, p) > 0 {
                for e in range(0, nplayers(s)) {
                    if e != p && ships(s, sys, e) > 0 { out = push(out, [2, sys, e]) }
                }
            }
        }
    } else if suit == 2 {
        for sys in range(0, systems()) {
            if ships(s, sys, p) > 0 && city(s, sys) == -1 && power(s, p) >= 2 && count_cities(s, p) < max_cities() {
                out = push(out, [3, sys, 0])
            }
            if city(s, sys) == p && power(s, p) >= 1 && pool(s, p) >= 1 {
                out = push(out, [4, sys, 0])
            }
        }
    } else {
        out = push(out, [5, 0, 0])
        if count_cities(s, p) > 0 { out = push(out, [7, 0, 0]) }
    }
    return out
}

fn legal_actions(s) {
    let out = []
    let ph = phase(s)
    let p = cur(s)
    if ph == 0 {
        for c in cards_in_hand(s, p) { out = push(out, [0, c, 0]) }
    } else if ph == 1 {
        let cards = cards_in_hand(s, p)
        let follow = []
        for c in cards {
            if card_suit(c) == s[h_lead_suit()] { follow = push(follow, c) }
        }
        if len(follow) > 0 { cards = follow }
        for c in cards { out = push(out, [0, c, 0]) }
    } else if ph == 2 {
        out = action_actions(s, s[h_asuit()])
        out = push(out, [6, 0, 0])
    }
    return out
}

fn same_action(a, b) { return a[0] == b[0] && a[1] == b[1] && a[2] == b[2] }

fn is_legal(s, a) {
    for x in legal_actions(s) {
        if same_action(x, a) { return true }
    }
    return false
}

// ---- applying actions ---------------------------------------------------

fn set_event(t, kind, a, b, c, d) {
    t[h_ev()] = kind
    t[h_ev() + 1] = a
    t[h_ev() + 2] = b
    t[h_ev() + 3] = c
    t[h_ev() + 4] = d
    return t
}

fn min_i(a, b) { if a < b { return a } return b }

// Applies a legal action; returns the next state (the input is untouched).
fn apply(s, a) {
    let t = s
    let p = cur(s)
    let kind = a[0]
    let pb = p_base() + p * p_stride()
    if kind == 0 {
        let c = a[1]
        let lead = phase(s) == 0
        t[pb + 3] = hand(s, p) - pow2(c)
        t[pb + 4] = c
        if lead {
            t[h_lead_suit()] = card_suit(c)
            t[h_best_rank()] = card_rank(c)
            t[h_best_player()] = p
            t[h_asuit()] = card_suit(c)
            t[h_acts()] = 2
        } else {
            if card_suit(c) == s[h_lead_suit()] {
                t[h_asuit()] = s[h_lead_suit()]
                if card_rank(c) > s[h_best_rank()] {
                    t[h_best_rank()] = card_rank(c)
                    t[h_best_player()] = p
                }
            } else {
                t[h_asuit()] = card_suit(c)
            }
            t[h_acts()] = 1
        }
        t[h_played()] = s[h_played()] + 1
        t[h_phase()] = 2
        t = set_event(t, 0, p, c, 0, 0)
        return settle(t)
    }
    if kind == 6 {
        t[h_acts()] = 0
        return settle(set_event(t, 6, p, 0, 0, 0))
    }
    if kind == 1 {
        let n = min_i(2, ships(s, a[1], p))
        t[ships_base() + a[1] * 4 + p] = ships(s, a[1], p) - n
        t[ships_base() + a[2] * 4 + p] = ships(s, a[2], p) + n
        t = set_event(t, 1, a[1], a[2], n, p)
    } else if kind == 2 {
        t = fight(t, a[1], a[2])
    } else if kind == 3 {
        t[city_base() + a[1]] = p
        t[pb + 1] = power(s, p) - 2
        t = set_event(t, 3, a[1], p, 0, 0)
    } else if kind == 4 {
        let n = min_i(2, pool(s, p))
        t[ships_base() + a[1] * 4 + p] = ships(s, a[1], p) + n
        t[pb + 2] = pool(s, p) - n
        t[pb + 1] = power(s, p) - 1
        t = set_event(t, 4, a[1], p, n, 0)
    } else if kind == 5 {
        t[pb + 1] = power(s, p) + 2
        t = set_event(t, 5, p, 2, 0, 0)
    } else if kind == 7 {
        let g = count_cities(s, p)
        t[pb + 1] = power(s, p) + g
        t = set_event(t, 7, p, g, 0, 0)
    }
    t[h_acts()] = s[h_acts()] - 1
    return settle(t)
}

// Up to three dice a side, 5 or 6 hits. Losses return to the owner's pool.
fn fight(s, sys, enemy) {
    let t = s
    let p = cur(s)
    let a_dice = min_i(3, ships(s, sys, p))
    let d_dice = min_i(3, ships(s, sys, enemy))
    let dh = 0
    let ah = 0
    for i in range(0, a_dice) {
        t = draw(t)
        if roll_d6(t) >= 5 { dh = dh + 1 }
    }
    for i in range(0, d_dice) {
        t = draw(t)
        if roll_d6(t) >= 5 { ah = ah + 1 }
    }
    let d_loss = min_i(dh, ships(s, sys, enemy))
    let a_loss = min_i(ah, ships(s, sys, p))
    t[ships_base() + sys * 4 + enemy] = ships(s, sys, enemy) - d_loss
    t[ships_base() + sys * 4 + p] = ships(s, sys, p) - a_loss
    let eb = p_base() + enemy * p_stride()
    let pb = p_base() + p * p_stride()
    t[eb + 2] = pool(s, enemy) + d_loss
    t[pb + 2] = pool(s, p) + a_loss
    if t[ships_base() + sys * 4 + enemy] == 0 && city(s, sys) == enemy && t[ships_base() + sys * 4 + p] > 0 {
        t[city_base() + sys] = p
        t[pb] = vp(s, p) + 1
    }
    return set_event(t, 2, sys, d_loss, a_loss, enemy)
}

// After any change: end the turn when no actions remain (or none are possible).
fn settle(s) {
    if phase(s) != 2 { return s }
    if acts(s) > 0 {
        if len(action_actions(s, s[h_asuit()])) > 0 { return s }
    }
    return end_turn(s)
}

fn end_turn(s) {
    let t = s
    t[h_acts()] = 0
    if t[h_played()] < nplayers(t) {
        t[h_cur()] = (cur(t) + 1) % nplayers(t)
        t[h_phase()] = 1
        return t
    }
    // trick over
    let w = t[h_best_player()]
    t[p_base() + w * p_stride() + 1] = power(t, w) + 1
    t[h_init()] = w
    t = set_event(t, 8, w, 0, 0, 0)
    if hand(t, w) == 0 { return score_round(t) }
    t[h_cur()] = w
    t[h_phase()] = 0
    t[h_lead_suit()] = -1
    t[h_played()] = 0
    t[h_best_rank()] = 0
    return t
}

// Ambition of round r: 0 most ships, 1 most cities, 2 most systems held.
fn metric(s, p, kind) {
    if kind == 0 { return total_ships(s, p) }
    if kind == 1 { return count_cities(s, p) }
    return count_controlled(s, p)
}

fn ambition(s) { return (round(s) - 1) % 3 }

fn score_round(s) {
    let t = s
    let n = nplayers(t)
    let kind = ambition(t)
    let top = 0
    for p in range(0, n) {
        if metric(t, p, kind) > top { top = metric(t, p, kind) }
    }
    if top > 0 {
        let at_top = 0
        for p in range(0, n) {
            if metric(t, p, kind) == top { at_top = at_top + 1 }
        }
        let second = 0
        for p in range(0, n) {
            let m = metric(t, p, kind)
            if m < top && m > second { second = m }
        }
        for p in range(0, n) {
            let m = metric(t, p, kind)
            let b = p_base() + p * p_stride()
            if m == top {
                t[b] = vp(t, p) + (if at_top == 1 { 3 } else { 2 })
            } else if m == second && second > 0 && at_top == 1 {
                let tied_second = 0
                for q in range(0, n) {
                    if metric(t, q, kind) == second { tied_second = tied_second + 1 }
                }
                if tied_second == 1 { t[b] = vp(t, p) + 1 }
            }
        }
    }
    for p in range(0, n) {
        let b = p_base() + p * p_stride()
        t[b + 1] = power(t, p) + count_cities(t, p)
    }
    t = set_event(t, 9, round(t), kind, 0, 0)
    if round(t) >= total_rounds() {
        t[h_phase()] = 3
        return t
    }
    t[h_round()] = round(t) + 1
    return deal(t)
}

fn is_over(s) { return phase(s) == 3 }

// Highest VP; ties broken by power, then ships on board. Lowest seat wins a full tie.
fn winner(s) {
    let best = 0
    for p in range(1, nplayers(s)) {
        let a = [vp(s, p), power(s, p), total_ships(s, p)]
        let b = [vp(s, best), power(s, best), total_ships(s, best)]
        if a[0] > b[0] || (a[0] == b[0] && (a[1] > b[1] || (a[1] == b[1] && a[2] > b[2]))) {
            best = p
        }
    }
    return best
}

// ---- bot ----------------------------------------------------------------

fn suit_weight(s, suit) {
    let p = cur(s)
    if suit == 2 && power(s, p) >= 2 { return 8 }
    if suit == 3 && power(s, p) < 2 { return 7 }
    if suit == 1 { return 5 }
    if suit == 0 { return 4 }
    return 3
}

fn score_action(s, a) {
    let p = cur(s)
    let k = a[0]
    if k == 0 { return card_rank(a[1]) + suit_weight(s, card_suit(a[1])) * 2 }
    if k == 3 { return 100 }
    if k == 4 { return 60 }
    if k == 2 {
        if ships(s, a[1], p) > ships(s, a[1], a[2]) { return 80 }
        return 15
    }
    if k == 1 {
        let to = a[2]
        let v = 5
        if ships(s, to, p) == 0 { v = v + 10 }
        for e in range(0, nplayers(s)) {
            if e != p && ships(s, to, e) > 0 {
                if ships(s, to, e) < ships(s, a[1], p) { v = v + 40 } else { v = v - 20 }
            }
        }
        if city(s, to) != p && city(s, to) != -1 { v = v + 15 }
        if city(s, a[1]) == p && ships(s, a[1], p) <= 2 { v = v - 25 }
        return v
    }
    if k == 5 { if power(s, p) < 2 { return 70 } return 20 }
    if k == 7 { return 25 + count_cities(s, p) * 8 }
    return 1
}

// Picks the highest-scoring legal action; the state's own generator breaks ties.
fn bot_choose(s) {
    let acts_l = legal_actions(s)
    let t = draw(s)
    let best = acts_l[0]
    let best_v = -1000000
    let i = 0
    for a in acts_l {
        t = draw(t)
        let v = score_action(s, a) * 10 + rng_val(t[h_rng()]) % 10
        if v > best_v {
            best_v = v
            best = a
        }
    }
    return best
}

// Plays a whole game of bots; returns the final state (for tests and tuning).
fn bot_game(n, seed) {
    let s = new_game(n, seed)
    let steps = 0
    while !is_over(s) && steps < 2000 {
        s = apply(s, bot_choose(s))
        steps = steps + 1
    }
    return s
}
