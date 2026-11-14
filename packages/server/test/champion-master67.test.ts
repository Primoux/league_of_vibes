import { describe, expect, it } from 'vitest';
import { getChampion } from '@lov/shared';
import { addBuff, getBuff, hasCC } from '../src/combat/buffs.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { advance, cast, damage, dummy, game } from './champion-helpers.ts';

describe('67 Master', () => {
  it('est un champion original sélectionnable avec un kit complet', () => {
    expect(getChampion('master67').name).toBe('67 Master');
    expect(getChampion('master67').abilities.R.name).toBe('SIX… SEVEN !');
  });
  it('Q ralentit le premier ennemi sans traverser les autres', () => {
    const { world, source, target } = game('master67');
    const behind = dummy(world, source, { x: 1500, y: 1000 });
    expect(cast(world, source, 'Q', target)).toBe(true); advance(world, .5);
    expect(damage(world, 'master67_q', target)).toBeGreaterThan(0);
    expect(damage(world, 'master67_q', behind)).toBe(0);
    expect(hasCC(target, 'slow')).toBe(true);
  });
  it('W attend sa télégraphie avant les dégâts et le root', () => {
    const { world, source, target } = game('master67');
    cast(world, source, 'W', target); advance(world, .8);
    expect(damage(world, 'master67_w')).toBe(0); expect(hasCC(target, 'root')).toBe(false);
    advance(world, .1); expect(damage(world, 'master67_w')).toBeGreaterThan(0); expect(hasCC(target, 'root')).toBe(true);
  });
  it('E respecte sa portée et son bouclier expire', () => {
    const { world, source } = game('master67');
    expect(cast(world, source, 'E', undefined, { x: 1600, y: 1000 })).toBe(true);
    expect(source.pos.x).toBeCloseTo(1367); expect(source.shield).toBe(67);
    advance(world, 2.7); expect(source.shield).toBe(0);
  });
  it('E est bloqué par une immobilisation', () => {
    const { world, source } = game('master67'); addBuff(world, source, 'root', { duration: 1 });
    expect(cast(world, source, 'E')).toBe(false); expect(source.pos.x).toBe(1000);
  });
  it('R produit deux impacts, puis la projection, aux délais annoncés', () => {
    const { world, source, target } = game('master67'); cast(world, source, 'R');
    advance(world, .6); expect(damage(world, 'master67_r')).toBe(0);
    advance(world, .1); const first = damage(world, 'master67_r'); expect(first).toBeGreaterThan(0);
    expect(hasCC(target, 'slow')).toBe(true); expect(hasCC(target, 'knockup')).toBe(false);
    advance(world, .7); expect(damage(world, 'master67_r')).toBeCloseTo(first * 2); expect(hasCC(target, 'knockup')).toBe(true);
    const waves = world.events.pending.filter(e => e.type === 'areaEffect' && e.fxKey.startsWith('master67_r_'));
    expect(waves.map(e => e.type === 'areaEffect' && e.fxKey)).toEqual(['master67_r_six', 'master67_r_seven']);
  });
  it('R est interruptible avant SIX et ne touche pas les alliés ni les cibles hors portée', () => {
    const { world, source, target } = game('master67');
    const ally = dummy(world, source, { x: 1100, y: 1000 }); ally.team = source.team;
    const far = dummy(world, source, { x: 1800, y: 1000 });
    cast(world, source, 'R'); addBuff(world, source, 'stun', { duration: 1 }); advance(world, 1.5);
    expect(damage(world, 'master67_r')).toBe(0); expect(hasCC(target, 'knockup')).toBe(false);
    source.champ!.cooldowns.R = 0; source.mana = source.stats.maxMana;
    cast(world, source, 'R'); advance(world, 1.5);
    expect(damage(world, 'master67_r', ally)).toBe(0); expect(damage(world, 'master67_r', far)).toBe(0);
    expect(damage(world, 'master67_r', target)).toBeGreaterThan(0);
  });
  it('la mort du lanceur annule la deuxième onde', () => {
    const { world, source, target } = game('master67'); cast(world, source, 'R'); advance(world, .7);
    const first = damage(world, 'master67_r');
    applyDamage(world, { source: target, target: source, amount: 1e6, type: 'true' }); advance(world, .8);
    expect(damage(world, 'master67_r')).toBe(first);
  });
  it('le septième impact déclenche exactement un bonus et remet le compteur à zéro', () => {
    const { world, source, target } = game('master67');
    for (let i = 0; i < 7; i++) {
      applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'master67_q' });
      if (i < 6) { expect(damage(world, 'master67_p')).toBe(0); advance(world, .7); }
    }
    expect(damage(world, 'master67_p')).toBeCloseTo(67 * 100 / (100 + target.stats.mr));
    expect(getBuff(source, 'master67_count')).toBeUndefined();
    advance(world, .7); applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'master67_q' });
    expect(getBuff(source, 'master67_count')?.stacks).toBe(1);
  });
  it('un septième impact déjà létal ne cause ni dégâts négatifs ni seconde mort', () => {
    const { world, source, target } = game('master67');
    for (let i = 0; i < 6; i++) {
      applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'master67_q' }); advance(world, .7);
    }
    target.hp = 1;
    applyDamage(world, { source, target, amount: 2, type: 'true', ability: 'master67_q' });
    expect(target.dead).toBe(true); expect(damage(world, 'master67_p')).toBe(0);
    expect(getBuff(source, 'master67_count')).toBeUndefined();
    expect(source.champ!.damageToChampions).toBe(7);
    expect(world.events.pending.filter(e => e.type === 'death' && e.target === target.id)).toHaveLength(1);
    expect(world.events.pending.every(e => e.type !== 'damage' || e.amount >= 0)).toBe(true);
  });
  it('une zone ne compte pas sept fois ; attaques et sbires ne comptent pas', () => {
    const { world, source, target } = game('master67');
    for (let i = 0; i < 9; i++) applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'master67_w' });
    expect(getBuff(source, 'master67_count')?.stacks).toBe(1); advance(world, .7);
    applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
    applyDamage(world, { source, target: dummy(world, source), amount: 1, type: 'true', ability: 'master67_q' });
    expect(getBuff(source, 'master67_count')?.stacks).toBe(1);
    advance(world, 6.8); expect(getBuff(source, 'master67_count')).toBeUndefined();
  });
});
