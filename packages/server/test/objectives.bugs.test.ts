import { describe, expect, it } from 'vitest';
import { World, createChampion } from '../src/world.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { buffsSystem } from '../src/systems/buffs.ts';
import { objectivesSystem, getObjectiveState } from '../src/systems/objectives.ts';
import { structuresSystem } from '../src/systems/structures.ts';

function combatants() {
  const world = new World([buffsSystem, structuresSystem, objectivesSystem]); world.init();
  const source = createChampion(world, { playerId: 'source', name: 'Source', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6000, y: 6000 } });
  const target = createChampion(world, { playerId: 'target', name: 'Cible', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6100, y: 6000 } });
  return { world, source, target, state: getObjectiveState(world)! };
}
describe('régressions des effets épiques', () => {
  it('l’Ancien brûle pendant des attaques répétées', () => {
    const { world, source, target, state } = combatants();
    state.teams.blue.elderUntil = 10; source.data.elderBuff = { expiresAt: 10 };
    for (let i = 0; i < 8; i++) {
      applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'test_hit' });
      world.step(0.25);
    }
    expect(world.events.pending.filter(e => e.type === 'damage' && e.ability === 'elder_burn')).toHaveLength(2);
  });
  it('l’Ancien résout les ticks échus après un pas de simulation plus long', () => {
    const { world, source, target, state } = combatants();
    state.teams.blue.elderUntil = 10; source.data.elderBuff = { expiresAt: 10 };
    applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'test_hit' });
    world.step(3 + 1e-6);
    expect(world.events.pending.filter(e => e.type === 'damage' && e.ability === 'elder_burn')).toHaveLength(3);
  });
  it('les larves ne réinfligent pas de dégâts à une tour dont les PV sont déjà épuisés', () => {
    const { world, source, state } = combatants(); world.time = 200; state.teams.blue.grubs = 1;
    const tower = [...world.query('tower')].find(e => e.team === 'red' && e.subtype === 'outer')!;
    applyDamage(world, { source, target: tower, amount: 100000, type: 'true' });
    const damage = world.events.pending.filter(e => e.type === 'damage' && e.target === tower.id);
    expect(damage).toHaveLength(1);
    expect(damage.every(e => e.type !== 'damage' || e.amount >= 0)).toBe(true);
  });
  it('une âme infernale explose sur un coup létal sans frapper de nouveau la victime mourante', () => {
    const { world, source, target, state } = combatants(); state.teams.blue.soul = 'infernal'; target.hp = 1;
    const neighbor = createChampion(world, { playerId: 'neighbor', name: 'Voisin', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 6150, y: 6000 } });
    applyDamage(world, { source, target, amount: 10, type: 'true' });
    expect(target.dead).toBe(true);
    expect(neighbor.hp).toBeLessThan(neighbor.stats.maxHp);
    const damage = world.events.pending.filter(e => e.type === 'damage' && e.target === target.id);
    expect(damage).toHaveLength(1);
    expect(damage.every(e => e.type !== 'damage' || e.amount >= 0)).toBe(true);
  });
});

describe('effets épiques sur petits dégâts', () => {
  it('l’Ancien exécute aussi sur un impact positif arrondi à zéro pour le réseau', () => {
    const { world, source, target, state } = combatants();
    state.teams.blue.elderUntil = 10; source.data.elderBuff = { expiresAt: 10 };
    target.hp = target.stats.maxHp * 0.19;
    applyDamage(world, { source, target, amount: 0.1, type: 'true' });
    expect(target.dead).toBe(true);
  });
  it('une âme infernale réagit à un impact positif inférieur à une unité', () => {
    const { world, source, target, state } = combatants(); state.teams.blue.soul = 'infernal';
    const hp = target.hp;
    applyDamage(world, { source, target, amount: 0.1, type: 'true' });
    expect(hp - target.hp).toBeGreaterThan(1);
  });
});
