import * as THREE from 'three';
import { describe, expect, it, vi } from 'vitest';
import type { FxRuntime } from '../../src/fx/core/FxRuntime.ts';
import { FxDispatcher, type AllHandlers } from '../../src/fx/dispatcher.ts';
import { onWorldEvent, registerFx } from '../../src/fx/registry.ts';

/** Faux runtime : le dispatcher n'utilise que colors/world/time. */
const fakeRt = {
  time: 0,
  enemyContext: false,
  entities: { isEnemy: (id: number) => id === 666 },
  colors: () => ({}),
  world: (p: { x: number; y: number }, h = 0, out = new THREE.Vector3()) => out.set(p.x / 100, h, -p.y / 100),
} as unknown as FxRuntime;

function makeDefaults(): AllHandlers {
  return {
    cast: vi.fn(),
    projectile: vi.fn(),
    hit: vi.fn(),
    area: vi.fn(),
    dash: vi.fn(),
    buff: vi.fn(),
    buffEnd: vi.fn(),
    summoner: vi.fn(),
  };
}

describe('FxDispatcher', () => {
  it('utilise le handler de la fxKey et retombe sur le générique pour les autres phases', () => {
    const defaults = makeDefaults();
    const d = new FxDispatcher(fakeRt, defaults);
    const customCast = vi.fn();
    registerFx('testchamp_q', { cast: customCast });
    // championId inconnu des données : la clé reste `testchamp_q`
    d.handle({ type: 'abilityCast', source: 1, championId: 'testchamp', slot: 'Q', abilityId: 'x' });
    expect(customCast).toHaveBeenCalledOnce();
    expect(defaults.cast).not.toHaveBeenCalled();
    d.handle({ type: 'areaEffect', source: 1, fxKey: 'testchamp_q', pos: { x: 0, y: 0 }, radius: 100, duration: 0.5 });
    expect(defaults.area).toHaveBeenCalledOnce();
    expect(d.stats).toEqual({ events: 2, defaults: 1, custom: 1 });
  });

  it('registerFx complète une entrée existante', () => {
    const defaults = makeDefaults();
    const d = new FxDispatcher(fakeRt, defaults);
    const a = vi.fn();
    const b = vi.fn();
    registerFx('merge_q', { cast: a });
    registerFx('merge_q', { area: b });
    d.handle({ type: 'abilityCast', source: 1, championId: 'merge', slot: 'Q', abilityId: 'x' });
    d.handle({ type: 'areaEffect', source: 1, fxKey: 'merge_q', pos: { x: 0, y: 0 }, radius: 1, duration: 0 });
    expect(a).toHaveBeenCalledOnce();
    expect(b).toHaveBeenCalledOnce();
  });

  it("arrête le projectile à l'impact, à la position de l'impact", () => {
    const defaults = makeDefaults();
    const finish = vi.fn();
    defaults.projectile = vi.fn(() => ({ finish }));
    const d = new FxDispatcher(fakeRt, defaults);
    d.handle({ type: 'projectileSpawn', id: 7, source: 1, fxKey: 'p_q', from: { x: 0, y: 0 }, to: { x: 1000, y: 0 }, speed: 1000, width: 80 });
    expect(d.inFlight).toBe(1);
    d.handle({ type: 'projectileHit', id: 7, pos: { x: 500, y: 0 }, fxKey: 'p_q' });
    expect(finish).toHaveBeenCalledOnce();
    expect((finish.mock.calls[0]![0] as THREE.Vector3).x).toBe(5);
    expect(defaults.hit).toHaveBeenCalledOnce();
    expect(d.inFlight).toBe(0);
  });

  it("marque le contexte ennemi pendant le handler d'un événement posé par un ennemi", () => {
    const defaults = makeDefaults();
    const seen: boolean[] = [];
    defaults.area = vi.fn((rt: FxRuntime) => void seen.push(rt.enemyContext));
    const d = new FxDispatcher(fakeRt, defaults);
    d.handle({ type: 'areaEffect', source: 666, fxKey: 'x_r', pos: { x: 0, y: 0 }, radius: 1, duration: 1 });
    d.handle({ type: 'areaEffect', source: 1, fxKey: 'x_r', pos: { x: 0, y: 0 }, radius: 1, duration: 1 });
    expect(seen).toEqual([true, false]);
    expect(fakeRt.enemyContext).toBe(false);
  });

  it('transmet les événements du monde aux handlers onWorldEvent', () => {
    const d = new FxDispatcher(fakeRt, makeDefaults());
    const fn = vi.fn();
    onWorldEvent('levelUp', fn);
    d.handle({ type: 'levelUp', target: 3, level: 6 });
    expect(fn).toHaveBeenCalledWith(fakeRt, { type: 'levelUp', target: 3, level: 6 });
  });
});
