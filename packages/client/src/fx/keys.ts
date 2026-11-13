/**
 * Traduction GameEvent → (fxKey, phase). Logique pure, testée.
 *
 * Une même fxKey (ex. `ilyra_w`) couvre plusieurs moments d'un sort, chacun porté par un événement différent :
 * `abilityCast` (anticipation) → `areaEffect` (télégraphie/zone) → `projectileHit` (impact)…
 */
import { CHAMPIONS, type GameEvent } from '@lov/shared';

export type FxPhase = 'cast' | 'projectile' | 'hit' | 'area' | 'dash' | 'buff' | 'buffEnd' | 'summoner';

export interface ResolvedFx {
  key: string;
  phase: FxPhase;
}

/**
 * Résolution de la fxKey d'un buff quand l'événement n'en porte pas : l'id du buff par défaut.
 * (Le serveur renseigne normalement `fxKey` : la clé du sort qui a posé le buff, ou `cc_stun`… du catalogue.)
 */
let buffFx: (buffId: string) => string = (id) => id;
export function setBuffFxResolver(fn: (buffId: string) => string): void {
  buffFx = fn;
}

export function abilityFxKey(championId: string, slot: string): string {
  return `${championId}_${slot.toLowerCase()}`;
}

/** Remplit `out` et renvoie true si l'événement déclenche un FX de sort/buff/invocateur. */
export function resolveFx(e: GameEvent, out: ResolvedFx): boolean {
  switch (e.type) {
    case 'abilityCast':
      out.key = e.fxKey ?? abilityFxKey(e.championId, e.slot);
      out.phase = 'cast';
      return true;
    case 'projectileSpawn':
      out.key = e.fxKey;
      out.phase = 'projectile';
      return true;
    case 'projectileHit':
      out.key = e.fxKey;
      out.phase = 'hit';
      return true;
    case 'areaEffect':
      out.key = e.fxKey;
      out.phase = 'area';
      return true;
    case 'dash':
      out.key = e.fxKey ?? 'dash';
      out.phase = 'dash';
      return true;
    case 'buffApplied':
      out.key = e.fxKey ?? buffFx(e.buffId);
      out.phase = 'buff';
      return true;
    case 'buffRemoved':
      out.key = e.fxKey ?? buffFx(e.buffId);
      out.phase = 'buffEnd';
      return true;
    case 'summonerCast':
      out.key = e.fxKey ?? `sum_${e.spell}`;
      out.phase = 'summoner';
      return true;
    default:
      return false;
  }
}

/** Zone de télégraphie d'un sort à effet retardé : le serveur émet `<fxKey>_telegraph` puis `<fxKey>`. */
export const TELEGRAPH_SUFFIX = '_telegraph';
export const isTelegraphKey = (key: string): boolean => key.endsWith(TELEGRAPH_SUFFIX);

/** Champion propriétaire d'une fxKey (`ilyra_r_shard` → `ilyra`), ou null. */
export function championOfKey(key: string): string | null {
  const i = key.indexOf('_');
  const id = i < 0 ? key : key.slice(0, i);
  return id in CHAMPIONS ? id : null;
}
