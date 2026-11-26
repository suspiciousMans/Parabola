/**
 * Ember Reach rules, as the Site runs them.
 *
 * The rules live in `rules/ox/reach.ox` (Oxidized) and are transpiled into
 * `rules/ox_gen/reach_gen.rs`. `scripts/build-rules-wasm.sh` compiles that file
 * to `reach_rules.wasm`, which the Worker imports as a WebAssembly module (the
 * runtime cannot compile wasm from bytes), so this Site runs exactly the same
 * rules code as the Rust game — nothing is re-implemented in TypeScript.
 *
 * State is the rules' own flat integer list; an action is `[kind, a, b, c]`.
 * Call `initRules()` once before using anything else.
 */

import type { Action, GameState } from './view';
import rulesModule from './reach_rules.wasm';

export type { Action, GameState } from './view';

/** i64 parameters and results cross as BigInt (see `i64` / `fromI64`). */
interface Exports {
  memory: WebAssembly.Memory;
  ox_alloc(len: number): number;
  ox_free(ptr: number, len: number): void;
  ox_state_len(): number;
  ox_new_game(players: bigint, seed: bigint, setup: bigint, outLen: number): number;
  ox_legal(state: number, len: number, outLen: number): number;
  ox_is_legal(
    state: number,
    len: number,
    a0: bigint,
    a1: bigint,
    a2: bigint,
    a3: bigint,
  ): number;
  ox_apply(
    state: number,
    len: number,
    a0: bigint,
    a1: bigint,
    a2: bigint,
    a3: bigint,
    outLen: number,
  ): number;
  ox_bot_choose(state: number, len: number, outLen: number): number;
  ox_current(state: number, len: number): bigint;
  ox_over(state: number, len: number): number;
  ox_winner(state: number, len: number): bigint;
  ox_setup_count(players: bigint): bigint;
  ox_redact(state: number, len: number, viewer: bigint, outLen: number): number;
}

let instance: Exports | null = null;
let outPtr = 0;
let booting: Promise<void> | null = null;

/** Instantiates the rules module. Safe to call repeatedly. */
export function initRules(): Promise<void> {
  if (!booting) {
    booting = WebAssembly.instantiate(rulesModule, {})
      .then((created) => {
        instance = created.exports as unknown as Exports;
        outPtr = instance.ox_alloc(1);
      })
      .catch((error: unknown) => {
        booting = null;
        throw error;
      });
  }
  return booting;
}

function wasm(): Exports {
  if (!instance) throw new Error('rules not ready: await initRules() first');
  return instance;
}

/** wasm i64 parameters are BigInt-only; the rules' values are all integers. */
const i64 = (value: number): bigint => BigInt(Math.trunc(value));
const fromI64 = (value: bigint): number => Number(value);

function readOut(ptr: number): number[] {
  if (ptr === 0) return [];
  const length = new Uint32Array(wasm().memory.buffer, outPtr, 1)[0];
  if (length === 0) {
    wasm().ox_free(ptr, 0);
    return [];
  }
  const raw = new BigInt64Array(wasm().memory.buffer, ptr, length);
  const values: number[] = new Array(length);
  for (let i = 0; i < length; i += 1) values[i] = Number(raw[i]);
  wasm().ox_free(ptr, length);
  return values;
}

function write(state: GameState): number {
  const ptr = wasm().ox_alloc(state.length);
  const view = new BigInt64Array(wasm().memory.buffer, ptr, state.length);
  for (let i = 0; i < state.length; i += 1) view[i] = BigInt(state[i]);
  return ptr;
}

function withState<T>(state: GameState, run: (ptr: number) => T): T {
  const ptr = write(state);
  try {
    return run(ptr);
  } finally {
    wasm().ox_free(ptr, state.length);
  }
}

export function newGame(players: number, seed: number, setup: number): GameState {
  const clamped = ((seed % 2_147_483_647) + 2_147_483_647) % 2_147_483_647;
  return readOut(wasm().ox_new_game(i64(players), i64(clamped), i64(setup), outPtr));
}

export function setupCount(players: number): number {
  return fromI64(wasm().ox_setup_count(i64(players)));
}

export function legalActions(state: GameState): Action[] {
  const flat = withState(state, (ptr) => readOut(wasm().ox_legal(ptr, state.length, outPtr)));
  const count = flat[0] ?? 0;
  const actions: Action[] = [];
  for (let i = 0; i < count; i += 1) {
    const at = 1 + i * 4;
    actions.push([flat[at], flat[at + 1], flat[at + 2], flat[at + 3]]);
  }
  return actions;
}

export function isLegal(state: GameState, action: Action): boolean {
  return withState(
    state,
    (ptr) =>
      wasm().ox_is_legal(ptr, state.length, i64(action[0]), i64(action[1]), i64(action[2]), i64(action[3])) ===
      1,
  );
}

/** `null` when the action is not legal. */
export function applyAction(state: GameState, action: Action): GameState | null {
  const next = withState(state, (ptr) =>
    readOut(
      wasm().ox_apply(
        ptr,
        state.length,
        i64(action[0]),
        i64(action[1]),
        i64(action[2]),
        i64(action[3]),
        outPtr,
      ),
    ),
  );
  return next.length > 0 ? next : null;
}

export function botChoose(state: GameState): Action | null {
  const picks = withState(state, (ptr) => readOut(wasm().ox_bot_choose(ptr, state.length, outPtr)));
  return picks.length === 4 ? [picks[0], picks[1], picks[2], picks[3]] : null;
}

export function currentPlayer(state: GameState): number {
  return withState(state, (ptr) => fromI64(wasm().ox_current(ptr, state.length)));
}

export function isOver(state: GameState): boolean {
  return withState(state, (ptr) => wasm().ox_over(ptr, state.length) === 1);
}

export function winnerOf(state: GameState): number {
  return withState(state, (ptr) => fromI64(wasm().ox_winner(ptr, state.length)));
}

/**
 * The state as one player may see it: other hands blanked, RNG cleared.
 * Use this before anything derived from state leaves the server.
 */
export function redact(state: GameState, viewer: number): GameState {
  return withState(state, (ptr) => readOut(wasm().ox_redact(ptr, state.length, i64(viewer), outPtr)));
}
