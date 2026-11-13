import { describe, expect, it } from 'vitest';
import { FOUNTAINS, PASSIVE_GOLD_PER_10S, TICK_RATE, scaled, PASSIVE_GOLD_START, type ClientMsg, type Stats, type Team, type Vec2 } from '@lov/shared';
import { applyDamage } from '../src/combat/damage.ts';
import { economySystem, KILL_GOLD, FIRST_BLOOD_GOLD, PLATE_GOLD, TOWER_GLOBAL_GOLD, TOWER_LOCAL_GOLD, OBJECTIVE_GOLD } from '../src/systems/economy.ts';
import { econ } from '../src/items/index.ts';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion, type Entity } from '../src/world.ts';

let n = 0;
function makeWorld(): World {
  const w = new World(createSystems());
  w.init();
  return w;
}
function addChamp(world: World, team: Team, pos: Vec2): Entity {
  return createChampion(world, { playerId: `p${++n}`, name: `T${n}`, team, championId: 'grondak', summoners: ['flash', 'ignite'], pos });
}
function run(world: World, seconds: number): void {
  for (let i = 0; i < Math.round(seconds * TICK_RATE); i++) world.step();
}
function cmd(world: World, ent: Entity, msg: ClientMsg): void {
  world.commands.push({ playerId: ent.champ!.playerId, msg });
  world.step();
}
const BASE: Stats = { maxHp: 1000, hpRegen: 0, maxMana: 0, manaRegen: 0, ad: 60, ap: 0, armor: 0, mr: 0, attackSpeed: 1, attackRange: 150, moveSpeed: 300, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0 };
function dummy(world: World, team: Team | 'neutral', pos: Vec2, kind: Entity['kind'] = 'minion', subtype = 'melee', hp = 200): Entity {
  return world.spawn({ kind, team, pos: { ...pos }, radius: 40, hp, mana: 0, subtype, stats: { ...BASE, maxHp: hp } });
}

