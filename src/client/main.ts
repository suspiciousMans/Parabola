import { AMBITIONS, LEADERS, RESOURCES, SUITS, cardName, type Action, type PublicView } from '../game/view';
import './styles.css';
import { canRunGame, gameError, isGameRoom, showGame } from './game';

type Status = 'lobby' | 'playing' | 'finished';

type Snapshot = {
  code: string;
  status: Status;
  revision: number;
  host: number;
  seats: { name: string; bot: boolean }[];
  maxPlayers: number;
  you: number | null;
  view: PublicView | null;
  state: number[] | null;
  log: number[][];
  logTotal: number;
  actions: { action: Action; label: string }[];
  winner: number | null;
};

const app = document.querySelector<HTMLDivElement>('#app')!;

const stored = (key: string): string => {
  try {
    return localStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
};

const store = (key: string, value: string): void => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private mode: preferences simply won't persist.
  }
};

const sessionGet = (key: string): string => {
  try {
    return sessionStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
};

const sessionSet = (key: string, value: string): void => {
  try {
    sessionStorage.setItem(key, value);
  } catch {
    // Private mode: the guest id simply won't persist across reloads.
  }
};

// Identity is per tab, not per browser: two tabs are two players. sessionStorage
// survives a reload of the tab, but a second tab gets its own guest id (with
// localStorage both tabs would act as the same seat).
let guestId = sessionGet('parabola.guest');
if (!guestId) {
  guestId = crypto.randomUUID();
  sessionSet('parabola.guest', guestId);
}

let playerName = stored('parabola.name');
let room: Snapshot | null = null;
let polling: number | null = null;
let message = '';
let pending: Action[] = [];

const pathCode = (): string => {
  const match = location.pathname.match(/^\/r\/([A-Za-z0-9]{3,8})$/);
  return match ? match[1].toUpperCase() : '';
};

const escapeHtml = (value: string): string =>
  value.replace(/[&<>"']/g, (char) => {
    switch (char) {
      case '&':
        return '&amp;';
      case '<':
        return '&lt;';
      case '>':
        return '&gt;';
      case '"':
        return '&quot;';
      default:
        return '&#39;';
    }
  });

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, { ...init, headers: { 'content-type': 'application/json' } });
  const data = (await response.json()) as T & { error?: string };
  if (!response.ok) throw new Error(data.error ?? `request-failed-${response.status}`);
  return data;
}

const errorText: Record<string, string> = {
  'code-taken': 'That code is already taken. Try another one.',
  'room-full': 'That room already has four players.',
  'already-started': 'That game has already started.',
  'need-players': 'You need at least two players to start.',
  'not-host': 'Only the host can start the game.',
  'not-your-turn': 'It is not your turn.',
  'illegal-action': 'The rules rejected that action.',
  'game-over': 'That game is already over.',
  stale: 'The game moved on — refreshed the board.',
  'not-found': 'No room with that code.',
  'invalid-code': 'Codes are 3-8 letters or digits.',
  'invalid-name': 'Pick a name first.',
};

const describe = (error: unknown): string => {
  const code = error instanceof Error ? error.message : String(error);
  return errorText[code] ?? code;
};

function stopPolling(): void {
  if (polling !== null) {
    window.clearInterval(polling);
    polling = null;
  }
}

function startPolling(): void {
  stopPolling();
  polling = window.setInterval(() => {
    void refresh();
  }, 800);
}

async function refresh(): Promise<void> {
  if (!room) return;
  try {
    const next = await call<Snapshot>(
      `/api/rooms/${room.code}?me=${encodeURIComponent(guestId)}`,
      { cache: 'no-store' },
    );
    // Only re-render when the game actually moved: re-rendering an unchanged
    // position on every poll would reset the action list's scroll position.
    const changed = !room || next.revision !== room.revision || next.status !== room.status;
    room = next;
    if (next.status === 'finished') stopPolling();
    if (changed) render();
  } catch (error) {
    if (error instanceof Error && error.message === 'not-found') {
      room = null;
      stopPolling();
      message = 'That room has expired.';
      render();
    }
  }
}

const resourceChips = (seat: PublicView['seats'][number]): string =>
  seat.resources
    .map((count, index) => `${RESOURCES[index].slice(0, 3)} ${count}`)
    .join(' · ');

function board(view: PublicView): string {
  const rows = view.systems
    .map((system, index) => {
      if (!system.inPlay) return '';
      const ships = system.ships
        .map(([fresh, damaged], player) => (fresh + damaged > 0 ? `P${player + 1}: ${fresh}+${damaged}d` : ''))
        .filter(Boolean)
        .join(' ');
      const buildings = system.slots
        .filter((slot) => slot.kind !== 0)
        .map((slot) => `${slot.kind === 1 ? 'city' : 'port'} P${slot.owner + 1}${slot.damaged ? ' (damaged)' : ''}`)
        .join(', ');
      if (!ships && !buildings) return '';
      return `<li><b>${index}</b> ${ships ? escapeHtml(ships) : '<span class="muted">no ships</span>'}${
        buildings ? ` — ${escapeHtml(buildings)}` : ''
      }</li>`;
    })
    .join('');
  return `<ul class="board">${rows || '<li class="muted">nothing on the board yet</li>'}</ul>`;
}

