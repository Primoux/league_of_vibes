import { describe, expect, it } from 'vitest';
import { applyDamage, applyHeal, applyShield, defaultRespawnTime } from '../src/combat/damage.ts';
import { champ, coreWorld, dummy, run } from './helpers.ts';

describe('applyDamage', () => {
  it('applique armure et RM, les dégâts bruts ignorent les résistances', () => {
    const w = coreWorld();
    const t = dummy(w, 'red', { x: 6000, y: 6000 }, { armor: 100, mr: 50 });
    expect(applyDamage(w, { source: undefined, target: t, amount: 100, type: 'physical' })).toBeCloseTo(50);
    expect(applyDamage(w, { source: undefined, target: t, amount: 150, type: 'magic' })).toBeCloseTo(100);
    expect(applyDamage(w, { source: undefined, target: t, amount: 100, type: 'true' })).toBeCloseTo(100);
    expect(t.hp).toBeCloseTo(1000 - 250);
    const evs = w.events.drain().filter((e) => e.type === 'damage');
    expect(evs).toHaveLength(3);
  });

  it('applique la pénétration fixe sans descendre sous 0', () => {
    const w = coreWorld();
    const s = dummy(w, 'blue', { x: 0, y: 0 }, { armorPen: 50, magicPen: 200 });
    const t = dummy(w, 'red', { x: 6000, y: 6000 }, { armor: 150, mr: 50 });
    expect(applyDamage(w, { source: s, target: t, amount: 200, type: 'physical' })).toBeCloseTo(100);
    expect(applyDamage(w, { source: s, target: t, amount: 100, type: 'magic' })).toBeCloseTo(100);
  });

  it('les résistances négatives amplifient les dégâts', () => {
    const w = coreWorld();
    const t = dummy(w, 'red', { x: 6000, y: 6000 }, { armor: -100 });
    expect(applyDamage(w, { source: undefined, target: t, amount: 100, type: 'physical' })).toBeCloseTo(150);
  });

  it('consomme les boucliers avant les PV et les fait expirer', () => {
    const w = coreWorld();
    const t = dummy(w, 'red', { x: 6000, y: 6000 });
    applyShield(w, { source: t, target: t, amount: 80, duration: 2 });
    applyShield(w, { source: t, target: t, amount: 50 });
    expect(t.shield).toBe(130);
    const dealt = applyDamage(w, { source: undefined, target: t, amount: 100, type: 'true' });
    expect(dealt).toBe(100);
    expect(t.hp).toBe(1000); // 80 (expire en premier) + 20 sur le permanent
    expect(t.shield).toBe(30);
    applyShield(w, { source: t, target: t, amount: 40, duration: 1 });
    run(w, 1.1);
    expect(t.shield).toBe(30);
  });

  it('tue, émet death avec killer et assists, met à jour K/D/A', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const b = champ(w, 'blue', { x: 6100, y: 3000 });
    const v = champ(w, 'red', { x: 6000, y: 3200 });
    w.hooks.bounty.push(() => 300);
    applyDamage(w, { source: b, target: v, amount: 100, type: 'true' });
    applyDamage(w, { source: a, target: v, amount: 1e6, type: 'true' });
    expect(v.dead).toBe(true);
    expect(v.champ!.deaths).toBe(1);
    expect(a.champ!.kills).toBe(1);
    expect(b.champ!.assists).toBe(1);
    expect(a.champ!.damageToChampions).toBeGreaterThan(500);
    const death = w.events.drain().find((e) => e.type === 'death');
    expect(death).toMatchObject({ type: 'death', target: v.id, killer: a.id, assists: [b.id], bounty: 300 });
    expect(v.champ!.respawnIn).toBeCloseTo(defaultRespawnTime(1, w.time));
  });

  it("crédite le kill au champion si une tour porte le coup final", () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const tower = dummy(w, 'blue', { x: 6000, y: 2500 }, {}, 'tower');
    const v = champ(w, 'red', { x: 6000, y: 3200 });
    applyDamage(w, { source: a, target: v, amount: 100, type: 'true' });
    applyDamage(w, { source: tower, target: v, amount: 1e6, type: 'true' });
    expect(a.champ!.kills).toBe(1);
    const death = w.events.drain().find((e) => e.type === 'death');
    expect(death).toMatchObject({ killer: a.id });
  });

  it('respawn à la fontaine après le timer', () => {
    const w = coreWorld();
    const v = champ(w, 'red', { x: 6000, y: 6000 });
    applyDamage(w, { source: undefined, target: v, amount: 1e6, type: 'true' });
    expect(v.dead).toBe(true);
    run(w, v.champ!.respawnIn + 0.1);
    expect(v.dead).toBe(false);
    expect(v.hp).toBe(v.stats.maxHp);
    expect(v.pos.x).toBeGreaterThan(11000);
    expect(w.events.drain().some((e) => e.type === 'respawn')).toBe(true);
  });

  it('hooks : beforeDamage (réduction) et preventDeath', () => {
    const w = coreWorld();
    const t = dummy(w, 'red', { x: 6000, y: 6000 });
    w.hooks.beforeDamage.push((ctx) => (ctx.multiplier *= 0.5));
    expect(applyDamage(w, { source: undefined, target: t, amount: 100, type: 'true' })).toBe(50);
    w.hooks.preventDeath.push((e) => ((e.hp = 1), true));
    applyDamage(w, { source: undefined, target: t, amount: 1e6, type: 'true' });
    expect(t.dead).toBe(false);
    expect(t.hp).toBe(1);
  });

  it('soins plafonnés aux PV max, lifesteal sur AA uniquement', () => {
    const w = coreWorld();
    const s = dummy(w, 'blue', { x: 0, y: 0 }, { lifesteal: 0.5 });
    const t = dummy(w, 'red', { x: 6000, y: 6000 });
    s.hp = 500;
    expect(applyHeal(w, { source: s, target: s, amount: 2000 })).toBe(500);
    s.hp = 500;
    applyDamage(w, { source: s, target: t, amount: 100, type: 'physical' });
    expect(s.hp).toBe(500);
    applyDamage(w, { source: s, target: t, amount: 100, type: 'physical', isAutoAttack: true });
    expect(s.hp).toBe(550);
  });

  it('crédite le propriétaire (data.owner)', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const pet = dummy(w, 'blue', { x: 6000, y: 3000 });
    pet.data.owner = a.id;
    const v = champ(w, 'red', { x: 6000, y: 3200 });
    applyDamage(w, { source: pet, target: v, amount: 1e6, type: 'true' });
    expect(a.champ!.kills).toBe(1);
  });
});
