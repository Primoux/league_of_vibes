/**
 * Bande-son originale générée (aucun asset) : moteur de séquence + thèmes composés pour le jeu.
 * - Menu / salon : « La Faille des Vibes » (héroïque).  - Draft : « Le Choix » (tendu).
 * - Partie : « Sous la canopée », dynamique : l'intensité monte avec les combats (tambours, cordes,
 *   cuivres) et redescend au calme.  - Fin : fanfare de victoire / complainte de défaite.
 * Propriétaire : mistral-economy (refonte musique : branche feat/music).
 */
import type { GameEvent } from '@lov/shared';
import { fxBus } from '../fx/FxBus.ts';
import type { ClientState } from '../state.ts';
import { store } from '../state.ts';
import { audio } from './AudioManager.ts';
import { MusicBus } from './music/instruments.ts';
import { defeat, THEMES, victory, type ThemeId } from './music/themes.ts';

const LOOKAHEAD = 0.18; // secondes programmées à l'avance
const TICK_MS = 40;
const FADE = 1.6;

/** Thème voulu selon l'état du client (pur, testé). */
export function themeFor(s: Pick<ClientState, 'game' | 'result' | 'draft' | 'lobby'>): ThemeId | 'end' | null {
  if (s.result) return 'end';
  if (s.game) return 'game';
  if (s.draft && s.lobby?.phase !== 'lobby') return 'draft';
  return 'menu';
}

/** Intensité de combat : montée par événements, décroissance continue (pur, testé). */
export class Intensity {
  value = 0;
  floor = 0;
  bump(amount: number): void {
    this.value = Math.min(1, this.value + amount);
  }
  decay(dt: number): void {
    this.value = Math.max(this.floor, this.value - dt * 0.06);
  }
}

class Music {
  private bus: MusicBus | null = null;
  private theme: ThemeId | null = null;
  private wanted: ThemeId | 'end' | null = null;
  private step = 0;
  private loop = 0;
  private nextTime = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  private switching = false;
  private ended = false;
  private lastTick = 0;
  readonly intensity = new Intensity();

  /** Branche le moteur sur l'état du client et le bus d'événements (appelé par installAudio). */
  install(): void {
    store.subscribe((s) => this.follow(s));
    fxBus.subscribe((e) => this.onEvent(e));
    this.follow(store.state);
    // le contexte audio n'existe qu'après un geste utilisateur : on réessaie régulièrement
    setInterval(() => this.follow(store.state), 500);
  }

  /** Compatibilité avec l'ancienne API. */
  start(): void {
    this.wanted = 'game';
    this.ensure();
  }
  stop(): void {
    this.bus?.fade(0, FADE);
  }
  stinger(kind: 'victory' | 'defeat'): void {
    if (!this.bus) return;
    const t = this.bus.ctx.currentTime + 0.1;
    this.bus.fade(0.9, 0.3);
    (kind === 'victory' ? victory : defeat)(this.bus, t);
  }

  private follow(s: ClientState): void {
    const want = themeFor(s);
    if (want === 'end') {
      if (!this.ended && this.bus && s.result) {
        this.ended = true;
        this.stopLoop();
        this.bus.fade(0, 0.4);
        const win = s.result.winner === s.game?.team;
        setTimeout(() => this.stinger(win ? 'victory' : 'defeat'), 450);
        this.theme = null;
      }
      this.wanted = 'end';
      return;
    }
    this.ended = false;
    if (want !== this.wanted) this.wanted = want;
    // fond de jeu : la tension de base monte avec le temps de partie (fin de partie plus épique)
    const time = s.snapshot?.time ?? 0;
    this.intensity.floor = time > 25 * 60 * 0.75 ? 0.3 : time > 15 * 60 * 0.75 ? 0.15 : 0;
    this.ensure();
  }

  private ensure(): void {
    if (!audio.ready) return;
    if (!this.bus) {
      let created: MusicBus | null = null;
      audio.sfx((ctx, dest) => {
        created = new MusicBus(ctx, dest);
      }, { channel: 'music' });
      this.bus = created;
      if (!this.bus) return;
    }
    const want = this.wanted;
    if (!want || want === 'end' || want === this.theme || this.switching) return;
    if (!this.theme) {
      this.begin(want);
      return;
    }
    // fondu enchaîné : on baisse, on change de thème, on remonte
    this.switching = true;
    this.bus.fade(0, FADE * 0.6);
    setTimeout(() => {
      this.switching = false;
      this.begin(this.wanted === 'end' || !this.wanted ? want : this.wanted);
    }, FADE * 600);
  }

  private begin(id: ThemeId): void {
    const bus = this.bus!;
    this.theme = id;
    this.step = 0;
    this.loop = 0;
    this.nextTime = bus.ctx.currentTime + 0.12;
    bus.setDelay((60 / THEMES[id].bpm) * 0.75);
    bus.fade(id === 'game' ? 0.8 : 0.95, FADE);
    if (!this.timer) {
      this.lastTick = performance.now();
      this.timer = setInterval(() => this.tick(), TICK_MS);
    }
  }

  private stopLoop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  private tick(): void {
    const bus = this.bus;
    if (!bus || !this.theme) return;
    const now = performance.now();
    this.intensity.decay((now - this.lastTick) / 1000);
    this.lastTick = now;
    const th = THEMES[this.theme];
    const sd = 60 / th.bpm / 4;
    const total = th.bars * 16;
    // onglet en arrière-plan : on ne rattrape pas des secondes de retard
    if (this.nextTime < bus.ctx.currentTime - 0.5) this.nextTime = bus.ctx.currentTime + 0.05;
    while (this.nextTime < bus.ctx.currentTime + LOOKAHEAD) {
      try {
        th.play(bus, this.step, this.loop, this.nextTime, this.intensity.value, sd);
      } catch {
        /* une note ratée ne doit jamais casser la boucle */
      }
      this.nextTime += sd;
      this.step++;
      if (this.step >= total) {
        this.step = 0;
        this.loop++;
      }
    }
  }

  private onEvent(e: GameEvent): void {
    if (this.theme !== 'game') return;
    const me = store.state.snapshot?.self?.entityId ?? store.state.game?.entityId;
    switch (e.type) {
      case 'damage':
        if (e.target === me || e.source === me) this.intensity.bump(e.target === me ? 0.07 : 0.05);
        break;
      case 'abilityCast':
        if (e.source === me) this.intensity.bump(0.03);
        break;
      case 'death': {
        const victim = store.state.snapshot?.entities.find((x) => x.id === e.target);
        if (victim?.kind === 'champion') this.intensity.bump(e.target === me || e.killer === me ? 0.45 : 0.25);
        break;
      }
      case 'structureDestroyed':
        this.intensity.bump(0.3);
        break;
      case 'objectiveTaken':
        if (e.monster === 'dragon' || e.monster === 'leviathan' || e.monster === 'elder_dragon' || e.monster === 'herald') this.intensity.bump(0.5);
        break;
      default:
        break;
    }
  }
}

export const music = new Music();
