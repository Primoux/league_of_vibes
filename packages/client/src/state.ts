/**
 * État client partagé (lobby, partie, derniers snapshots). Propriétaire : claude-client.
 * L'UI (mistral-ui) s'y abonne en lecture via useStore(); le rendu (claude-client) et les FX (claude-fx) aussi.
 */
import type { DraftState, GameOver, GameStart, LobbyState, Snapshot } from '@lov/shared';

export interface ClientState {
  connected: boolean;
  playerId: string | null;
  name: string;
  lobby: LobbyState | null;
  draft: DraftState | null;
  game: GameStart | null;
  snapshot: Snapshot | null;
  result: GameOver | null;
  error: string | null;
}

type Listener = (s: ClientState) => void;

class Store {
  state: ClientState = {
    connected: false,
    playerId: null,
    name: localStorageGet('lov.name') ?? '',
    lobby: null,
    draft: null,
    game: null,
    snapshot: null,
    result: null,
    error: null,
  };
  private listeners = new Set<Listener>();

  set(patch: Partial<ClientState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l(this.state));
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }
}

function localStorageGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

export const store = new Store();
