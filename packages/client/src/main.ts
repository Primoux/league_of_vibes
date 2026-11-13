import { render, h } from 'preact';
import { installAudio } from './audio/AudioManager.ts';
import { installFx } from './fx/index.ts';
import { installControls } from './input/controls.ts';
import { GameRenderer } from './render/GameRenderer.ts';
import { App } from './ui/App.tsx';

const canvas = document.getElementById('game') as HTMLCanvasElement;
const renderer = new GameRenderer(canvas);
installControls(renderer);
installFx(renderer.scene);
installAudio();
render(h(App, {}), document.getElementById('ui')!);
