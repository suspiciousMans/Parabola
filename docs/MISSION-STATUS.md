# Scoreboard HUD mission — remaining work

This note records the state of the on-board HUD work at the point it was paused.
It is a working handoff, not a claim that the final mission proof is complete.

## Mission

Keep score, resources, secure-able Court cards, captives and trophies visible on
the board itself, with a layout that stays usable on desktop and phone widths and
that follows the rules library. The remaining proof is a complete game, played
through the real browser controls, showing that each HUD value matches the engine
at capture, secure and trophy events.

## Completed

- Standings panel: per-player power, goal progress, resources, trophies and
  captives, secured Court cards and the clickable Court row.
- Court art: procedural emblems for Envoy, Seal, Writ, Pact and Accord.
- Board guide (`G`): Court, gates, ring, played cards, ambitions and power-track
  labels with adaptive compact mode; hand/action strip step aside while open.
- Field Manual (`R`, menu, RULES button): six-step tutorial plus TURN, ACTIONS,
  COURT and SCORING reference sections.
- Ledger (`L`): full score, resource, trophy, captive, city/port and Court details.
- Played cards are a horizontal row above the status strip.
- Hand is centred on wide screens and horizontally scrollable at phone widths.
- Status text wraps at phone widths instead of clipping.
- Power track is inside the board with staggered, raised tokens so equal scores
  remain distinct and low tokens are not hidden behind the HUD.
- Narrow standings panel is width-capped so long secured-card lines cannot cover
  the chapter panel.
- Game-over panel is anchored below the Court row and keeps the final Court
  visible; its title reads `COURT · SECURED CARDS KEPT`.
- Repaint-until-dealt fix for the initial hand animation.

## Work in progress at the pause

- Added small procedural **trophy** and **captive** icons in front of the
  `TROPHIES n` / `CAPTIVES n` counts in every player's standings row, and made a
  non-zero count use the normal ink colour instead of the dim colour.
- `cargo check -p ember_reach` and `npm run build:game` passed after that change.
- No new HUD captures or assertions have been run since the icons were added.

## Remaining proof and known issues

1. **Finish a complete game through the browser UI**, from setup to game over.
   A 2-player `--autoplay --human` game was started at 719x936 for this purpose.
   The chapter-1 Pass click advanced to chapter 2 correctly, and leading an
   Administration card worked.
2. **Action-strip click issue** (2026-11-27: not reproduced in the native build. Under Xvfb at 1280x860
   and 719x936, leading Administration then clicking Tax and End turn applied both. Recheck in the browser build;
   the strip reflows after each action, so a scripted click at a stale position misses): in that live 2-player test, the Tax and End turn
   buttons in the action strip did not register, although the card Lead button,
   the Pass button and the Court tiles did. Investigate before claiming the
   controls are fully exercised. The camera also returned to a cinematic view
   while the action strip was waiting, so distinguish camera motion from input.
3. **Verify spoils at events:** observe at least one trophy and one captive
   event (including an opponent's) and confirm the on-board counts, ledger and
   engine state agree. Trophy/captive event capture still needs a suitable game.
4. **Verify a complete-game game over** through the UI, then capture the final
   standings and game-over layout at desktop and 460x900.
5. **Regenerate evidence** invalidated by the icon change:
   `docs/hud-standings.png` and its probe at minimum; check the other four
   captures, then run `python3 scripts/assert_hud.py`.
6. **Run the full check set:** `rules/regen.sh`, game tests (10), Rust rules
   tests (9), Ox tests (14), `npm run build`, `npm run build:rules`,
   `npm run build:game`, and the placeholder search.
7. Inspect 460x900 and desktop captures for collisions with the power track,
   played cards, Court row, hand, action strip and guide labels.
8. Leave the completed work uncommitted unless a commit is requested.

## Verification already recorded before the pause

- Live Court secure at 460x900: Envoy secured, power 0 → 1, Relic 1 → 0,
  Court availability 5 → 4, action pips unchanged.
- Five HUD screenshot assertions passed against the captures taken before the
  trophy/captive icon change.
- Rules: 14 Ox tests passed; Rust rules: 9 passed; game: 10 passed.
- `npm run build:rules`, `npm run build` and `npm run build:game` passed.
- Placeholder search and `git diff --check` were clean.

## Files touched by this HUD work

- `game/src/ui.rs`, `game/src/main.rs`, `game/src/cards.rs`,
  `game/src/courtart.rs`, `game/src/guides.rs`, `game/src/fieldmanual.rs`,
  `game/src/probe.rs`
- `rules/ox/reach.ox`, `rules/ox/tests/test_reach.ox`, `rules/src/lib.rs`
- `web/index.html`, `src/client/game.ts`, `src/client/styles.css`
- `docs/hud-*.png`, `docs/probe-*.json`, `scripts/assert_hud.py`,
  `scripts/probe_server.py`
