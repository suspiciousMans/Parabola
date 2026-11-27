#!/usr/bin/env python3
"""Machine checks for the live Ember Reach HUD evidence.

The board is an egui canvas, so there is no DOM text to query for the HUD.  The
game exports a frame probe after every painted HUD frame: exact label strings,
egui Response rectangles, panel layers, and the arrays the ledger paints.  The
standalone web page POSTs that probe to the local harness, and the screenshots
in docs/ are captured from the same held frame.

This script asserts the probes against the rules state and against the pixels
in the saved screenshots (panel/text geometry inside the images, and measured
ink coverage in each label rectangle).  It deliberately fails on any mismatch.
"""

from __future__ import annotations

import json
import math
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"

TITLE = "SCORING CARDS · THE FIVE AMBITIONS"
COURT = "COURT"
FINAL = "FINAL CHAPTER SCORED"
MAGICK = shutil.which("magick") or shutil.which("convert")

if not MAGICK:
    sys.exit("ImageMagick (magick or convert) is required for pixel checks")


def fail(msg: str) -> None:
    raise AssertionError(msg)


def load_probe(name: str) -> dict:
    path = DOCS / f"probe-{name}.json"
    if not path.is_file():
        fail(f"missing live probe {path}")
    data = json.loads(path.read_text())
    for key in ("width", "height", "held", "over", "ledger", "panels", "texts", "rules", "table_cards"):
        if key not in data:
            fail(f"probe {name} lacks {key}")
    return data


def norm(text: str) -> str:
    return " ".join(str(text).split())


def inside(inner, outer, tol: float = 1.5) -> bool:
    x, y, w, h = inner
    X, Y, W, H = outer
    return x >= X - tol and y >= Y - tol and x + w <= X + W + tol and y + h <= Y + H + tol


def image_size(path: Path) -> tuple[int, int]:
    out = subprocess.check_output(
        [MAGICK, "identify", "-format", "%w %h", str(path)], text=True
    ).split()
    return int(out[0]), int(out[1])


def ink_fraction(path: Path, rect, threshold: int = 30) -> float:
    """Fraction of pixels in rect that are brighter than the dark panel ink."""
    x, y, w, h = rect
    x, y = max(0, math.floor(x)), max(0, math.floor(y))
    w, h = max(1, math.ceil(w)), max(1, math.ceil(h))
    out = subprocess.check_output(
        [
            MAGICK,
            str(path),
            "-crop",
            f"{w}x{h}+{x}+{y}",
            "+repage",
            "-colorspace",
            "Gray",
            "-threshold",
            f"{threshold}%",
            "-format",
            "%[fx:mean]",
            "info:",
        ],
        text=True,
        stderr=subprocess.DEVNULL,
    ).strip()
    return float(out)


def panel(probe: dict, panel_id: str) -> dict:
    found = [p for p in probe["panels"] if p["id"] == panel_id]
    if len(found) != 1:
        fail(f"expected one {panel_id} panel, got {len(found)}")
    return found[0]


def find(probe: dict, role: str, panel_id: str | None = None) -> dict:
    found = [
        t
        for t in probe["texts"]
        if t["role"] == role and (panel_id is None or t["panel"] == panel_id)
    ]
    if len(found) != 1:
        fail(f"expected one text role {role} in {panel_id or 'any panel'}, got {len(found)}")
    return found[0]


def check_text(
    probe: dict,
    image: Path,
    role: str,
    panel_id: str,
    expected: str | None = None,
    min_ink: float = 0.008,
) -> dict:
    t = find(probe, role, panel_id)
    p = panel(probe, panel_id)
    if expected is not None and norm(t["text"]) != norm(expected):
        fail(f"{role}: expected {expected!r}, rendered {t['text']!r}")
    if not inside(t["rect"], p["rect"]):
        fail(f"{role}: rect {t['rect']} leaves panel {p['rect']}")
    if not inside(t["rect"], [0, 0, probe["width"], probe["height"]]):
        fail(f"{role}: rect {t['rect']} leaves the captured screen")
    frac = ink_fraction(image, t["rect"])
    if frac < min_ink:
        fail(f"{role}: only {frac:.3f} ink in {t['rect']} in {image.name}")
    return t


