/**
 * Paramètres propres à l'UI, persistés en localStorage (`lov.ui`). Propriétaire : mistral-ui.
 * Les réglages de jeu (quickCast, quality, edgePan, cameraSpeed) vivent dans `store.settings` (claude-client) ;
 * le panneau Paramètres les modifie via `store.setSettings`.
 *  - mistral-economy (audio) : `volumes` (0..1 par canal). Si `AudioManager` exporte `setVolume(channel, v)`,
 *    l'UI l'appelle automatiquement. Lecture possible : `settings.state.volumes` + `settings.subscribe(fn)`.
 */
import * as audio from '../audio/AudioManager.ts';
import { MiniStore } from './useStore.ts';

export type AudioChannel = 'master' | 'sfx' | 'voice' | 'music' | 'ui';

export interface UiSettings {
  volumes: Record<AudioChannel, number>;
  hudScale: number; // 0.8 .. 1.3
  showTimers: boolean;
  reduceMotion: boolean;
}

export const DEFAULT_SETTINGS: UiSettings = {
  volumes: { master: 0.8, sfx: 0.8, voice: 0.9, music: 0.4, ui: 0.7 },
  hudScale: 1,
  showTimers: true,
  reduceMotion: false,
};

const KEY = 'lov.ui';

function load(): UiSettings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const p = JSON.parse(raw) as Partial<UiSettings>;
    return { ...DEFAULT_SETTINGS, ...p, volumes: { ...DEFAULT_SETTINGS.volumes, ...p.volumes } };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export const settings = new MiniStore<UiSettings>(load());

type SetVolume = (channel: string, v: number) => void;
function pushAudio(s: UiSettings): void {
  // recherche dynamique : AudioManager (mistral-economy) n'exporte pas encore setVolume
  const fn = Object.entries(audio).find(([k]) => k === 'setVolume')?.[1];
  if (typeof fn !== 'function') return;
  for (const [ch, v] of Object.entries(s.volumes)) (fn as SetVolume)(ch, v);
}

function applyDom(s: UiSettings): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  root.style.setProperty('--hud-scale', String(s.hudScale));
  root.classList.toggle('lov-reduce-motion', s.reduceMotion);
}

export function installSettings(): void {
  applyDom(settings.state);
  pushAudio(settings.state);
  settings.subscribe((s) => {
    try {
      localStorage.setItem(KEY, JSON.stringify(s));
    } catch {
      /* stockage indisponible */
    }
    applyDom(s);
    pushAudio(s);
  });
}