function scoreboard(view: PublicView, snapshot: Snapshot): string {
  const rows = view.seats
    .map((seat, index) => {
      const isBot = snapshot.seats[index]?.bot === true;
      // Nobody is "on turn" once the game is over.
      const isTurn = snapshot.status !== 'finished' && index === view.current;
      const tags: string[] = [];
      if (index === snapshot.host) tags.push('host');
      if (isTurn) tags.push('turn');
      if (index === snapshot.you) tags.push('you');
      return `<tr class="${isTurn ? 'acting' : ''}">
        <td data-label="player">${escapeHtml(snapshot.seats[index]?.name ?? `P${index + 1}`)}${isBot ? ' <span class="tag bot">bot</span>' : ''}${
          tags.length ? ` <span class="tag">${tags.join(' ')}</span>` : ''
        }</td>
        <td data-label="leader">${escapeHtml(LEADERS[seat.leader] ?? '?')}${seat.lore > 0 ? ` · lore ${seat.lore}` : ''}</td>
        <td data-label="power">${seat.power}</td>
        <td data-label="resources">${escapeHtml(resourceChips(seat))}</td>
        <td data-label="cities left">${seat.citiesLeft}</td>
        <td data-label="trophies">${seat.trophies}</td>
      </tr>`;
    })
    .join('');
  return `<table class="score">
    <thead><tr><th>player</th><th>leader</th><th>power</th><th>resources</th><th>cities left</th><th>trophies</th></tr></thead>
    <tbody>${rows}</tbody>
  </table>`;
}

function home(): string {
  const prefill = pathCode();
  return `<main class="card">
    <h1>Ember Reach</h1>
    <p class="muted">Trick-taking area control for 2–4 players. Pick a room code and share it.</p>
    <label>Room code<input id="code" maxlength="8" placeholder="MIRA" value="${escapeHtml(prefill)}" /></label>
    <label>Your name<input id="name" maxlength="16" placeholder="player" value="${escapeHtml(playerName)}" /></label>
    <div class="row">
      <button id="create">Create room</button>
      <button id="join" class="ghost">Join room</button>
    </div>
    ${message ? `<p class="error">${escapeHtml(message)}</p>` : ''}
  </main>`;
}

function roomView(snapshot: Snapshot): string {
  const shareUrl = `${location.origin}/r/${snapshot.code}`;
  const isHost = snapshot.you !== null && snapshot.you === snapshot.host;
  const view = snapshot.view;

  const seatsHtml = snapshot.seats
    .map((seat, index) => `<li>${index + 1}. ${escapeHtml(seat.name)}${
      index === snapshot.host ? ' <span class="tag">host</span>' : ''
    }${seat.bot ? ' <span class="tag bot">bot</span>' : ''}${
      index === snapshot.you ? ' <span class="tag you">you</span>' : ''
    }</li>`)
    .join('');
  const emptySeats = snapshot.maxPlayers - snapshot.seats.length;

  let body = '';
  if (!view) {
    body = `<ul class="seats">${seatsHtml}${Array.from(
      { length: Math.max(0, emptySeats) },
      (_, index) => `<li class="empty">${snapshot.seats.length + index + 1}. empty seat</li>`,
    ).join('')}</ul>
      ${
        isHost
          ? `<div class="row">
              <button id="start" ${snapshot.seats.length < 2 ? 'disabled' : ''}>Start game</button>
              <button id="addBot" class="ghost" ${emptySeats <= 0 ? 'disabled' : ''}>Add bot</button>
            </div>`
          : '<p class="muted">Waiting for the host to start.</p>'
      }
      ${snapshot.seats.length < 2 ? '<p class="muted">Needs at least two players — add a bot to play solo.</p>' : ''}`;
  } else {
    const turnName = snapshot.seats[view.current]?.name ?? `P${view.current + 1}`;
    const myTurn = snapshot.you !== null && snapshot.you === view.current;
    const over = snapshot.status === 'finished';
    const winner = snapshot.winner !== null ? snapshot.seats[snapshot.winner]?.name : null;
    const hand = snapshot.you !== null ? view.seats[snapshot.you]?.hand ?? [] : [];
    const actions = snapshot.actions
      .map((entry, index) => `<button class="action" data-index="${index}">${escapeHtml(entry.label)}</button>`)
      .join('');

    body = `${scoreboard(view, snapshot)}
      <p class="muted">Chapter ${view.chapter} · ${view.phase}${
        view.leadSuit !== null ? ` · led ${SUITS[view.leadSuit] ?? ''}` : ''
      }${view.ambitions.some((rank) => rank > 0) ? ` · declared ${view.ambitions.map((r, i) => (r > 0 ? AMBITIONS[i] : '')).filter(Boolean).join(', ')}` : ''}</p>
      ${hand.length ? `<p class="muted">Your hand: ${hand.map((card) => `<span class="chip">${escapeHtml(cardName(card))}</span>`).join(' ')}</p>` : ''}
      ${over ? `<p class="win">${winner ? `${escapeHtml(winner)} wins.` : 'Game over.'}</p>` : ''}
      ${board(view)}
      ${
        over
          ? '<button id="leave" class="ghost">Leave room</button>'
          : myTurn
            ? `<div class="actions">${actions || '<p class="muted">No actions available.</p>'}</div>`
            : `<p class="muted">Waiting for ${escapeHtml(turnName)}.</p>`
      }`;
  }

  return `<main class="card">
    <header class="head">
      <h1>Room ${escapeHtml(snapshot.code)}</h1>
      <p class="muted">Invite: <code>${escapeHtml(shareUrl)}</code></p>
    </header>
    ${body}
    ${message ? `<p class="error">${escapeHtml(message)}</p>` : ''}
    <button id="copy" class="link">Copy invite link</button>
  </main>`;
}