def check_image(probe: dict, path: Path) -> tuple[int, int]:
    if not path.is_file():
        fail(f"missing screenshot {path}")
    size = image_size(path)
    if size != (probe["width"], probe["height"]):
        fail(f"{path.name}: {size} != live probe {(probe['width'], probe['height'])}")
    return size


def check_panel_in_screen(probe: dict, panel_id: str) -> dict:
    p = panel(probe, panel_id)
    if not inside(p["rect"], [0, 0, probe["width"], probe["height"]]):
        fail(f"panel {panel_id} leaves the screen: {p['rect']}")
    return p


def check_state(probe: dict, label: str) -> None:
    n = probe["rules"]["players"]
    caps = probe["rules"]["captives"]
    tro = probe["rules"]["trophies"]
    rules = probe["rules"]
    for key in ("court", "court_names", "court_available", "secured"):
        if key not in rules:
            fail(f"{label}: probe lacks rules.{key}")
    if len(rules["court"]) != 5 or len(rules["court_names"]) < 5:
        fail(f"{label}: Court row/name arrays are incomplete")
    if len(rules["secured"]) < n:
        fail(f"{label}: secured-card arrays are incomplete")
    available = sum(1 for c in rules["court"] if c >= 0)
    if available != rules["court_available"]:
        fail(f"{label}: court_available {rules['court_available']} != {available}")
    for c in rules["court"]:
        if c < -1 or c >= len(rules["court_names"]):
            fail(f"{label}: invalid Court card id {c}")
    for p in range(n):
        for c in rules["secured"][p]:
            if c < 0 or c >= len(rules["court_names"]):
                fail(f"{label}: invalid secured Court card {c} for player {p}")
    if len(caps) < n or len(tro) < n:
        fail(f"{label}: incomplete rules arrays")
    for p in range(n):
        if caps[p] <= 0:
            fail(f"{label}: player {p} has non-positive captives {caps[p]}")
        if tro[p] <= 0:
            fail(f"{label}: player {p} has non-positive trophies {tro[p]}")
    for p in range(n):
        if probe["rules"]["tallies"][p][1] != caps[p]:
            fail(
                f"{label}: Tyrant tally {probe['rules']['tallies'][p][1]} "
                f"!= captives {caps[p]} for player {p}"
            )


def check_spoils(probe: dict, panel_id: str, image: Path) -> None:
    n = probe["rules"]["players"]
    for p in range(n):
        role = f"spoils.{p}"
        expected = (
            f"{probe['rules']['trophies'][p]} trophies  ·  "
            f"{probe['rules']['captives'][p]} captives"
        )
        check_text(probe, image, role, panel_id, expected)
        if "trophies" not in norm(find(probe, role, panel_id)["text"]):
            fail(f"{panel_id} {role} lacks trophies")
        if "captives" not in norm(find(probe, role, panel_id)["text"]):
            fail(f"{panel_id} {role} lacks captives")


def court_name(probe: dict, slot: int) -> str:
    c = probe["rules"]["court"][slot]
    return "—" if c < 0 else probe["rules"]["court_names"][c].upper()


def secured_line(probe: dict, p: int) -> str:
    names = [probe["rules"]["court_names"][c] for c in probe["rules"]["secured"][p]]
    return "SECURED —" if not names else "SECURED  " + " · ".join(names)


def check_court_row(probe: dict, image: Path, panel_id: str) -> None:
    for slot in range(5):
        check_text(probe, image, f"court.{slot}", panel_id, court_name(probe, slot))
    for p in range(probe["rules"]["players"]):
        check_text(probe, image, f"secure.{p}", panel_id, secured_line(probe, p))


def check_ledger_court(probe: dict, image: Path) -> None:
    title = find(probe, "title.court", "ledger")
    if not norm(title["text"]).startswith(COURT):
        fail(f"ledger court title missing: {title['text']!r}")
    check_text(probe, image, "title.court", "ledger")
    n = probe["rules"]["players"]
    seats = probe["rules"]["seats"]
    for slot in range(5):
        name = court_name(probe, slot)
        text = norm(find(probe, f"court.{slot}", "ledger")["text"])
        if name != "—" and not text.startswith(name):
            fail(f"ledger court.{slot} {text!r} does not start with {name!r}")
        check_text(probe, image, f"court.{slot}", "ledger")
    for p in range(n):
        names = [probe["rules"]["court_names"][c].upper() for c in probe["rules"]["secured"][p]]
        seat = seats[p].upper()
        want = f"{seat}: none secured" if not names else f"{seat}: " + " · ".join(names)
        check_text(probe, image, f"secured.{p}", "ledger", want)
    note = find(probe, "court.note", "ledger")
    if not norm(note["text"]).startswith("Each secured"):
        fail(f"ledger court note missing: {note['text']!r}")
    check_text(probe, image, "court.note", "ledger")


