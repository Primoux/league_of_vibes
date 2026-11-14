import { ITEMS, type EntityId, type ItemDef } from '@lov/shared';
import { describe, expect, it } from 'vitest';
import { FeedTracker, trimFeed, type UnitRef } from '../src/ui/logic/feed.ts';
import { buffName, formatClock, formatCooldown, formatGoldShort, formatKdaRatio, formatRanks, formatStat, isDebuff } from '../src/ui/logic/format.ts';
import { canRankUp, hastedCooldown, sortScoreRows, type ScoreRow } from '../src/ui/logic/game.ts';
import { canBuy, filterItems, quoteItem } from '../src/ui/logic/shop.ts';

describe('format', () => {
  it('formate le chrono', () => {
    expect(formatClock(0)).toBe('0:00');
    expect(formatClock(125.9)).toBe('2:05');
    expect(formatClock(3725)).toBe('1:02:05');
  });
  it('formate les cooldowns', () => {
    expect(formatCooldown(0)).toBe('');
    expect(formatCooldown(0.42)).toBe('0.4');
    expect(formatCooldown(4.1)).toBe('5');
  });
  it('formate or, KDA, rangs et stats', () => {
    expect(formatGoldShort(950)).toBe('950');
    expect(formatGoldShort(12345)).toBe('12,3k');
    expect(formatKdaRatio(3, 0, 2)).toBe('Parfait');
    expect(formatKdaRatio(4, 2, 2)).toBe('3,00');
    expect(formatRanks([70, 70, 70])).toBe('70');
    expect(formatRanks([6, 5.5, 5])).toBe('6 / 5,5 / 5');
    expect(formatStat('critChance', 0.25, true)).toBe('+25 %');
    expect(formatStat('attackSpeed', 0.65)).toBe('0,65');
  });
  it('nomme les buffs (registre, drakes, âmes, repli)', () => {
    expect(buffName('stun')).toBe('Étourdissement');
    expect(isDebuff('stun')).toBe(true);
    expect(isDebuff('haste')).toBe(false);
    expect(buffName('dragon_ocean')).toBe('Drake océanique');
    expect(buffName('soul_infernal')).toBe('Âme infernale');
    expect(buffName('blue_buff')).toBe('Gardien d’azur');
    expect(buffName('ilyra_frost')).toBe('Ilyra frost');
  });
});

const fakeItems: Record<string, ItemDef> = {
  a: { id: 'a', name: 'A', description: '', category: 'basic', cost: 400, buildsFrom: [], stats: {}, icon: 'sword' },
  b: { id: 'b', name: 'B', description: '', category: 'basic', cost: 300, buildsFrom: [], stats: {}, icon: 'sword' },
  mid: { id: 'mid', name: 'Mid', description: '', category: 'epic', cost: 1100, buildsFrom: ['a', 'b'], stats: {}, icon: 'sword' },
  top: { id: 'top', name: 'Top', description: '', category: 'legendary', cost: 3000, buildsFrom: ['mid', 'a', 'a'], stats: {}, icon: 'sword', unique: true },
};

describe('boutique', () => {
  it('déduit les composants possédés, récursivement', () => {
    expect(quoteItem('mid', [], fakeItems).price).toBe(1100);
    expect(quoteItem('mid', ['a', null], fakeItems)).toEqual({ price: 700, consumedSlots: [0] });
    // top = mid + a + a ; on possède b, a, a -> mid non possédé -> on descend : a (slot 1) et b (slot 0), puis a (slot 2)... il ne reste qu'un seul "a" pour la 2e ligne
    const q = quoteItem('top', ['b', 'a', 'a'], fakeItems);
    expect(q.price).toBe(3000 - 300 - 400 - 400);
    expect(q.consumedSlots).toEqual([0, 1, 2]);
    expect(quoteItem('top', ['mid', 'a'], fakeItems).price).toBe(3000 - 1100 - 400);
  });
  it('refuse hors fontaine, sans or, inventaire plein, unique', () => {
    const top = fakeItems.top!;
    expect(canBuy(top, [], 5000, false, fakeItems)).toBe('closed');
    expect(canBuy(top, [], 100, true, fakeItems)).toBe('gold');
    expect(canBuy(fakeItems.b!, ['a', 'a', 'a', 'a', 'a', 'a'], 5000, true, fakeItems)).toBe('full');
    expect(canBuy(fakeItems.mid!, ['a', 'a', 'a', 'a', 'a', 'a'], 5000, true, fakeItems)).toBe('ok'); // consomme un "a"
    expect(canBuy(top, ['top'], 5000, true, fakeItems)).toBe('unique');
  });
  it('filtre sans accents et trie par catégorie puis prix', () => {
    const list = Object.values(ITEMS);
    expect(filterItems(list, 'epee', 'all').map((i) => i.id)).toContain('long_sword');
    const basics = filterItems(list, '', 'basic');
    expect(basics.every((i) => i.category === 'basic')).toBe(true);
    for (let i = 1; i < basics.length; i++) expect(basics[i]!.cost).toBeGreaterThanOrEqual(basics[i - 1]!.cost);
  });
});

