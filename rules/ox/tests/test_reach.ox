import "../reach.ox"

fn count_pieces(s, p) {
    let total = reserve(s, p)
    for sys in range(0, n_sys()) { total = total + all_ships(s, sys, p) }
    return total
}

fn test_setup_all_cards() {
    for n in [2, 3, 4] {
        for k in range(0, setup_count(n)) {
            let s = new_game(n, 5 + k, k)
            assert_eq(nplayers(s), n)
            for p in range(0, n) {
                assert_eq(count_pieces(s, p), 15)
                assert_eq(cities_left(s, p), 4)
                assert_eq(ports_left(s, p), 4)
                assert_eq(len(cards_in_hand(s, p)), 6)
            }
        }
    }
}

fn test_adjacency_symmetric() {
    for n in [2, 3, 4] {
        let s = new_game(n, 3, 0)
        for a in range(0, n_sys()) {
            for b in neighbors(s, a) { assert(adjacent(s, b, a)) }
        }
    }
}

fn test_out_of_play_not_adjacent() {
    let s = new_game(2, 3, 0)
    for a in range(0, n_sys()) {
        if !in_play(s, a) { assert_eq(len(neighbors(s, a)), 0) }
        for b in neighbors(s, a) { assert(in_play(s, b)) }
    }
}

fn test_deal_sizes() {
    let s = new_game(4, 9, 0)
    let seen = []
    for i in range(0, 28) { seen = push(seen, 0) }
    for p in range(0, 4) { for c in cards_in_hand(s, p) { seen[c] = seen[c] + 1 } }
    for c in seen { assert(c <= 1) }
    let s2 = new_game(2, 9, 0)
    for p in range(0, 2) {
        for c in cards_in_hand(s2, p) { assert(card_num(c) >= 2 && card_num(c) <= 6) }
    }
}

fn test_lead_then_follow() {
    let s = new_game(3, 11, 0)
    assert_eq(phase(s), 0)
    let opts = legal_actions(s)
    assert(len(opts) > 1)
    let lead = opts[0]
    assert_eq(lead[0], 0)
    let t = apply(s, lead)
    assert(phase(t) == 2 || phase(t) == 1)
}

fn test_apply_does_not_mutate() {
    let s = new_game(3, 11, 0)
    let before = len(s)
    let t = apply(s, legal_actions(s)[0])
    assert_eq(len(s), before)
    assert_eq(phase(s), 0)
}

fn test_bots_finish() {
    for n in [2, 3, 4] {
        for seed in [1, 2, 3, 4] {
            let s = bot_game(n, seed, seed)
            assert(is_over(s))
            let w = winner(s)
            assert(w >= 0 && w < n)
        }
    }
}

fn test_random_play_terminates() {
    for n in [2, 3, 4] {
        let s = new_game(n, 77 + n, 0)
        let steps = 0
        while !is_over(s) && steps < 8000 {
            let opts = legal_actions(s)
            assert(len(opts) > 0)
            s = draw(s)
            let a = opts[rng_val(s[h_rng()]) % len(opts)]
            s = apply(s, a)
            steps = steps + 1
        }
        assert(is_over(s))
    }
}

fn test_piece_conservation() {
    let s = new_game(3, 21, 0)
    let steps = 0
    while !is_over(s) && steps < 400 {
        s = apply(s, bot_choose(s))
        steps = steps + 1
        for p in range(0, 3) {
            let on_board = 0
            for sys in range(0, n_sys()) { on_board = on_board + all_ships(s, sys, p) }
            assert(on_board + reserve(s, p) <= 15)
        }
    }
}

