import {
  applyAction,
  botChoose,
  currentPlayer,
  initRules,
  isOver,
  legalActions,
  newGame,
  redact,
  setupCount,
  winnerOf,
} from '../game/rules';
import {
  MAX_PLAYERS,
  actionLabel,
  viewOf,
  type Action,
  type GameState,
  type PublicView,
} from '../game/view';

interface D1Stmt {
  bind(...values: unknown[]): D1Stmt;
  first<T = Record<string, unknown>>(): Promise<T | null>;
  run(): Promise<{ meta: { changes: number } }>;
}

interface D1Database {
  prepare(query: string): D1Stmt;
}

interface AssetFetcher {
  fetch(request: Request): Promise<Response>;
}

export interface Env {
  DB: D1Database;
  ASSETS: AssetFetcher;
}

const ROOM_TTL_MS = 2 * 60 * 60 * 1000;
const CODE_RE = /^[A-Z0-9]{3,8}$/;
const GUEST_RE = /^[A-Za-z0-9_-]{8,64}$/;
const NAME_MAX = 16;

type Status = 'lobby' | 'playing' | 'finished';

type Seat = { id: string; name: string; bot?: boolean };

type Game = { state: GameState; seed: number; setup: number };

type RoomState = { seats: Seat[]; game: Game | null };

type RoomRow = {
  code: string;
  host_id: string;
  status: Status;
  revision: number;
  state_json: string;
  expires_at: number;
};

type Snapshot = {
  code: string;
  status: Status;
  revision: number;
  host: number;
  seats: { name: string; bot: boolean }[];
  maxPlayers: number;
  you: number | null;
  view: PublicView | null;
  actions: { action: Action; label: string }[];
  winner: number | null;
};

let schemaReady: Promise<void> | null = null;

function ensureSchema(env: Env): Promise<void> {
  if (!schemaReady) {
    schemaReady = env.DB.prepare(
      `CREATE TABLE IF NOT EXISTS rooms (
         code       TEXT PRIMARY KEY COLLATE NOCASE,
         host_id    TEXT NOT NULL,
         status     TEXT NOT NULL DEFAULT 'lobby',
         revision   INTEGER NOT NULL DEFAULT 0,
         state_json TEXT NOT NULL,
         created_at INTEGER NOT NULL,
         expires_at INTEGER NOT NULL
       ) STRICT`,
    )
      .run()
      .then(() => undefined)
      .catch((error) => {
        schemaReady = null;
        throw error;
      });
  }
  return schemaReady;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' },
  });
}

