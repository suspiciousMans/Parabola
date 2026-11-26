# Style guide

Why this exists: an early version of the board and HUD read as generated and placeholder-like: neon glow
everywhere, flat unlit spheres, a default font, centered glass cards on cards. This guide records what we
checked and the rules we now hold the game to. Everything here is applied in `game/`.

## Sources

Read for this guide (the guidance below is taken from what these pages say):

- Unity, [Video game UI/UX basics](https://unity.com/blog/video-game-ui-ux-basics): keep the centre clear and key
  elements at the edges; show only what matters now; one meaning per icon; immediate feedback; never rely on
  colour alone; gray out impossible actions.
- UX art director, [7 obvious beginner mistakes with your game's HUD](https://bootcamp.uxdesign.cc/7-obvious-beginner-mistakes-with-your-games-hud-from-a-ui-ux-art-director-d852e255184a):
  rank every element by importance; secondary info is opt-in or fades; at most two fonts (a title face and a legible
  sans); one hot accent colour used consistently; do not make the whole HUD red; replace raw numbers with meters or
  icons; group related widgets; test in context at the real viewing distance.
- [AI slop anti-patterns catalog](https://cdn.jsdelivr.net/npm/@wystack/agent-kit@0.12.0/wystack-agent-kit/references/anti-patterns.md):
  avoid default fonts; avoid pure black and white (tint neutrals); avoid grey text on coloured grounds; avoid
  neon-on-dark glow as the default language; avoid cards inside cards; avoid centring everything; avoid decorative
  glass and generic shadows; avoid bounce easing; use specific verb labels.
- Velan Studios, [Creating stylized art in a PBR world](https://medium.com/velan-studios/tip-of-the-brush-creating-stylized-art-in-a-pbr-world-b803b91c082f):
  exaggerate the key shapes so the silhouette reads; prefer macro detail to micro noise and leave areas of rest;
  keep roughness above real-world values; use true metals for contrast against non-metals; check materials under
  neutral light first.

Looked up but not read in full (so nothing here relies on them): No Man's Sky UI write-ups (the Medium series
"Defining Design Principles" is only a short framing note), a game-typography article from Space Ape.

## Rules we follow

**Hierarchy.** Three levels, in this order: (1) whose turn it is and what they can do now (status line and the
hand, bottom centre); (2) the standings (left column: power large, resources as small coloured pips); (3) chapter
and ambitions (top right, quiet). The log is dim and fades. Nothing sits in the middle of the board.

**Type.** Two faces only: Chakra Petch SemiBold for headings, numbers and labels (set in caps at small sizes) and
IBM Plex Sans for body text. Both are SIL Open Font License and ship in `game/assets/fonts` with their licences.
Sizes: 10.5 to 12 captions, 13.5 body, 15 to 16 status, 20 to 26 numbers, 40 title.

**Colour.** Tinted near-black neutrals (no pure black or white), one brass accent (`#D6A456`) for the thing to
act on, red reserved for alerts. Player colours are muted and used only on small marks (bars, tokens, ships).
Resource and suit colours appear only on pips and card edges. Defined once in `game/src/ui.rs`.

**Surfaces.** Flat panels with one hairline and square-ish corners. No glow borders, no gradients, no panels
inside panels. Layout is left-aligned and asymmetric; hand and status are the only centred element because they
anchor the player's attention.

**Board lighting and materials.** The scene is lit by one key light with a cool fill, so planets show a
terminator and cast shadows. Planets use generated colour and normal maps (`game/src/planets.rs`), roughness 0.55
to 0.9, one distinct silhouette per type (rocky with dust belt, banded giant with ring, scorched with spires,
crystal with shards, violet with moons). Metal is used only on ships, gates and spires. Emissive is for things
that emit light (engines, gate cores, ember cracks) and kept low.

**Ambient sky.** The board floats in a painted galaxy map (`game/src/galaxy.rs`): one equirect sky pass with
violet and magenta gas, dark dust lanes and a single hot core, then real star points, drifting stardust and soft
gas billboards. The sky is a backdrop, never a light source: the gas stays below the HUD's contrast, stars stay
small, and the core is the only part of the sky hot enough to bloom. The board, its pieces and the HUD remain the
first, second and third things to read.

**Glow budget.** Bloom threshold 2.0 and strength 0.55, so only genuinely hot pixels bloom. Lanes are thin and
muted. Stars are small, many and faint, with a few bright coloured ones; dust is sparse. If everything glows,
nothing is bright.

**Motion.** Camera moves ease out and are short; one cinematic move per event; a toggle (key `C`) turns them off.
No bounce.

**Labels.** Verb first: "Build city", "Move 3", "Surpass". No bare "OK".

## Checklist before shipping a visual change

1. Does the screenshot have a clear first, second and third thing to look at?
2. Is any element glowing that is not a light source?
3. Any default font, pure black or white, or a card on a card?
4. Is the accent colour used for one purpose only?
5. Do planets, ships and the board read at overview distance and up close?
6. Compare against the previous screenshot in `docs/` and keep the better of the two.
