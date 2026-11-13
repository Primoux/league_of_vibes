import { describe, expect, it } from 'vitest';
import { FOUNTAINS, TICK_RATE, type ClientMsg, type Vec2 } from '@lov/shared';
import { hit } from '../src/systems/combat.ts';
import { econ } from '../src/items/index.ts';
import { WARD_CAST_RANGE, WARD_HP, WARD_LIFETIME, WARD_MAX_CHARGES, WARD_RECHARGE, WARD_SIGHT } from '../src/systems/economy.ts';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion, type Entity } from '../src/world.ts';

let n = 0;
function makeWorld(): World {
  const w = new World(createSystems());
  w.init();
  return w;
}
function addChamp(world: World, team: 'blue' | 'red', pos: Vec2, championId = 'grondak'): Entity {
  return createChampion(world, { playerId: `p${++n}`, name: `T${n}`, team, championId, summoners: ['flash', 'ignite'], pos });
}
function run(world: World, seconds: number): void {
  for (let i = 0; i < Math.round(seconds * TICK_RATE); i++) world.step();
}
function cmd(world: World, ent: Entity, msg: ClientMsg): void {
  world.commands.push({ playerId: ent.champ!.playerId, msg });
  world.step();
}
function wards(world: World): Entity[] {
  return [...world.query('ward')];
}

describe('balises (trinket)', () => {
  it('pose une balise, consomme une charge, 2 charges max', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    expect(econ(c).trinketCharges).toBe(WARD_MAX_CHARGES);
    cmd(w, c, { t: 'placeWard', pos: { x: 600, y: 600 } });
    expect(wards(w)).toHaveLength(1);
    expect(econ(c).trinketCharges).toBe(1);
    expect(wards(w)[0]!.data.invisible).toBe(true);
    cmd(w, c, { t: 'placeWard', pos: { x: 700, y: 600 } });
    expect(wards(w)).toHaveLength(2);
    expect(econ(c).trinketCharges).toBe(0);
    cmd(w, c, { t: 'placeWard', pos: { x: 800, y: 600 } });
    expect(wards(w)).toHaveLength(2); // plus de charge
  });

  it('refuse la pose sans trinket ou hors de portée', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'placeWard', pos: { x: 600, y: 600 } });
    expect(wards(w)).toHaveLength(0);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    cmd(w, c, { t: 'placeWard', pos: { x: FOUNTAINS.blue.x + WARD_CAST_RANGE + 500, y: FOUNTAINS.blue.y } });
    expect(wards(w)).toHaveLength(0);
  });

  it('recharge une charge toutes les 90 s', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    cmd(w, c, { t: 'placeWard', pos: { x: 600, y: 600 } });
    cmd(w, c, { t: 'placeWard', pos: { x: 700, y: 600 } });
    expect(econ(c).trinketCharges).toBe(0);
    run(w, WARD_RECHARGE + 1);
    expect(econ(c).trinketCharges).toBe(1);
  });

  it('la balise expire après 90 s', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    cmd(w, c, { t: 'placeWard', pos: { x: 600, y: 600 } });
    expect(wards(w)).toHaveLength(1);
    run(w, WARD_LIFETIME - 1);
    expect(wards(w)).toHaveLength(1);
    run(w, 4); // expiration à 90 s + 2 s de cadavre (combat)
    expect(wards(w)).toHaveLength(0);
  });

  it('donne la vision à son équipe (rayon 900) et se révèle si un ennemi approche', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    c.pos = { x: 3000, y: 3000 };
    cmd(w, c, { t: 'placeWard', pos: { x: 3100, y: 3000 } });
    const ward = wards(w)[0]!;
    expect(ward).toBeDefined();
    expect(ward.data.sightRadius).toBe(WARD_SIGHT);
    const enemy = addChamp(w, 'red', { x: 3000 + WARD_SIGHT - 50, y: 3000 });
    run(w, 0.1);
    expect(ward.data.revealedUntil).toBeGreaterThan(w.time - 1);
    expect(enemy.pos.x).toBeLessThanOrEqual(3000 + WARD_SIGHT);
  });

  it('est détruite en 3 auto-attaques (1 dégât par attaque)', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', FOUNTAINS.blue);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    c.pos = { x: 3000, y: 3000 };
    cmd(w, c, { t: 'placeWard', pos: { x: 3100, y: 3000 } });
    const ward = wards(w)[0]!;
    expect(ward).toBeDefined();
    expect(ward.hp).toBe(WARD_HP);
    const enemy = addChamp(w, 'red', { x: 3150, y: 3000 });
    run(w, 0.1); // révèle la balise
    hit(w, enemy, ward, false);
    expect(ward.hp).toBe(WARD_HP - 1);
    hit(w, enemy, ward, false);
    hit(w, enemy, ward, false);
    expect(ward.dead).toBe(true);
  });
});
