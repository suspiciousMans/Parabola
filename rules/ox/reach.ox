// Ember Reach rules, stage 1: map, pieces, setup, rounds, actions, battle,
// ambitions. A faithful adaptation of the published rules for PRIVATE play:
// see data.ox for the transcribed tables (marked UNVERIFIED) and README for
// the interpretations still to be checked against the physical game.
//
// The whole game state is a flat List of Ints and an action is a short List
// of Ints, so both are trivially serialisable (saves, network) and identical
// on every peer. Randomness comes from an LCG stored in the state.
//
// Court: a deck of ten original court cards; a face-up row of 3 (2 players) or
// 4 (3-4 players) sits along the map edge. Influence places one of your ten
// agents on a card; Secure takes a card where you have more agents than each
// rival: your agents come home, rival agents become your captives, the card
// joins your court (a resource icon for ambitions plus a lasting effect) and
// the slot is refilled from the deck.
//
// Leaders: each player is dealt one of six leaders at setup. A leader adds one
// starting resource and one rule bend. Lore: after each chapter is scored the
// player(s) furthest behind gain a Lore token (max 3); once per turn in the
// action step a Lore buys one more action of the current suit.
// Card play follows the published round structure: lead (optionally declaring
// the ambition on the card, which sets its number to 0), then each other player
// with cards must Surpass, Copy or Pivot; seizing; passing the initiative.

import "data.ox"

// ---- layout -------------------------------------------------------------
fn n_sys() -> Int { return 24 }
fn h_n() -> Int { return 0 }
fn h_chapter() -> Int { return 1 }
fn h_phase() -> Int { return 2 }      // 0 lead, 1 follow, 2 act, 4 game over
fn h_cur() -> Int { return 3 }
fn h_init() -> Int { return 4 }
fn h_rng() -> Int { return 5 }
fn h_lead_suit() -> Int { return 6 }
fn h_best_num() -> Int { return 7 }
fn h_best_player() -> Int { return 8 }
fn h_seized() -> Int { return 9 }
fn h_turns() -> Int { return 10 }
fn h_acts() -> Int { return 11 }
fn h_act_suit() -> Int { return 12 }
fn h_x_build() -> Int { return 14 }
fn h_x_move() -> Int { return 15 }
fn h_x_battle() -> Int { return 16 }
fn h_x_lead() -> Int { return 17 }
fn h_acted() -> Int { return 18 }
fn h_taxed() -> Int { return 13 }     // bitmask (sys * 2 + slot) of cities taxed this turn
fn h_ported() -> Int { return 19 }    // bitmask (sys * 2 + k) of starport ship builds this turn
fn h_lead_num() -> Int { return 28 }  // lead card number, 0 once an ambition is declared
fn h_lore_used() -> Int { return 21 }  // a Lore was spent this turn
fn h_spill() -> Int { return 29 }      // battle scratch: hits with no ship left to take them
fn h_out() -> Int { return 20 }       // bitmask of clusters out of play
fn h_amb() -> Int { return 22 }       // 5 entries: markers on each ambition this chapter
fn h_amb_count() -> Int { return 27 }  // markers placed this chapter (max 3)
fn h_court() { return 35 }     // court row card ids (3 or 4 used), -1 = empty
fn h_ev() -> Int { return 30 }        // last event: kind, a..e
fn p_base() -> Int { return 40 }
fn p_stride() -> Int { return 20 }
fn ships_base() -> Int { return 120 }
fn slots_base() -> Int { return 312 }
fn supply_base() -> Int { return 456 }
fn h_passes() -> Int { return 461 }   // consecutive initiative passes
fn h_deck() -> Int { return 462 }     // bitmask of court cards still in the deck
fn h_flips() -> Int { return 463 }    // ambition markers flipped to their high side (0..3)
fn h_mk_taken() -> Int { return 464 } // bitmask of markers placed this chapter
fn h_x_secure() -> Int { return 465 } // Relic prelude: extra Secure actions
fn h_amb_first() -> Int { return 466 }  // 5 entries: summed first-place power
fn h_amb_second() -> Int { return 471 } // 5 entries: summed second-place power
fn agents_base() -> Int { return 476 }  // court slot i, player p: 476 + i * 4 + p
fn cap_base() -> Int { return 492 }     // captives held by h of owner o: 492 + h * 4 + o
fn state_len() -> Int { return 508 }
fn n_agents() { return 10 }

fn pb(p) { return p_base() + p_stride() * p }
fn power(s, p) { return s[pb(p)] }
fn res(s, p, t) { return s[pb(p) + 1 + t] }
fn cities_left(s, p) { return s[pb(p) + 6] }
fn ports_left(s, p) { return s[pb(p) + 7] }
fn reserve(s, p) { return s[pb(p) + 8] }
fn trophies(s, p) { return s[pb(p) + 9] }
fn captives(s, p) { return s[pb(p) + 10] }
fn hand(s, p) { return s[pb(p) + 11] }
fn secured(s, p) { return s[pb(p) + 15] }   // bitmask of court card ids
fn leader(s, p) { return s[pb(p) + 16] }    // 0..5, see leader_res
fn lore(s, p) { return s[pb(p) + 17] }
fn play_mode(s, p) { return s[pb(p) + 18] }   // this round: -1 none, 0 lead, 1 surpass, 2 copy, 3 pivot
fn seize_card(s, p) { return s[pb(p) + 19] }  // card spent to seize this round, -1 none
fn lore_cap() { return 3 }

// Court card ids 0..9; the effect is id % 5: 0 Envoy, 1 Seal, 2 Writ, 3 Pact,
// 4 Accord (cards 5..9 repeat those effects under other names and icons).
fn n_court() { return 10 }
fn court_icon(c) { return [0, 2, 3, 4, 1, 1, 3, 0, 2, 4][c] }   // resource suit
fn has_court(s, p, e) {
    let m = secured(s, p)
    return has_card(m, e) || has_card(m, e + 5)
}
fn court_icons(s, p, ty) {
    let n = 0
    for c in range(0, n_court()) {
        if has_card(secured(s, p), c) && court_icon(c) == ty { n = n + 1 }
    }
    return n
}
fn row_size(s) { if nplayers(s) == 2 { return 3 } return 4 }
fn agents_on(s, i, p) { return s[agents_base() + i * 4 + p] }
fn cap_of(s, h, o) { return s[cap_base() + h * 4 + o] }
// Agents in p's supply: ten, less those in the Court and those held captive.
fn agents_left(s, p) {
    let n = n_agents()
    for i in range(0, 4) { n = n - agents_on(s, i, p) }
    for h in range(0, nplayers(s)) { n = n - cap_of(s, h, p) }
    return n
}
fn court_envoy() { return 0 }    // Tax takes one more of that resource
fn court_seal() { return 1 }     // the first building hit in each battle against you is ignored
fn court_writ() { return 2 }     // Repair fixes every damaged piece of yours in the system
fn court_pact() { return 3 }     // the first self hit in each of your attacks is ignored
fn court_accord() { return 4 }   // +1 power at each chapter end

