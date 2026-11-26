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
