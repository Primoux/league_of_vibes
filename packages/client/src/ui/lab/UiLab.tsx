/**
 * `?uilab` : démo de toute l'UI sans serveur. Remplace `net.send` par un faux serveur (mock.ts) et
 * propose un panneau pour naviguer entre les écrans et déclencher des événements de jeu.
 */
import { LOADING_SECONDS } from '@lov/shared';
import { useEffect, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { handleMsg, labEvent, mockDraft, mockLobby, startGame, stopGame } from './mock.ts';

let patched = false;
function patchNet(): void {
  if (patched) return;
  patched = true;
  net.setLoopback(handleMsg);
  net.connect = () => store.set({ connected: true });
}

const SCREENS: [string, () => void][] = [
  ['Accueil', () => (stopGame(), store.set({ lobby: null, draft: null, game: null, snapshot: null, result: null, error: null }))],
  ['Lobby', () => (stopGame(), store.set({ playerId: 'p-me', lobby: mockLobby(), draft: null, game: null, snapshot: null, result: null }))],
  ['Draft (mon tour)', () => (stopGame(), store.set({ playerId: 'p-me', lobby: { ...mockLobby(8), phase: 'draft' }, draft: mockDraft(0), game: null, result: null }))],
  ['Draft (tour 3)', () => (stopGame(), store.set({ playerId: 'p-me', lobby: { ...mockLobby(8), phase: 'draft' }, draft: mockDraft(2), game: null, result: null }))],
  ['Finalisation', () => (stopGame(), store.set({ playerId: 'p-me', lobby: { ...mockLobby(8), phase: 'draft' }, draft: mockDraft(5), game: null, result: null }))],
  [
    'Chargement',
    () => {
      stopGame();
      store.set({ playerId: 'p-me', lobby: { ...mockLobby(8), phase: 'loading' }, draft: mockDraft(5), game: null, result: null });
      store.set({ loadingEndsAt: Date.now() + LOADING_SECONDS * 1000 });
    },
  ],
  ['Partie', () => startGame()],
];

const EVENTS: [string, string][] = [
  ['Kill ennemi', 'killEnemy'],
  ['Allié tué', 'killAlly'],
  ['Triple kill', 'multi'],
  ['Ace', 'ace'],
  ['Exécution', 'execute'],
  ['Mourir', 'die'],
  ['Tour ennemie', 'tower'],
  ['Tour perdue', 'towerLost'],
  ['Inhibiteur', 'inhib'],
  ['Drake', 'drake'],
  ['Léviathan', 'leviathan'],
  ['Annonce serveur', 'announce'],
  ['Niveau +1', 'levelUp'],
  ['+1500 or', 'gold'],
  ['Fontaine on/off', 'shopToggle'],
  ['Victoire', 'end'],
  ['Défaite', 'endLose'],
];

/** `?uilab=game&panel=shop&ev=multi` : ouvre directement un écran (captures automatiques, démos). */
function autoStart(): void {
  const q = new URLSearchParams(location.search);
  const screen = q.get('uilab');
  const idx: Record<string, number> = { home: 0, lobby: 1, draft: 2, draft3: 3, final: 4, loading: 5, game: 6 };
  const i = screen ? idx[screen] : undefined;
  if (i !== undefined) SCREENS[i]![1]();
  const ev = q.get('ev');
  if (ev) setTimeout(() => ev.split(',').forEach((e) => labEvent(e)), 300);
  if (screen === 'end' || screen === 'endlose') {
    startGame();
    labEvent(screen === 'end' ? 'end' : 'endLose');
  }
  const panel = q.get('panel');
  if (panel === 'shop') store.set({ shopOpen: true });
  if (panel === 'scores') store.set({ scoreboardOpen: true });
  if (panel === 'stats') store.set({ statsOpen: true });
  if (panel === 'settings') store.set({ settingsOpen: true });
  if (q.has('hidelab')) document.body.classList.add('lov-lab-hidden');
  if (q.has('still')) document.documentElement.classList.add('lov-still');
}

export function UiLab() {
  const [open, setOpen] = useState(true);
  const [inGame, setInGame] = useState(!!store.state.game);
  useEffect(() => {
    patchNet();
    document.body.classList.add('lov-uilab');
    autoStart();
    return store.subscribe((s) => setInGame(!!s.game));
  }, []);
  return (
    <div class={`lov-lab ${open ? '' : 'closed'}`}>
      <button class="lov-lab-toggle" onClick={() => setOpen(!open)}>
        {open ? '× UI Lab' : 'UI Lab'}
      </button>
      {open && (
        <div class="lov-lab-body">
          <h4>Écrans</h4>
          <div class="lov-lab-grid">
            {SCREENS.map(([l, fn]) => (
              <button key={l} onClick={fn}>
                {l}
              </button>
            ))}
          </div>
          {inGame && (
            <>
              <h4>Événements</h4>
              <div class="lov-lab-grid">
                {EVENTS.map(([l, k]) => (
                  <button key={k} onClick={() => labEvent(k)}>
                    {l}
                  </button>
                ))}
              </div>
              <p>P boutique · Tab scores · C stats · Échap paramètres · Q/W/E/R lancent (faux) sorts</p>
            </>
          )}
        </div>
      )}
    </div>
  );
}