describe('règles HUD', () => {
  it('limite les rangs selon le niveau', () => {
    expect(canRankUp('Q', 1, 0, 1)).toBe(true);
    expect(canRankUp('Q', 2, 1, 1)).toBe(false);
    expect(canRankUp('Q', 3, 1, 1)).toBe(true);
    expect(canRankUp('R', 5, 0, 1)).toBe(false);
    expect(canRankUp('R', 6, 0, 1)).toBe(true);
    expect(canRankUp('R', 10, 1, 1)).toBe(false);
    expect(canRankUp('R', 11, 1, 1)).toBe(true);
    expect(canRankUp('Q', 18, 5, 1)).toBe(false);
    expect(canRankUp('W', 9, 2, 0)).toBe(false);
  });
  it('applique l’accélération', () => {
    expect(hastedCooldown(10, 100)).toBe(5);
  });
  it('trie le tableau par rôle', () => {
    const row = (name: string, role?: ScoreRow['role']): ScoreRow => ({ playerId: name, name, team: 'blue', championId: 'ilyra', role, level: 1, kills: 0, deaths: 0, assists: 0, cs: 0, items: [], summoners: ['flash', 'ignite'], dead: false, isMe: false });
    expect(sortScoreRows([row('d', 'bot'), row('a', 'mid'), row('z'), row('c', 'top')]).map((r) => r.name)).toEqual(['c', 'a', 'd', 'z']);
  });
});

describe('kill feed et annonces', () => {
  const units: Record<number, UnitRef> = {
    1: { id: 1, kind: 'champion', team: 'blue', subtype: 'ilyra', name: 'Ilyra' },
    2: { id: 2, kind: 'champion', team: 'blue', subtype: 'sable', name: 'Sable' },
    10: { id: 10, kind: 'champion', team: 'red', subtype: 'nyx', name: 'Nyx' },
    11: { id: 11, kind: 'champion', team: 'red', subtype: 'grondak', name: 'Grondak' },
    12: { id: 12, kind: 'champion', team: 'red', subtype: 'corvin', name: 'Corvin' },
    13: { id: 13, kind: 'champion', team: 'red', subtype: 'zephyr', name: 'Zephyr' },
    50: { id: 50, kind: 'tower', team: 'red', subtype: 'outer' },
    60: { id: 60, kind: 'minion', team: 'red', subtype: 'melee' },
  };
  const lookup = (id: EntityId): UnitRef => units[id] ?? { id, kind: 'unknown', team: 'neutral' };
  const death = (target: number, killer?: number) => ({ type: 'death' as const, target, killer });

  it('premier sang puis double élimination', () => {
    const t = new FeedTracker('blue', lookup);
    const r1 = t.process([death(10, 1)], 100);
    expect(r1.feed).toHaveLength(1);
    expect(r1.announce[0]!.title).toBe('Premier sang !');
    const r2 = t.process([death(11, 1)], 105);
    expect(r2.announce[0]!.title).toBe('Double élimination !');
    expect(r2.announce[0]!.tone).toBe('ally');
    const r3 = t.process([death(12, 1)], 130); // hors fenêtre
    expect(r3.announce.map((a) => a.title)).toContain('Ilyra : Série meurtrière !');
  });

  it('arrêt de série, exécution et équipe anéantie', () => {
    const t = new FeedTracker('blue', lookup);
    t.process([death(1, 10)], 10);
    t.process([death(2, 10)], 40);
    t.process([death(1, 10)], 80); // Nyx : 3 kills
    const shut = t.process([death(10, 2)], 120);
    expect(shut.announce[0]!.title).toBe('Arrêt de série !');
    const exec = t.process([death(11, 60)], 130);
    expect(exec.feed[0]!.kind).toBe('execute');
    expect(exec.announce[0]!.title).toBe('Un ennemi a été exécuté');
    const ace = t.process([death(12, 1), death(13, 1)], 131);
    expect(ace.announce.map((a) => a.title)).toContain('Équipe anéantie !');
  });

  it('ignore les morts de sbires, annonce tours et objectifs', () => {
    const t = new FeedTracker('blue', lookup);
    expect(t.process([death(60, 1)], 10).feed).toHaveLength(0);
    const s = t.process([{ type: 'structureDestroyed', target: 50, kind: 'tower', team: 'red' }], 20);
    expect(s.announce[0]!.title).toBe('Tour ennemie détruite !');
    const o = t.process([{ type: 'objectiveTaken', monster: 'dragon', team: 'red', element: 'infernal' }], 30);
    expect(o.announce[0]!.title).toBe('L’équipe ennemie a vaincu : Drake infernal');
    expect(o.announce[0]!.tone).toBe('enemy');
    expect(t.process([{ type: 'structureDestroyed', target: 50, kind: 'tower', team: 'red', plate: true }], 40).feed).toHaveLength(0);
  });

  it('annonces de codex-objectives : âme, colosse, doublons ignorés', () => {
    const t = new FeedTracker('blue', lookup);
    const soul = t.process([{ type: 'announcement', key: 'dragon_soul', team: 'blue', params: { element: 'infernal' } }], 10);
    expect(soul.announce[0]!.title).toBe('Votre équipe obtient l’Âme infernale !');
    expect(soul.announce[0]!.element).toBe('infernal');
    expect(t.process([{ type: 'announcement', key: 'herald_summoned', team: 'red' }], 11).announce[0]!.tone).toBe('enemy');
    expect(t.process([{ type: 'announcement', key: 'objective_taken', team: 'red', params: { monster: 'dragon' } }], 12).announce).toHaveLength(0);
    expect(t.process([{ type: 'announcement', key: 'victory', team: 'red' }], 13).announce).toHaveLength(0);
  });

  it('élague le feed', () => {
    const e = [1, 2, 3, 20, 21].map((time) => ({ time }));
    expect(trimFeed(e, 22, 6, 10)).toEqual([{ time: 20 }, { time: 21 }]);
  });
});
