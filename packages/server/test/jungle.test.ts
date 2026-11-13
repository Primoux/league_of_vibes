import { describe, expect, it } from 'vitest';
import { CAMPS, TIMERS, scaled } from '@lov/shared';
import { World, createChampion } from '../src/world.ts';
import { jungleSystem, getCampTimers } from '../src/systems/jungle.ts';
import { movementSystem } from '../src/systems/movement.ts';
import { applyDamage } from '../src/world-ai/runtime.ts';
function setup() {
  const world = new World([movementSystem, jungleSystem]); world.init();
  const champion = createChampion(world, { playerId: 'p', name: 'Test', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 3700, y: 6100 } });
  return { world, champion };
}
describe('jungle', () => {
  it('fait apparaître tous les camps et leurs petits monstres au bon moment', () => {
    const { world } = setup(); world.step(scaled(TIMERS.jungleFirstSpawn) - 0.1); expect([...world.query('monster')]).toHaveLength(0);
    world.step(0.1); expect([...world.query('monster')]).toHaveLength(CAMPS.filter(c => c.monster !== 'scuttle').reduce((n, c) => n + 1 + (c.smalls?.length ?? 0), 0));
    world.step(scaled(TIMERS.scuttleFirstSpawn) - world.time); expect([...world.query('monster')].filter(e => e.subtype === 'scuttle')).toHaveLength(2);
  });
  it('aggro tout le camp et reset hors leash en revenant avec régénération', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.jungleFirstSpawn));
    const wolf = [...world.query('monster')].find(e => e.subtype === 'wolves')!;
    champion.pos = { ...wolf.pos }; applyDamage(world, { source: champion, target: wolf, amount: 100, type: 'true' });
    const pack = [...world.query('monster')].filter(e => (e.data.monster as { camp: string }).camp === 'blue_wolves');
    expect(pack.every(e => e.attackTarget === champion.id)).toBe(true);
    const home = { ...wolf.pos }; wolf.pos.x += 900; world.step(); expect((wolf.data.monster as { resetting: boolean }).resetting).toBe(true);
    for (let i = 0; i < 180; i++) world.step();
    expect(wolf.hp).toBe(wolf.stats.maxHp); expect(wolf.pos).toEqual(home); expect(wolf.attackTarget).toBeUndefined();
  });
  it('ne respawn le camp qu’après la mort de tous ses monstres', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.jungleFirstSpawn));
    const pack = [...world.query('monster')].filter(e => (e.data.monster as { camp: string }).camp === 'blue_wolves');
    applyDamage(world, { source: champion, target: pack[0]!, amount: 100000, type: 'true' }); world.step();
    expect(getCampTimers(world).blue_wolves).toBeUndefined();
    for (const e of pack.slice(1)) applyDamage(world, { source: champion, target: e, amount: 100000, type: 'true' });
    world.step(); const at = getCampTimers(world).blue_wolves!; expect(at).toBeCloseTo(world.time + scaled(TIMERS.jungleSmallRespawn));
    world.step(at - world.time); expect([...world.query('monster')].filter(e => !e.dead && (e.data.monster as { camp: string }).camp === 'blue_wolves')).toHaveLength(3);
  });
  it('accorde le buff bleu et transfère sa durée restante au tueur', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.jungleFirstSpawn));
    const blue = [...world.query('monster')].find(e => e.subtype === 'blue_buff')!;
    applyDamage(world, { source: champion, target: blue, amount: 100000, type: 'true' });
    const expires = (champion.data.jungleBuffs as { blue: number }).blue;
    const enemy = createChampion(world, { playerId: 'e', name: 'Ennemi', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { ...champion.pos } });
    world.step(10); applyDamage(world, { source: enemy, target: champion, amount: 100000, type: 'true' });
    expect((enemy.data.jungleBuffs as { blue: number }).blue).toBe(expires);
    expect(champion.data.jungleBuffs).toEqual({});
  });
  it('crée un sanctuaire temporaire allié après le crabe', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.scuttleFirstSpawn));
    const crab = [...world.query('monster')].find(e => e.subtype === 'scuttle')!;
    applyDamage(world, { source: champion, target: crab, amount: 100000, type: 'true' });
    expect([...world.query('ward')]).toHaveLength(1); expect([...world.query('ward')][0]!.team).toBe('blue');
    world.step(90); expect([...world.query('ward')]).toHaveLength(0);
  });
});
