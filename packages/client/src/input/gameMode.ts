/**
 * Mode jeu et protections contre le navigateur. Propriétaire : claude-client.
 *
 * Le navigateur vole beaucoup d'entrées à un MOBA : barre d'onglets qui réapparaît en haut de l'écran
 * (plein écran F11 du navigateur), Ctrl+W / Ctrl+Q qui ferment l'onglet, boutons Précédent/Suivant de la
 * souris, molette cliquée (défilement automatique), Ctrl+molette (zoom de la page), menu contextuel,
 * sélection de texte, glisser d'images, F5…
 *
 * - **Mode jeu** (`enterGameMode`) : plein écran *de la page* (Fullscreen API : aucune barre du navigateur
 *   ne réapparaît au bord de l'écran) + Keyboard Lock API (Chrome/Edge/Opera : Ctrl+W, Ctrl+Q, Ctrl+T, Alt+Tab
 *   dans la fenêtre… arrivent au jeu au lieu du navigateur ; maintenir Échap pour quitter le plein écran).
 *   F11 est intercepté pour utiliser ce mode plutôt que le plein écran du navigateur.
 *   Avec `settings.autoFullscreen`, le premier clic en partie active le mode jeu.
 * - **Gardes** (actives en partie) : raccourcis du navigateur bloqués quand c'est possible, confirmation
 *   avant de quitter/recharger (`settings.confirmLeave`, filet de sécurité si Ctrl+W n'est pas capturable,
 *   ex. Firefox), navigation arrière neutralisée (boutons souris, Alt+←, geste), clic molette, Ctrl+molette,
 *   menu contextuel, sélection et glisser désactivés hors champs de saisie.
 */
import { store } from '../state.ts';

interface KeyboardApi {
  lock?(keyCodes?: string[]): Promise<void>;
  unlock?(): void;
  getLayoutMap?(): Promise<ReadonlyMap<string, string>>;
}

function keyboardApi(): KeyboardApi | undefined {
  return (navigator as Navigator & { keyboard?: KeyboardApi }).keyboard;
}

/** Keyboard Lock disponible (Chromium) : Ctrl+W/Q… capturables en plein écran. */
export function keyboardLockSupported(): boolean {
  return typeof keyboardApi()?.lock === 'function';
}

export function fullscreenSupported(): boolean {
  return typeof document !== 'undefined' && !!document.documentElement.requestFullscreen;
}

const inGame = () => !!store.state.game && !store.state.result;

/** Plein écran de la page + verrouillage clavier. À appeler depuis un geste utilisateur (clic, touche). */
export async function enterGameMode(): Promise<void> {
  if (!fullscreenSupported()) return;
  try {
    if (!document.fullscreenElement) await document.documentElement.requestFullscreen({ navigationUI: 'hide' });
  } catch {
    return; // refusé (pas de geste utilisateur, iframe sans permission…)
  }
  let locked = false;
  const kb = keyboardApi();
  if (kb?.lock) {
    try {
      await kb.lock();
      locked = true;
    } catch {
      /* non autorisé : le reste du mode jeu fonctionne */
    }
  }
  if (store.state.keyboardLocked !== locked) store.set({ keyboardLocked: locked });
}

export async function exitGameMode(): Promise<void> {
  keyboardApi()?.unlock?.();
  if (document.fullscreenElement) await document.exitFullscreen().catch(() => undefined);
}

export function toggleGameMode(): void {
  if (document.fullscreenElement) void exitGameMode();
  else void enterGameMode();
}

// --- disposition clavier (lettres des sorts même avec Ctrl/Alt/Option enfoncés)
let layoutMap: ReadonlyMap<string, string> | null = null;

/**
 * Lettre de la touche (majuscule), en respectant la disposition (AZERTY, QWERTZ…).
 * `e.key` est fiable sans modificateur ; avec Alt/Option (macOS : Alt+Q = « œ ») ou Ctrl, on passe par
 * la carte de disposition (Chromium) ou, à défaut, par la position physique.
 */
export function keyLetter(e: KeyboardEvent): string {
  const k = e.key;
  if (k.length === 1) {
    const u = k.toUpperCase();
    if (u >= 'A' && u <= 'Z') return u;
  }
  const mapped = layoutMap?.get(e.code);
  if (mapped && mapped.length === 1) {
    const u = mapped.toUpperCase();
    if (u >= 'A' && u <= 'Z') return u;
  }
  return e.code.startsWith('Key') && e.code.length === 4 ? e.code[3]! : '';
}

const isEditable = (t: EventTarget | null): boolean => {
  const el = t as HTMLElement | null;
  return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable);
};

/** Touches de fonction gênantes en partie (aide, recherche, rechargement, barre d'adresse, navigation au curseur, menu). */
const BLOCKED_F_KEYS = new Set(['F1', 'F3', 'F5', 'F6', 'F7', 'F10']);

