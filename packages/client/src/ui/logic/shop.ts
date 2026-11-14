/** Logique boutique côté UI (pure, testée). La boutique autoritaire reste côté serveur (mistral-economy). */
import type { ItemCategory, ItemDef, Stats } from '@lov/shared';
import { STAT_LABEL } from './format.ts';

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
    .filter((i) => !q || searchText(i).includes(q))
    .sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.cost - b.cost || a.name.localeCompare(b.name));
}

const searchCache = new WeakMap<ItemDef, string>();
/** Texte indexé : nom, description, passif/actif et libellés des stats (« armure », « vol de vie »…). */
function searchText(i: ItemDef): string {
  let t = searchCache.get(i);
  if (t === undefined) {
    const stats = (Object.keys(i.stats) as (keyof Stats)[]).map((k) => STAT_LABEL[k] ?? k).join(' ');
    t = normalize(`${i.name} ${i.description} ${i.passive?.name ?? ''} ${i.passive?.description ?? ''} ${i.active?.name ?? ''} ${stats}`);
    searchCache.set(i, t);
  }
  return t;
}

export type ShopSort = 'category' | 'priceAsc' | 'priceDesc' | 'name';

/** Trie (copie) selon le mode ; le prix utilisé est celui après déduction des composants. */
export function sortItems(list: readonly ItemDef[], mode: ShopSort, price: (i: ItemDef) => number = (i) => i.cost): ItemDef[] {
  const out = [...list];
  if (mode === 'priceAsc') out.sort((a, b) => price(a) - price(b) || a.name.localeCompare(b.name));
  else if (mode === 'priceDesc') out.sort((a, b) => price(b) - price(a) || a.name.localeCompare(b.name));
  else if (mode === 'name') out.sort((a, b) => a.name.localeCompare(b.name, 'fr'));
  else out.sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.cost - b.cost || a.name.localeCompare(b.name));
  return out;
}

/** Objets qui réutilisent au moins un composant de l'inventaire (prix réduit), du moins cher au plus cher. */
export function upgradesFor(inventory: readonly (string | null)[], list: readonly ItemDef[], items: Record<string, ItemDef>): ItemDef[] {
  if (!inventory.some((x) => x)) return [];
  return list
    .filter((i) => i.buildsFrom.length > 0 && !(i.unique && inventory.includes(i.id)))
    .map((i) => ({ i, p: quoteItem(i.id, inventory, items).price }))
    .filter(({ i, p }) => p < i.cost)
    .sort((a, b) => a.p - b.p)
    .map(({ i }) => i);
}

/** Valeur estimée d'un inventaire (affichée au tableau des scores, comme l'or des objets). */
export function inventoryValue(inventory: readonly (string | null)[], items: Record<string, ItemDef>): number {
  return inventory.reduce((sum, id) => sum + (id ? (items[id]?.cost ?? 0) : 0), 0);
}
