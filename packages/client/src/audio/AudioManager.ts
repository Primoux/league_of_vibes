/**
 * Gestionnaire audio du jeu. Propriétaire : mistral-economy (PLAN.md §12).
 * WebAudio, canaux (master, effets, voix/annonces, musique, interface) exposés pour les
 * paramètres (mistral-ui), spatialisation simple (atténuation selon la distance à la caméra),
 * limitation de polyphonie. S'abonne au fxBus (GameEvent) via installAudio().
 * Tous les sons sont synthétisés (voir synth.ts) : aucun asset externe.
 */
import type { GameEvent, Vec2 } from '@lov/shared';
import { fxBus } from '../fx/FxBus.ts';
import { store } from '../state.ts';
import { playEvent, resetKillAnnouncements } from './sounds.ts';
import { music } from './music.ts';

export type AudioChannel = 'master' | 'sfx' | 'voice' | 'music' | 'ui';
export const AUDIO_CHANNELS: AudioChannel[] = ['master', 'sfx', 'voice', 'music', 'ui'];
const MAX_SFX = 24; // polyphonie max sur le canal sfx
const VOL_KEY = 'lov.audio';

export type SfxFn = (ctx: AudioContext, dest: AudioNode) => void;

export interface SfxOpts {
  pos?: Vec2; // position en unités de jeu (spatialisation)
  vol?: number;
  channel?: AudioChannel;
}

export class AudioManager {
  private ctx: AudioContext | null = null;
  private out: Record<AudioChannel, GainNode> | null = null;
  private volumes: Record<AudioChannel, number>;
  private muted = false;
  private sfxActive = 0;
  private listener: () => Vec2 = () => ({ x: 6000, y: 6000 });
  private unlocked = false;

  constructor() {
    this.volumes = { master: 0.8, sfx: 0.9, voice: 1, music: 0.5, ui: 0.8 };
    try {
      const saved = JSON.parse(localStorage.getItem(VOL_KEY) ?? 'null') as Partial<Record<AudioChannel, number>> & { muted?: boolean } | null;
      if (saved) {
        for (const ch of AUDIO_CHANNELS) if (typeof saved[ch] === 'number') this.volumes[ch] = saved[ch]!;
        this.muted = !!saved.muted;
      }
    } catch { /* stockage indisponible */ }
  }

  /** Crée/reprend le contexte audio (obligatoirement appelé depuis un geste utilisateur). */
  unlock(): void {
    if (!this.ctx) {
      // Cast justifié : Safari < 14.1 n'expose que webkitAudioContext (pas de `any`).
      const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!AC) return;
      this.ctx = new AC();
      const master = this.ctx.createGain();
      master.connect(this.ctx.destination);
      const out = {} as Record<AudioChannel, GainNode>;
      for (const ch of AUDIO_CHANNELS) {
        const g = this.ctx.createGain();
        g.connect(master);
        out[ch] = g;
      }
      this.out = out;
      this.applyVolumes();
    }
    if (this.ctx.state === 'suspended') void this.ctx.resume();
    this.unlocked = true;
  }

  get ready(): boolean {
    return this.ctx !== null && this.ctx.state === 'running';
  }

  /** Position de l'écouteur (caméra), en unités de jeu — câblé par claude-client. */
  setListener(fn: () => Vec2): void {
    this.listener = fn;
  }

  setVolume(ch: AudioChannel, v: number): void {
    this.volumes[ch] = Math.max(0, Math.min(1, v));
    this.persist();
    this.applyVolumes();
  }

  getVolume(ch: AudioChannel): number {
    return this.volumes[ch];
  }

  setMuted(m: boolean): void {
    this.muted = m;
    this.persist();
    this.applyVolumes();
  }

  getMuted(): boolean {
    return this.muted;
  }

  private persist(): void {
    try {
      localStorage.setItem(VOL_KEY, JSON.stringify({ ...this.volumes, muted: this.muted }));
    } catch { /* ignore */ }
  }

  private applyVolumes(): void {
    if (!this.out || !this.ctx) return;
    const master = this.muted ? 0 : this.volumes.master;
    this.out.master.gain.value = master;
    for (const ch of AUDIO_CHANNELS) {
      if (ch !== 'master') this.out[ch].gain.value = this.volumes[ch];
    }
  }

  /** Gain spatial : 1 près de la caméra, atténué avec la distance. */
  private spatialGain(pos?: Vec2): number {
    if (!pos) return 1;
    const l = this.listener();
    const d = Math.hypot(pos.x - l.x, pos.y - l.y);
    if (d <= 800) return 1;
    return Math.max(0.1, 1 - (d - 800) / 3200);
  }

  /** Joue un effet sonore. `fn` reçoit le contexte et le nœud de destination du canal. */
  sfx(fn: SfxFn, opts: SfxOpts = {}): void {
    if (!this.ctx || !this.out || this.ctx.state !== 'running') return;
    const channel = opts.channel ?? 'sfx';
    const vol = opts.vol ?? 1;
    if (channel === 'sfx') {
      if (this.sfxActive >= MAX_SFX) return; // limite de polyphonie
      this.sfxActive++;
      const g = this.ctx.createGain();
      g.gain.value = this.spatialGain(opts.pos) * vol;
      g.connect(this.out.sfx);
      const cleanup = (): void => {
        this.sfxActive = Math.max(0, this.sfxActive - 1);
        g.disconnect();
      };
      fn(this.ctx, g);
      setTimeout(cleanup, 2500);
    } else {
      const g = this.ctx.createGain();
      g.gain.value = this.spatialGain(opts.pos) * vol;
      g.connect(this.out[channel]);
      fn(this.ctx, g);
    }
  }

  /** Convertit un id d'entité en position (snapshot courant) pour la spatialisation. */
  entityPos(id: number | undefined): Vec2 | undefined {
    if (id === undefined) return undefined;
    const s = store.state.snapshot?.entities.find((e) => e.id === id);
    return s ? { x: s.x, y: s.y } : undefined;
  }

  handleEvent(e: GameEvent): void {
    if (!this.ready) return;
    playEvent(this, e);
  }
}

export const audio = new AudioManager();

/** Exports nommés attendus par mistral-ui (settings) : volumes par canal 0..1. */
export function setVolume(ch: AudioChannel, v: number): void {
  audio.setVolume(ch, v);
}
export function getVolume(ch: AudioChannel): number {
  return audio.getVolume(ch);
}
export function setMuted(m: boolean): void {
  audio.setMuted(m);
}

let wasInGame = false;
let hadResult = false;

export function installAudio(): void {
  // Le navigateur exige un geste utilisateur avant de jouer : on débloque au premier clic/touche.
  const unlock = (): void => audio.unlock();
  window.addEventListener('pointerdown', unlock);
  window.addEventListener('keydown', unlock);

  fxBus.subscribe((e) => audio.handleEvent(e));

  // musique : le moteur choisit seul le thème (menu, draft, partie, fin) et suit l'intensité des combats
  music.install();

  store.subscribe((s) => {
    if (s.game && !wasInGame) {
      wasInGame = true;
      hadResult = false;
      audio.unlock();
      resetKillAnnouncements();
    }
    if (s.result && !hadResult) {
      hadResult = true;
    }
    if (!s.game && !s.result) {
      wasInGame = false;
      hadResult = false;
    }
  });
}
