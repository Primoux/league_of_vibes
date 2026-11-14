import { describe, expect, it } from 'vitest';
import { TIMERS, scaled } from '@lov/shared';
import { World, createChampion } from '../src/world.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { buffsSystem } from '../src/systems/buffs.ts';
import { jungleSystem } from '../src/systems/jungle.ts';
import { stats } from '../src/world-ai/runtime.ts';

function redBuffWorld() {
  const world = new World([buffsSystem, jungleSystem]); world.init();
  const source = createChampion(world, { playerId: 'source', name: 'Source', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6200, y: 3600 } });
  const speed = source.stats.moveSpeed;
  world.step(scaled(TIMERS.jungleFirstSpawn));
  const red = [...world.query('monster')].find(e => e.subtype === 'red_buff')!;
  applyDamage(world, { source, target: red, amount: 100000, type: 'true' });
  const target = createChampion(world, { playerId: 'target', name: 'Cible', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6250, y: 3600 } });
  world.events.drain();
  return { world, source, target, speed };
}
const burns = (world: World) => world.events.pending.filter(e => e.type === 'damage' && e.ability === 'red_burn');

describe('régressions du buff rouge', () => {
  it('ne ralentit pas son porteur et ne le classe pas comme contrôle de foule', () => {
    const { source, speed } = redBuffWorld();
    expect(source.stats.moveSpeed).toBe(speed);
    expect(hasCC(source, 'slow')).toBe(false);
    expect(getBuff(source, 'red_buff')?.def.cc).toBeUndefined();
  });
  it('déclenche ses effets uniquement sur une vraie auto-attaque', () => {
    const { world, source, target } = redBuffWorld();
    applyDamage(world, { source, target, amount: 1, type: 'true' });
    expect(getBuff(target, 'red_slow')).toBeUndefined();
    expect(target.data.redBurn).toBeUndefined();
  });
  it.each(['minion', 'monster'] as const)('brûle et ralentit aussi une unité %s ennemie', kind => {
    const { world, source } = redBuffWorld();
    const target = world.spawn({ kind, team: kind === 'monster' ? 'neutral' : 'red', pos: { x: 6250, y: 3600 }, radius: 30, hp: 2000, mana: 0, stats: stats(2000, 0, 0, 125, 300) });
    applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
    expect(getBuff(target, 'red_slow')).toBeDefined();
    world.step(1);
    expect(burns(world)).toHaveLength(1);
  });
  it('continue de brûler pendant des attaques rapides au lieu de repousser chaque tick', () => {
    const { world, source, target } = redBuffWorld();
    for (let i = 0; i < 8; i++) {
      applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
      world.step(0.25);
    }
    expect(burns(world)).toHaveLength(2);
  });
  it('résout les trois ticks même si un pas dépasse légèrement la fin de la brûlure', () => {
    const { world, source, target } = redBuffWorld();
    applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
    world.step(3 + 1e-6);
    expect(burns(world)).toHaveLength(3);
  });
});

describe('précision et transfert des buffs de jungle', () => {
  it('ne raccourcit pas le buff du tueur quand la victime avait moins de durée restante', () => {
    const { world, source, target } = redBuffWorld();
    const expiry = (source.data.jungleBuffs as { red: number }).red;
    target.data.jungleBuffs = { red: world.time + 10 };
    applyDamage(world, { source, target, amount: 100000, type: 'true' });
    expect((source.data.jungleBuffs as { red: number }).red).toBe(expiry);
  });
  it('un camp réagit même à des dégâts positifs inférieurs à une unité', () => {
    const { world, source } = redBuffWorld();
    const monster = [...world.query('monster')].find(e => e.subtype === 'gromp')!;
    applyDamage(world, { source, target: monster, amount: 0.1, type: 'true' });
    expect(monster.attackTarget).toBe(source.id);
  });
});

it('un projectile fait réagir le camp contre son propriétaire, pas contre le projectile', () => {
  const { world, source } = redBuffWorld();
  const monster = [...world.query('monster')].find(e => e.subtype === 'gromp')!;
  const projectile = world.spawn({ kind: 'projectile', team: 'blue', pos: { ...monster.pos }, radius: 1, hp: 1, mana: 0, stats: stats(1, 0), data: { owner: source.id } });
  applyDamage(world, { source: projectile, target: monster, amount: 0.1, type: 'true' });
  expect(monster.attackTarget).toBe(source.id);
});
