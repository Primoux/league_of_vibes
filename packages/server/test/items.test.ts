import { describe, expect, it } from 'vitest';
import { FOUNTAINS, TICK_RATE, type ClientMsg, type Vec2 } from '@lov/shared';
import { applyDamage, applyHeal } from '../src/combat/damage.ts';
import { hit } from '../src/systems/combat.ts';
import { econ } from '../src/items/index.ts';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion, type Entity } from '../src/world.ts';
import type { Stats } from '@lov/shared';

let n = 0;
function makeWorld(): World {
  const w = new World(createSystems());
  w.init();
  return w;
}
function addChamp(world: World, pos: Vec2, championId = 'grondak'): Entity {
  return createChampion(world, { playerId: `p${++n}`, name: `T${n}`, team: 'blue', championId, summoners: ['flash', 'ignite'], pos });
}
function run(world: World, seconds: number): void {
  for (let i = 0; i < Math.round(seconds * TICK_RATE); i++) world.step();
}
function cmd(world: World, ent: Entity, msg: ClientMsg): void {
  world.commands.push({ playerId: ent.champ!.playerId, msg });
  world.step();
}
const BASE: Stats = { maxHp: 1000, hpRegen: 0, maxMana: 0, manaRegen: 0, ad: 60, ap: 0, armor: 0, mr: 0, attackSpeed: 1, attackRange: 150, moveSpeed: 300, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0 };
function dummy(world: World, pos: Vec2): Entity {
  return world.spawn({ kind: 'minion', team: 'red', pos: { ...pos }, radius: 40, hp: 500, mana: 0, stats: { ...BASE, maxHp: 500 } });
}
const AT_FOUNTAIN = FOUNTAINS.blue;

describe('boutique', () => {
  it('achète un objet à la fontaine (or déduit, slot rempli, stats appliquées)', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    const adBefore = c.stats.ad;
    const hpBefore = c.stats.maxHp;
    cmd(w, c, { t: 'buy', itemId: 'long_sword' });
    expect(c.champ!.gold).toBe(150);
    expect(c.champ!.items[0]).toBe('long_sword');
    run(w, 0.1);
    expect(c.stats.ad).toBeCloseTo(adBefore + 10, 5);
    expect(c.stats.maxHp).toBe(hpBefore);
  });

  it('refuse l’achat hors fontaine (vivant) mais l’autorise en mort', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    cmd(w, c, { t: 'buy', itemId: 'long_sword' });
    expect(c.champ!.gold).toBe(500);
    expect(c.champ!.items.every((i) => i === null)).toBe(true);
    c.dead = true;
    cmd(w, c, { t: 'buy', itemId: 'long_sword' });
    expect(c.champ!.items[0]).toBe('long_sword');
  });

  it('refuse l’achat sans or suffisant', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 100;
    cmd(w, c, { t: 'buy', itemId: 'long_sword' });
    expect(c.champ!.items.every((i) => i === null)).toBe(true);
    expect(c.champ!.gold).toBe(100);
  });

  it('recette : consomme les composants et déduit leur coût', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 2000;
    const msBase = c.stats.moveSpeed;
    const asBase = c.stats.attackSpeed;
    cmd(w, c, { t: 'buy', itemId: 'boots' });
    expect(c.champ!.gold).toBe(1700);
    cmd(w, c, { t: 'buy', itemId: 'berserker_greaves' }); // 1100 - 300 (bottes possédées) = 800
    expect(c.champ!.gold).toBe(900);
    expect(c.champ!.items).toContain('berserker_greaves');
    expect(c.champ!.items).not.toContain('boots');
    expect(econ(c).paidPrice[c.champ!.items.indexOf('berserker_greaves')]).toBe(1100);
    run(w, 0.1);
    expect(c.stats.moveSpeed).toBeCloseTo(msBase + 45, 5);
    expect(c.stats.attackSpeed).toBeCloseTo(asBase + 0.35, 5);
  });

  it('undo : rembourse l’achat et rend les composants', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 2000;
    cmd(w, c, { t: 'buy', itemId: 'boots' });
    cmd(w, c, { t: 'buy', itemId: 'berserker_greaves' });
    expect(c.champ!.gold).toBe(900);
    cmd(w, c, { t: 'undoShop' }); // annule seulement le dernier achat (800 or)
    expect(c.champ!.gold).toBe(1700);
    expect(c.champ!.items).toContain('boots');
    expect(c.champ!.items).not.toContain('berserker_greaves');
  });

  it('vente à 70 % du prix payé, undo de la vente', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    cmd(w, c, { t: 'buy', itemId: 'long_sword' }); // 350 -> 150
    cmd(w, c, { t: 'sell', slot: 0 }); // +245
    expect(c.champ!.gold).toBe(150 + 245);
    expect(c.champ!.items[0]).toBeNull();
    cmd(w, c, { t: 'undoShop' });
    expect(c.champ!.gold).toBe(150);
    expect(c.champ!.items[0]).toBe('long_sword');
  });

  it('objet unique : pas deux fois la même paire de bottes', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 2000;
    cmd(w, c, { t: 'buy', itemId: 'boots' });
    cmd(w, c, { t: 'buy', itemId: 'boots' });
    expect(c.champ!.items.filter((i) => i === 'boots')).toHaveLength(1);
    expect(c.champ!.gold).toBe(1700);
  });

  it('consommable : empile les charges dans un seul slot', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 1000;
    cmd(w, c, { t: 'buy', itemId: 'health_potion' });
    cmd(w, c, { t: 'buy', itemId: 'health_potion' });
    expect(c.champ!.gold).toBe(900);
    const slots = c.champ!.items.filter((i) => i === 'health_potion');
    expect(slots).toHaveLength(1);
    expect(econ(c).charges[c.champ!.items.indexOf('health_potion')]).toBe(2);
  });

  it('inventaire plein : refuse l’achat au-delà de 6 slots', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 99999;
    for (const id of ['long_sword', 'amplifying_tome', 'ruby_crystal', 'sapphire_crystal', 'dagger', 'cloth_armor']) {
      cmd(w, c, { t: 'buy', itemId: id });
    }
    expect(c.champ!.items.every((i) => i !== null)).toBe(true);
    cmd(w, c, { t: 'buy', itemId: 'negatron_cloak' });
    expect(c.champ!.items).not.toContain('negatron_cloak');
  });

  it('trinket : achat gratuit, un seul trinket', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    expect(econ(c).trinket).toBe('stealth_ward');
    expect(econ(c).trinketCharges).toBe(2);
    cmd(w, c, { t: 'buy', itemId: 'stealth_ward' });
    expect(c.champ!.items.filter((i) => i === 'stealth_ward')).toHaveLength(0); // pas dans les 6 slots
  });

  it('stats : un gain de PV max soigne du delta', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    c.dead = true; // achat autorisé en mort, hors fontaine (pas de soin de fontaine)
    c.champ!.respawnIn = 9999;
    c.champ!.gold = 1000;
    c.hp = 400;
    const hpMaxBefore = c.stats.maxHp;
    cmd(w, c, { t: 'buy', itemId: 'ruby_crystal' });
    expect(c.stats.maxHp).toBeCloseTo(hpMaxBefore + 150, 5);
    expect(c.hp).toBeCloseTo(550, 5); // 400 + 150 de delta de PV max
  });
});

