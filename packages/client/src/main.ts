import { render, h } from 'preact';
import { installAudio } from './audio/AudioManager.ts';
import { fxIndicators, installFx, setFxQuality } from './fx/index.ts';
import { installControls } from './input/controls.ts';
import { net } from './net/client.ts';
import { GameRenderer, registerIndicatorRenderer } from './render/GameRenderer.ts';
import { fxIndicatorRenderer } from './render/fxIndicatorsAdapter.ts';
import { store } from './state.ts';
import { App } from './ui/App.tsx';

const params = new URLSearchParams(location.search);
const canvas = document.getElementById('game') as HTMLCanvasElement;

if (params.has('fxlab')) {
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
  render(h(App, {}), document.getElementById('ui')!);
  // Reprise de session après rechargement de page (token en sessionStorage).
  if (hasSessionToken()) net.connect(store.state.name || 'Invocateur');
}

function hasSessionToken(): boolean {
  try {
    return !!sessionStorage.getItem('lov.token');
  } catch {
    return false;
  }
}
