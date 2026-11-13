/** Logique boutique côté UI (pure, testée). La boutique autoritaire reste côté serveur (mistral-economy). */
import type { ItemCategory, ItemDef } from '@lov/shared';

export interface RecipeQuote {
  /** Prix à payer après déduction des composants possédés. */
  price: number;
  /** Index d'inventaire des composants qui seront consommés. */
  consumedSlots: number[];
}

/**
 * Prix d'un objet en tenant compte des composants possédés (récursif, comme LoL) :
 * un composant possédé est déduit à son coût total ; sinon on cherche ses propres composants.
 */
export function quoteItem(itemId: string, inventory: readonly (string | null)[], items: Record<string, ItemDef>): RecipeQuote {
  const item = items[itemId];
  if (!item) return { price: Infinity, consumedSlots: [] };
  const used = new Set<number>();
  let discount = 0;
  const walk = (id: string, depth: number) => {
    const def = items[id];
    if (!def || depth > 8) return;
    for (const comp of def.buildsFrom) {
      const slot = inventory.findIndex((it, i) => it === comp && !used.has(i));
      if (slot >= 0) {
        used.add(slot);
        discount += items[comp]?.cost ?? 0;
      } else walk(comp, depth + 1);
    }
  };
  walk(itemId, 0);
  return { price: Math.max(0, item.cost - discount), consumedSlots: [...used].sort((a, b) => a - b) };
}

export type ShopVerdict = 'ok' | 'gold' | 'full' | 'unique' | 'closed';

export function canBuy(
  item: ItemDef,
  inventory: readonly (string | null)[],
  gold: number,
  canShop: boolean,
  items: Record<string, ItemDef>,
  slots = 6,
): ShopVerdict {
  if (!canShop) return 'closed';
  const q = quoteItem(item.id, inventory, items);
  if (q.price > gold) return 'gold';
  if (item.unique && inventory.includes(item.id)) return 'unique';
  if (item.category === 'trinket') return 'ok';
  const main = inventory.slice(0, slots);
  const used = main.filter((x) => x !== null).length;
  const consumedInMain = q.consumedSlots.filter((s) => s < slots).length;
  const stacksOnExisting = !!item.maxStacks && main.includes(item.id);
  if (!stacksOnExisting && used - consumedInMain >= slots) return 'full';
  return 'ok';
}

/** Objets construits à partir de `itemId` (pour l'arbre « Utilisé dans »). */
export function buildsInto(itemId: string, list: readonly ItemDef[]): ItemDef[] {
  return list.filter((i) => i.buildsFrom.includes(itemId));
}

export const CATEGORY_ORDER: ItemCategory[] = ['starter', 'consumable', 'boots', 'basic', 'epic', 'legendary', 'trinket'];

export const CATEGORY_LABEL: Record<ItemCategory, string> = {
  starter: 'Départ',
  consumable: 'Consommables',
  boots: 'Bottes',
  basic: 'Basiques',
  epic: 'Épiques',
  legendary: 'Légendaires',
  trinket: 'Babioles',
};

/** Recherche tolérante aux accents et à la casse sur nom, description et stats. */
export function normalize(s: string): string {
  return s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

export function filterItems(list: readonly ItemDef[], query: string, category: ItemCategory | 'all'): ItemDef[] {
  const q = normalize(query.trim());
  return list
    .filter((i) => (category === 'all' ? true : i.category === category))
    .filter((i) => !q || normalize(`${i.name} ${i.description} ${i.passive?.name ?? ''}`).includes(q))
    .sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.cost - b.cost || a.name.localeCompare(b.name));
}

/** Valeur estimée d'un inventaire (affichée au tableau des scores, comme l'or des objets). */
export function inventoryValue(inventory: readonly (string | null)[], items: Record<string, ItemDef>): number {
  return inventory.reduce((sum, id) => sum + (id ? (items[id]?.cost ?? 0) : 0), 0);
}
