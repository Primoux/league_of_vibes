import { describe, expect, it } from 'vitest';
import { TIMERS, scaled } from '@lov/shared';
import { World, createChampion } from '../src/world.ts';
import { objectivesSystem, getObjectiveState, getObjectiveTimers, useHeraldEye } from '../src/systems/objectives.ts';
import { minionsSystem, spawnWave } from '../src/systems/minions.ts';
import { structuresSystem, structureState } from '../src/systems/structures.ts';
import { applyDamage } from '../src/world-ai/runtime.ts';
function setup() {
  const world = new World([minionsSystem, structuresSystem, objectivesSystem]); world.init();
  const champion = createChampion(world, { playerId: 'p', name: 'Test', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 7900, y: 4100 } });
  return { world, champion };
}
function dragon(world: World, champion: ReturnType<typeof createChampion>) {
  const slot = getObjectiveState(world)!.dragon;
  world.step(Math.max(0.01, slot.next - world.time));
  const target = world.get(slot.ids[0])!;
  applyDamage(world, { source: champion, target, amount: 100000, type: 'true' }); return target;
}
describe('objectifs épiques', () => {
  it('publie les timers et garde un état isolé par partie', () => {
    const { world } = setup(); expect(getObjectiveTimers(world, 'blue').dragon).toBe(scaled(TIMERS.dragonFirstSpawn));
    expect(getObjectiveState(setup().world)).not.toBe(getObjectiveState(world));
  });
  it('fixe le type de la Faille au troisième drake puis donne l’âme au quatrième', () => {
    const { world, champion } = setup();
    dragon(world, champion); dragon(world, champion); const third = dragon(world, champion); const fourth = dragon(world, champion);
    const state = getObjectiveState(world)!;
    expect(fourth.data.dragonElement).toBe(third.data.dragonElement);
    expect(state.teams.blue.dragons).toHaveLength(4); expect(state.teams.blue.soul).toBe(third.data.dragonElement);
    expect(state.elder).toBe(true); expect(getObjectiveTimers(world, 'blue').elder_dragon).toBe(world.time + scaled(TIMERS.elderRespawn));
    expect(world.events.pending.filter(e => e.type === 'announcement' && e.key === 'dragon_soul')).toHaveLength(1);
  });
  it('l’Ancien exécute sous 20 % et n’accorde pas son buff aux morts', () => {
    const { world, champion } = setup(); for (let i = 0; i < 4; i++) dragon(world, champion);
    const deadAlly = createChampion(world, { playerId: 'd', name: 'Mort', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { ...champion.pos } }); deadAlly.dead = true;
    const elder = dragon(world, champion); expect(elder.subtype).toBe('elder_dragon'); expect(deadAlly.data.elderBuff).toBeUndefined();
    const enemy = createChampion(world, { playerId: 'e', name: 'Ennemi', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { ...champion.pos } });
    enemy.hp = enemy.stats.maxHp * 0.19;
    applyDamage(world, { source: champion, target: enemy, amount: 1, type: 'true' });
    expect(enemy.dead).toBe(true); expect(world.events.pending.some(e => e.type === 'damage' && e.ability === 'elder_execute')).toBe(true);
  });
  it('le Léviathan renforce les vivants et les sbires proches puis expire', () => {
    const { world, champion } = setup(); champion.pos = { x: 2000, y: 2000 };
    const deadAlly = createChampion(world, { playerId: 'd', name: 'Mort', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { ...champion.pos } }); deadAlly.dead = true;
    world.step(scaled(TIMERS.leviathanSpawn)); const target = world.get(getObjectiveState(world)!.leviathan.ids[0])!;
    applyDamage(world, { source: champion, target, amount: 100000, type: 'true' });
    expect(champion.data.leviathanBuff).toMatchObject({ recallSeconds: 4 }); expect(deadAlly.data.leviathanBuff).toBeUndefined();
    spawnWave(world, 1); world.step();
    const minion = [...world.query('minion')].find(e => e.team === 'blue' && (e.data.minion as { lane: string }).lane === 'mid')!;
    expect(minion.data.leviathanMinion).toBe(true);
    world.step(scaled(TIMERS.leviathanBuffDuration)); expect(champion.data.leviathanBuff).toBeUndefined(); expect(minion.data.leviathanMinion).toBeUndefined();
  });
  it('trois larves donnent trois cumuls et sont remplacées par le Colosse', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.grubsSpawn));
    for (const id of [...getObjectiveState(world)!.grubs.ids]) applyDamage(world, { source: champion, target: world.get(id)!, amount: 100000, type: 'true' });
    expect(getObjectiveState(world)!.teams.blue.grubs).toBe(3);
    world.step(scaled(TIMERS.heraldSpawn) - world.time); expect(getObjectiveState(world)!.herald.ids).toHaveLength(1);
    expect([...world.query('monster')].filter(e => !e.dead && e.subtype === 'grub')).toHaveLength(0);
  });
  it('accorde un œil à usage unique dont l’invocation charge une tour', () => {
    const { world, champion } = setup(); world.step(scaled(TIMERS.heraldSpawn));
    const herald = world.get(getObjectiveState(world)!.herald.ids[0])!;
    applyDamage(world, { source: champion, target: herald, amount: 100000, type: 'true' });
    const tower = [...world.query('tower')].find(e => e.team === 'red' && e.subtype === 'outer' && structureState(e).lane === 'mid')!;
    champion.pos = { x: tower.pos.x - 300, y: tower.pos.y };
    expect(useHeraldEye(world, champion)).toBe(true); expect(useHeraldEye(world, champion)).toBe(false);
    world.step(); expect(tower.hp).toBeLessThan(tower.stats.maxHp - 1000);
  });
  it('retire le Colosse non tué à 19:45 et laisse la fosse au Léviathan', () => {
    const { world } = setup(); world.step(scaled(TIMERS.heraldSpawn)); const id = getObjectiveState(world)!.herald.ids[0]!;
    world.step(scaled(TIMERS.heraldDespawn) - world.time); expect(world.get(id)).toBeUndefined();
    world.step(scaled(TIMERS.leviathanSpawn) - world.time); expect(getObjectiveState(world)!.leviathan.ids).toHaveLength(1);
  });
});
