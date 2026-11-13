import { MAX_LEVEL, ULT_LEVELS, dist2, getChampion, type AbilitySlot } from '@lov/shared';
import { recalculateStats } from '../combat/buffs.ts';
import { applyHeal } from '../combat/damage.ts';
import type { Entity, System, World } from '../world.ts';

/** XP within the current level; 280 to reach level 2, then +100 per level. */
export const XP_TO_NEXT: readonly number[] = Array.from({ length: 17 }, (_, i) => 280 + i * 100);
export const xpToNextLevel = (level: number): number => level >= MAX_LEVEL ? 0 : XP_TO_NEXT[Math.max(0, level - 1)]!;
export const xpForLevel = (level: number): number => XP_TO_NEXT.slice(0, Math.max(0, level - 1)).reduce((a, b) => a + b, 0);
export const cooldownWithHaste = (seconds: number, haste: number): number => seconds * 100 / (100 + Math.max(0, haste));
export function maxAbilityRank(e: Entity, slot: AbilitySlot): number {
  if (!e.champ) return 0;
  if (slot === 'R') return ULT_LEVELS.filter((n) => e.champ!.level >= n).length;
  return Math.min(5, Math.ceil(e.champ.level / 2));
}
export function levelAbility(e: Entity, slot: AbilitySlot): boolean {
  if (!e.champ || !['Q', 'W', 'E', 'R'].includes(slot) || e.champ.skillPoints <= 0 || e.champ.abilityLevels[slot] >= maxAbilityRank(e, slot)) return false;
  e.champ.abilityLevels[slot]++;
  e.champ.skillPoints--;
  return true;
}
export function grantXP(world: World, e: Entity, amount: number): void {
  const c = e.champ;
  if (!c || c.level >= MAX_LEVEL || !Number.isFinite(amount) || amount <= 0) return;
  c.xp += amount;
  while (c.level < MAX_LEVEL && c.xp + 1e-8 >= xpToNextLevel(c.level)) {
    c.xp -= xpToNextLevel(c.level);
    const oldHp = e.stats.maxHp, oldMana = e.stats.maxMana;
    c.level++;
    c.skillPoints++;
    recalculateStats(world, e);
    if (!e.dead) e.hp = Math.min(e.stats.maxHp, e.hp + e.stats.maxHp - oldHp);
    e.mana = Math.min(e.stats.maxMana, e.mana + e.stats.maxMana - oldMana);
    world.events.emit({ type: 'levelUp', target: e.id, level: c.level, fxKey: 'level_up' });
  }
  if (c.level >= MAX_LEVEL) c.xp = 0;
}
const initialized = new WeakSet<World>();
function xpReward(e: Entity): number {
  if (typeof e.data.xpReward === 'number') return e.data.xpReward;
  const domain = (e.data.minion ?? e.data.jungle ?? e.data.monster) as { xp?: number } | undefined;
  if (typeof domain?.xp === 'number') return domain.xp;
  if (e.champ) return 50 + 50 * e.champ.level;
  if (e.kind === 'minion') return { melee: 60.45, caster: 29.76, cannon: 93, super: 97 }[e.subtype ?? 'melee'] ?? 60;
  if (e.kind === 'monster') return 100;
  return 0;
}
export function initLeveling(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  world.events.on('death', ({ target, killer }) => {
    const victim = world.get(target), source = world.get(killer);
    if (!victim || victim.data.xpRewarded || victim.kind === 'projectile' || victim.kind === 'zone') return;
    victim.data.xpRewarded = true;
    const team = victim.team === 'neutral' ? source?.team : victim.team === 'blue' ? 'red' : 'blue';
    if (!team || team === 'neutral') return;
    const recipients: Entity[] = [];
    for (const ally of world.query('champion')) {
      if (!ally.dead && ally.team === team && ally.champ && (victim.data.xpGlobal === true || dist2(ally.pos, victim.pos) <= 1400 ** 2)) recipients.push(ally);
    }
    const reward = xpReward(victim) / Math.max(1, recipients.length);
    for (const ally of recipients) {
      const hasSmite = Object.values(ally.champ!.summoners).includes('smite');
      grantXP(world, ally, reward * (victim.kind === 'monster' && !hasSmite ? 0.5 : 1));
    }
  });
}
export const levelingSystem: System = {
  name: 'leveling', init: initLeveling,
  update(world, dt) {
    initLeveling(world);
    for (const { playerId, msg } of world.commands) if (msg.t === 'levelAbility') {
      const e = world.championOf(playerId);
      if (e) levelAbility(e, msg.slot);
    }
    for (const e of world.query('champion')) {
      if (e.dead || !e.champ) continue;
      e.data.xpRewarded = false;
      if (e.hp < e.stats.maxHp) applyHeal(world, { source: e, target: e, amount: e.stats.hpRegen / 5 * dt });
      if (getChampion(e.champ.championId).resource !== 'none') e.mana = Math.min(e.stats.maxMana, e.mana + e.stats.manaRegen / 5 * dt);
    }
  },
};