// The capture path: a destroyed city is a trophy and a captive; a destroyed
// starport is only a trophy. The rest of the defender's slots are cleared so
// the hit lands on the piece under test.
fn test_destroyed_city_takes_a_captive() {
    let s = new_game(2, 11, 0)
    let sys = -1
    let slot = -1
    for i in range(0, n_sys()) {
        for sl in range(0, 2) {
            if sys < 0 && slot_kind(s, i, sl) == 1 && slot_owner(s, i, sl) == 0 {
                sys = i
                slot = sl
            }
        }
    }
    assert(sys >= 0)
    let t = s
    for sl in range(0, 2) {
        if sl != slot && slot_kind(t, sys, sl) != 0 {
            t[slot_i(sys, sl)] = -1
            t[slot_i(sys, sl) + 1] = 0
            t[slot_i(sys, sl) + 2] = 0
        }
    }
    t[slot_i(sys, slot) + 2] = 1
    let before_t = trophies(t, 1)
    let before_c = captives(t, 1)
    let after = hit_buildings(t, sys, 0, 1, 1)
    assert_eq(slot_kind(after, sys, slot), 0)
    assert_eq(trophies(after, 1), before_t + 1)
    assert_eq(captives(after, 1), before_c + 1)
}

fn test_court_row_starts_full() {
    for n in [2, 3, 4] {
        let s = new_game(n, 11, 0)
        for i in range(0, 5) { assert_eq(court(s, i), i) }
        for p in range(0, n) { assert_eq(secured(s, p), 0) }
    }
}

fn test_court_secure_spends_a_relic_and_scores_one_power() {
    let s = new_game(3, 11, 0)
    let t = s
    t[h_cur()] = 0
    t[h_phase()] = 2
    t[h_acts()] = 0
    t[pb(0) + 1 + 3] = 1
    let a = [15, 0, 0, 0]
    assert(is_legal(t, a))
    let before_power = power(t, 0)
    let before_bank = supply(t, 3)
    let u = apply(t, a)
    assert_eq(court(u, 0), -1)
    assert_eq(res(u, 0, 3), 0)
    assert_eq(supply(u, 3), before_bank + 1)
    assert_eq(power(u, 0), before_power + 1)
    assert_eq(secured(u, 0), pow2(0))
}

fn test_court_secure_needs_a_relic_and_a_card() {
    let s = new_game(3, 11, 0)
    let t = s
    t[h_cur()] = 0
    t[h_phase()] = 2
    t[h_acts()] = 0
    t[pb(0) + 1 + 3] = 0
    assert(!is_legal(t, [15, 0, 0, 0]))
    t[pb(0) + 1 + 3] = 1
    t[h_court()] = -1
    assert(!is_legal(t, [15, 0, 0, 0]))
    assert(is_legal(t, [15, 1, 0, 0]))
}

fn test_destroyed_starport_is_only_a_trophy() {
    let s = new_game(2, 11, 0)
    let sys = -1
    let slot = -1
    for i in range(0, n_sys()) {
        for sl in range(0, 2) {
            if sys < 0 && slot_kind(s, i, sl) == 2 && slot_owner(s, i, sl) == 0 {
                sys = i
                slot = sl
            }
        }
    }
    assert(sys >= 0)
    let t = s
    for sl in range(0, 2) {
        if sl != slot && slot_kind(t, sys, sl) != 0 {
            t[slot_i(sys, sl)] = -1
            t[slot_i(sys, sl) + 1] = 0
            t[slot_i(sys, sl) + 2] = 0
        }
    }
    t[slot_i(sys, slot) + 2] = 1
    let before_t = trophies(t, 1)
    let before_c = captives(t, 1)
    let after = hit_buildings(t, sys, 0, 1, 1)
    assert_eq(slot_kind(after, sys, slot), 0)
    assert_eq(trophies(after, 1), before_t + 1)
    assert_eq(captives(after, 1), before_c)
}

// ---- leaders, court effects, lore -----------------------------------------

fn act_as(s, p) {
    let t = s
    t[h_cur()] = p
    t[h_phase()] = 2
    t[h_acts()] = 0
    t[h_acted()] = 0
    t[h_lore_used()] = 0
    return t
}

fn give_court(s, p, c) {
    let t = s
    t[pb(p) + 15] = secured(t, p) + pow2(c)
    t[h_court() + c] = -1
    return t
}

