/**
 * Registre des FX par fxKey. Chaque fxKey déclare les phases qu'elle personnalise ; les phases non déclarées
 * retombent sur l'effet générique (`defaults.ts`) : tout sort a donc toujours un FX propre.
 *
 *   registerFx('ilyra_w', {
 *     cast: (rt, e, c) => …,   // anticipation (abilityCast)
 *     area: (rt, e, c) => …,   // télégraphie + nova (areaEffect)
 *   });
 */
import type { GameEvent } from '@lov/shared';
import type { FxColors, FxRuntime } from './core/FxRuntime.ts';
import type { FxPhase } from './keys.ts';

export type Ev<T extends GameEvent['type']> = Extract<GameEvent, { type: T }>;

/** Objet renvoyé par un handler `projectile` pour que l'impact serveur puisse l'arrêter. */
export interface ProjectileHandle {
  finish(at?: import('three').Vector3): void;
}

export interface FxHandlers {
  cast?(rt: FxRuntime, e: Ev<'abilityCast'>, c: FxColors): void;
  projectile?(rt: FxRuntime, e: Ev<'projectileSpawn'>, c: FxColors): ProjectileHandle | void;
  hit?(rt: FxRuntime, e: Ev<'projectileHit'>, c: FxColors): void;
  area?(rt: FxRuntime, e: Ev<'areaEffect'>, c: FxColors): void;
  dash?(rt: FxRuntime, e: Ev<'dash'>, c: FxColors): void;
  buff?(rt: FxRuntime, e: Ev<'buffApplied'>, c: FxColors): void;
  buffEnd?(rt: FxRuntime, e: Ev<'buffRemoved'>, c: FxColors): void;
  summoner?(rt: FxRuntime, e: Ev<'summonerCast'>, c: FxColors): void;
}

// La phase et le nom du handler coïncident : on peut indexer FxHandlers par FxPhase.
type _Check = FxPhase extends keyof FxHandlers ? true : never;
const _check: _Check = true;
void _check;

const registry = new Map<string, FxHandlers>();

/** Enregistre (ou complète) les handlers d'une fxKey. */
export function registerFx(key: string, handlers: FxHandlers): void {
  registry.set(key, { ...registry.get(key), ...handlers });
}

export function getFx(key: string): FxHandlers | undefined {
  return registry.get(key);
}

/** Liste des fxKeys personnalisées (fxlab, couverture). */
export function registeredFxKeys(): string[] {
  return [...registry.keys()];
}

/** Handlers d'événements sans fxKey (dégâts, mort, level-up, structures…). */
export type WorldFxHandler<T extends GameEvent['type']> = (rt: FxRuntime, e: Ev<T>) => void;
const worldHandlers = new Map<string, ((rt: FxRuntime, e: GameEvent) => void)[]>();

export function onWorldEvent<T extends GameEvent['type']>(type: T, fn: WorldFxHandler<T>): void {
  const list = worldHandlers.get(type) ?? [];
  list.push(fn as (rt: FxRuntime, e: GameEvent) => void);
  worldHandlers.set(type, list);
}

export function getWorldHandlers(type: string): readonly ((rt: FxRuntime, e: GameEvent) => void)[] | undefined {
  return worldHandlers.get(type);
}
