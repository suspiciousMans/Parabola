# Parabola: Ember Reach

A bird's-eye 3D tabletop strategy game on the [Dust engine](https://github.com/suspiciousMans/dust-engine),
with all rules written in [Oxidized](https://github.com/suspiciousMans/oxidized). It is an original game in the
style of trick-taking action-card area-control games: lead or follow an action card, spend its actions
moving, fighting, building and levying across a ring of seven star systems, win tricks to hold the
initiative, and score an ambition at the end of each of three chapters.

![overview](docs/overview.png)

## Play

```
cargo run --release -p ember_reach
```

Pick 2 to 4 players, each Human or Bot (hot-seat asks the next human to confirm before showing their hand).

| Camera | |
|---|---|
| Right-drag | orbit |
| Middle-drag or WASD / arrows | pan |
| Wheel | zoom |
| Q / E | quarter turns |
| Space | back to the overview |
| Double-click a system | glide to it |
| Click a system | filter the action list to it |

## Rules in one screen

* Deal: 6 action cards each (4 suits: Move, Fight, Build, Influence; ranks 1 to 7).
* A trick: the holder of the initiative leads a card and takes **2 actions** of its suit. Everyone else plays in
  turn order and takes **1 action**: they must follow the lead suit if they can, otherwise they may pivot with any
  card and take an action of *that* card's suit. The highest card of the lead suit wins: +1 power and the lead.
* Actions: **Move** up to 2 ships to an adjacent system; **Fight** (3 dice a side, 5 or 6 hits, wiping out a
  defender takes their city and scores 1); **Build** a city where you have a ship (2 power, max 4); **Levy** 2 ships
  into one of your cities (1 power); **Sway** +2 power; **Tax** +1 power per city.
* After 6 tricks the chapter's ambition scores (most ships, most cities, most systems held: 3 VP for a clear
  winner, 1 for a clear second), then cities pay 1 power each. After 3 chapters, most VP wins.

The rules are a plain, testable library: `rules/ox/reach.ox` (Oxidized), 21 tests in `rules/ox/tests/`.

## Layout

```
rules/   reach_rules crate
  ox/reach.ox          the rules (state = flat list of ints, actions = [kind, a, b])
  ox/tests/            `oxidized test rules/ox/tests`
  ox_gen/reach_gen.rs  committed transpiled copy, used when no `oxidized` binary is installed
  src/lib.rs           typed wrapper: State, Action, Event, validate(); serde, so it can be saved or sent
game/    ember_reach crate (Dust game)
  src/models.rs        procedural ships, cities, platforms: no asset files
  src/camera.rs        orbit / pan / zoom / glide camera (tested)
  src/ui.rs            egui menu, scoreboard, hand, action list, log
web/     browser build: index.html, build.sh, web_smoke.js
src/     the site: Cloudflare Worker (rooms, validation, bots), page client, game.ts hosts the 3D board
scripts/ build-game.sh (3D board for the site), build-worker.mjs, build-rules-wasm.sh
```

After editing `rules/ox/reach.ox`, run `rules/regen.sh` (needs `oxidized`; set `OXIDIZED=/path/to/oxidized`)
to run the Ox tests and refresh the committed copy.

## Web

```
source /path/to/emsdk/emsdk_env.sh        # emsdk 3.1.74 tested
rustup target add wasm32-unknown-emscripten
web/build.sh
node web/web_smoke.js dist-web ember_reach shot.png    # headless Chromium: loads, plays a bot game, checks the canvas
```

The game uses no threads, files or native-only crates, and generates all geometry in code, so there is nothing to preload.

## The site: rooms, bots and online play

`src/` is the site. The Worker (`src/worker`) keeps rooms in D1, validates every action with the shipped
rules (`src/game/reach_rules.wasm`) and plays bot seats. The page (`src/client`) creates and joins rooms by
code; once a game starts it loads the 3D board (`src/client/public/game/`, the same Rust game built for the
browser) and the board is driven by the Worker: the Worker sends each player the state with the other hands
hidden plus the last events, the board animates them, and the player's clicks go back as actions.

```
npm ci
npm run build          # type check, page, Worker
npm run dev            # wrangler dev --local on http://localhost:3000 (D1 is emulated locally)
```

Open it in two tabs (each tab is its own player): create a room in one, join with the code in the other,
add bots if you want, start. `?classic=1` on the page shows the plain HTML board instead of the 3D one.

After changing the game, rebuild the 3D board (needs emsdk, see Web above) and commit the result, so deploys
don't need Rust: `npm run build:game`. After changing the rules run `rules/regen.sh` and
`scripts/build-rules-wasm.sh`.

## Screenshots and autoplay

`ember_reach --autoplay --shot out.png --after 60 [--focus 3]` plays 60 bot actions headlessly (use `xvfb-run`
on Linux) and saves a frame.

## Not done yet

Spectators and reconnect polish, a lobby inside the 3D board (the page handles the lobby), better bots (one heuristic bot today), sound, a rules reference screen, undo, card art.


See [docs/STYLE.md](docs/STYLE.md) for the art and UI rules.

## Rules status (Stage 1)

The rules in `rules/ox/reach.ox` follow the physical game's structure: 24 systems in six clusters
(planet types, gates, out-of-play clusters for 2/3 players), the 12 setup cards, 4-suit card play
(Lead, Surpass, Copy, Pivot, Seize, Pass), ambitions with ranked markers, resources and the prelude,
tax/build/move/repair/battle with dice, outrage, trophies and captives, chapters and power goals.

Interpretations not yet checked against the rulebook (please verify): Pivot gives one action of the played
card's suit; repair fixes one piece; builds cost no resources; the lead passing ends the chapter; ambition
marker ranks follow declaration order; ties for first share the second value; destroyed ships are lost;
the intercept round; catapult moves along any-length paths; seat roles A/B/C on the setup cards.

Deferred: Court, Influence, Secure, Guild and Vox cards, leaders, lore, ambition city bonuses.
