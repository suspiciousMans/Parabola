/**
 * The 3D board. The Rust game (built to WebAssembly by `npm run build:game`,
 * served from /game/) renders the room; this module is its only link to the
 * page: it hands the game each snapshot the Worker sends and forwards the
 * actions the player picks. The Worker still validates every action.
 */
import type { Action, GameState } from '../game/view';

export type RoomSnapshot = {
  code: string;
  status: 'lobby' | 'playing' | 'finished';
  revision: number;
  you: number | null;
  seats: { name: string; bot: boolean }[];
  state: GameState | null;
  log: number[][];
  logTotal: number;
};

type Handlers = {
  submit: (action: Action) => void;
  leave: () => void;
};

type Payload = {
  code: string;
  revision: number;
  you: number;
  finished: boolean;
  seats: { name: string; bot: boolean }[];
  state: GameState;
  log: number[][];
  logTotal: number;
  error?: string;
};

declare global {
  interface Window {
    emberSite?: { take(): string; submit(action: Action): void; leave(): void };
    Module?: Record<string, unknown>;
  }
}

let started = false;
let pending: Payload | null = null;
let lastSent = '';

/** Can this browser run the 3D board? (Otherwise the plain HTML board is used.) */
export function canRunGame(): boolean {
  try {
    return new URLSearchParams(location.search).get('classic') === null && !!document.createElement('canvas').getContext('webgl2');
  } catch {
    return false;
  }
}

export function isGameRoom(room: RoomSnapshot | null): room is RoomSnapshot & { state: GameState; you: number } {
  return !!room && room.state !== null && room.you !== null && room.status !== 'lobby';
}

function payload(room: RoomSnapshot & { state: GameState; you: number }, error?: string): Payload {
  return {
    code: room.code,
    revision: room.revision,
    you: room.you,
    finished: room.status === 'finished',
    seats: room.seats,
    state: room.state,
    log: room.log,
    logTotal: room.logTotal,
    error,
  };
}

/** Shows (or updates) the 3D board for this room. Safe to call on every refresh. */
export function showGame(
  app: HTMLElement,
  room: RoomSnapshot & { state: GameState; you: number },
  handlers: Handlers,
  error?: string,
): void {
  pending = payload(room, error);
  if (started) return;
  started = true;

  app.innerHTML = `<div class="stage">
      <canvas id="canvas" tabindex="-1"></canvas>
      <div class="stagebar">
        <span>Room <b>${room.code}</b></span>
        <button id="invite" class="link">Copy invite link</button>
      </div>
    </div>`;
  document.body.classList.add('playing');
  document.querySelector('#invite')?.addEventListener('click', () => {
    void navigator.clipboard?.writeText(`${location.origin}/r/${room.code}`);
  });

  window.emberSite = {
    take(): string {
      if (!pending) return '';
      const text = JSON.stringify(pending);
      // The game asks many times a second: only answer when something changed.
      const key = `${pending.revision}:${pending.logTotal}:${pending.error ?? ''}:${pending.finished}`;
      if (key === lastSent) return '';
      lastSent = key;
      return text;
    },
    submit: handlers.submit,
    leave: handlers.leave,
  };

  const canvas = document.querySelector<HTMLCanvasElement>('#canvas')!;
  canvas.addEventListener('contextmenu', (event) => event.preventDefault());
  window.Module = {
    canvas,
    arguments: ['--site'],
    print: (text: string) => console.log(text),
    printErr: (text: string) => console.warn(text),
  };
  const script = document.createElement('script');
  script.src = '/game/ember_reach.js';
  document.body.appendChild(script);
}

/** Tells the game its last action was refused. */
export function gameError(room: RoomSnapshot & { state: GameState; you: number }, error: string): void {
  pending = payload(room, error);
}