describe('economy — or', () => {
  it('donne 500 or de départ', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', { x: 5000, y: 5000 });
    expect(c.champ!.gold).toBe(500);
  });

  it('verse l’or passif toutes les 10 s à partir de scaled(1:05)', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', { x: 5000, y: 5000 });
    run(w, scaled(PASSIVE_GOLD_START) - 1);
    expect(c.champ!.gold).toBe(500);
    run(w, 2); // passe scaled(65) = 48.75 s
    expect(c.champ!.gold).toBeCloseTo(500 + PASSIVE_GOLD_PER_10S, 5);
    run(w, 10);
    expect(c.champ!.gold).toBeCloseTo(500 + 2 * PASSIVE_GOLD_PER_10S, 5);
  });

  it('donne l’or du last hit et compte le CS', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', { x: 5000, y: 5000 });
    const m = dummy(w, 'red', { x: 5050, y: 5000 }, 'minion', 'melee', 100);
    applyDamage(w, { source: c, target: m, amount: 500, type: 'true' });
    expect(m.dead).toBe(true);
    expect(c.champ!.cs).toBe(1);
    expect(c.champ!.gold).toBeCloseTo(500 + 21, 5);
  });

  it('donne la prime de kill + premier sang au tueur', () => {
    const w = makeWorld();
    const a = addChamp(w, 'blue', { x: 5000, y: 5000 });
    const b = addChamp(w, 'red', { x: 5100, y: 5000 });
    applyDamage(w, { source: a, target: b, amount: 100000, type: 'true' });
    expect(b.dead).toBe(true);
    expect(b.champ!.deaths).toBe(1);
    expect(a.champ!.kills).toBe(1);
    expect(a.champ!.gold).toBeCloseTo(500 + KILL_GOLD + FIRST_BLOOD_GOLD, 5);
  });

  it('partage 50 % de la prime entre les assistants', () => {
    const w = makeWorld();
    const a = addChamp(w, 'blue', { x: 5000, y: 5000 });
    const c = addChamp(w, 'blue', { x: 5000, y: 5200 });
    const b = addChamp(w, 'red', { x: 5100, y: 5000 });
    applyDamage(w, { source: c, target: b, amount: 10, type: 'true' }); // C touche B -> assist
    applyDamage(w, { source: a, target: b, amount: 100000, type: 'true' });
    expect(c.champ!.assists).toBe(1);
    expect(c.champ!.gold).toBeCloseTo(500 + (KILL_GOLD * 0.5), 5);
    expect(a.champ!.gold).toBeCloseTo(500 + KILL_GOLD + FIRST_BLOOD_GOLD, 5);
  });

  it('augmente la prime avec la série de la victime et la réduit avec ses morts', () => {
    const w1 = makeWorld();
    const a1 = addChamp(w1, 'blue', { x: 5000, y: 5000 });
    const b1 = addChamp(w1, 'red', { x: 5100, y: 5000 });
    econ(b1).killStreak = 3;
    applyDamage(w1, { source: a1, target: b1, amount: 100000, type: 'true' });
    expect(a1.champ!.gold).toBeCloseTo(500 + 450 + FIRST_BLOOD_GOLD, 5); // 300 + 150 série + 100 first blood

    const w2 = makeWorld();
    const a2 = addChamp(w2, 'blue', { x: 5000, y: 5000 });
    const b2 = addChamp(w2, 'red', { x: 5100, y: 5000 });
    econ(b2).deathStreak = 2;
    applyDamage(w2, { source: a2, target: b2, amount: 100000, type: 'true' });
    expect(a2.champ!.gold).toBeCloseTo(500 + (KILL_GOLD - 50) + FIRST_BLOOD_GOLD, 5);
  });

  it('donne l’or global + local des tours et des plaques', () => {
    const w = makeWorld();
    const near = addChamp(w, 'blue', { x: 5000, y: 5000 });
    const far = addChamp(w, 'blue', { x: 11000, y: 11000 });
    const tower = dummy(w, 'red', { x: 5100, y: 5000 }, 'tower', 'outer', 5000);
    w.events.emit({ type: 'structureDestroyed', target: tower.id, kind: 'tower', team: 'red' });
    expect(near.champ!.gold).toBeCloseTo(500 + TOWER_GLOBAL_GOLD + TOWER_LOCAL_GOLD, 5);
    expect(far.champ!.gold).toBeCloseTo(500 + TOWER_GLOBAL_GOLD, 5);
    w.events.emit({ type: 'structureDestroyed', target: tower.id, kind: 'tower', team: 'red', plate: true });
    expect(near.champ!.gold).toBeCloseTo(500 + TOWER_GLOBAL_GOLD + TOWER_LOCAL_GOLD + PLATE_GOLD, 5);
    expect(far.champ!.gold).toBeCloseTo(500 + TOWER_GLOBAL_GOLD, 5); // plaque = local seulement
  });

  it('simulation : or total à 10 min cohérent avec une partie LoL-like', () => {
    const w = makeWorld();
    const c = addChamp(w, 'blue', { x: 5000, y: 5000 });
    // 10 min de jeu : un last hit de sbire mêlée toutes les 30 s (vagues)
    for (let wave = 0; wave < 20; wave++) {
      const m = dummy(w, 'red', { x: 5050, y: 5000 }, 'minion', 'melee', 100);
      applyDamage(w, { source: c, target: m, amount: 500, type: 'true' });
      run(w, 30);
    }
    // 500 départ + passif (~56 × 20,4 après 48,75 s) + 20 × 21 de last hits
    expect(c.champ!.gold).toBeGreaterThan(1900);
    expect(c.champ!.gold).toBeLessThan(2300);
    expect(c.champ!.cs).toBe(20);
  });

  it('donne l’or d’équipe des objectifs', () => {
    const w = makeWorld();
    const a = addChamp(w, 'blue', { x: 5000, y: 5000 });
    const b = addChamp(w, 'red', { x: 9000, y: 9000 });
    w.events.emit({ type: 'objectiveTaken', monster: 'dragon', team: 'blue' });
    expect(a.champ!.gold).toBeCloseTo(500 + (OBJECTIVE_GOLD.dragon ?? 25), 5);
    expect(b.champ!.gold).toBe(500);
  });
});
