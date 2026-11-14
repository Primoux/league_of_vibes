import { describe, expect, it } from 'vitest';
import { isTelegraphKey, resolveFx, type ResolvedFx } from '../../src/fx/keys.ts';
import { statusOf } from '../../src/fx/world/status.ts';

describe('statuts', () => {
  it('classe les buffs par catégorie visuelle', () => {
    expect(statusOf('stun')).toBe('stun');
    expect(statusOf('ilyra_q_slow')).toBe('slow');
    expect(statusOf('exhaust')).toBe('slow');
    expect(statusOf('knockup')).toBe('knockup');
    expect(statusOf('grievous')).toBe('grievous');
    expect(statusOf('grondak_w')).toBeNull();
  });
});

describe('fxKey portée par les événements serveur', () => {
  const out: ResolvedFx = { key: '', phase: 'cast' };
  it('buffApplied : la fxKey du sort prime sur le buffId', () => {
    resolveFx({ type: 'buffApplied', target: 1, buffId: 'stun', duration: 1, fxKey: 'grondak_e' }, out);
    expect(out).toEqual({ key: 'grondak_e', phase: 'buff' });
    resolveFx({ type: 'buffRemoved', target: 1, buffId: 'stun' }, out);
    expect(out).toEqual({ key: 'stun', phase: 'buffEnd' });
  });
  it('abilityCast / summonerCast : fxKey explicite', () => {
    resolveFx({ type: 'abilityCast', source: 1, championId: 'kaelen', slot: 'Q', abilityId: 'kaelen_q', fxKey: 'kaelen_q' }, out);
    expect(out.key).toBe('kaelen_q');
    resolveFx({ type: 'summonerCast', source: 1, spell: 'flash', from: { x: 0, y: 0 }, fxKey: 'sum_flash' }, out);
    expect(out).toEqual({ key: 'sum_flash', phase: 'summoner' });
  });
  it('détecte les zones de télégraphie', () => {
    expect(isTelegraphKey('grondak_r_telegraph')).toBe(true);
    expect(isTelegraphKey('grondak_r')).toBe(false);
  });
});
