import { describe, expect, it } from 'vitest';
import { FOUNTAINS, dist, getChampion } from '@lov/shared';
import { pendingAttackProjectiles } from '../src/systems/combat.ts';
import { champ, coreWorld, dummy, run } from './helpers.ts';

describe('auto-attaques', () => {
  it('cadence = attackSpeed, dégâts après le windup', () => {
    const w = coreWorld();
    const a = dummy(w, 'blue', { x: 6000, y: 3000 }, { attackSpeed: 2, ad: 50, attackRange: 150 });
    a.data.attackConfig = { windup: 0.5, projectileSpeed: 0 };
    const t = dummy(w, 'red', { x: 6200, y: 3000 }, { maxHp: 100000 });
    t.hp = 100000;
    a.attackTarget = t.id;
    w.step(); // windup = 0.25 s
    expect(t.hp).toBe(100000);
    run(w, 0.25);
    expect(t.hp).toBe(100000 - 50);
    run(w, 2);
    // ~ 1 + 4 attaques en 2.25 s
    const hits = (100000 - t.hp) / 50;
    expect(hits).toBeGreaterThanOrEqual(4);
    expect(hits).toBeLessThanOrEqual(5);
    expect(w.events.drain().filter((e) => e.type === 'autoAttack').length).toBe(hits);
  });

  it('poursuit une cible hors de portée puis attaque', () => {
    const w = coreWorld();
    const a = dummy(w, 'blue', { x: 6000, y: 3000 }, { attackRange: 125, moveSpeed: 400 });
    const t = dummy(w, 'red', { x: 7000, y: 3000 });
    a.attackTarget = t.id;
    run(w, 3);
    expect(dist(a.pos, t.pos) - a.radius - t.radius).toBeLessThanOrEqual(125 + 1);
    expect(t.hp).toBeLessThan(1000);
  });

  it('distance : projectile qui voyage avant de toucher', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 }, 'sable');
    const t = dummy(w, 'red', { x: 6500, y: 3000 });
    w.commands.push({ playerId: a.champ!.playerId, msg: { t: 'attack', targetId: t.id } });
    const def = getChampion('sable');
    run(w, def.attackWindup / a.stats.attackSpeed + 0.04);
    expect(pendingAttackProjectiles(w)).toBe(1);
    expect(t.hp).toBe(1000);
    run(w, 0.3);
    expect(t.hp).toBeLessThan(1000);
    const aa = w.events.drain().find((e) => e.type === 'autoAttack');
    expect(aa).toMatchObject({ ranged: true, speed: def.projectileSpeed });
  });

  it('se déplacer pendant le windup annule l’attaque', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const t = dummy(w, 'red', { x: 6200, y: 3000 });
    const pid = a.champ!.playerId;
    w.commands.push({ playerId: pid, msg: { t: 'attack', targetId: t.id } });
    w.step();
    w.commands.push({ playerId: pid, msg: { t: 'move', pos: { x: 5000, y: 3000 } } });
    run(w, 1);
    expect(t.hp).toBe(1000);
  });

  it('attaque-déplacement : engage la première cible rencontrée puis reprend', () => {
    const w = coreWorld();
    // le long de la voie mid (toujours dégagée, cf. shared/test/map.test.ts)
    const a = champ(w, 'blue', { x: 4000, y: 4000 });
    const t = dummy(w, 'red', { x: 5000, y: 5050 }, { maxHp: 50 });
    t.hp = 50;
    w.commands.push({ playerId: a.champ!.playerId, msg: { t: 'attackMove', pos: { x: 6500, y: 6500 } } });
    run(w, 16);
    expect(t.dead).toBe(true);
    expect(a.pos).toEqual({ x: 6500, y: 6500 });
  });

  it('crits : x1.75 et hooks beforeAttackHit', () => {
    const w = coreWorld();
    const a = dummy(w, 'blue', { x: 6000, y: 3000 }, { critChance: 1, ad: 100 });
    const t = dummy(w, 'red', { x: 6150, y: 3000 }, { maxHp: 10000 });
    t.hp = 10000;
    w.hooks.beforeAttackHit.push((ctx) => (ctx.bonus += 10));
    a.attackTarget = t.id;
    run(w, 0.5);
    expect(t.hp).toBe(10000 - 185);
    expect(w.events.drain().find((e) => e.type === 'damage')).toMatchObject({ crit: true, amount: 185 });
  });

  it('ne cible pas un allié ni une cible non-ciblable', () => {
    const w = coreWorld();
    const a = dummy(w, 'blue', { x: 6000, y: 3000 });
    const ally = dummy(w, 'blue', { x: 6100, y: 3000 });
    a.attackTarget = ally.id;
    run(w, 1);
    expect(ally.hp).toBe(1000);
    expect(a.attackTarget).toBeUndefined();
    const e = dummy(w, 'red', { x: 6100, y: 3000 });
    w.hooks.isTargetable.push((t) => t !== e);
    a.attackTarget = e.id;
    run(w, 1);
    expect(e.hp).toBe(1000);
  });

  it('retire les sbires morts après CORPSE_SECONDS, garde les champions', () => {
    const w = coreWorld();
    const a = dummy(w, 'blue', { x: 6000, y: 3000 }, { ad: 5000 });
    const t = dummy(w, 'red', { x: 6150, y: 3000 });
    a.attackTarget = t.id;
    run(w, 1);
    expect(t.dead).toBe(true);
    run(w, 2.5);
    expect(w.get(t.id)).toBeUndefined();
  });
});

describe('fontaine', () => {
  it('soigne les alliés et tue les ennemis', () => {
    const w = coreWorld();
    const ally = champ(w, 'blue', { ...FOUNTAINS.blue });
    ally.hp = 1;
    const foe = champ(w, 'red', { x: FOUNTAINS.blue.x + 100, y: FOUNTAINS.blue.y });
    run(w, 2);
    expect(ally.hp).toBeGreaterThan(ally.stats.maxHp * 0.2);
    expect(foe.dead).toBe(true);
  });
});
