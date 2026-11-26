import "../reach.ox"

fn test_new_game_layout() {
    let s = new_game(3, 42)
    assert_eq(len(s), 79)
    assert_eq(nplayers(s), 3)
    assert_eq(phase(s), 0)
    assert_eq(power(s, 0), 3)
    assert_eq(ships(s, 1, 0), 3)
    assert_eq(city(s, 1), 0)
    assert_eq(city(s, 3), 1)
    assert_eq(city(s, 0), -1)
}

fn test_hands_are_six_distinct_cards() {
    let s = new_game(4, 7)
    let seen = []
    for i in range(0, 28) { seen = push(seen, 0) }
    for p in range(0, 4) {
        assert_eq(len(cards_in_hand(s, p)), 6)
        for c in cards_in_hand(s, p) { seen[c] = seen[c] + 1 }
    }
    for c in seen { assert(c <= 1) }
}

fn test_adjacency() {
    assert(adjacent(0, 3))
    assert(adjacent(1, 2))
    assert(adjacent(6, 1))
    assert(!adjacent(1, 3))
    assert(!adjacent(4, 4))
}

fn test_lead_offers_whole_hand() {
    let s = new_game(2, 1)
    assert_eq(len(legal_actions(s)), 6)
    for a in legal_actions(s) { assert_eq(a[0], 0) }
}

fn test_follow_must_match_suit_when_possible() {
    let s = new_game(2, 5)
    let lead = legal_actions(s)[0]
    let t = apply(s, lead)
    // finish leader's actions
    while phase(t) == 2 && cur(t) == 0 { t = apply(t, [6, 0, 0]) }
    assert_eq(phase(t), 1)
    let suit = card_suit(lead[1])
    let have = 0
    for c in cards_in_hand(t, 1) { if card_suit(c) == suit { have = have + 1 } }
    let opts = legal_actions(t)
    if have > 0 {
        assert_eq(len(opts), have)
        for a in opts { assert_eq(card_suit(a[1]), suit) }
    } else {
        assert_eq(len(opts), 6)
    }
}

fn test_is_legal() {
    let s = new_game(2, 9)
    assert(is_legal(s, legal_actions(s)[0]))
    assert(!is_legal(s, [3, 1, 0]))
    assert(!is_legal(s, [0, 99, 0]))
}

fn test_apply_does_not_mutate_input() {
    let s = new_game(2, 3)
    let copy = s
    let t = apply(s, legal_actions(s)[0])
    assert_eq(s, copy)
    assert(t != s)
}

fn test_move_moves_up_to_two() {
    let s = new_game(2, 1)
    s[h_phase()] = 2
    s[h_asuit()] = 0
    s[h_acts()] = 2
    let t = apply(s, [1, 1, 2])
    assert_eq(ships(t, 1, 0), 1)
    assert_eq(ships(t, 2, 0), 2)
    assert_eq(acts(t), 1)
}

fn test_build_costs_power_and_places_city() {
    let s = new_game(2, 1)
    s[h_phase()] = 2
    s[h_asuit()] = 2
    s[h_acts()] = 2
    s[ships_base() + 2 * 4 + 0] = 1
    let t = apply(s, [3, 2, 0])
    assert_eq(city(t, 2), 0)
    assert_eq(power(t, 0), 1)
}

fn test_levy_moves_pool_to_system() {
    let s = new_game(2, 1)
    s[h_phase()] = 2
    s[h_asuit()] = 2
    s[h_acts()] = 2
    let t = apply(s, [4, 1, 0])
    assert_eq(ships(t, 1, 0), 5)
    assert_eq(pool(t, 0), 7)
    assert_eq(power(t, 0), 2)
}

fn test_sway_and_tax() {
    let s = new_game(2, 1)
    s[h_phase()] = 2
    s[h_asuit()] = 3
    s[h_acts()] = 2
    let t = apply(s, [5, 0, 0])
    assert_eq(power(t, 0), 5)
    let u = apply(t, [7, 0, 0])
    assert_eq(power(u, 0), 6)
}

fn test_fight_conserves_ships() {
    let s = new_game(2, 11)
    s[h_phase()] = 2
    s[h_asuit()] = 1
    s[h_acts()] = 2
    s[ships_base() + 1 * 4 + 1] = 3
    let before = total_ships(s, 0) + pool(s, 0) + total_ships(s, 1) + pool(s, 1)
    let t = apply(s, [2, 1, 1])
    let after = total_ships(t, 0) + pool(t, 0) + total_ships(t, 1) + pool(t, 1)
    assert_eq(before, after)
}