async function readBody(request: Request): Promise<Record<string, unknown>> {
  try {
    const value = (await request.json()) as unknown;
    return typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function normaliseCode(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const code = value.trim().toUpperCase().replace(/[^A-Z0-9]/g, '');
  return CODE_RE.test(code) ? code : null;
}

function cleanName(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const name = value.trim().replace(/\s+/g, ' ').slice(0, NAME_MAX);
  return name.length > 0 ? name : null;
}

function readGuest(value: unknown): string | null {
  return typeof value === 'string' && GUEST_RE.test(value) ? value : null;
}

function readAction(value: unknown): Action | null {
  if (!Array.isArray(value) || value.length !== 4) return null;
  const parts = value.every((entry) => typeof entry === 'number' && Number.isInteger(entry));
  return parts ? (value as Action) : null;
}

/** `null` once the room has expired or if it predates the current game shape. */
async function loadRoom(env: Env, code: string): Promise<{ row: RoomRow; state: RoomState } | null> {
  const row = await env.DB.prepare(
    'SELECT code, host_id, status, revision, state_json, expires_at FROM rooms WHERE code = ?',
  )
    .bind(code)
    .first<RoomRow>();

  if (!row) return null;

  if (row.expires_at <= Date.now()) {
    await env.DB.prepare('DELETE FROM rooms WHERE code = ?').bind(code).run();
    return null;
  }

  const state = JSON.parse(row.state_json) as RoomState;
  if (state.game && !Array.isArray(state.game.state)) {
    // A room created before the real rules landed: start again with the same code.
    await env.DB.prepare('DELETE FROM rooms WHERE code = ?').bind(code).run();
    return null;
  }

  return { row, state };
}

/** Compare-and-set on `revision`, so two concurrent actions cannot both land. */
async function writeRoom(
  env: Env,
  code: string,
  expectedRevision: number,
  status: Status,
  state: RoomState,
): Promise<boolean> {
  const result = await env.DB.prepare(
    `UPDATE rooms
        SET status = ?, revision = revision + 1, state_json = ?, expires_at = ?
      WHERE code = ? AND revision = ?`,
  )
    .bind(status, JSON.stringify(state), Date.now() + ROOM_TTL_MS, code, expectedRevision)
    .run();

  return result.meta.changes === 1;
}

function snapshot(row: RoomRow, state: RoomState, me: string | null): Snapshot {
  const seat = me ? state.seats.findIndex((entry) => entry.id === me) : -1;
  const game = state.game;
  const over = game ? isOver(game.state) : false;

  // Hands stay on the server: the view is built from a redacted copy, and only
  // the player whose turn it is receives their own legal actions.
  const view = game ? viewOf(redact(game.state, seat)) : null;
  const mine = game && seat >= 0 && !over && currentPlayer(game.state) === seat;
  const actions = mine
    ? legalActions(game.state).map((action) => ({ action, label: actionLabel(action) }))
    : [];

  return {
    code: row.code,
    status: over ? 'finished' : row.status,
    revision: row.revision,
    host: state.seats.findIndex((entry) => entry.id === row.host_id),
    seats: state.seats.map((entry) => ({ name: entry.name, bot: entry.bot === true })),
    maxPlayers: MAX_PLAYERS,
    you: seat >= 0 ? seat : null,
    view,
    actions,
    winner: over && game ? winnerOf(game.state) : null,
  };
}

/**
 * The one place a move is applied. Human moves and bot moves both come through
 * here, so a bot is held to exactly the rules a person is: not over, right
 * player's turn, action legal.
 */
function takeAction(
  game: Game,
  seat: number,
  action: Action,
): { ok: true; state: GameState } | { ok: false; error: string } {
  if (isOver(game.state)) return { ok: false, error: 'game-over' };
  if (currentPlayer(game.state) !== seat) return { ok: false, error: 'not-your-turn' };
  const next = applyAction(game.state, action);
  if (!next) return { ok: false, error: 'illegal-action' };
  return { ok: true, state: next };
}

/**
 * Plays bot turns until it is a person's turn (or the game ends). There are no
 * background workers, so this runs on the requests that could have handed the
 * turn to a bot, and on reads, so a stalled bot game recovers on the next poll.
 *
 * The bot driver is best-effort: a failure here must never fail the human's own
 * request, which has already been committed by the time this runs. Whatever is
 * left is picked up by the next poll, because reads drive bots too.
 */
async function runBots(env: Env, code: string): Promise<void> {
  try {
    await driveBots(env, code);
  } catch (error) {
    console.error('bot driver failed', code, error);
  }
}

async function driveBots(env: Env, code: string): Promise<void> {
  for (let step = 0; step < 64; step += 1) {
    const room = await loadRoom(env, code);
    if (!room) return;

    const { row, state } = room;
    if (row.status !== 'playing' || !state.game) return;

    const seat = currentPlayer(state.game.state);
    if (!state.seats[seat]?.bot) return;

    const action = botChoose(state.game.state);
    if (!action) {
      console.warn('bot produced no action', code, 'seat', seat, 'phase', state.game.state[2]);
      return;
    }

    const result = takeAction(state.game, seat, action);
    if (!result.ok) {
      // Should be impossible (botChoose comes from the same rules), so this is
      // a loud signal rather than a silent stall.
      console.error('bot action rejected', code, 'seat', seat, action, result.error);
      return;
    }

    const finished = isOver(result.state);
    const next: RoomState = { ...state, game: { ...state.game, state: result.state } };
    const status: Status = finished ? 'finished' : row.status;

    // A conflict means another request moved first; reload and re-check.
    await writeRoom(env, code, row.revision, status, next);
  }
}

async function addBot(request: Request, env: Env, code: string): Promise<Response> {
  const body = await readBody(request);
  const guestId = readGuest(body.guestId);
  if (!guestId) return json({ error: 'invalid-guest' }, 400);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const room = await loadRoom(env, code);
    if (!room) return json({ error: 'not-found' }, 404);

    const { row, state } = room;
    if (row.host_id !== guestId) return json({ error: 'not-host' }, 403);
    if (row.status !== 'lobby') return json({ error: 'already-started' }, 409);
    if (state.seats.length >= MAX_PLAYERS) return json({ error: 'room-full' }, 409);

    const bots = state.seats.filter((seat) => seat.bot).length;
    const next: RoomState = {
      ...state,
      seats: [...state.seats, { id: `bot-${crypto.randomUUID()}`, name: `Bot ${bots + 1}`, bot: true }],
    };

    if (await writeRoom(env, code, row.revision, row.status, next)) {
      return json(snapshot({ ...row, revision: row.revision + 1 }, next, guestId));
    }
  }

  return json({ error: 'conflict' }, 409);
}

async function createRoom(request: Request, env: Env): Promise<Response> {
  const body = await readBody(request);
  const code = normaliseCode(body.code);
  const name = cleanName(body.name);
  const guestId = readGuest(body.guestId);

  if (!code) return json({ error: 'invalid-code' }, 400);
  if (!name) return json({ error: 'invalid-name' }, 400);
  if (!guestId) return json({ error: 'invalid-guest' }, 400);

  const now = Date.now();
  await env.DB.prepare('DELETE FROM rooms WHERE code = ? AND expires_at <= ?').bind(code, now).run();

  const state: RoomState = { seats: [{ id: guestId, name }], game: null };
  try {
    await env.DB.prepare(
      `INSERT INTO rooms (code, host_id, status, revision, state_json, created_at, expires_at)
       VALUES (?, ?, 'lobby', 1, ?, ?, ?)`,
    )
      .bind(code, guestId, JSON.stringify(state), now, now + ROOM_TTL_MS)
      .run();
  } catch {
    return json({ error: 'code-taken' }, 409);
  }

  const row: RoomRow = {
    code,
    host_id: guestId,
    status: 'lobby',
    revision: 1,
    state_json: '',
    expires_at: now + ROOM_TTL_MS,
  };
  return json(snapshot(row, state, guestId), 201);
}

async function readRoom(env: Env, code: string, me: string | null): Promise<Response> {
  const room = await loadRoom(env, code);
  if (!room) return json({ error: 'not-found' }, 404);

  // A bot that owes a move and no request has driven it yet: let it play.
  const game = room.state.game;
  if (room.row.status === 'playing' && game && room.state.seats[currentPlayer(game.state)]?.bot) {
    await runBots(env, code);
    const after = await loadRoom(env, code);
    // The room can expire or be deleted while bots were playing: don't answer
    // with the snapshot we loaded before them.
    if (!after) return json({ error: 'not-found' }, 404);
    return json(snapshot(after.row, after.state, me));
  }

  return json(snapshot(room.row, room.state, me));
}

async function joinRoom(request: Request, env: Env, code: string): Promise<Response> {
  const body = await readBody(request);
  const name = cleanName(body.name);
  const guestId = readGuest(body.guestId);
  if (!name) return json({ error: 'invalid-name' }, 400);
  if (!guestId) return json({ error: 'invalid-guest' }, 400);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const room = await loadRoom(env, code);
    if (!room) return json({ error: 'not-found' }, 404);

    const { row, state } = room;
    if (state.seats.some((seat) => seat.id === guestId)) return json(snapshot(row, state, guestId));
    if (row.status !== 'lobby') return json({ error: 'already-started' }, 409);
    if (state.seats.length >= MAX_PLAYERS) return json({ error: 'room-full' }, 409);

    const next: RoomState = { ...state, seats: [...state.seats, { id: guestId, name }] };
    if (await writeRoom(env, code, row.revision, row.status, next)) {
      return json(snapshot({ ...row, revision: row.revision + 1 }, next, guestId));
    }
  }

  return json({ error: 'conflict' }, 409);
}

