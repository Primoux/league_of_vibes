/**
 * Suivi des éliminations ENTRE CHAMPIONS (pur, testé) : multi-kills (fenêtre de 10 s) et séries
 * (kills sans mourir). Les morts de sbires, monstres ou structures ne passent jamais ici.
 */
import type { EntityId } from '@lov/shared';

export const MULTI_WINDOW = 10; // secondes entre deux kills d'un même multi-kill

export interface KillInfo {
  /** 1 = kill simple, 2 = double… (plafonné à 5). */
  multi: number;
  /** Kills consécutifs du tueur sans mourir. */
  spree: number;
  firstBlood: boolean;
}

export class KillStreaks {
  private multi = new Map<EntityId, { count: number; last: number }>();
  private spree = new Map<EntityId, number>();
  private firstBloodDone = false;

  reset(): void {
    this.multi.clear();
    this.spree.clear();
    this.firstBloodDone = false;
  }

  /** Un champion (`victim`) a été tué par un champion ennemi (`killer`) à l'instant `now` (secondes). */
  onChampionKill(victim: EntityId, killer: EntityId, now: number): KillInfo {
    this.spree.delete(victim);
    this.multi.delete(victim);
    const m = this.multi.get(killer);
    const count = m && now - m.last <= MULTI_WINDOW ? m.count + 1 : 1;
    this.multi.set(killer, { count, last: now });
    const spree = (this.spree.get(killer) ?? 0) + 1;
    this.spree.set(killer, spree);
    const firstBlood = !this.firstBloodDone;
    this.firstBloodDone = true;
    return { multi: Math.min(5, count), spree, firstBlood };
  }

  /** Mort sans tueur champion (tour, sbire, monstre) : la série de la victime s'arrête aussi. */
  onChampionDeath(victim: EntityId): void {
    this.spree.delete(victim);
    this.multi.delete(victim);
  }
}

/**
 * Niveau d'escalade du jingle (0..9) : le multi-kill domine, la série ajoute un cran tous les 2 kills.
 * Kill simple sans série = 0 ; penta = 8+.
 */
export function stingLevel(info: KillInfo): number {
  const fromMulti = (info.multi - 1) * 2;
  const fromSpree = Math.min(3, Math.floor(Math.max(0, info.spree - 1) / 2));
  return Math.min(9, fromMulti + fromSpree);
}
