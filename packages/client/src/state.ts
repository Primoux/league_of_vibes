/**
 * État client partagé (lobby, partie, derniers snapshots). Propriétaire : claude-client.
 * L'UI (mistral-ui) s'y abonne en lecture via useStore(); le rendu (claude-client) et les FX (claude-fx) aussi.
 *
 * Champs « à haute fréquence » (hover, cursorGame, selfEntity, camera) : mis à jour par le renderer/input
 * via `store.setQuiet` (pas de notification à chaque frame). L'UI qui en a besoin à chaque frame les lit
 * directement dans `store.state` depuis un `renderer.onFrame`, ou réagit aux snapshots (20 Hz).
 */
import type { AbilitySlot, DraftState, EntityId, EntitySnapshot, GameOver, GameStart, LobbyState, Snapshot, SummonerSlot, Vec2 } from '@lov/shared';

export type Quality = 'low' | 'high';

/** Mode de ciblage en cours (cast normal : indicateur affiché, clic gauche pour confirmer). */
export type Targeting =
  | { kind: 'ability'; slot: AbilitySlot }
  | { kind: 'summoner'; slot: SummonerSlot }
  | { kind: 'attackMove' }
  | { kind: 'ward' }
  | null;

export interface Settings {
  /** Lance le sort immédiatement à l'appui de la touche (vers le curseur / l'entité survolée). */
  quickCast: boolean;
  quality: Quality;
  /** Déplacement de la caméra quand la souris touche le bord de l'écran. */
  edgePan: boolean;
  /** Vitesse de défilement caméra (unités de jeu / s). */
  cameraSpeed: number;
  /** Passe en mode jeu (plein écran + verrouillage clavier) au premier clic en partie. */
  autoFullscreen: boolean;
  /** Demande confirmation avant de quitter/recharger la page pendant une partie. */
  confirmLeave: boolean;
  /** Clic gauche au sol (sans sort en cours) = attaque-déplacement. */
  leftClickAttackMove: boolean;
}

export interface ClientState {
  connected: boolean;
  playerId: string | null;
  name: string;
  lobby: LobbyState | null;
  draft: DraftState | null;
  game: GameStart | null;
  /**
   * Dernier snapshot reçu, avec `entities` = vue COMPLÈTE fusionnée (les deltas sont appliqués par net/).
   * `events` = événements de CE snapshot uniquement.
   */
  snapshot: Snapshot | null;
  result: GameOver | null;
  error: string | null;
  /** Le serveur envoie `loading` : timestamp serveur de fin du chargement. */
  loadingEndsAt: number | null;

  // --- en jeu (claude-client)
  /** Entité du joueur local dans le dernier snapshot. */
  selfEntity: EntitySnapshot | null;
  /** Entité survolée par la souris (id), null sinon. */
  hover: EntityId | null;
  /** Position jeu du curseur sur le sol, null si hors carte. */
  cursorGame: Vec2 | null;
  /** Ciblage en cours (indicateur de sort à afficher par claude-fx). */
  targeting: Targeting;
  /** Caméra verrouillée sur le champion (Y). */
  cameraLocked: boolean;
  /** Centre de la caméra (coordonnées jeu), mis à jour chaque frame en silence (cadre de la minimap). */
  cameraCenter: Vec2;
  /**
   * Emprise de la vue caméra au sol (unités de jeu, silencieux, mis à jour chaque frame) : trapèze `quad`
   * (bas-gauche, bas-droite, haut-droite, haut-gauche à l'écran) et sa boîte englobante. Pour le cadre de la minimap.
   */
  cameraView: { quad: [Vec2, Vec2, Vec2, Vec2]; minX: number; minY: number; maxX: number; maxY: number; w: number; h: number };
  /** Demande de déplacement caméra (clic minimap…) : incrémenter `seq` pour une nouvelle demande. */
  cameraFocus: { pos: Vec2; seq: number } | null;

  // --- écrans transmis à l'UI par l'input
  shopOpen: boolean;
  scoreboardOpen: boolean; // maintenu tant que Tab est enfoncé
  statsOpen: boolean;
  settingsOpen: boolean;

  settings: Settings;
  /** Mode jeu (input/gameMode.ts) : page en plein écran, et touches système (Ctrl+W, Ctrl+Q…) capturées. */
  fullscreen: boolean;
  keyboardLocked: boolean;
  /** Latence aller-retour estimée (ms), si disponible. */
  rtt: number | null;
}

type Listener = (s: ClientState) => void;

export const DEFAULT_GAME_SETTINGS: Readonly<Settings> = {
  quickCast: false,
  quality: 'high',
  edgePan: true,
  cameraSpeed: 3000,
  autoFullscreen: true,
  confirmLeave: true,
  leftClickAttackMove: false,
};
const DEFAULT_SETTINGS = DEFAULT_GAME_SETTINGS;

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
    loadingEndsAt: null,
    selfEntity: null,
    hover: null,
    cursorGame: null,
    targeting: null,
    cameraLocked: true,
    cameraCenter: { x: 1500, y: 1500 },
    cameraView: { quad: [{ x: 250, y: 900 }, { x: 2750, y: 900 }, { x: 2900, y: 2300 }, { x: 100, y: 2300 }], minX: 100, minY: 900, maxX: 2900, maxY: 2300, w: 2800, h: 1400 },
    cameraFocus: null,
    shopOpen: false,
    scoreboardOpen: false,
    statsOpen: false,
    settingsOpen: false,
    settings: loadSettings(),
    fullscreen: false,
    keyboardLocked: false,
    rtt: null,
  };
  private listeners = new Set<Listener>();

  set(patch: Partial<ClientState>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l(this.state));
  }

  /** Mise à jour sans notification (champs à haute fréquence). Mutation en place : aucune allocation. */
  setQuiet<K extends keyof ClientState>(key: K, value: ClientState[K]): void {
    this.state[key] = value;
  }

  setSettings(patch: Partial<Settings>): void {
    const settings = { ...this.state.settings, ...patch };
    localStorageSet('lov.settings', JSON.stringify(settings));
    this.set({ settings });
  }

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }
}

function loadSettings(): Settings {
  const raw = localStorageGet('lov.settings');
  if (!raw) return { ...DEFAULT_SETTINGS };
  try {
    return { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<Settings>) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function localStorageGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}
function localStorageSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* stockage indisponible */
  }
}

export const store = new Store();