async function startRoom(request: Request, env: Env, code: string): Promise<Response> {
  const body = await readBody(request);
  const guestId = readGuest(body.guestId);
  if (!guestId) return json({ error: 'invalid-guest' }, 400);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const room = await loadRoom(env, code);
    if (!room) return json({ error: 'not-found' }, 404);

    const { row, state } = room;
    if (row.host_id !== guestId) return json({ error: 'not-host' }, 403);
    if (row.status !== 'lobby') return json({ error: 'already-started' }, 409);
    if (state.seats.length < 2) return json({ error: 'need-players' }, 400);

    const seed = crypto.getRandomValues(new Uint32Array(1))[0];
    const setup = seed % setupCount(state.seats.length);
    const next: RoomState = {
      ...state,
      game: { state: newGame(state.seats.length, seed, setup), seed, setup },
    };

    if (await writeRoom(env, code, row.revision, 'playing', next)) {
      await runBots(env, code);
      const fresh = await loadRoom(env, code);
      if (fresh) return json(snapshot(fresh.row, fresh.state, guestId));
      return json({ error: 'not-found' }, 404);
    }
  }

  return json({ error: 'conflict' }, 409);
}

async function playAction(request: Request, env: Env, code: string): Promise<Response> {
  const body = await readBody(request);
  const guestId = readGuest(body.guestId);
  if (!guestId) return json({ error: 'invalid-guest' }, 400);

  const revision = typeof body.revision === 'number' ? body.revision : Number.NaN;
  if (!Number.isInteger(revision)) return json({ error: 'invalid-revision' }, 400);

  const action = readAction(body.action);
  if (!action) return json({ error: 'invalid-action' }, 400);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const room = await loadRoom(env, code);
    if (!room) return json({ error: 'not-found' }, 404);

    const { row, state } = room;
    const seat = state.seats.findIndex((entry) => entry.id === guestId);
    if (seat < 0) return json({ error: 'not-in-room' }, 403);
    if (!state.game) return json({ error: 'not-playing' }, 409);

    if (row.revision !== revision) {
      return json({ ...snapshot(row, state, guestId), error: 'stale' }, 409);
    }

    // The rules decide: only the current player, and only a legal action.
    const result = takeAction(state.game, seat, action);
    if (!result.ok) return json({ error: result.error }, 409);

    const finished = isOver(result.state);
    const next: RoomState = { ...state, game: { ...state.game, state: result.state } };
    const status: Status = finished ? 'finished' : 'playing';

    if (await writeRoom(env, code, row.revision, status, next)) {
      // Hand the turn on: any bots that now owe a move play before we answer.
      await runBots(env, code);
      const fresh = await loadRoom(env, code);
      if (fresh) return json(snapshot(fresh.row, fresh.state, guestId));
      return json({ error: 'not-found' }, 404);
    }
  }

  return json({ error: 'conflict' }, 409);
}

