import { describe, expect, it } from 'vitest';
import { TIMERS, scaled } from '@lov/shared';
import { World } from '../src/world.ts';
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
  it('une tour défend une vague seule et une partie vide ne se termine pas', () => {
    const world = setup(true); spawnWave(world, 1);
    for (let i = 0; i < 30 * 180; i++) { world.step(); world.events.drain(); }
    expect(world.winner).toBeNull();
    expect([...world.query('tower')].every(e => !e.dead)).toBe(true);
    expect([...world.query('minion')].filter(e => e.dead).length).toBeLessThan(50);
  });
});