fn test_leaders_are_distinct_and_start_with_their_resource() {
    for n in [2, 3, 4] {
        for seed in [3, 8, 21] {
            let s = new_game(n, seed, 0)
            let total = 0
            for p in range(0, n) {
                let l = leader(s, p)
                assert(l >= 0 && l < n_leaders())
                for q in range(0, n) { if q != p { assert(leader(s, q) != l) } }
                assert(res(s, p, leader_res(l)) >= 1)
                for t in range(0, 5) { total = total + res(s, p, t) }
            }
            for t in range(0, 5) { total = total + supply(s, t) }
            assert_eq(total, 25)
        }
    }
}

fn test_battle_does_not_touch_the_court_row() {
    let s = new_game(2, 11, 0)
    let t = give_court(s, 1, 0)
    // put both fleets in one system and attack
    let sys = 10
    t[ship_i(sys, 0, 0)] = 6
    t[ship_i(sys, 1, 0)] = 3
    let u = do_battle(t, 0, sys, 1, pack_dice(0, 6, 0))
    assert_eq(court(u, 0), -1)
    for i in range(1, 5) { assert_eq(court(u, i), i) }
}

fn test_envoy_taxes_two() {
    let s = new_game(2, 11, 0)
    let sys = -1
    for i in range(0, n_sys()) {
        for sl in range(0, 2) { if sys < 0 && slot_kind(s, i, sl) == 1 && slot_owner(s, i, sl) == 0 { sys = i } }
    }
    let ty = planet_type(sys)
    let plain = do_tax(s, 0, sys)
    assert_eq(res(plain, 0, ty), res(s, 0, ty) + 1)
    let t = give_court(s, 0, court_envoy())
    let u = do_tax(t, 0, sys)
    assert_eq(res(u, 0, ty), res(s, 0, ty) + 2)
    assert_eq(supply(u, ty), supply(s, ty) - 2)
}

fn test_writ_repairs_everything_in_the_system() {
    let s = new_game(2, 11, 0)
    let t = give_court(s, 0, court_writ())
    t[ship_i(10, 0, 0)] = 1
    t[ship_i(10, 0, 1)] = 3
    let u = do_repair(t, 0, 10, 0)
    assert_eq(fresh(u, 10, 0), 4)
    assert_eq(damaged(u, 10, 0), 0)
    // without the Writ, one ship at a time
    let w = t
    w[pb(0) + 15] = 0
    let v = do_repair(w, 0, 10, 0)
    assert_eq(damaged(v, 10, 0), 2)
}

fn test_pilgrim_secures_with_psionic() {
    let s = new_game(3, 11, 0)
    let t = act_as(s, 0)
    t[pb(0) + 16] = 4
    t[pb(0) + 1 + 3] = 0
    t[pb(0) + 1 + 4] = 1
    assert(is_legal(t, [15, 2, 4, 0]))
    assert(!is_legal(t, [15, 2, 0, 0]))
    let u = apply(t, [15, 2, 4, 0])
    assert_eq(res(u, 0, 4), 0)
    assert(has_court(u, 0, 2))
    t[pb(0) + 16] = 0
    assert(!is_legal(t, [15, 2, 4, 0]))
}

fn test_lore_goes_to_the_trailing_player_and_buys_an_action() {
    let s = new_game(2, 11, 0)
    let t = s
    t[pb(0)] = 7
    t[pb(1)] = 2
    let u = grant_lore(t)
    assert_eq(lore(u, 0), 0)
    assert_eq(lore(u, 1), 1)
    let level = grant_lore(s)
    assert_eq(lore(level, 0), 0)
    let a = act_as(u, 1)
    a[h_acts()] = 1
    assert(is_legal(a, [16, 0, 0, 0]))
    let b = apply(a, [16, 0, 0, 0])
    assert_eq(acts(b), 2)
    assert_eq(lore(b, 1), 0)
    assert(!is_legal(b, [16, 0, 0, 0]))
}

fn test_accord_scores_at_chapter_end() {
    let s = new_game(2, 11, 0)
    let base = s
    base[pb(0) + 16] = 0
    base[pb(1) + 16] = 0
    let t = give_court(base, 0, court_accord())
    let u = end_chapter(t)
    let v = end_chapter(base)
    assert_eq(power(u, 0) - power(v, 0), 1)
}