async function handleApi(request: Request, env: Env, url: URL): Promise<Response> {
  const parts = url.pathname.split('/').filter(Boolean); // api / rooms / CODE / action
  const method = request.method.toUpperCase();

  if (parts.length === 2 && parts[1] === 'health') return json({ ok: true });
  if (parts[1] !== 'rooms') return json({ error: 'not-found' }, 404);

  if (parts.length === 2) {
    if (method === 'POST') return createRoom(request, env);
    return json({ error: 'method-not-allowed' }, 405);
  }

  const code = normaliseCode(parts[2]);
  if (!code) return json({ error: 'not-found' }, 404);

  const action = parts[3];
  if (!action) {
    if (method !== 'GET') return json({ error: 'method-not-allowed' }, 405);
    return readRoom(env, code, url.searchParams.get('me'));
  }

  if (method !== 'POST') return json({ error: 'method-not-allowed' }, 405);

  if (action === 'join') return joinRoom(request, env, code);
  if (action === 'bot') return addBot(request, env, code);
  if (action === 'start') return startRoom(request, env, code);
  if (action === 'action') return playAction(request, env, code);

  return json({ error: 'not-found' }, 404);
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname.startsWith('/api/')) {
      try {
        await initRules();
        await ensureSchema(env);
        return await handleApi(request, env, url);
      } catch (error) {
        console.error('api error', error);
        return json({ error: 'server-error' }, 500);
      }
    }

    return env.ASSETS.fetch(request);
  },
};