def _tyrrant_leader(probe: dict) -> tuple[int, int]:
    caps = probe["rules"]["captives"]
    top = max(caps)
    leaders = [i for i, c in enumerate(caps) if c == top]
    if len(leaders) != 1:
        fail(f"captives leader is not unique: {leaders}")
    return leaders[0], top


def _check_tyrant_frame(probe: dict, image, panel_id: str) -> None:
    n = probe["rules"]["players"]
    for p in range(n):
        if probe["rules"]["tallies"][p][1] != probe["rules"]["captives"][p]:
            fail(f"Tyrant tally != captives for player {p}")
    seat, top = _tyrrant_leader(probe)
    note = norm(find(probe, "leader.1", panel_id)["text"])
    want = f"{probe['rules']['seats'][seat].upper()} {top}"
    if note != want:
        fail(f"Tyrant note {note!r} != {want!r}")
    check_text(probe, image, "leader.1", panel_id, want)


def _leader_note(probe: dict, rank: int, a: int) -> str:
    n = probe["rules"]["players"]
    tally = [probe["rules"]["tallies"][p][a] for p in range(n)]
    top = max(tally)
    leaders = [p for p, v in enumerate(tally) if top > 0 and v == top]
    if rank == 0:
        return "not declared"
    if top == 0:
        return "no tally yet"
    if len(leaders) == 1:
        return f"{probe['rules']['seats'][leaders[0]].upper()} {top}"
    return f"TIE {top}"


def check_standings(probe: dict) -> None:
    n = probe["rules"]["players"]
    if probe["over"] or not probe["held"] or probe["ledger"]:
        fail("standings state is not a held, unfinished board frame")
    if probe["width"] != 719 or probe["height"] != 936:
        fail(f"standings probe is {probe['width']}x{probe['height']}, expected 719x936")
    stand = check_panel_in_screen(probe, "standings")
    amb = check_panel_in_screen(probe, "ambitions")
    if stand["z"] != 1 or amb["z"] != 1:
        fail("standings HUD panels must sit on layer 1")

    image = DOCS / "hud-standings.png"
    check_image(probe, image)
    for p in range(n):
        check_text(probe, image, f"name.{p}", "standings")
        check_text(probe, image, f"tro.{p}", "standings",
                   f"TROPHIES {probe['rules']['trophies'][p]}")
        check_text(probe, image, f"cap.{p}", "standings",
                   f"CAPTIVES {probe['rules']['captives'][p]}")
    check_text(probe, image, "title.cards", "ambitions", TITLE)
    check_text(probe, image, "court", "ambitions", f"COURT · {probe['rules']['court_available']} available · 1 relic each")
    check_court_row(probe, image, "standings")
    check_text(probe, image, "rank.1", "ambitions")
    _check_tyrant_frame(probe, image, "ambitions")


