import { render, h } from 'preact';
import { installAudio } from './audio/AudioManager.ts';
import { installFx } from './fx/index.ts';
import { installControls } from './input/controls.ts';
import { GameRenderer } from './render/GameRenderer.ts';
import { App } from './ui/App.tsx';

if (new URLSearchParams(location.search).has('fxlab')) {
  // Banc d'essai FX sans serveur (claude-fx), chargé à la demande.
  void import('./fx/lab/fxlab.ts').then((m) => m.startFxLab());
} else {
  const canvas = document.getElementById('game') as HTMLCanvasElement;
  const renderer = new GameRenderer(canvas);
  installControls(renderer);
  installFx(renderer);
  installAudio();
  render(h(App, {}), document.getElementById('ui')!);
}
