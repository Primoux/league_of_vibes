import type { GameEvent } from '@lov/shared';
import { describe, expect, it } from 'vitest';
import { FxBus } from '../../src/fx/FxBus.ts';
import { paletteForKey, DEFAULT_PALETTE } from '../../src/fx/core/palette.ts';
import { championOfKey, resolveFx, type ResolvedFx } from '../../src/fx/keys.ts';

const lvl = (level: number): GameEvent => ({ type: 'levelUp', target: 1, level });

describe('FxBus', () => {
  it('retarde les abonnés normaux, pas les immédiats', () => {
    const bus = new FxBus();
    bus.delayMs = 100;
    const late: number[] = [];
    const early: number[] = [];
    bus.subscribe((e) => e.type === 'levelUp' && late.push(e.level));
    bus.subscribe((e) => e.type === 'levelUp' && early.push(e.level), { immediate: true });
    bus.dispatch(lvl(2), 1000);
    bus.dispatch(lvl(3), 1050);
    expect(early).toEqual([2, 3]);
    bus.update(1099);
    expect(late).toEqual([]);
    bus.update(1100);
    expect(late).toEqual([2]);
    bus.update(2000);
    expect(late).toEqual([2, 3]);
    expect(bus.pending).toBe(0);
  });

  it('livre directement quand delayMs = 0', () => {
    const bus = new FxBus();
    bus.delayMs = 0;
    const got: GameEvent[] = [];
    bus.subscribe((e) => got.push(e));
    bus.dispatch(lvl(5));
    expect(got).toHaveLength(1);
  });

  it('désabonnement', () => {
    const bus = new FxBus();
    bus.delayMs = 0;
    let n = 0;
    const off = bus.subscribe(() => n++);
    off();
    bus.dispatch(lvl(2));
    expect(n).toBe(0);
  });
});

describe('resolveFx', () => {
  const out: ResolvedFx = { key: '', phase: 'cast' };

  it('abilityCast → <champion>_<slot>', () => {
    const ok = resolveFx({ type: 'abilityCast', source: 1, championId: 'ilyra', slot: 'R', abilityId: 'ilyra_r' }, out);
    expect(ok).toBe(true);
    expect(out).toEqual({ key: 'ilyra_r', phase: 'cast' });
  });

  it('événements porteurs de fxKey', () => {
    resolveFx({ type: 'areaEffect', source: 1, fxKey: 'grondak_r', pos: { x: 0, y: 0 }, radius: 400, duration: 0.75 }, out);
    expect(out).toEqual({ key: 'grondak_r', phase: 'area' });
    resolveFx({ type: 'projectileHit', id: 9, pos: { x: 0, y: 0 }, fxKey: 'ilyra_q' }, out);
    expect(out.phase).toBe('hit');
    resolveFx({ type: 'dash', source: 1, from: { x: 0, y: 0 }, to: { x: 1, y: 1 }, duration: 0.2 }, out);
    expect(out).toEqual({ key: 'dash', phase: 'dash' });
  });

  it('sorts d’invocateur et buffs', () => {
    resolveFx({ type: 'summonerCast', source: 1, spell: 'flash', from: { x: 0, y: 0 } }, out);
    expect(out).toEqual({ key: 'sum_flash', phase: 'summoner' });
    resolveFx({ type: 'buffRemoved', target: 1, buffId: 'stun' }, out);
    expect(out).toEqual({ key: 'stun', phase: 'buffEnd' });
  });

  it('ignore les événements sans FX de sort', () => {
    expect(resolveFx({ type: 'chat', playerId: 'p', text: 'gg', teamOnly: false }, out)).toBe(false);
  });
});

describe('palettes', () => {
  it('trouve le champion depuis la fxKey', () => {
    expect(championOfKey('ilyra_r')).toBe('ilyra');
    expect(championOfKey('zephyr_q_return')).toBe('zephyr');
    expect(championOfKey('sum_flash')).toBeNull();
    expect(paletteForKey('ilyra_q').primary).toBe('#8fd3ff');
    expect(paletteForKey('sum_ignite').primary).toBe('#ff7a1a');
    expect(paletteForKey('inconnu')).toBe(DEFAULT_PALETTE);
  });
});

describe('couleur vive', () => {
  it("choisit l'accent pour les palettes sombres et garantit une luminosité minimale", async () => {
    const THREE = await import('three');
    const { makeVivid } = await import('../../src/fx/core/FxRuntime.ts');
    const hsl = { h: 0, s: 0, l: 0 };
    const pick = (id: string): { h: number; s: number; l: number } => {
      const p = paletteForKey(`${id}_q`);
      makeVivid(new THREE.Color(p.primary), new THREE.Color(p.accent)).getHSL(hsl);
      return { ...hsl };
    };
    const accentHue = (hex: string): number => new THREE.Color(hex).getHSL(hsl).h;
    expect(pick('grondak').h).toBeCloseTo(accentHue('#ffb347'), 2); // brun terne → accent orange
    expect(pick('nyx').h).toBeCloseTo(accentHue('#ff2e63'), 2); // quasi-noir → accent rose
    expect(pick('ilyra').h).toBeCloseTo(accentHue('#8fd3ff'), 2); // primaire déjà vive
    for (const id of ['grondak', 'nyx', 'corvin', 'tharox']) expect(pick(id).l).toBeGreaterThanOrEqual(0.579);
  });
});