def check_ambitions(probe: dict) -> None:
    n = probe["rules"]["players"]
    if probe["over"] or not probe["held"] or probe["ledger"]:
        fail("ambitions state is not a held, unfinished board frame")
    if probe["width"] != 719 or probe["height"] != 936:
        fail(f"ambitions probe is {probe['width']}x{probe['height']}, expected 719x936")
    stand = check_panel_in_screen(probe, "standings")
    amb = check_panel_in_screen(probe, "ambitions")
    if stand["z"] != 1 or amb["z"] != 1:
        fail("ambitions HUD panels must sit on layer 1")

    image = DOCS / "hud-ambitions.png"
    check_image(probe, image)

    # The non-zero spoils are the same frame's left-hand values.
    for p in range(n):
        check_text(probe, image, f"name.{p}", "standings")
        check_text(probe, image, f"tro.{p}", "standings",
                   f"TROPHIES {probe['rules']['trophies'][p]}")
        check_text(probe, image, f"cap.{p}", "standings",
                   f"CAPTIVES {probe['rules']['captives'][p]}")

    # Every ambition card, declaration tag and leader note is painted from the
    # rules arrays, not inferred from another screenshot.
    tags = ["1ST", "2ND", "3RD"]
    for a in range(5):
        name = probe["rules"]["ambitions"][a].upper()
        check_text(probe, image, f"amb.{a}", "ambitions", name)
        rank = probe["rules"]["ranks"][a]
        tag = tags[rank - 1] if rank > 0 else "—"
        check_text(probe, image, f"rank.{a}", "ambitions", tag)
        check_text(probe, image, f"leader.{a}", "ambitions", _leader_note(probe, rank, a))

    check_text(probe, image, "title.cards", "ambitions", TITLE)
    check_text(probe, image, "court", "ambitions", f"COURT · {probe['rules']['court_available']} available · 1 relic each")
    check_court_row(probe, image, "standings")
    _check_tyrant_frame(probe, image, "ambitions")


def check_ledger(probe: dict, state: str, image_name: str, expected_size: tuple[int, int]) -> None:
    n = probe["rules"]["players"]
    if probe["over"] or not probe["ledger"] or not probe["held"]:
        fail(f"{state}: expected a held, unfinished ledger frame")
    if (probe["width"], probe["height"]) != expected_size:
        fail(f"{state}: probe size {probe['width']}x{probe['height']} != {expected_size}")
    if probe["table_cards"]:
        fail(f"{state}: table cards must not paint while the ledger is open")

    image = DOCS / image_name
    check_image(probe, image)
    check_panel_in_screen(probe, "veil")
    led = check_panel_in_screen(probe, "ledger")
    veil = panel(probe, "veil")
    if veil["z"] >= led["z"]:
        fail(f"{state}: veil {veil['z']} is not below ledger {led['z']}")
    top_z = max([p["z"] for p in probe["panels"]] + [t["z"] for t in probe["texts"]])
    if top_z != led["z"]:
        fail(f"{state}: something sits above the ledger (top z {top_z}, ledger z {led['z']})")

    ledger_texts = [t for t in probe["texts"] if t["panel"] == "ledger"]
    if not ledger_texts:
        fail(f"{state}: no ledger text recorded")
    for t in ledger_texts:
        if not inside(t["rect"], led["rect"]):
            fail(f"{state}: ledger text {t['role']} leaves the panel: {t['rect']}")
        if t["z"] != led["z"]:
            fail(f"{state}: ledger text {t['role']} is not on the ledger layer")

    if state == "ledger":
        # Wide table: all 12 headers and the captives column must be present.
        for hdr in ("PLAYER", "POWER", "MAT", "FUEL", "WPN", "REL", "PSI", "TRO", "CAP", "CITY", "PORT", "RES"):
            check_text(probe, image, f"hdr.{hdr}", "ledger")
        for p in range(n):
            check_text(probe, image, f"cap.{p}", "ledger", str(probe["rules"]["captives"][p]))
            check_text(probe, image, f"tro.{p}", "ledger", str(probe["rules"]["trophies"][p]))
        for p in range(n):
            check_text(probe, image, f"name.{p}", "ledger")
    else:
        # Narrow layout: the per-seat spoil lines are the readable columns.
        for p in range(n):
            check_text(
                probe, image, f"spoils.{p}.trophies", "ledger",
                f"{probe['rules']['trophies'][p]} trophies",
            )
            check_text(
                probe, image, f"spoils.{p}.captives", "ledger",
                f"{probe['rules']['captives'][p]} captives",
            )
        for p in range(n):
            check_text(probe, image, f"name.{p}", "ledger")

    check_text(probe, image, "title.cards", "ledger", TITLE)
    check_ledger_court(probe, image)

    # The Tyrant marker row and the CAP column agree with the rules arrays.
    tyr = [t for t in probe["texts"] if t["panel"] == "ledger" and norm(t["text"]) == "TYRANT"]
    if not tyr:
        fail(f"{state}: ledger has no Tyrant ambition row")
    for p in range(n):
        if probe["rules"]["tallies"][p][1] != probe["rules"]["captives"][p]:
            fail(f"{state}: Tyrant tally != captives for player {p}")