fn test_capture_city_when_defender_wiped_out() {
    // many seeds: whenever the defender ends with no ships, the city flips
    let flipped = 0
    for seed in range(1, 60) {
        let s = new_game(2, seed)
        s[h_phase()] = 2
        s[h_asuit()] = 1
        s[h_acts()] = 2
        s[ships_base() + 1 * 4 + 0] = 6
        s[ships_base() + 1 * 4 + 1] = 1
        s[city_base() + 1] = 1
        let t = apply(s, [2, 1, 1])
        if ships(t, 1, 1) == 0 {
            assert_eq(city(t, 1), 0)
            assert_eq(vp(t, 0), 1)
            flipped = flipped + 1
        } else {
            assert_eq(city(t, 1), 1)
        }
    }
    assert(flipped > 0)
}

fn test_no_actions_possible_ends_turn() {
    // lead plays a Fight card with no enemy in reach: turn passes at once
    let s = new_game(2, 2)
    s[p_base() + 3] = pow2(7)        // player 0 holds only card 7 (suit 1, rank 1)
    let t = apply(s, [0, 7, 0])
    assert_eq(cur(t), 1)
    assert_eq(phase(t), 1)
}

fn test_trick_winner_leads_next() {
    let s = new_game(2, 4)
    s[p_base() + 3] = pow2(0) + pow2(1)         // p0: suit 0 ranks 1,2
    s[p_base() + 6 + 3] = pow2(2) + pow2(3)     // p1: suit 0 ranks 3,4
    let t = apply(s, [0, 0, 0])
    while cur(t) == 0 && phase(t) == 2 { t = apply(t, [6, 0, 0]) }
    t = apply(t, [0, 3, 0])                      // p1 rank 4 beats rank 1
    while phase(t) == 2 { t = apply(t, [6, 0, 0]) }
    assert_eq(cur(t), 1)
    assert_eq(phase(t), 0)
    assert_eq(t[h_init()], 1)
}

fn test_pivot_does_not_win_trick() {
    let s = new_game(2, 4)
    s[p_base() + 3] = pow2(0)                   // p0 leads suit 0 rank 1
    s[p_base() + 6 + 3] = pow2(27)              // p1 holds only suit 3 rank 7
    let t = apply(s, [0, 0, 0])
    while cur(t) == 0 && phase(t) == 2 { t = apply(t, [6, 0, 0]) }
    assert_eq(len(legal_actions(t)), 1)
    t = apply(t, [0, 27, 0])
    while phase(t) == 2 { t = apply(t, [6, 0, 0]) }
    // hands empty: round scored, new round dealt, p0 holds initiative
    assert_eq(t[h_init()], 0)
}

fn test_bot_game_terminates_with_scores() {
    for n in range(2, 5) {
        for seed in range(1, 8) {
            let s = bot_game(n, seed * 31)
            assert(is_over(s))
            assert_eq(round(s), 3)
            let total = 0
            for p in range(0, n) { total = total + vp(s, p) }
            assert(total > 0)
        }
    }
}

fn test_bot_games_are_deterministic() {
    assert_eq(bot_game(3, 123), bot_game(3, 123))
    assert(bot_game(3, 123) != bot_game(3, 124))
}

// Random legal play (not the bot): the game must still always end.
fn test_random_legal_play_always_ends() {
    for seed in range(1, 40) {
        let s = new_game(2 + seed % 3, seed)
        let steps = 0
        let r = seed * 7919
        while !is_over(s) && steps < 3000 {
            let acts_l = legal_actions(s)
            assert(len(acts_l) > 0)
            r = rng_step(r)
            s = apply(s, acts_l[rng_val(r) % len(acts_l)])
            steps = steps + 1
        }
        assert(is_over(s))
    }
}

fn test_winner_is_a_valid_seat() {
    let s = bot_game(4, 99)
    let w = winner(s)
    assert(w >= 0 && w < 4)
    for p in range(0, 4) { assert(vp(s, w) >= vp(s, p)) }
}

fn test_vp_total_is_conserved_by_legal_ships() {
    // ships + pool per player is always 12
    let s = bot_game(3, 5)
    for p in range(0, 3) { assert_eq(total_ships(s, p) + pool(s, p), 12) }
}
