/**
 * Paramètres joueur, persistés en localStorage. Propriétaire : mistral-ui.
 * Lecture par les autres modules : `settings.state` + `settings.subscribe(fn)`.
 *  - mistral-economy (audio) : `volumes` (0..1 par canal). Si `AudioManager` exporte `setVolume(channel, v)`,
 *    l'UI l'appelle automatiquement.
 *  - claude-client : `quickCast` (par sort), `graphics` ('low' | 'medium' | 'high'), `edgePan`, `cameraLock`.
 */
import * as audio from '../audio/AudioManager.ts';
import { MiniStore } from './useStore.ts';

export type AudioChannel = 'master' | 'sfx' | 'voice' | 'music' | 'ui';
export type Graphics = 'low' | 'medium' | 'high';

export interface Settings {
  volumes: Record<AudioChannel, number>;
  quickCast: Record<'Q' | 'W' | 'E' | 'R' | 'D' | 'F', boolean>;
  graphics: Graphics;
  edgePan: boolean;
  cameraLock: boolean;
  hudScale: number; // 0.8 .. 1.3
  showTimers: boolean;
  reduceMotion: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  volumes: { master: 0.8, sfx: 0.8, voice: 0.9, music: 0.4, ui: 0.7 },
  quickCast: { Q: false, W: false, E: false, R: false, D: true, F: true },
  graphics: 'high',
  edgePan: true,
  cameraLock: false,
  hudScale: 1,
  showTimers: true,
  reduceMotion: false,
};

const KEY = 'lov.settings';

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const p = JSON.parse(raw) as Partial<Settings>;
    return {
      ...DEFAULT_SETTINGS,
      ...p,
      volumes: { ...DEFAULT_SETTINGS.volumes, ...p.volumes },
      quickCast: { ...DEFAULT_SETTINGS.quickCast, ...p.quickCast },
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export const settings = new MiniStore<Settings>(load());

type SetVolume = (channel: string, v: number) => void;
function pushAudio(s: Settings): void {
  // recherche dynamique : AudioManager (mistral-economy) n'exporte pas encore setVolume
  const fn = Object.entries(audio).find(([k]) => k === 'setVolume')?.[1];
  if (typeof fn !== 'function') return;
  for (const [ch, v] of Object.entries(s.volumes)) (fn as SetVolume)(ch, v);
}

function applyDom(s: Settings): void {
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