def check_over(probe: dict) -> None:
    n = probe["rules"]["players"]
    if not probe["over"] or not probe["held"]:
        fail("game-over probe is not a held finished frame")
    if probe["ledger"]:
        fail("game-over state has the ledger open")
    if probe["table_cards"]:
        fail("game-over state still paints table cards")
    if probe["width"] != 719 or probe["height"] != 936:
        fail(f"game-over probe is {probe['width']}x{probe['height']}, expected 719x936")

    image = DOCS / "hud-game-over.png"
    check_image(probe, image)
    stand = check_panel_in_screen(probe, "standings")
    final_panel = check_panel_in_screen(probe, "ambitions_final")
    over_panel = check_panel_in_screen(probe, "game_over")
    if over_panel["z"] <= stand["z"] or over_panel["z"] <= final_panel["z"]:
        fail("game-over spoils panel is not above the standings/final panels")

    # No empty ambition strip: no ambition cards, ranks or "not declared" at over.
    for t in probe["texts"]:
        if norm(t["text"]).lower() == "not declared":
            fail(f"game over still renders 'not declared': {t['role']}")
        if t["role"].split(".")[0] in ("amb", "rank", "leader"):
            fail(f"game over still renders ambition card text {t['role']}")

    check_text(probe, image, "title.cards", "ambitions_final", TITLE)
    check_text(probe, image, "final", "ambitions_final", FINAL)
    check_text(probe, image, "final_note", "ambitions_final")
    check_text(probe, image, "scoring_note", "ambitions_final")
    check_text(probe, image, "court", "ambitions_final", "COURT · secured cards kept")
    check_court_row(probe, image, "standings")
    note = norm(find(probe, "scoring_note", "ambitions_final")["text"])
    if "Tyrant counts captives" not in note or "Warlord counts trophies" not in note:
        fail(f"final scoring note is incomplete: {note!r}")

    # The central spoils are the rules' final trophies and captives.
    for p in range(n):
        check_text(
            probe, image, f"tro.{p}", "standings",
            f"TROPHIES {probe['rules']['trophies'][p]}",
        )
        check_text(
            probe, image, f"cap.{p}", "standings",
            f"CAPTIVES {probe['rules']['captives'][p]}",
        )
        expected = (
            f"{probe['rules']['trophies'][p]} trophies  ·  "
            f"{probe['rules']['captives'][p]} captives"
        )
        check_text(probe, image, f"spoils.{p}", "game_over", expected)
        if probe["rules"]["tallies"][p][1] != probe["rules"]["captives"][p]:
            fail(f"final Tyrant tally != final captives for player {p}")
    check_text(probe, image, "winner", "game_over")
    check_text(probe, image, "hdr.SPOILS", "game_over")
    if any(p["id"] == "veil" for p in probe["panels"]):
        fail("game over has a ledger veil")
    if any(t["panel"] == "ledger" for t in probe["texts"]):
        fail("game over has ledger labels")
    if probe["rules"]["ranks"] and any(probe["rules"]["ranks"]):
        fail("game over still has ambition markers, so the final note is misleading")


def main() -> int:
    standings = load_probe("standings")
    ambitions = load_probe("ambitions")
    ledger = load_probe("ledger")
    narrow = load_probe("narrow")
    over = load_probe("over")

    check_state(standings, "standings")
    check_standings(standings)
    print("PASS standings: its own probe/image shows all trophies/captives, copy and Tyrant tally")

    check_state(ambitions, "ambitions")
    check_ambitions(ambitions)
    print("PASS ambitions: its own probe/image shows every card value, copy and Tyrant tally")

    check_state(ledger, "ledger")
    check_ledger(ledger, "ledger", "hud-ledger.png", (719, 936))
    print("PASS ledger: panel above veil, 12 columns, CAP values, copy, ink in all labels")

    check_state(narrow, "narrow")
    check_ledger(narrow, "narrow", "hud-ledger-narrow.png", (460, 900))
    print("PASS narrow: 460x900 ledger is unoccluded, all spoil lines and captives inside")

    check_state(over, "over")
    check_over(over)
    print("PASS game over: real final spoils, final-note copy, no empty ambition strip")
    print("ALL FIVE HUD SCREENSHOT ASSERTIONS PASSED")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except AssertionError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        sys.exit(1)