/** Ctrl/Cmd + touche : on bloque tout en partie, sauf copier/coller/tout sélectionner dans un champ et les outils de dev. */
function shouldBlockCtrl(e: KeyboardEvent): boolean {
  if (e.shiftKey && (e.code === 'KeyI' || e.code === 'KeyJ' || e.code === 'KeyC')) return false; // outils de dev
  if (isEditable(e.target) && (e.code === 'KeyA' || e.code === 'KeyC' || e.code === 'KeyV' || e.code === 'KeyX' || e.code === 'KeyZ')) return false;
  return true;
}

export function installGameMode(): void {
  if (typeof window === 'undefined') return;

  const kb = keyboardApi();
  kb?.getLayoutMap?.().then((m) => (layoutMap = m)).catch(() => undefined);

  // --- état plein écran
  let userExitedThisGame = false;
  let lastGameId: number | null = null;
  document.addEventListener('fullscreenchange', () => {
    const fs = !!document.fullscreenElement;
    if (!fs) {
      keyboardApi()?.unlock?.();
      if (inGame()) userExitedThisGame = true; // ne pas le relancer au clic suivant : l'utilisateur l'a quitté
    }
    store.set({ fullscreen: fs, keyboardLocked: fs && store.state.keyboardLocked });
  });

  // nouvelle partie : on réarme le passage automatique en mode jeu et le verrou d'historique
  store.subscribe((s) => {
    const id = s.game ? s.game.entityId : null;
    if (id === lastGameId) return;
    lastGameId = id;
    if (id !== null) {
      userExitedThisGame = false;
      armHistoryGuard();
    }
  });

  // --- premier clic en partie → mode jeu
  addEventListener(
    'pointerdown',
    () => {
      if (!inGame() || !store.state.settings.autoFullscreen || userExitedThisGame) return;
      if (document.fullscreenElement || !fullscreenSupported()) return;
      void enterGameMode();
    },
    true,
  );

  // --- clavier
  addEventListener(
    'keydown',
    (e) => {
      if (e.code === 'F11') {
        // F11 : plein écran de la page (pas celui du navigateur, qui laisse revenir la barre d'onglets)
        e.preventDefault();
        if (!e.repeat) toggleGameMode();
        return;
      }
      if (!inGame()) return;
      if (BLOCKED_F_KEYS.has(e.code)) {
        e.preventDefault();
        return;
      }
      // Alt+←/→ : navigation arrière/avant
      if (e.altKey && (e.code === 'ArrowLeft' || e.code === 'ArrowRight')) {
        e.preventDefault();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && shouldBlockCtrl(e)) e.preventDefault();
      // Retour arrière hors champ de saisie (ancienne navigation arrière)
      if (e.code === 'Backspace' && !isEditable(e.target)) e.preventDefault();
    },
    true,
  );
  // Alt relâché seul : ouvre la barre de menus (Firefox / Windows) et vole le focus
  addEventListener(
    'keyup',
    (e) => {
      if (inGame() && (e.key === 'Alt' || e.key === 'AltGraph' || e.code === 'F10')) e.preventDefault();
    },
    true,
  );

  // --- souris
  const blockAux = (e: MouseEvent) => {
    if (!inGame()) return;
    // 1 = molette (défilement automatique, collage Linux), 3/4 = Précédent/Suivant
    if (e.button === 1 || e.button === 3 || e.button === 4) e.preventDefault();
  };
  addEventListener('mousedown', blockAux, true);
  addEventListener('mouseup', blockAux, true);
  addEventListener('auxclick', blockAux, true);
  addEventListener('pointerdown', blockAux, true);
  addEventListener('pointerup', blockAux, true);

  addEventListener(
    'wheel',
    (e) => {
      if (inGame() && (e.ctrlKey || e.metaKey)) e.preventDefault(); // zoom de la page
    },
    { passive: false, capture: true },
  );
  addEventListener(
    'contextmenu',
    (e) => {
      if (inGame() && !isEditable(e.target)) e.preventDefault();
    },
    true,
  );
  addEventListener(
    'dragstart',
    (e) => {
      if (inGame() && !isEditable(e.target)) e.preventDefault();
    },
    true,
  );
  addEventListener(
    'selectstart',
    (e) => {
      if (inGame() && !isEditable(e.target)) e.preventDefault();
    },
    true,
  );

  // --- quitter / recharger la page
  addEventListener('beforeunload', (e) => {
    if (!inGame() || !store.state.settings.confirmLeave) return;
    e.preventDefault();
    e.returnValue = ''; // anciens navigateurs
  });

  // --- navigation arrière (bouton souris non bloquable, geste, Alt+← sous Firefox…)
  addEventListener('popstate', () => {
    if (inGame()) history.pushState({ lovGuard: true }, '', location.href);
  });
}

let historyArmed = false;
function armHistoryGuard(): void {
  if (historyArmed) return;
  historyArmed = true;
  try {
    history.pushState({ lovGuard: true }, '', location.href);
  } catch {
    /* historique indisponible */
  }
}
