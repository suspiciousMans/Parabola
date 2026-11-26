// Setup cards: [name index, out-of-play clusters, per-seat start systems A, B, C(, C2)].
// System id = (cluster - 1) * 4 + kind, kind 0 arrow, 1 crescent, 2 hex, 3 gate.
// Transcribed from the fan data in the plan: UNVERIFIED against the physical cards.
fn setup_count(n) { return 4 }

fn setup_out(n, i) {
    if n == 2 { return [[0, 5], [1, 4], [0, 3], [0, 3]][i] }
    if n == 3 { return [[0, 3], [1, 2], [4, 5], [2, 5]][i] }
    if n == 4 { return [[2], [3], [4], [5]][i] }
    return []
}

fn setup_seat(n, i, p) {
    if n == 2 { return [[[18, 14, 11, 8], [10, 16, 19, 12]], [[13, 9, 3, 20], [22, 8, 15, 2]], [[16, 20, 19, 18], [8, 10, 11, 6]], [[17, 6, 11, 21], [5, 20, 19, 8]]][i][p] }
    if n == 3 { return [[[10, 17, 7], [4, 18, 11], [6, 8, 19]], [[2, 14, 23], [18, 1, 19], [13, 20, 3]], [[10, 9, 11], [2, 6, 7], [0, 14, 15]], [[2, 5, 3], [6, 1, 7], [0, 4, 15]]][i][p] }
    if n == 4 { return [[[12, 22, 3], [14, 18, 23], [16, 2, 15], [20, 0, 19]], [[18, 8, 7], [10, 17, 3], [6, 2, 11], [0, 4, 19]], [[2, 9, 7], [10, 22, 11], [13, 6, 23], [0, 20, 15]], [[8, 17, 3], [0, 9, 7], [2, 14, 11], [12, 5, 19]]][i][p] }
    return []
}

fn setup_name(n, i) {
    if n == 2 { return ["Frontiers", "Mix Up 1", "Homelands", "Mix Up 2"][i] }
    if n == 3 { return ["Mix Up", "Frontiers", "Homelands", "Core Conflict"][i] }
    if n == 4 { return ["Mix Up 1", "Mix Up 2", "Frontiers", "Mix Up 3"][i] }
    return ""
}

// ---- planets (UNVERIFIED fan transcription; one cell per line to fix) ------
// Resource types: 0 Material, 1 Fuel, 2 Weapons, 3 Relics, 4 Psionic. Gates: -1.
// Index = system id = cluster * 4 + kind (cluster 0..5, kind arrow/crescent/hex/gate).
fn planet_type(sys) {
    return [
        2, 1, 0, -1,   // cluster 1: Weapons, Fuel, Material
        4, 2, 3, -1,   // cluster 2: Psionic, Weapons, Relics
        0, 1, 2, -1,   // cluster 3: Material, Fuel, Weapons
        3, 1, 0, -1,   // cluster 4: Relics, Fuel, Material
        2, 3, 4, -1,   // cluster 5: Weapons, Relics, Psionic
        0, 1, 4, -1,   // cluster 6: Material, Fuel, Psionic
    ][sys]
}

fn planet_slots(sys) {
    return [
        2, 1, 2, 0,
        1, 1, 2, 0,
        1, 1, 2, 0,
        2, 2, 1, 0,
        1, 1, 2, 0,
        1, 2, 1, 0,
    ][sys]
}

// Action card pips by suit (0 Administration, 1 Aggression, 2 Construction,
// 3 Mobilization) and number 1..7. UNVERIFIED against the printed cards.
fn card_pips(suit, number) {
    return [
        [4, 4, 3, 3, 3, 2, 1],
        [3, 3, 2, 2, 2, 2, 1],
        [4, 4, 3, 3, 2, 2, 1],
        [4, 4, 3, 3, 2, 2, 1],
    ][suit][number - 1]
}

// Battle dice faces: [self hits, intercepts, ship hits, building hits, raid keys].
// kind 0 Assault, 1 Skirmish, 2 Raid. UNVERIFIED.
fn die_face(kind, roll) {
    if kind == 0 {
        return [[0, 0, 0, 0, 0], [1, 0, 1, 0, 0], [0, 1, 1, 0, 0], [1, 0, 2, 0, 0], [0, 0, 2, 0, 0], [1, 0, 1, 0, 0]][roll]
    }
    if kind == 1 {
        return [[0, 0, 0, 0, 0], [0, 0, 0, 0, 0], [0, 0, 0, 0, 0], [0, 0, 1, 0, 0], [0, 0, 1, 0, 0], [0, 0, 1, 0, 0]][roll]
    }
    return [[0, 1, 0, 0, 2], [1, 0, 0, 0, 1], [0, 1, 0, 0, 0], [1, 0, 0, 1, 0], [0, 0, 0, 1, 1], [1, 0, 0, 1, 0]][roll]
}

// Ambition marker values by order of declaration in a chapter: [first place, second place].
fn marker_values(rank) {
    return [[5, 3], [3, 2], [2, 0]][rank]
}

// Power needed to end the game after a chapter: 33 (2p), 30 (3p), 27 (4p).
fn power_goal(n) {
    return [0, 0, 33, 30, 27][n]
}