function leaveRoom(): void {
  // The 3D board can't be unloaded, so going back home is a fresh page load.
  stopPolling();
  room = null;
  location.assign('/');
}

function render(): void {
  if (isGameRoom(room) && canRunGame()) {
    showGame(app, room, { submit: (action) => void play(action), leave: leaveRoom }, message || undefined);
    message = '';
    return;
  }
  app.innerHTML = room ? roomView(room) : home();
  message = '';

  if (!room) {
    document.querySelector('#create')?.addEventListener('click', () => void submit('create'));
    document.querySelector('#join')?.addEventListener('click', () => void submit('join'));
    return;
  }

  document.querySelector('#copy')?.addEventListener('click', () => {
    void navigator.clipboard?.writeText(`${location.origin}/r/${room!.code}`);
  });
  document.querySelector('#start')?.addEventListener('click', () => void startGame());
  document.querySelector('#addBot')?.addEventListener('click', () => void addBot());
  document.querySelector('#leave')?.addEventListener('click', () => {
    stopPolling();
    room = null;
    history.replaceState(null, '', '/');
    render();
  });

  pending = room.actions.map((entry) => entry.action);
  document.querySelectorAll<HTMLButtonElement>('.action').forEach((button) => {
    button.addEventListener('click', () => {
      const index = Number(button.dataset.index);
      const action = pending[index];
      if (action) void play(action);
    });
  });
}

async function submit(kind: 'create' | 'join'): Promise<void> {
  const codeInput = document.querySelector<HTMLInputElement>('#code');
  const nameInput = document.querySelector<HTMLInputElement>('#name');
  const code = (codeInput?.value ?? '').trim().toUpperCase().replace(/[^A-Z0-9]/g, '');
  const name = (nameInput?.value ?? '').trim();

  if (!name) {
    message = describe('invalid-name');
    render();
    return;
  }

  playerName = name;
  store('parabola.name', name);

  try {
    const snapshot =
      kind === 'create'
        ? await call<Snapshot>('/api/rooms', {
            method: 'POST',
            body: JSON.stringify({ code, name, guestId }),
          })
        : await call<Snapshot>(`/api/rooms/${code}/join`, {
            method: 'POST',
            body: JSON.stringify({ name, guestId }),
          });

    room = snapshot;
    history.replaceState(null, '', `/r/${snapshot.code}`);
    startPolling();
    render();
  } catch (error) {
    message = describe(error);
    render();
  }
}

async function addBot(): Promise<void> {
  if (!room) return;
  try {
    room = await call<Snapshot>(`/api/rooms/${room.code}/bot`, {
      method: 'POST',
      body: JSON.stringify({ guestId }),
    });
    render();
  } catch (error) {
    message = describe(error);
    render();
  }
}

async function startGame(): Promise<void> {
  if (!room) return;
  try {
    room = await call<Snapshot>(`/api/rooms/${room.code}/start`, {
      method: 'POST',
      body: JSON.stringify({ guestId }),
    });
    startPolling();
    render();
  } catch (error) {
    message = describe(error);
    render();
  }
}

async function play(action: Action): Promise<void> {
  if (!room) return;
  try {
    room = await call<Snapshot>(`/api/rooms/${room.code}/action`, {
      method: 'POST',
      body: JSON.stringify({ guestId, action, revision: room.revision }),
    });
    if (room.status === 'finished') stopPolling();
    render();
  } catch (error) {
    message = describe(error);
    await refresh();
    if (isGameRoom(room)) gameError(room, message);
    render();
  }
}

render();
