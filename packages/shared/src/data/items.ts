import type { ItemDef } from '../schema.ts';

/**
 * Objets. Propriétaire : mistral-economy. BOOTSTRAP : quelques exemples ; la liste complète (~30 objets)
 * est décrite dans PLAN.md §8 et docs/plans/mistral-economy.md.
 */
export const ITEM_LIST: ItemDef[] = [
  { id: 'doran_blade', name: 'Lame du novice', description: '+8 AD, +80 PV, 2,5 % vol de vie.', category: 'starter', cost: 450, buildsFrom: [], stats: { ad: 8, maxHp: 80, lifesteal: 0.025 }, icon: 'blade_small' },
  { id: 'doran_ring', name: 'Anneau du novice', description: '+18 AP, +90 PV, régénération de mana.', category: 'starter', cost: 400, buildsFrom: [], stats: { ap: 18, maxHp: 90, manaRegen: 5 }, icon: 'ring_small' },
  { id: 'health_potion', name: 'Potion de soin', description: 'Rend 120 PV en 15 s.', category: 'consumable', cost: 50, buildsFrom: [], stats: {}, active: { id: 'potion', name: 'Boire', description: 'Rend 120 PV en 15 s.', cooldown: 0 }, maxStacks: 5, icon: 'potion_red' },
  { id: 'boots', name: 'Bottes', description: '+25 vitesse de déplacement.', category: 'boots', cost: 300, buildsFrom: [], stats: { moveSpeed: 25 }, icon: 'boots' },
  { id: 'long_sword', name: 'Épée longue', description: '+10 AD.', category: 'basic', cost: 350, buildsFrom: [], stats: { ad: 10 }, icon: 'sword' },
  { id: 'amplifying_tome', name: 'Tome amplificateur', description: '+20 AP.', category: 'basic', cost: 400, buildsFrom: [], stats: { ap: 20 }, icon: 'tome' },
  { id: 'ruby_crystal', name: 'Cristal de rubis', description: '+150 PV.', category: 'basic', cost: 400, buildsFrom: [], stats: { maxHp: 150 }, icon: 'ruby' },
  { id: 'stealth_ward', name: 'Totem de vision', description: 'Trinket : pose une balise invisible (90 s), 2 charges.', category: 'trinket', cost: 0, buildsFrom: [], stats: {}, icon: 'ward' },
];
export const ITEMS: Record<string, ItemDef> = Object.fromEntries(ITEM_LIST.map((i) => [i.id, i]));
