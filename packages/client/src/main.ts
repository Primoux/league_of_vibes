import { render, h } from 'preact';
import { installAudio } from './audio/AudioManager.ts';
import { installFx } from './fx/index.ts';
import { installControls } from './input/controls.ts';
import { net } from './net/client.ts';
import { GameRenderer } from './render/GameRenderer.ts';
import { store } from './state.ts';
import { App } from './ui/App.tsx';

const params = new URLSearchParams(location.search);
const canvas = document.getElementById('game') as HTMLCanvasElement;

if (params.has('maplab')) {
  // Démo : carte seule, caméra libre, sans serveur.
  void import('./render/maplab.ts').then((m) => m.startMapLab(canvas));
} else {
  const renderer = new GameRenderer(canvas);
  installControls(renderer);
  installFx(renderer.scene);
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
