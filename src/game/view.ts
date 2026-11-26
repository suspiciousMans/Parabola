/**
 * Display layer for Ember Reach: types, state decoding and labels.
 *
 * Deliberately free of any WASM dependency so the browser can render a
 * server-provided view without loading the rules module.
 */

export type Action = [number, number, number, number];
/** The rules' flat integer state. */
export type GameState = number[];

export const MAX_PLAYERS = 4;
export const SYSTEMS = 24;

export const SUITS = ['Administration', 'Aggression', 'Construction', 'Mobilization'];
export const RESOURCES = ['Material', 'Fuel', 'Weapons', 'Relics', 'Psionic'];
export const AMBITIONS = ['Tycoon', 'Tyrant', 'Warlord', 'Keeper', 'Empath'];

// Layout from rules/ox/reach.ox (h_* role indices, p_base / p_stride and the
// ships / slots / supply blocks).
const P_BASE = 40;
const P_STRIDE = 20;
const SHIPS_BASE = 120;
const SLOTS_BASE = 312;
const SUPPLY_BASE = 456;

export type SeatView = {
  power: number;
  resources: number[];
  citiesLeft: number;
  portsLeft: number;
  reserve: number;
  trophies: number;
  captives: number;
  hand: number[];
};

export type SlotView = { kind: number; owner: number; damaged: boolean };
export type SystemView = { inPlay: boolean; ships: [number, number][]; slots: SlotView[] };

export type PublicView = {
  players: number;
  chapter: number;
  phase: 'lead' | 'follow' | 'act' | 'over';
  current: number;
  initiative: number;
  leadSuit: number | null;
  actionsLeft: number;
  actionSuit: number;
  over: boolean;
  ambitions: number[];
  supply: number[];
  seats: SeatView[];
  systems: SystemView[];
  event: number[];
};

function bits(mask: number, count: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i += 1) if (Math.floor(mask / 2 ** i) % 2 === 1) out.push(i);
  return out;
}

/** Decodes a (redacted) state into the shape the UI renders. */
export function viewOf(state: GameState): PublicView {
  const players = Math.max(0, Math.min(MAX_PLAYERS, state[0] ?? 0));
  const outMask = state[20] ?? 0;
  const phase = state[2] === 0 ? 'lead' : state[2] === 1 ? 'follow' : state[2] === 2 ? 'act' : 'over';

  const seats: SeatView[] = [];
  for (let p = 0; p < players; p += 1) {
    const base = P_BASE + P_STRIDE * p;
    seats.push({
      power: state[base] ?? 0,
      resources: [0, 1, 2, 3, 4].map((t) => state[base + 1 + t] ?? 0),
      citiesLeft: state[base + 6] ?? 0,
      portsLeft: state[base + 7] ?? 0,
      reserve: state[base + 8] ?? 0,
      trophies: state[base + 9] ?? 0,
      captives: state[base + 10] ?? 0,
      hand: bits(state[base + 11] ?? 0, 28),
    });
  }

  const systems: SystemView[] = [];
  for (let sys = 0; sys < SYSTEMS; sys += 1) {
    const ships: [number, number][] = [];
    for (let p = 0; p < players; p += 1) {
      const at = SHIPS_BASE + sys * 8 + p * 2;
      ships.push([state[at] ?? 0, state[at + 1] ?? 0]);
    }
    const slots: SlotView[] = [];
    for (let sl = 0; sl < 2; sl += 1) {
      const at = SLOTS_BASE + sys * 6 + sl * 3;
      slots.push({
        owner: state[at] ?? -1,
        kind: state[at + 1] ?? 0,
        damaged: (state[at + 2] ?? 0) !== 0,
      });
    }
    const cluster = Math.floor(sys / 4);
    systems.push({ inPlay: Math.floor(outMask / 2 ** cluster) % 2 === 0, ships, slots });
  }

  return {
    players,
    chapter: state[1] ?? 1,
    phase,
    current: state[3] ?? 0,
    initiative: state[4] ?? 0,
    leadSuit: (state[6] ?? -1) >= 0 ? state[6] : null,
    actionsLeft: state[11] ?? 0,
    actionSuit: Math.max(0, state[12] ?? 0),
    over: state[2] === 4,
    ambitions: [22, 23, 24, 25, 26].map((i) => state[i] ?? 0),
    supply: [0, 1, 2, 3, 4].map((t) => state[SUPPLY_BASE + t] ?? 0),
    seats,
    systems,
    event: [30, 31, 32, 33, 34].map((i) => state[i] ?? 0),
  };
}

export function cardName(card: number): string {
  const suit = Math.floor(card / 7);
  return `${SUITS[suit] ?? '?'} ${(card % 7) + 1}`;
}

const KIND_LABELS = [
  'lead',
  'surpass',
  'copy',
  'pivot',
  'pass',
  'seize',
  'prelude',
  'end turn',
  'tax',
  'build city',
  'build port',
  'build ships',
  'move',
  'repair',
  'battle',
];

/** A short, human label for a legal action, used as the button text. */
export function actionLabel(action: Action): string {
  const [kind, a, b, c] = action;
  switch (kind) {
    case 0:
      return `Lead ${cardName(a)}${b >= 0 ? ` · ${AMBITIONS[b] ?? ''}` : ''}`;
    case 1:
      return `Surpass with ${cardName(a)}`;
    case 2:
      return `Copy with ${cardName(a)}`;
    case 3:
      return `Pivot with ${cardName(a)}`;
    case 5:
      return `Seize with ${cardName(a)}`;
    case 6:
      return `Prelude: spend ${RESOURCES[a] ?? '?'}`;
    case 8:
      return `Tax system ${a}`;
    case 9:
      return `Build city in ${a}`;
    case 10:
      return `Build starport in ${a}`;
    case 11:
      return `Build ships in ${a}`;
    case 12:
      return `Move ${c} from ${a} → ${b}`;
    case 13:
      return `Repair ${b === 0 ? 'ship' : 'building'} in ${a}`;
    case 14: {
      const assault = Math.floor(c / 49);
      const skirmish = Math.floor(c / 7) % 7;
      const raid = c % 7;
      return `Battle in ${a} vs player ${b + 1} (${assault}A ${skirmish}S ${raid}R)`;
    }
    default:
      return KIND_LABELS[kind] ?? `action ${kind}`;
  }
}
