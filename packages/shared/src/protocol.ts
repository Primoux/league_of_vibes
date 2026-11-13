/**
 * Protocole WebSocket client <-> serveur (JSON). Propriétaire : claude-server.
 * Toute modification = commit `contract:` + note dans docs/progress.
 */
import type {
  AbilitySlot,
  EntityId,
  EntitySnapshot,
  GameEvent,
  PlayerId,
  Role,
  SelfState,
  SummonerId,
  SummonerSlot,
  Team,
  Vec2,
} from './types.ts';

export const PROTOCOL_VERSION = 1;
export const WS_PATH = '/ws';

export type RoomPhase = 'lobby' | 'draft' | 'loading' | 'game' | 'end';

export interface LobbyPlayer {
  id: PlayerId;
  name: string;
  team: Team;
  ready: boolean;
  isHost: boolean;
  connected: boolean;
  role?: Role;
}

export interface LobbyState {
  code: string;
  phase: RoomPhase;
  players: LobbyPlayer[];
  devMode: boolean; // autorise une partie à moins de 8 joueurs (tests)
}

export interface DraftPick {
  playerId: PlayerId;
  team: Team;
  championId: string | null; // null = pas encore choisi
  hoverChampionId: string | null;
  locked: boolean;
  summoners: [SummonerId, SummonerId];
}

export interface DraftState {
  /** Ordre des tours de pick : chaque tour liste les joueurs qui choisissent simultanément. Ordre 1-2-2-2-1. */
  turns: PlayerId[][];
  currentTurn: number;
  turnEndsAt: number; // timestamp serveur (ms)
  picks: DraftPick[];
  /** Fin de la phase de finalisation (sorts d'invocateur, swaps) après le dernier pick. */
  finalizeEndsAt?: number;
}

export interface GameStart {
  you: PlayerId;
  team: Team;
  entityId: EntityId;
  players: { id: PlayerId; name: string; team: Team; championId: string; summoners: [SummonerId, SummonerId] }[];
  serverTime: number;
}

export interface Snapshot {
  tick: number;
  time: number; // secondes de jeu depuis le début
  serverTime: number; // ms epoch, pour l'interpolation
  entities: EntitySnapshot[];
  /** Ids d'entités qui n'existent plus ou ne sont plus visibles (snapshots delta). */
  removed?: EntityId[];
  events: GameEvent[];
  self?: SelfState;
  /** Timers d'objectifs connus de l'équipe (secondes de jeu du prochain spawn). */
  objectiveTimers?: Record<string, number>;
  score: { blue: { kills: number; towers: number; dragons: string[]; grubs: number }; red: { kills: number; towers: number; dragons: string[]; grubs: number } };
}

export interface GameOver {
  winner: Team;
  duration: number;
  stats: {
    playerId: PlayerId;
    championId: string;
    team: Team;
    kills: number;
    deaths: number;
    assists: number;
    cs: number;
    gold: number;
    damageToChampions: number;
    items: (string | null)[];
  }[];
}

// ---------------------------------------------------------------- Client -> Serveur
export type ClientMsg =
  | { t: 'hello'; name: string; version: number; reconnectToken?: string }
  | { t: 'createRoom'; devMode?: boolean }
  | { t: 'joinRoom'; code: string }
  | { t: 'leaveRoom' }
  | { t: 'setTeam'; team: Team }
  | { t: 'setRole'; role: Role }
  | { t: 'setReady'; ready: boolean }
  | { t: 'startGame' } // hôte uniquement
  | { t: 'draftHover'; championId: string }
  | { t: 'draftLock'; championId: string }
  | { t: 'draftSummoners'; summoners: [SummonerId, SummonerId] }
  // --- en jeu
  | { t: 'move'; pos: Vec2 }
  | { t: 'attackMove'; pos: Vec2 }
  | { t: 'attack'; targetId: EntityId }
  | { t: 'stop' }
  | { t: 'cast'; slot: AbilitySlot; targetPos?: Vec2; targetId?: EntityId }
  | { t: 'summoner'; slot: SummonerSlot; targetPos?: Vec2; targetId?: EntityId }
  | { t: 'levelAbility'; slot: AbilitySlot }
  | { t: 'buy'; itemId: string }
  | { t: 'sell'; slot: number }
  | { t: 'undoShop' }
  | { t: 'recall' }
  | { t: 'placeWard'; pos: Vec2 }
  | { t: 'ping'; pos: Vec2; kind: string }
  | { t: 'chat'; text: string; teamOnly: boolean };

export type ClientMsgType = ClientMsg['t'];

// ---------------------------------------------------------------- Serveur -> Client
export type ServerMsg =
  | { t: 'welcome'; playerId: PlayerId; reconnectToken: string; version: number }
  | { t: 'error'; message: string }
  | { t: 'lobby'; state: LobbyState }
  | { t: 'draft'; state: DraftState }
  | { t: 'loading'; endsAt: number }
  | { t: 'gameStart'; start: GameStart }
  | { t: 'snapshot'; snap: Snapshot }
  | { t: 'gameOver'; result: GameOver }
  | { t: 'pong'; serverTime: number };

export type ServerMsgType = ServerMsg['t'];

export function encode(msg: ClientMsg | ServerMsg): string {
  return JSON.stringify(msg);
}
export function decode<T extends ClientMsg | ServerMsg>(raw: string): T {
  return JSON.parse(raw) as T;
}