// Leaders: 0 Broker, 1 Admiral, 2 Architect, 3 Seer, 4 Pilgrim, 5 Corsair.
fn n_leaders() { return 6 }
fn leader_res(l) { return [1, 2, 0, 4, 3, 1][l] }    // starting resource
// Pilgrim (4): Influence places two agents.
fn is_leader(s, p, l) { return leader(s, p) == l }
fn outraged(s, p, t) { return (s[pb(p) + 13] / pow2(t)) % 2 == 1 }
fn court(s, i) { return s[h_court() + i] }
fn supply(s, t) { return s[supply_base() + t] }
fn phase(s) { return s[h_phase()] }
fn cur(s) { return s[h_cur()] }
fn nplayers(s) { return s[h_n()] }
fn chapter(s) { return s[h_chapter()] }
fn acts(s) { return s[h_acts()] }

fn ship_i(sys, p, d) { return ships_base() + sys * 8 + p * 2 + d }
fn fresh(s, sys, p) { return s[ship_i(sys, p, 0)] }
fn damaged(s, sys, p) { return s[ship_i(sys, p, 1)] }
fn all_ships(s, sys, p) { return fresh(s, sys, p) + damaged(s, sys, p) }
fn slot_i(sys, sl) { return slots_base() + sys * 6 + sl * 3 }
fn slot_owner(s, sys, sl) { return s[slot_i(sys, sl)] }
fn slot_kind(s, sys, sl) { return s[slot_i(sys, sl) + 1] }     // 0 empty, 1 city, 2 starport
fn slot_dmg(s, sys, sl) { return s[slot_i(sys, sl) + 2] }

fn pow2(n) {
    let r = 1
    let i = 0
    while i < n {
        r = r * 2
        i = i + 1
    }
    return r
}

fn card_suit(c) { return c / 7 }
fn card_num(c) { return c % 7 + 1 }
fn has_card(mask, c) { return (mask / pow2(c)) % 2 == 1 }
fn min_i(a, b) { if a < b { return a } return b }
fn max_i(a, b) { if a > b { return a } return b }

// ---- map ----------------------------------------------------------------
fn cluster_of(sys) { return sys / 4 }
fn kind_of(sys) { return sys % 4 }
fn is_gate(sys) { return sys % 4 == 3 }
fn gate_of(c) { return c * 4 + 3 }
fn cluster_out(s, c) { return (s[h_out()] / pow2(c)) % 2 == 1 }
fn in_play(s, sys) { return !cluster_out(s, cluster_of(sys)) }

// Next cluster in play going `dir` (+1/-1) around the ring.
fn next_cluster(s, c, dir) {
    let k = (c + dir + 6) % 6
    let guard = 0
    while cluster_out(s, k) && guard < 6 {
        k = (k + dir + 6) % 6
        guard = guard + 1
    }
    return k
}

fn adjacent(s, a, b) {
    if a == b { return false }
    if !in_play(s, a) || !in_play(s, b) { return false }
    let ca = cluster_of(a)
    let cb = cluster_of(b)
    if ca == cb {
        if is_gate(a) || is_gate(b) { return true }
        let ka = kind_of(a)
        let kb = kind_of(b)
        return (ka == 0 && kb == 1) || (ka == 1 && kb == 0) || (ka == 1 && kb == 2) || (ka == 2 && kb == 1)
    }
    if is_gate(a) && is_gate(b) {
        return next_cluster(s, ca, 1) == cb || next_cluster(s, ca, -1) == cb
    }
    // cross-cluster planet links: 2-hex <-> 3-arrow, 5-hex <-> 6-arrow
    if (a == 6 && b == 8) || (a == 8 && b == 6) { return true }
    if (a == 18 && b == 20) || (a == 20 && b == 18) { return true }
    return false
}

fn neighbors(s, a) {
    let out = []
    for b in range(0, n_sys()) {
        if adjacent(s, a, b) { out = push(out, b) }
    }
    return out
}

// ---- pieces, control ----------------------------------------------------
fn count_buildings(s, sys, p) {
    let c = 0
    for sl in range(0, 2) {
        if slot_kind(s, sys, sl) != 0 && slot_owner(s, sys, sl) == p { c = c + 1 }
    }
    return c
}

fn buildings_anywhere(s, p) {
    let c = 0
    for sys in range(0, n_sys()) { c = c + count_buildings(s, sys, p) }
    return c
}

fn count_kind(s, sys, p, kind, dmg) {
    let c = 0
    for sl in range(0, 2) {
        if slot_kind(s, sys, sl) == kind && slot_owner(s, sys, sl) == p && slot_dmg(s, sys, sl) == dmg { c = c + 1 }
    }
    return c
}

fn has_piece(s, sys, p) { return all_ships(s, sys, p) > 0 || count_buildings(s, sys, p) > 0 }

