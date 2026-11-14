/**
 * Tests d'équilibrage (branche codex-objectives-balance, demande utilisateur).
 * Mesurés headless contre la vraie simulation ; bornes volontairement larges pour rester
 * robustes en CI, mais resserrées par rapport aux valeurs d'avant équilibrage
 * (tour externe 5000 PV / backdoor +200 ; sbires caster 24 AD ; gromp 2050 PV / 74 AD).
 */
import { describe, expect, it } from 'vitest';
import { scaled } from '@lov/shared';
import { MINIONS } from '../../shared/src/data/minions.ts';
import { MONSTERS } from '../../shared/src/data/monsters.ts';
import { STRUCTURES } from '../../shared/src/data/structures.ts';
import { World, createChampion, type Entity } from '../src/world.ts';
import { createSystems } from '../src/systems/index.ts';
import { spawnWave } from '../src/systems/minions.ts';
import { grantXP, xpForLevel } from '../src/systems/leveling.ts';
import { recalculateStats } from '../src/combat/buffs.ts';
import { structureState } from '../src/systems/structures.ts';

function makeWorld(): World {
  const w = new World(createSystems());
  w.init();
  return w;
}
function run(w: World, seconds: number, until?: () => boolean): number {
  const ticks = Math.round(seconds * 30);
  let t = 0;
  for (let i = 0; i < ticks; i++) {
    w.step();
    t += 1 / 30;
    w.events.drain();
    if (until?.()) break;
  }
  return t;
}
function levelUp(w: World, e: Entity, level: number) {
  grantXP(w, e, xpForLevel(level));
  e.champ!.skillPoints = 0;
  recalculateStats(w, e);
  e.hp = e.stats.maxHp;
  e.mana = e.stats.maxMana;
}
function giveItems(w: World, e: Entity, items: string[]) {
  e.champ!.items = [...items, null, null, null, null, null] as (string | null)[];
  recalculateStats(w, e);
  e.hp = e.stats.maxHp;
}

describe('équilibrage — données', () => {
  it('tours : PV et backdoor dans des bornes jouables', () => {
    expect(STRUCTURES.towers.outer.hp).toBeLessThanOrEqual(4000);
    expect(STRUCTURES.towers.outer.armor).toBeLessThanOrEqual(35);
    expect(STRUCTURES.backdoorResistance).toBeLessThanOrEqual(100);
  });
  it('sbires : dégâts de vague contenus', () => {
    expect(MINIONS.caster.ad).toBeLessThanOrEqual(20);
    expect(MINIONS.melee.ad).toBeLessThanOrEqual(12);
  });
  it('monstres : camps clearingables tôt, dégâts modérés', () => {
    expect(MONSTERS.gromp.hp).toBeLessThanOrEqual(1500);
    expect(MONSTERS.gromp.ad).toBeLessThanOrEqual(60);
    expect(MONSTERS.blue_buff.hp).toBeLessThanOrEqual(1600);
  });
  it('monstres : XP de jungle suffisante (parité voie à 5 min mesurée en sim)', () => {
    expect(MONSTERS.gromp.xp).toBeGreaterThanOrEqual(250);
    expect(MONSTERS.blue_buff.xp).toBeGreaterThanOrEqual(250);
    expect(MONSTERS.scuttle.xp).toBeGreaterThanOrEqual(150);
    expect(MONSTERS.dragon.xp).toBeGreaterThanOrEqual(200);
  });
});

describe('équilibrage — simulation', () => {
  it('une tour externe clear une vague en 8-20 s sans mourir à une seule vague', () => {
    const w = makeWorld();
    w.time = scaled(400); // après la fortification
    const tower = [...w.query('tower')].find((e) => e.team === 'red' && e.subtype === 'outer' && structureState(e).lane === 'mid')!;
    spawnWave(w, 1);
    const wave = [...w.query('minion')].filter((m) => m.team === 'blue');
    for (const [i, m] of wave.entries()) {
      m.pos = { x: tower.pos.x + 100 + (i % 6) * 40, y: tower.pos.y + Math.floor(i / 6) * 40 };
      m.attackTarget = tower.id;
    }
    const tClear = run(w, 60, () => wave.every((m) => m.dead));
    expect(tClear).toBeGreaterThan(8);
    expect(tClear).toBeLessThan(20);
    expect(tower.dead).toBe(false); // une vague seule ne tue pas une tour
  }, 30000);

  it('un jungler niveau 3 (machete) clear le gromp en moins de 35 s et survit', () => {
    const w = makeWorld();
    w.time = scaled(90);
    run(w, 0.5);
    const camp = [...w.query('monster')].find((m) => m.subtype === 'gromp' && !m.dead)!;
    const c = createChampion(w, { playerId: 'j', name: 'J', team: 'blue', championId: 'tharox', summoners: ['smite', 'flash'], pos: { x: camp.pos.x + 100, y: camp.pos.y } });
    levelUp(w, c, 3);
    giveItems(w, c, ['hunter_machete']);
    let t = 0;
    const ticks = 40 * 30;
    for (let i = 0; i < ticks && !camp.dead; i++) {
      if (i % 30 === 0) {
        c.attackTarget = camp.id;
        c.pos = { x: camp.pos.x + 100, y: camp.pos.y };
      }
      w.step();
      t += 1 / 30;
      w.events.drain();
    }
    expect(camp.dead).toBe(true);
    expect(t).toBeLessThan(35);
    expect(c.dead).toBe(false);
    expect(c.hp / c.stats.maxHp).toBeGreaterThan(0.2);
  }, 30000);
});
