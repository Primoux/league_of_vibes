import { render, h } from 'preact';
import { audio, installAudio } from './audio/AudioManager.ts';
import { fxIndicators, installFx, setFxQuality } from './fx/index.ts';
import { installControls } from './input/controls.ts';
import { net } from './net/client.ts';
import { GameRenderer, registerIndicatorRenderer } from './render/GameRenderer.ts';
import { fxIndicatorRenderer } from './render/fxIndicatorsAdapter.ts';
import { store } from './state.ts';
import { App } from './ui/App.tsx';

const params = new URLSearchParams(location.search);
const canvas = document.getElementById('game') as HTMLCanvasElement;

if (params.has('modellab')) {
  void import('./render/champions/portrait/modellab.ts').then(m => m.startModelLab(canvas));
} else if (params.has('melenchonlab')) {
  void import('./render/champions/melenchonlab.ts').then((m) => m.startMelenchonLab(canvas));
} else if (params.has('fxlab')) {
  // Banc d'essai FX sans serveur (claude-fx), chargé à la demande.
  void import('./fx/lab/fxlab.ts').then((m) => m.startFxLab());
} else if (params.has('maplab')) {
  // Démo : carte seule, caméra libre, sans serveur.
  void import('./render/maplab.ts').then((m) => m.startMapLab(canvas));
} else {
  const renderer = new GameRenderer(canvas);
  installControls(renderer);
  installFx(renderer);
  if (fxIndicators.attached) registerIndicatorRenderer(fxIndicatorRenderer(fxIndicators));
  // la qualité graphique (paramètres) pilote aussi les FX
  let fxQuality = store.state.settings.quality;
  setFxQuality(fxQuality);
  store.subscribe((s) => {
    if (s.settings.quality !== fxQuality) setFxQuality((fxQuality = s.settings.quality));
  });
  installAudio();
  // écouteur audio = centre de la caméra (objet mis à jour en place chaque frame : aucune allocation)
  audio.setListener(() => store.state.cameraCenter);
  render(h(App, {}), document.getElementById('ui')!);
  if (params.has('autoplay')) autoplay();
  // Reprise de session après rechargement de page (token en sessionStorage).
  else if (hasSessionToken()) net.connect(store.state.name || 'Invocateur');
}

/**
 * `?autoplay` (dev) : se connecte, crée une salle dev, se met prêt et lance la partie.
 * Avec le serveur en `SKIP_DRAFT=1`, une partie solo démarre sans aucun clic.
 */
function autoplay(): void {
  let started = false;
  net.on((msg) => {
    if (msg.t !== 'lobby' || started || msg.state.phase !== 'lobby') return;
    const me = msg.state.players.find((p) => p.id === store.state.playerId);
    if (!me) return;
    if (!me.ready) net.send({ t: 'setReady', ready: true });
    else if (me.isHost) {
      started = true;
      net.send({ t: 'startGame' });
    }
  });
  net.connect(store.state.name || 'Testeur');
  net.send({ t: 'createRoom', devMode: true });
}

function hasSessionToken(): boolean {
  try {
    return !!sessionStorage.getItem('lov.token');
  } catch {
    return false;
  }
}
