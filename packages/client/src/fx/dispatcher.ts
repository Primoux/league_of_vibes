/**
 * Aiguillage GameEvent → handler FX : handler spécifique de la fxKey, sinon handler générique.
 * Garde la trace des projectiles en vol pour que `projectileHit` les arrête au bon endroit.
 */
import * as THREE from 'three';
import type { GameEvent } from '@lov/shared';
import type { FxRuntime } from './core/FxRuntime.ts';
import { resolveFx, type ResolvedFx } from './keys.ts';
import { getFx, getWorldHandlers, type FxHandlers, type ProjectileHandle } from './registry.ts';

export type AllHandlers = { [K in keyof FxHandlers]-?: NonNullable<FxHandlers[K]> };

/** Durée max de suivi d'un projectile sans impact (skillshot raté…), en secondes. */
const PROJECTILE_TTL = 8;

export class FxDispatcher {
  private readonly resolved: ResolvedFx = { key: '', phase: 'cast' };
  private readonly projectiles = new Map<number, { h: ProjectileHandle; at: number }>();
  private readonly tmp = new THREE.Vector3();
  /** Compteurs de debug (fxlab). */
  readonly stats = { events: 0, defaults: 0, custom: 0 };

  constructor(
    private readonly rt: FxRuntime,
    private readonly defaults: AllHandlers,
  ) {}

  handle(e: GameEvent): void {
    this.stats.events++;
    const world = getWorldHandlers(e.type);
    if (world) for (const fn of world) fn(this.rt, e);
    if (!resolveFx(e, this.resolved)) return;
    const { key, phase } = this.resolved;
    const custom = getFx(key)?.[phase];
    if (custom) this.stats.custom++;
    else this.stats.defaults++;
    const c = this.rt.colors(key);
    const rt = this.rt;
    // Le switch conserve le typage précis de chaque événement.
    switch (e.type) {
      case 'abilityCast':
        return (getFx(key)?.cast ?? this.defaults.cast)(rt, e, c);
      case 'projectileSpawn': {
        const h = (getFx(key)?.projectile ?? this.defaults.projectile)(rt, e, c);
        if (h) {
          if (this.projectiles.size > 64) this.prune();
          this.projectiles.set(e.id, { h, at: rt.time });
        }
        return;
      }
      case 'projectileHit': {
        const p = this.projectiles.get(e.id);
        if (p) {
          p.h.finish(rt.world(e.pos, 0, this.tmp));
          this.projectiles.delete(e.id);
        }
        return (getFx(key)?.hit ?? this.defaults.hit)(rt, e, c);
      }
      case 'areaEffect':
        return (getFx(key)?.area ?? this.defaults.area)(rt, e, c);
      case 'dash':
        return (getFx(key)?.dash ?? this.defaults.dash)(rt, e, c);
      case 'buffApplied':
        return (getFx(key)?.buff ?? this.defaults.buff)(rt, e, c);
      case 'buffRemoved':
        return (getFx(key)?.buffEnd ?? this.defaults.buffEnd)(rt, e, c);
      case 'summonerCast':
        return (getFx(key)?.summoner ?? this.defaults.summoner)(rt, e, c);
    }
  }

  private prune(): void {
    for (const [id, p] of this.projectiles) if (this.rt.time - p.at > PROJECTILE_TTL) this.projectiles.delete(id);
  }

  get inFlight(): number {
    return this.projectiles.size;
  }
}