describe('passifs et actifs d’objets', () => {
  it('potion : soin sur 15 s à l’utilisation', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 1000;
    cmd(w, c, { t: 'buy', itemId: 'health_potion' });
    c.pos = { x: 6000, y: 6000 }; // hors fontaine : pas de soin de fontaine parasite
    c.hp = 400;
    const slot = c.champ!.items.indexOf('health_potion');
    cmd(w, c, { t: 'useItem', slot });
    expect(econ(c).hot).not.toBeNull();
    run(w, 15);
    expect(c.hp).toBeCloseTo(400 + 120, 0); // soin complet de la potion
    expect(c.champ!.items[slot]).toBeNull(); // plus de charges -> slot vidé
  });

  it('hache du bourreau : blessure grave réduit les soins de 40 %', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 5000, y: 5000 });
    c.champ!.gold = 5000;
    c.dead = false;
    c.pos = AT_FOUNTAIN;
    cmd(w, c, { t: 'buy', itemId: 'hache_du_bourreau' });
    const e = dummy(w, { x: 5100, y: 5000 });
    applyDamage(w, { source: c, target: e, amount: 50, type: 'physical' });
    expect(econ(e).grievousUntil).toBeGreaterThan(w.time);
    e.hp = 100;
    const healed = applyHeal(w, { source: c, target: e, amount: 100 });
    expect(healed).toBeCloseTo(60, 5);
  });

  it('sablier : stase de 2,5 s, invulnérable', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN);
    c.champ!.gold = 5000;
    cmd(w, c, { t: 'buy', itemId: 'sablier_eternel' });
    const slot = c.champ!.items.indexOf('sablier_eternel');
    cmd(w, c, { t: 'useItem', slot });
    expect(econ(c).staseUntil).toBeGreaterThan(w.time);
    const hpBefore = c.hp;
    applyDamage(w, { source: dummy(w, { x: 0, y: 0 }), target: c, amount: 99999, type: 'true' });
    expect(c.dead).toBe(false);
    expect(c.hp).toBe(1);
    expect(hpBefore).toBeGreaterThan(1);
  });

  it('sceptre de l’orage : sort-lame armé par un sort, consommé par l’attaque', () => {
    const w = makeWorld();
    const c = addChamp(w, AT_FOUNTAIN, 'ilyra');
    c.champ!.gold = 5000;
    cmd(w, c, { t: 'buy', itemId: 'sceptre_de_lorage' });
    const target = dummy(w, { x: 600, y: 600 });
    expect(econ(c).spellbladeReady).toBe(false);
    w.events.emit({ type: 'abilityCast', source: c.id, championId: 'ilyra', slot: 'Q', abilityId: 'ilyra_q' });
    expect(econ(c).spellbladeReady).toBe(true);
    hit(w, c, target, false);
    expect(econ(c).spellbladeReady).toBe(false);
  });
});