// Player with strictly the most fresh ships in the system, or -1.
fn controller(s, sys) {
    let best = -1
    let best_v = 0
    let tie = false
    for p in range(0, nplayers(s)) {
        let v = fresh(s, sys, p)
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

fn free_slot(s, sys) {
    for sl in range(0, planet_slots(sys)) {
        if slot_kind(s, sys, sl) == 0 { return sl }
    }
    return -1
}

fn total_resources(s, p) {
    let c = 0
    for t in range(0, 5) { c = c + res(s, p, t) }
    return c
}

// ---- randomness ---------------------------------------------------------
fn rng_step(r) { return (r * 1103515245 + 12345) % 2147483648 }
fn rng_val(r) { return r / 65536 }
fn draw(s) {
    let t = s
    t[h_rng()] = rng_step(s[h_rng()])
    return t
}
fn roll_d6(s) { return rng_val(s[h_rng()]) % 6 }

// ---- setup --------------------------------------------------------------
fn new_game(n, seed, setup_idx) {
    let s = []
    for i in range(0, state_len()) { s = push(s, 0) }
    s[h_n()] = n
    s[h_chapter()] = 1
    s[h_rng()] = seed % 2147483648
    let outs = setup_out(n, setup_idx % setup_count(n))
    let mask = 0
    for c in outs { mask = mask + pow2(c) }
    s[h_out()] = mask
    s[h_seized()] = -1
    s[h_lead_suit()] = -1
    for i in range(0, 5) { s[h_court() + i] = -1 }
    s[h_deck()] = pow2(n_court()) - 1
    for t in range(0, 5) { s[supply_base() + t] = 5 }
    for sys in range(0, n_sys()) {
        for sl in range(0, 2) { s[slot_i(sys, sl)] = -1 }
    }
    for p in range(0, n) {
        let b = pb(p)
        s[b + 6] = 5
        s[b + 7] = 5
        s[b + 8] = 15
        s[b + 14] = -1
        let seat = setup_seat(n, setup_idx % setup_count(n), p)
        let a = seat[0]
        let bb = seat[1]
        // A: 3 ships + city, B: 3 ships + starport, C: 2 ships in each remaining system
        s[ship_i(a, p, 0)] = s[ship_i(a, p, 0)] + 3
        s[slot_i(a, 0)] = p
        s[slot_i(a, 0) + 1] = 1
        s[b + 6] = s[b + 6] - 1
        s[ship_i(bb, p, 0)] = s[ship_i(bb, p, 0)] + 3
        s[slot_i(bb, 0)] = p
        s[slot_i(bb, 0) + 1] = 2
        s[b + 7] = s[b + 7] - 1
        s[b + 8] = s[b + 8] - 6
        for k in range(2, len(seat)) {
            s[ship_i(seat[k], p, 0)] = s[ship_i(seat[k], p, 0)] + 2
            s[b + 8] = s[b + 8] - 2
        }
        for sysd in [a, bb] {
            let t = planet_type(sysd)
            s[b + 1 + t] = s[b + 1 + t] + 1
            s[supply_base() + t] = s[supply_base() + t] - 1
        }
    }
    s = deal_leaders(s)
    s = shuffle_init(s)
    for i in range(0, row_size(s)) { s = refill_court(s, i) }
    return deal_chapter(s)
}

// Draw a random card from the court deck into row slot i (empty if the deck is out).
fn refill_court(s, i) {
    let t = s
    t[h_court() + i] = -1
    let left = []
    for c in range(0, n_court()) {
        if has_card(t[h_deck()], c) { left = push(left, c) }
    }
    if len(left) == 0 { return t }
    t = draw(t)
    let c = left[rng_val(t[h_rng()]) % len(left)]
    t[h_deck()] = t[h_deck()] - pow2(c)
    t[h_court() + i] = c
    return t
}

// Shuffle the six leaders and give one to each player, with its resource.
fn deal_leaders(s) {
    let t = s
    let ids = []
    for l in range(0, n_leaders()) { ids = push(ids, l) }
    let i = len(ids) - 1
    while i > 0 {
        t = draw(t)
        let j = rng_val(t[h_rng()]) % (i + 1)
        let tmp = ids[i]
        ids[i] = ids[j]
        ids[j] = tmp
        i = i - 1
    }
    for p in range(0, nplayers(t)) {
        let l = ids[p]
        t[pb(p) + 16] = l
        let r = leader_res(l)
        if supply(t, r) > 0 {
            t[pb(p) + 1 + r] = res(t, p, r) + 1
            t[supply_base() + r] = supply(t, r) - 1
        }
    }
    return t
}

// Random starting initiative.
fn shuffle_init(s) {
    let t = draw(s)
    t[h_init()] = rng_val(t[h_rng()]) % nplayers(t)
    return t
}

fn deck_size(n) { if n == 4 { return 28 } return 20 }

// Fresh shuffled deck (cards 2-6, plus 1s and 7s with 4 players), 6 cards each.
fn deal_chapter(s) {
    let t = s
    let ids = []
    for c in range(0, 28) {
        let num = card_num(c)
        if nplayers(t) == 4 || (num >= 2 && num <= 6) { ids = push(ids, c) }
    }
    let i = len(ids) - 1
    while i > 0 {
        t = draw(t)
        let j = rng_val(t[h_rng()]) % (i + 1)
        let tmp = ids[i]
        ids[i] = ids[j]
        ids[j] = tmp
        i = i - 1
    }
    let k = 0
    for p in range(0, nplayers(t)) {
        let mask = 0
        for m in range(0, 6) {
            mask = mask + pow2(ids[k])
            k = k + 1
        }
        t[pb(p) + 11] = mask
        t[pb(p) + 14] = -1
    }
    t[h_cur()] = t[h_init()]
    return start_round(t)
}

fn start_round(s) {
    let t = s
    t[h_phase()] = 0
    t[h_cur()] = t[h_init()]
    t[h_lead_suit()] = -1
    t[h_best_num()] = 0
    t[h_best_player()] = -1
    t[h_lead_num()] = 0
    t[h_seized()] = -1
    t[h_turns()] = 0
    t[h_x_secure()] = 0
    t[h_acts()] = 0
    t[h_x_build()] = 0
    t[h_x_move()] = 0
    t[h_x_battle()] = 0
    t[h_x_lead()] = 0
    t[h_acted()] = 0
    for p in range(0, nplayers(t)) {
        t[pb(p) + 18] = -1
        t[pb(p) + 19] = -1
    }
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

fn amb_declared(s, a) { return s[h_amb() + a] > 0 }

// Ambition types a lead card of this number may declare (-1 alone = none).
// An ambition may be declared again in a later round: another marker joins it.
fn ambition_options(s, num) {
    let out = [-1]
    if s[h_amb_count()] >= 3 { return out }
    if num >= 2 && num <= 6 { out = push(out, num - 2) }
    if num == 7 {
        for a in range(0, 5) { out = push(out, a) }
    }
    return out
}

// Ambition markers: [first, second] by marker index, low side then high side.
// Each chapter end flips the lowest unflipped marker.
fn marker_pair(s, i) {
    if i < s[h_flips()] { return [[4, 2], [6, 3], [9, 4]][i] }
    return [[2, 0], [3, 2], [5, 3]][i]
}

// The free marker worth the most first-place power, or -1.
fn best_marker(s) {
    let best = -1
    let bv = -1
    for i in range(0, 3) {
        if !has_card(s[h_mk_taken()], i) && marker_pair(s, i)[0] > bv {
            bv = marker_pair(s, i)[0]
            best = i
        }
    }
    return best
}

// Which action kinds each suit's pips cover (kinds: 8 tax, 9-11 build,
// 12 move, 13 repair, 14 battle, 15 secure, 17 influence).
fn suit_allows(suit, kind) {
    if suit == 0 { return kind == 8 || kind == 13 || kind == 17 }   // Administration: tax, repair, influence
    if suit == 1 { return kind == 14 || kind == 12 || kind == 15 }  // Aggression: battle, move, secure
    if suit == 2 { return kind == 9 || kind == 10 || kind == 11 || kind == 13 }  // Construction: build, repair
    return kind == 12 || kind == 17                                 // Mobilization: move, influence
}

fn kind_group(kind) {
    if kind == 9 || kind == 10 || kind == 11 || kind == 13 { return 0 }   // build / repair
    if kind == 12 { return 1 }
    if kind == 14 { return 2 }
    if kind == 15 { return 4 }
    return 3
}

// Is this action kind available right now, and through which source?
// 1 = suit pip, 2 = prelude extra, 0 = not available. A Weapon spent in the
// prelude lets this turn's pips take Battle actions.
fn source_for(s, kind) {
    if acts(s) > 0 && suit_allows(s[h_act_suit()], kind) { return 1 }
    if kind == 14 && acts(s) > 0 && s[h_x_battle()] > 0 { return 1 }
    let g = kind_group(kind)
    if g == 0 && s[h_x_build()] > 0 { return 2 }
    if g == 1 && s[h_x_move()] > 0 { return 2 }
    if g == 4 && s[h_x_secure()] > 0 { return 2 }
    if s[h_x_lead()] > 0 && suit_allows(s[h_lead_suit()], kind) { return 2 }
    return 0
}

fn extras_left(s) {
    return s[h_x_build()] + s[h_x_move()] + s[h_x_lead()] + s[h_x_secure()]
}

fn can_spend(s, p, t) {
    return res(s, p, t) > 0 && !outraged(s, p, t)
}

fn legal_actions(s) {
    let out = []
    let ph = phase(s)
    let p = cur(s)
    if ph == 0 {
        for c in cards_in_hand(s, p) {
            for a in ambition_options(s, card_num(c)) { out = push(out, [0, c, a, 0]) }
        }
        out = push(out, [4, 0, 0, 0])
    } else if ph == 1 {
        // Following is not optional: Surpass (lead suit, higher than the lead
        // card's number), Copy (any card face down) or Pivot (another suit).
        let ls = s[h_lead_suit()]
        let cards = cards_in_hand(s, p)
        for c in cards {
            if card_suit(c) == ls {
                if card_num(c) > s[h_lead_num()] { out = push(out, [1, c, 0, 0]) }
            } else {
                out = push(out, [3, c, 0, 0])
            }
            out = push(out, [2, c, 0, 0])
        }
        // Seize: an extra card face down first (not by the initiative holder).
        if s[h_seized()] == -1 && p != s[h_init()] && len(cards) >= 2 {
            for c in cards { out = push(out, [5, c, 0, 0]) }
        }
    } else if ph == 2 {
        if s[h_acted()] == 0 {
            // Prelude: spend resources before any pip. Material builds or
            // repairs, Fuel moves, a Weapon lets pips battle, a Relic secures,
            // Psionic takes an action of the lead card.
            for t in range(0, 5) {
                if can_spend(s, p, t) { out = push(out, [6, t, 0, 0]) }
            }
        }
        if lore(s, p) > 0 && s[h_lore_used()] == 0 { out = push(out, [16, 0, 0, 0]) }
        for x in action_list(s) { out = push(out, x) }
        out = push(out, [7, 0, 0, 0])
    }
    return out
}

fn action_list(s) {
    let p = cur(s)
    let out = []
    let n = nplayers(s)
    // Tax
    // Tax a Loyal city, or a Rival city you control; each city once per turn.
    if source_for(s, 8) > 0 {
        for sys in range(0, n_sys()) {
            if in_play(s, sys) && !is_gate(sys) {
                for sl in range(0, 2) {
                    if slot_kind(s, sys, sl) == 1 && !has_card(s[h_taxed()], sys * 2 + sl) {
                        let o = slot_owner(s, sys, sl)
                        let rival_ok = o != p && controller(s, sys) == p
                        let gains = supply(s, planet_type(sys)) > 0
                        if (o == p && gains) || (rival_ok && (gains || agents_left(s, o) > 0)) {
                            out = push(out, [8, sys, sl, 0])
                        }
                    }
                }
            }
        }
    }
    // Influence: one agent on any card in the Court.
    if source_for(s, 17) > 0 && agents_left(s, p) > 0 {
        for i in range(0, row_size(s)) {
            if court(s, i) >= 0 { out = push(out, [17, i, 0, 0]) }
        }
    }
    // Secure: a card where you have more agents than each rival.
    if source_for(s, 15) > 0 {
        for i in range(0, row_size(s)) {
            if court(s, i) >= 0 && can_secure(s, i, p) { out = push(out, [15, i, 0, 0]) }
        }
    }
    // Build
    let b_city = source_for(s, 9)
    if b_city > 0 {
        for sys in range(0, n_sys()) {
            if in_play(s, sys) && !is_gate(sys) && has_piece(s, sys, p) {
                let sl = free_slot(s, sys)
                if sl >= 0 {
                    if cities_left(s, p) > 0 { out = push(out, [9, sys, sl, 0]) }
                    if ports_left(s, p) > 0 { out = push(out, [10, sys, sl, 0]) }
                }
            }
            if ports_ready(s, sys, p) > 0 && reserve(s, p) > 0 { out = push(out, [11, sys, 0, 0]) }
        }
    }
    // Repair: one damaged piece
    if source_for(s, 13) > 0 {
        for sys in range(0, n_sys()) {
            if damaged(s, sys, p) > 0 { out = push(out, [13, sys, 0, 0]) }
            for sl in range(0, 2) {
                if slot_kind(s, sys, sl) != 0 && slot_owner(s, sys, sl) == p && slot_dmg(s, sys, sl) == 1 {
                    out = push(out, [13, sys, sl + 1, 0])
                }
            }
        }
    }
    // Move
    if source_for(s, 12) > 0 {
        for from in range(0, n_sys()) {
            let total = all_ships(s, from, p)
            if total > 0 {
                let dests = move_targets(s, from, p)
                for to in dests {
                    for cnt in range(1, total + 1) { out = push(out, [12, from, to, cnt]) }
                }
            }
        }
    }
    // Battle
    if source_for(s, 14) > 0 {
        for sys in range(0, n_sys()) {
            let ships = all_ships(s, sys, p)
            if ships > 0 {
                for e in range(0, n) {
                    if e != p && has_piece(s, sys, e) {
                        for x in battle_options(s, sys, e, ships) { out = push(out, x) }
                    }
                }
            }
        }
    }
    return out
}

fn can_secure(s, i, p) {
    let mine = agents_on(s, i, p)
    if mine == 0 { return false }
    for e in range(0, nplayers(s)) {
        if e != p && agents_on(s, i, e) >= mine { return false }
    }
    return true
}

// Starports of p in sys (any damage state counts as Loyal; damaged ones cannot
// build) that have not built a ship this turn.
fn ports_ready(s, sys, p) {
    let n = count_kind(s, sys, p, 2, 0)
    for k in range(0, 2) {
        if has_card(s[h_ported()], sys * 2 + k) { n = n - 1 }
    }
    return n
}

// Adjacent systems, or every system reachable by path from a Loyal starport (catapult).
fn move_targets(s, from, p) {
    let catapult = count_kind(s, from, p, 2, 0) > 0
    if !catapult { return neighbors(s, from) }
    let seen = []
    for i in range(0, n_sys()) { seen = push(seen, 0) }
    seen[from] = 1
    let frontier = [from]
    let out = []
    while len(frontier) > 0 {
        let next = []
        for a in frontier {
            for b in neighbors(s, a) {
                if seen[b] == 0 {
                    seen[b] = 1
                    next = push(next, b)
                    out = push(out, b)
                }
            }
        }
        frontier = next
    }
    return out
}

fn pack_dice(a, k, r) { return a * 49 + k * 7 + r }

fn battle_options(s, sys, e, ships) {
    let out = []
    let n = min_i(ships, 18)
    let raid_ok = count_buildings(s, sys, e) > 0 || buildings_anywhere(s, e) == 0
    for a in range(0, 7) {
        for k in range(0, 7) {
            let r = n - a - k
            if r >= 0 && r <= 6 {
                if r == 0 || raid_ok { out = push(out, [14, sys, e, pack_dice(a, k, r)]) }
            }
        }
    }
    return out
}

fn same_action(a, b) {
    return a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}

fn is_legal(s, a) {
    for x in legal_actions(s) {
        if same_action(x, a) { return true }
    }
    return false
}

// ---- applying -----------------------------------------------------------
fn set_event(t, kind, a, b, c, d) {
    t[h_ev()] = kind
    t[h_ev() + 1] = a
    t[h_ev() + 2] = b
    t[h_ev() + 3] = c
    t[h_ev() + 4] = d
    return t
}

fn remove_card(t, p, c) {
    t[pb(p) + 11] = hand(t, p) - pow2(c)
    t[pb(p) + 14] = c
    return t
}

fn begin_actions(t, suit, pips) {
    t[h_phase()] = 2
    t[h_act_suit()] = suit
    t[h_acts()] = pips
    t[h_acted()] = 0
    t[h_lore_used()] = 0
    t[h_taxed()] = 0
    t[h_ported()] = 0
    t[h_x_secure()] = 0
    return t
}

fn apply(s, a) {
    let t = s
    let p = cur(s)
    let kind = a[0]
    if kind == 0 {
        // lead a card, optionally declaring an ambition
        let c = a[1]
        t = remove_card(t, p, c)
        t[h_lead_suit()] = card_suit(c)
        t[h_lead_num()] = card_num(c)
        t[h_best_num()] = 0
        t[h_best_player()] = -1
        t[h_turns()] = 1
        t[h_passes()] = 0
        t[pb(p) + 18] = 0
        if a[2] >= 0 {
            // Declaring: the best free marker goes on the ambition and the
            // lead card's number becomes 0 (its pips are unchanged).
            let m = best_marker(t)
            let v = marker_pair(t, m)
            t[h_mk_taken()] = t[h_mk_taken()] + pow2(m)
            t[h_amb_count()] = s[h_amb_count()] + 1
            t[h_amb() + a[2]] = s[h_amb() + a[2]] + 1
            t[h_amb_first() + a[2]] = s[h_amb_first() + a[2]] + v[0]
            t[h_amb_second() + a[2]] = s[h_amb_second() + a[2]] + v[1]
            t[h_lead_num()] = 0
        }
        t = set_event(t, 0, p, c, a[2], 0)
        t = begin_actions(t, card_suit(c), card_pips(card_suit(c), card_num(c)))
        return settle(t)
    }
    if kind == 1 || kind == 2 || kind == 3 {
        let c = a[1]
        t = remove_card(t, p, c)
        t[h_turns()] = s[h_turns()] + 1
        t[pb(p) + 18] = kind
        if kind == 1 {
            if card_num(c) > s[h_best_num()] {
                t[h_best_num()] = card_num(c)
                t[h_best_player()] = p
            }
            if card_num(c) == 7 && nplayers(s) == 4 && s[h_seized()] == -1 { t[h_seized()] = p }
            t = begin_actions(t, s[h_lead_suit()], card_pips(card_suit(c), card_num(c)))
        } else if kind == 2 {
            t = begin_actions(t, s[h_lead_suit()], 1)
        } else {
            t = begin_actions(t, card_suit(c), 1)
        }
        t = set_event(t, kind, p, c, 0, 0)
        return settle(t)
    }
    if kind == 5 {
        // seize the initiative: an extra card face down, then play as usual
        t = remove_card(t, p, a[1])
        t[h_seized()] = p
        t[pb(p) + 19] = a[1]
        return set_event(t, 5, p, 0, 0, 0)
    }
    if kind == 4 {
        // Pass the initiative: it goes to the next player with cards and the
        // round ends. If everyone with cards passes in a row, the chapter ends.
        t = set_event(t, 4, p, 0, 0, 0)
        t[h_passes()] = s[h_passes()] + 1
        let holders = 0
        for q in range(0, nplayers(t)) {
            if hand(t, q) != 0 { holders = holders + 1 }
        }
        if t[h_passes()] >= holders { return end_chapter(t) }
        let nx = next_with_cards(t, p)
        if nx < 0 { return end_chapter(t) }
        t[h_init()] = nx
        return start_round(t)
    }
    if kind == 6 {
        let r = a[1]
        t[pb(p) + 1 + r] = res(s, p, r) - 1
        t[supply_base() + r] = supply(s, r) + 1
        if r == 0 { t[h_x_build()] = s[h_x_build()] + 1 }
        if r == 1 { t[h_x_move()] = s[h_x_move()] + 1 }
        if r == 2 { t[h_x_battle()] = 1 }
        if r == 3 { t[h_x_secure()] = s[h_x_secure()] + 1 }
        if r == 4 { t[h_x_lead()] = s[h_x_lead()] + 1 }
        return set_event(t, 6, p, r, 0, 0)
    }
    if kind == 7 {
        t[h_acts()] = 0
        t[h_x_build()] = 0
        t[h_x_move()] = 0
        t[h_x_battle()] = 0
        t[h_x_lead()] = 0
        t[h_x_secure()] = 0
        t = set_event(t, 7, p, 0, 0, 0)
        return advance_turn(t)
    }

    if kind == 16 {
        // Lore: one more action of the current suit, once per turn.
        if lore(s, p) <= 0 || s[h_lore_used()] == 1 { return t }
        t[pb(p) + 17] = lore(s, p) - 1
        t[h_acts()] = s[h_acts()] + 1
        t[h_lore_used()] = 1
        return set_event(t, 16, p, lore(t, p), 0, 0)
    }
    // Real actions: pay with a pip or a prelude extra.
    let src = source_for(s, kind)
    if src == 1 {
        t[h_acts()] = s[h_acts()] - 1
    } else {
        let g = kind_group(kind)
        if g == 0 && s[h_x_build()] > 0 { t[h_x_build()] = s[h_x_build()] - 1 }
        else if g == 1 && s[h_x_move()] > 0 { t[h_x_move()] = s[h_x_move()] - 1 }
        else if g == 4 && s[h_x_secure()] > 0 { t[h_x_secure()] = s[h_x_secure()] - 1 }
        else { t[h_x_lead()] = s[h_x_lead()] - 1 }
    }
    if src == 1 { t[h_acted()] = 1 }
    if kind == 8 { t = do_tax(t, p, a[1], a[2]) }
    else if kind == 15 { t = do_secure(t, p, a[1]) }
    else if kind == 17 { t = do_influence(t, p, a[1]) }
    else if kind == 9 || kind == 10 { t = do_build_building(t, p, a[1], a[2], kind) }
    else if kind == 11 { t = do_build_ships(t, p, a[1]) }
    else if kind == 12 { t = do_move(t, p, a[1], a[2], a[3]) }
    else if kind == 13 { t = do_repair(t, p, a[1], a[2]) }
    else if kind == 14 { t = do_battle(t, p, a[1], a[2], a[3]) }
    return settle(t)
}

// Tax a city: gain its planet's resource; taxing a Rival city also captures
// one of that Rival's agents from their supply.
fn do_tax(t, p, sys, sl) {
    let ty = planet_type(sys)
    let n = min_i(1, supply(t, ty))
    if has_court(t, p, court_envoy()) && supply(t, ty) >= 2 { n = 2 }
    t[pb(p) + 1 + ty] = res(t, p, ty) + n
    t[supply_base() + ty] = supply(t, ty) - n
    t[h_taxed()] = t[h_taxed()] + pow2(sys * 2 + sl)
    let o = slot_owner(t, sys, sl)
    if o >= 0 && o != p && agents_left(t, o) > 0 {
        t[cap_base() + p * 4 + o] = cap_of(t, p, o) + 1
        t[pb(p) + 10] = captives(t, p) + 1
    }
    return set_event(t, 8, p, sys, ty, n)
}

fn do_influence(t, p, i) {
    let n = 1
    if is_leader(t, p, 4) && agents_left(t, p) >= 2 { n = 2 }    // Pilgrim
    t[agents_base() + i * 4 + p] = agents_on(t, i, p) + n
    return set_event(t, 17, p, i, court(t, i), n)
}

fn do_secure(t, p, i) {
    let c = court(t, i)
    let taken = 0
    for e in range(0, nplayers(t)) {
        let k = agents_on(t, i, e)
        if e != p && k > 0 {
            t[cap_base() + p * 4 + e] = cap_of(t, p, e) + k
            t[pb(p) + 10] = captives(t, p) + k
            taken = taken + k
        }
        t[agents_base() + i * 4 + e] = 0
    }
    t[pb(p) + 15] = secured(t, p) + pow2(c)
    t = refill_court(t, i)
    return set_event(t, 15, p, i, c, taken)
}

fn build_dmg(t, sys, p) {
    if is_leader(t, p, 2) { return 0 }    // Architect: builds are never damaged
    let c = controller(t, sys)
    if c >= 0 && c != p { return 1 }
    return 0
}

fn do_build_building(t, p, sys, sl, kind) {
    let d = build_dmg(t, sys, p)
    t[slot_i(sys, sl)] = p
    t[slot_i(sys, sl) + 1] = kind - 8        // 9 -> 1 city, 10 -> 2 starport
    t[slot_i(sys, sl) + 2] = d
    if kind == 9 { t[pb(p) + 6] = cities_left(t, p) - 1 } else { t[pb(p) + 7] = ports_left(t, p) - 1 }
    return set_event(t, 9, p, sys, kind - 8, d)
}

// One ship at a Loyal starport; each starport builds once per turn.
fn do_build_ships(t, p, sys) {
    let d = build_dmg(t, sys, p)
    let k = 0
    if has_card(t[h_ported()], sys * 2) { k = 1 }
    t[h_ported()] = t[h_ported()] + pow2(sys * 2 + k)
    let want = 1
    if is_leader(t, p, 0) { want = want + 1 }    // Broker: one more ship
    let n = min_i(want, reserve(t, p))
    t[ship_i(sys, p, d)] = t[ship_i(sys, p, d)] + n
    t[pb(p) + 8] = reserve(t, p) - n
    return set_event(t, 11, p, sys, n, d)
}

fn do_move(t, p, from, to, cnt) {
    let f = fresh(t, from, p)
    let mf = min_i(cnt, f)
    let md = cnt - mf
    t[ship_i(from, p, 0)] = f - mf
    t[ship_i(from, p, 1)] = damaged(t, from, p) - md
    t[ship_i(to, p, 0)] = fresh(t, to, p) + mf
    t[ship_i(to, p, 1)] = damaged(t, to, p) + md
    return set_event(t, 12, p, from, to, cnt)
}

fn do_repair(t, p, sys, what) {
    if has_court(t, p, court_writ()) {
        // Writ: every damaged piece of yours here is repaired.
        t[ship_i(sys, p, 0)] = fresh(t, sys, p) + damaged(t, sys, p)
        t[ship_i(sys, p, 1)] = 0
        for sl in range(0, 2) {
            if slot_kind(t, sys, sl) != 0 && slot_owner(t, sys, sl) == p { t[slot_i(sys, sl) + 2] = 0 }
        }
        return set_event(t, 13, p, sys, what, 1)
    }
    if what == 0 {
        t[ship_i(sys, p, 1)] = damaged(t, sys, p) - 1
        t[ship_i(sys, p, 0)] = fresh(t, sys, p) + 1
    } else {
        t[slot_i(sys, what - 1) + 2] = 0
    }
    return set_event(t, 13, p, sys, what, 0)
}

// ---- battle -------------------------------------------------------------
// Returns the state with p's pieces in `sys` hit `n` times: damaged ships are
// destroyed first when `kill_first`, otherwise fresh ones are damaged first.
// Destroyed ships are lost (they become a trophy for `taker`, -1 for none).
fn hit_ships(t, sys, p, n, kill_first, taker) {
    let left = n
    let lost = 0
    while left > 0 && all_ships(t, sys, p) > 0 {
        let did = false
        if kill_first {
            if damaged(t, sys, p) > 0 {
                t[ship_i(sys, p, 1)] = damaged(t, sys, p) - 1
                lost = lost + 1
                did = true
            } else {
                t[ship_i(sys, p, 0)] = fresh(t, sys, p) - 1
                t[ship_i(sys, p, 1)] = damaged(t, sys, p) + 1
                did = true
            }
        } else {
            if fresh(t, sys, p) > 0 {
                t[ship_i(sys, p, 0)] = fresh(t, sys, p) - 1
                t[ship_i(sys, p, 1)] = damaged(t, sys, p) + 1
                did = true
            } else {
                t[ship_i(sys, p, 1)] = damaged(t, sys, p) - 1
                lost = lost + 1
                did = true
            }
        }
        left = left - 1
    }
    if taker >= 0 { t[pb(taker) + 9] = trophies(t, taker) + lost }
    t[h_spill()] = t[h_spill()] + left
    return t
}

// Building hits: each hit damages a fresh building, or destroys a damaged one.
fn hit_buildings(t, sys, p, n, taker) {
    let left = n
    while left > 0 && count_buildings(t, sys, p) > 0 {
        let done = false
        for sl in range(0, 2) {
            if !done && slot_kind(t, sys, sl) != 0 && slot_owner(t, sys, sl) == p && slot_dmg(t, sys, sl) == 0 {
                t[slot_i(sys, sl) + 2] = 1
                done = true
            }
        }
        if !done {
            for sl in range(0, 2) {
                if !done && slot_kind(t, sys, sl) != 0 && slot_owner(t, sys, sl) == p {
                    let was_city = slot_kind(t, sys, sl) == 1
                    t[slot_i(sys, sl)] = -1
                    t[slot_i(sys, sl) + 1] = 0
                    t[slot_i(sys, sl) + 2] = 0
                    // Destroyed pieces are trophies (captives come from taxing and securing).
                    if taker >= 0 { t[pb(taker) + 9] = trophies(t, taker) + 1 }
                    if was_city { t = outrage(t, p, planet_type(sys)) }
                    done = true
                }
            }
        }
        left = left - 1
    }
    return t
}

// A destroyed city: its owner discards that type's resources and is outraged.
fn outrage(t, p, ty) {
    let n = res(t, p, ty)
    t[pb(p) + 1 + ty] = 0
    t[supply_base() + ty] = supply(t, ty) + n
    if !outraged(t, p, ty) { t[pb(p) + 13] = t[pb(p) + 13] + pow2(ty) }
    return t
}

fn do_battle(s, p, sys, e, packed) {
    let t = s
    let assault = packed / 49
    let skirm = (packed / 7) % 7
    let raid = packed % 7
    let selfh = 0
    let inter = 0
    let hits = 0
    let bhits = 0
    let keys = 0
    for k in range(0, 3) {
        let cnt = [assault, skirm, raid][k]
        for i in range(0, cnt) {
            t = draw(t)
            let f = die_face(k, roll_d6(t))
            selfh = selfh + f[0]
            inter = inter + f[1]
            hits = hits + f[2]
            bhits = bhits + f[3]
            keys = keys + f[4]
        }
    }
    t[h_spill()] = 0
    if has_court(t, p, court_pact()) { selfh = max_i(0, selfh - 1) }
    if is_leader(t, p, 1) && hits > 0 { hits = hits + 1 }    // Admiral: one more ship hit
    // 1. self hits (the attacker minimises its losses: damage fresh ships first)
    t = hit_ships(t, sys, p, selfh, false, e)
    // 2. one round of interception: each intercept lets a fresh defender ship hit back
    let back = min_i(inter, fresh(t, sys, e))
    t = hit_ships(t, sys, p, back, false, e)
    // 3. ship hits, spilling to buildings once the defender has no ships left
    let ships_before = all_ships(t, sys, e)
    t = hit_ships(t, sys, e, hits, true, p)
    let spill = t[h_spill()]
    t[h_spill()] = 0
    // 4. building hits (plus any spill)
    let bh = bhits + spill
    if has_court(t, e, court_seal()) { bh = max_i(0, bh - 1) }
    t = hit_buildings(t, sys, e, bh, p)
    // 5. raids: each key steals the defender's most plentiful resource
    let stolen = 0
    let takes = keys
    if is_leader(t, p, 5) { takes = keys * 2 }    // Corsair: each key takes two
    for k in range(0, takes) {
        let best = -1
        let bv = 0
        for ty in range(0, 5) {
            if res(t, e, ty) > bv { bv = res(t, e, ty)
                best = ty }
        }
        if best >= 0 {
            t[pb(e) + 1 + best] = res(t, e, best) - 1
            t[pb(p) + 1 + best] = res(t, p, best) + 1
            stolen = stolen + 1
        }
    }
    t[h_spill()] = 0
    return set_event(t, 14, p, sys, e, stolen)
}

// ---- turn flow ----------------------------------------------------------
// After any change: end the turn when no actions remain or none are possible.
fn settle(s) {
    if phase(s) != 2 { return s }
    let any = len(action_list(s)) > 0
    if !any && s[h_acted()] == 1 { return advance_turn(clear_extras(s)) }
    if !any && acts(s) == 0 && extras_left(s) == 0 {
        return advance_turn(s)
    }
    if acts(s) == 0 && extras_left(s) == 0 && s[h_acted()] == 1 {
        return advance_turn(s)
    }
    return s
}

fn clear_extras(s) {
    let t = s
    t[h_acts()] = 0
    t[h_x_build()] = 0
    t[h_x_move()] = 0
    t[h_x_battle()] = 0
    t[h_x_lead()] = 0
    t[h_x_secure()] = 0
    return t
}

// Next player clockwise from p (not p) who holds cards, or -1.
fn next_with_cards(s, p) {
    let n = nplayers(s)
    for k in range(1, n) {
        let q = (p + k) % n
        if hand(s, q) != 0 { return q }
    }
    return -1
}

// Hand the turn to the next player clockwise who has cards; once play is
// back round to the lead player, close the round.
fn advance_turn(s) {
    let t = clear_extras(s)
    let n = nplayers(t)
    let lead = t[h_init()]
    let q = (cur(t) + 1) % n
    while q != lead && hand(t, q) == 0 { q = (q + 1) % n }
    if q == lead { return end_round(t) }
    t[h_cur()] = q
    t[h_phase()] = 1
    t[h_acted()] = 0
    return t
}

fn all_hands_empty(s) {
    for p in range(0, nplayers(s)) {
        if hand(s, p) != 0 { return false }
    }
    return true
}

fn end_round(s) {
    let t = s
    let next = t[h_init()]
    if t[h_seized()] >= 0 {
        next = t[h_seized()]
    } else if t[h_best_player()] >= 0 {
        next = t[h_best_player()]
    }
    t[h_init()] = next
    t = set_event(t, 20, next, 0, 0, 0)
    if all_hands_empty(t) { return end_chapter(t) }
    // an initiative holder with no cards passes it to the next player with cards
    if hand(t, t[h_init()]) == 0 { t[h_init()] = next_with_cards(t, t[h_init()]) }
    return start_round(t)
}

// ---- ambitions and chapters --------------------------------------------
// 0 Tycoon (Fuel + Material), 1 Tyrant (captives), 2 Warlord (trophies),
// 3 Keeper (Relics), 4 Empath (Psionic).
// Resources and court card icons both count.
fn amb_tally(s, p, a) {
    if a == 0 { return res(s, p, 0) + res(s, p, 1) + court_icons(s, p, 0) + court_icons(s, p, 1) }
    if a == 1 { return captives(s, p) }
    if a == 2 { return trophies(s, p) }
    if a == 3 { return res(s, p, 3) + court_icons(s, p, 3) }
    return res(s, p, 4) + court_icons(s, p, 4)
}

// Bonus city power to a sole first place: +2 with 4 cities built, +5 with all 5.
fn city_bonus(s, p) {
    let built = 5 - cities_left(s, p)
    if built >= 5 { return 5 }
    if built >= 4 { return 2 }
    return 0
}

// Score one ambition with every marker on it. Ties for first take the second
// value and nobody else places; a tie for second scores nothing; you need at
// least one of the thing counted to place.
fn score_ambition(s, a) {
    let t = s
    if s[h_amb() + a] == 0 { return t }
    let first_v = s[h_amb_first() + a]
    let second_v = s[h_amb_second() + a]
    let n = nplayers(t)
    let top = 0
    for p in range(0, n) { top = max_i(top, amb_tally(t, p, a)) }
    if top == 0 { return t }
    let at_top = 0
    for p in range(0, n) {
        if amb_tally(t, p, a) == top { at_top = at_top + 1 }
    }
    let second = 0
    for p in range(0, n) {
        let m = amb_tally(t, p, a)
        if m < top { second = max_i(second, m) }
    }
    let at_second = 0
    for p in range(0, n) {
        if second > 0 && amb_tally(t, p, a) == second { at_second = at_second + 1 }
    }
    for p in range(0, n) {
        let m = amb_tally(t, p, a)
        if m == top {
            if at_top == 1 {
                t[pb(p)] = power(t, p) + first_v + city_bonus(t, p)
            } else {
                t[pb(p)] = power(t, p) + second_v
            }
        } else if at_top == 1 && at_second == 1 && m == second {
            t[pb(p)] = power(t, p) + second_v
        }
    }
    return t
}

fn end_chapter(s) {
    let t = s
    for a in range(0, 5) { t = score_ambition(t, a) }
    for p in range(0, nplayers(t)) {
        if has_court(t, p, court_accord()) { t[pb(p)] = power(t, p) + 1 }
        if is_leader(t, p, 3) && res(t, p, 4) > 0 { t[pb(p)] = power(t, p) + 1 }    // Seer
    }
    t = grant_lore(t)
    t = set_event(t, 21, chapter(t), 0, 0, 0)
    // Clean up: scored Warlord returns trophies, scored Tyrant returns captives.
    if s[h_amb() + 2] > 0 {
        for p in range(0, nplayers(t)) { t[pb(p) + 9] = 0 }
    }
    if s[h_amb() + 1] > 0 {
        for p in range(0, nplayers(t)) { t[pb(p) + 10] = 0 }
        for i in range(0, 16) { t[cap_base() + i] = 0 }
    }
    for a in range(0, 5) {
        t[h_amb() + a] = 0
        t[h_amb_first() + a] = 0
        t[h_amb_second() + a] = 0
    }
    t[h_amb_count()] = 0
    t[h_mk_taken()] = 0
    t[h_passes()] = 0
    t[h_flips()] = min_i(3, s[h_flips()] + 1)
    for p in range(0, nplayers(t)) { t[pb(p) + 13] = 0 }
    let goal = power_goal(nplayers(t))
    let hit = false
    for p in range(0, nplayers(t)) {
        if power(t, p) >= goal { hit = true }
    }
    if hit || chapter(t) >= 5 {
        t[h_phase()] = 4
        return t
    }
    t[h_chapter()] = chapter(t) + 1
    return deal_chapter(t)
}

// The player(s) with the least power gain one Lore, unless everyone is level.
fn grant_lore(s) {
    let t = s
    let lo = power(t, 0)
    let hi = power(t, 0)
    for p in range(1, nplayers(t)) {
        lo = min_i(lo, power(t, p))
        hi = max_i(hi, power(t, p))
    }
    if lo == hi { return t }
    for p in range(0, nplayers(t)) {
        if power(t, p) == lo && lore(t, p) < lore_cap() { t[pb(p) + 17] = lore(t, p) + 1 }
    }
    return t
}

fn is_over(s) { return phase(s) == 4 }

// Highest power; a tie goes to the tied player earliest in turn order from
// the initiative holder.
fn winner(s) {
    let n = nplayers(s)
    let best = s[h_init()]
    for k in range(1, n) {
        let p = (s[h_init()] + k) % n
        if power(s, p) > power(s, best) { best = p }
    }
    return best
}

// ---- bot ----------------------------------------------------------------
fn score_action(s, a) {
    let p = cur(s)
    let k = a[0]
    if k == 0 {
        let c = a[1]
        let v = card_pips(card_suit(c), card_num(c)) * 10 + card_num(c)
        if a[2] >= 0 { v = v + 25 }
        return v
    }
    if k == 1 { return 60 + card_num(a[1]) }
    if k == 2 { return 20 }
    if k == 3 { return 30 }
    if k == 4 { return 0 }
    if k == 5 { return 5 }
    if k == 6 {
        if a[1] == 2 { return 3 }
        return 12
    }
    if k == 15 { return 85 }
    if k == 17 {
        // contest a card we do not lead yet; do not pile on one we already lead
        if can_secure(s, a[1], p) { return 6 }
        let mine = agents_on(s, a[1], p)
        let best = 0
        for e in range(0, nplayers(s)) {
            if e != p { best = max_i(best, agents_on(s, a[1], e)) }
        }
        if best - mine > 2 || mine >= 3 { return 8 }
        return 36 + mine * 2
    }
    if k == 16 { if acts(s) > 0 { return 50 } return 2 }
    if k == 7 { return 1 }
    if k == 8 { return 40 + (5 - supply(s, planet_type(a[1]))) }
    if k == 9 { return 90 }
    if k == 10 { return 80 }
    if k == 11 { return 70 }
    if k == 13 { return 35 }
    if k == 12 {
        let to = a[2]
        let v = 8 + a[3]
        if free_slot(s, to) >= 0 && !is_gate(to) { v = v + 12 }
        for e in range(0, nplayers(s)) {
            if e != p && all_ships(s, to, e) > 0 {
                if fresh(s, to, e) < a[3] { v = v + 45 } else { v = v - 25 }
            }
        }
        if fresh(s, a[1], p) == a[3] && count_buildings(s, a[1], p) > 0 { v = v - 30 }
        return v
    }
    if k == 14 {
        let e = a[2]
        let mine = all_ships(s, a[1], p)
        let theirs = all_ships(s, a[1], e)
        let v = 10
        if mine > theirs { v = 75 }
        if mine > theirs * 2 { v = 95 }
        let dice = a[3]
        let assault = dice / 49
        v = v - assault * 2
        return v
    }
    return 1
}

fn bot_choose(s) {
    let opts = legal_actions(s)
    let t = draw(s)
    let best = opts[0]
    let best_v = -1000000
    for a in opts {
        t = draw(t)
        let v = score_action(s, a) * 10 + rng_val(t[h_rng()]) % 10
        if v > best_v {
            best_v = v
            best = a
        }
    }
    return best
}

fn bot_game(n, seed, setup_idx) {
    let s = new_game(n, seed, setup_idx)
    let steps = 0
    while !is_over(s) && steps < 6000 {
        s = apply(s, bot_choose(s))
        steps = steps + 1
    }
    return s
}
