import { describe, expect, it } from 'vitest';
import { TIMERS, scaled } from '@lov/shared';
import { World, createChampion } from '../src/world.ts';
import { minionsSystem, spawnWave } from '../src/systems/minions.ts';
import { structuresSystem } from '../src/systems/structures.ts';
import { movementSystem } from '../src/systems/movement.ts';
function setup(structures = false) { const world = new World([movementSystem, minionsSystem, ...(structures ? [structuresSystem] : [])]); world.init(); return world; }
describe('sbires', () => {
  it('respecte les timers, les trois voies et la composition canon', () => {
    const world = setup(); world.step(scaled(TIMERS.minionFirstWave) - 0.1); expect(world.entities.size).toBe(0);
    world.step(0.1); expect(world.entities.size).toBe(36);
    spawnWave(world, 3); expect([...world.query('minion')].filter(e => e.subtype === 'cannon')).toHaveLength(6);
    world.time = scaled(TIMERS.cannonLateAt); spawnWave(world, 4); expect([...world.query('minion')].filter(e => e.subtype === 'cannon')).toHaveLength(12);
  });
  it('ajoute un super-sbire uniquement dans la voie d’un inhibiteur ennemi détruit', () => {
    const world = setup(true); const inhibitor = [...world.query('inhibitor')].find(e => e.team === 'red')!; inhibitor.dead = true;
    spawnWave(world, 1); const supers = [...world.query('minion')].filter(e => e.subtype === 'super');
    expect(supers).toHaveLength(1); expect(supers[0]!.team).toBe('blue');
  });
  it('conserve la symétrie de la voie mid et engage près du milieu', () => {
    const world = setup(); spawnWave(world, 1);
    let met = false;
    for (let i = 0; i < 1200; i++) {
      world.step(); world.events.drain();
      const blue = [...world.query('minion')].find(e => !e.dead && e.team === 'blue' && (e.data.minion as { lane: string }).lane === 'mid');
      const red = [...world.query('minion')].find(e => !e.dead && e.team === 'red' && (e.data.minion as { lane: string }).lane === 'mid');
      if (blue?.attackTarget && red?.attackTarget) {
        expect(blue.pos.x + red.pos.x).toBeCloseTo(12000, 0); expect(blue.pos.x).toBeGreaterThan(5400); met = true; break;
      }
    }
    expect(met).toBe(true);
  });
  it('réagit à une agression de champion même avec une larvelette alliée proche', async () => {
    const { applyDamage, stats } = await import('../src/world-ai/runtime.ts');
    const world = setup(); spawnWave(world, 1);
    const defender = [...world.query('minion')].find(e => e.team === 'blue' && (e.data.minion as { lane: string }).lane === 'mid')!;
    defender.pos = { x: 6000, y: 6000 };
    const ally = createChampion(world, { playerId: 'ally', name: 'Allié', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6100, y: 6100 } });
    const aggressor = createChampion(world, { playerId: 'enemy', name: 'Ennemi', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6400, y: 6000 } });
    world.spawn({ kind: 'minion', subtype: 'grubling', team: 'blue', pos: { x: 6050, y: 6000 }, radius: 30, hp: 250, mana: 0, stats: stats(250, 25), data: { grubling: {} } });
    world.spawn({ kind: 'minion', subtype: 'melee', team: 'red', pos: { x: 6200, y: 6000 }, radius: 30, hp: 500, mana: 0, stats: stats(500, 10) });
    applyDamage(world, { source: aggressor, target: ally, amount: 1, type: 'true' }); world.step();
    expect(defender.attackTarget).toBe(aggressor.id);
  });
  it('une tour défend une vague seule et une partie vide ne se termine pas', () => {
    const world = setup(true); spawnWave(world, 1);
    for (let i = 0; i < 10 * 180; i++) { world.step(0.1); world.events.drain(); }
    expect(world.winner).toBeNull();
    expect([...world.query('tower')].every(e => !e.dead)).toBe(true);
    expect([...world.query('minion')].filter(e => e.dead).length).toBeLessThan(50);
  });
});

// Validation d'intégration contre le vrai pathfinding, les projectiles et le combat.
describe('intégration PvE', () => {
  it('ne double pas les attaques quand le combat central est présent', async () => {
    const { combatSystem } = await import('../src/systems/combat.ts');
    const { stats } = await import('../src/world-ai/runtime.ts');
    const { applyDamage } = await import('../src/combat/damage.ts');
    const world = new World([combatSystem, structuresSystem]); world.init();
    const tower = [...world.query('tower')].find(e => e.team === 'blue' && e.subtype === 'outer')!;
    const minion = world.spawn({ kind: 'minion', subtype: 'melee', team: 'red', pos: { x: tower.pos.x + 200, y: tower.pos.y }, radius: 30, hp: 1000, mana: 0, stats: stats(1000, 0) });
    world.step(); expect(minion.hp).toBe(1000);
    for (let i = 0; i < 20; i++) world.step(); expect(minion.hp).toBe(550);
    const inner = [...world.query('tower')].find(e => e.team === 'blue' && e.subtype === 'inner')!;
    applyDamage(world, { source: minion, target: inner, amount: 10000, type: 'true' }); expect(inner.dead).toBe(false);
  });
  it('une partie sans joueurs reste défendable durant trente minutes simulées', async () => {
    const { createSystems } = await import('../src/systems/index.ts');
    const world = new World(createSystems()); world.init();
    for (let i = 0; i < 3600; i++) { world.step(0.5); world.events.drain(); }
    expect(world.time).toBe(1800);
    expect(world.winner).toBeNull(); expect([...world.query('nexus')].every(e => !e.dead)).toBe(true);
    expect(world.entities.size).toBeLessThan(1000);
    // Le pas de 0,5 s accélère seulement ce scénario d'endurance ; perf.test.ts garde le tick réel.
  }, 10000);
});
