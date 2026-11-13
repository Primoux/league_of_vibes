import { CHAMPIONS, ITEM_LIST, ITEMS, type ChampionClass, type ItemCategory, type ItemDef, type Stats } from '@lov/shared';
import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { hideTip, ItemCard, itemTip, tip } from '../components.tsx';
import { selfEntity, useWorld } from '../game/world.ts';
import { Glyph, ItemIcon } from '../icons/Icons.tsx';
import { useSelect } from '../useStore.ts';
import { buildsInto, canBuy, CATEGORY_LABEL, CATEGORY_ORDER, filterItems, quoteItem, type ShopVerdict } from '../logic/shop.ts';

type Tab = ItemCategory | 'all' | 'reco';

const CLASS_STATS: Record<ChampionClass, (keyof Stats)[]> = {
  tank: ['armor', 'mr', 'maxHp', 'hpRegen'],
  fighter: ['ad', 'maxHp', 'abilityHaste', 'armor', 'lifesteal'],
  mage: ['ap', 'magicPen', 'maxMana', 'abilityHaste'],
  marksman: ['ad', 'critChance', 'attackSpeed', 'lifesteal'],
  assassin: ['ad', 'armorPen', 'abilityHaste'],
  support: ['abilityHaste', 'maxMana', 'manaRegen', 'maxHp', 'ap'],
};

function recommended(cls: ChampionClass | undefined): ItemDef[] {
  if (!cls) return [];
  const wanted = CLASS_STATS[cls];
  const score = (i: ItemDef) => Object.keys(i.stats).filter((k) => wanted.includes(k as keyof Stats)).length;
  return SHOP_LIST.filter((i) => i.category !== 'trinket' && (score(i) > 0 || i.category === 'consumable' || i.category === 'boots'))
    .sort((a, b) => score(b) - score(a) || b.cost - a.cost)
    .slice(0, 14)
    .sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.cost - b.cost);
}

/** Objets achetables (`shop: false` = donné par un objectif, ex. Œil du colosse). */
const SHOP_LIST = ITEM_LIST.filter((i) => i.shop !== false);

const VERDICT_TEXT: Record<ShopVerdict, string> = {
  ok: 'Acheter',
  gold: 'Or insuffisant',
  full: 'Inventaire plein',
  unique: 'Déjà possédé',
  closed: 'Hors de la fontaine',
};

export function Shop() {
  const open = useSelect((s) => s.shopOpen);
  if (!open) return null;
  return <ShopPanel />;
}

function ShopPanel() {
  const d = useWorld(() => {
    const self = store.state.snapshot?.self;
    const e = selfEntity();
    return { gold: self?.gold ?? 0, canShop: self?.canShop ?? false, items: e?.champ?.items ?? [], cls: e?.subtype ? CHAMPIONS[e.subtype]?.class : undefined, dead: !!e?.dead };
  });
  const [tab, setTab] = useState<Tab>('reco');
  const [query, setQuery] = useState('');
  const [sel, setSel] = useState<string | null>(null);
  const [sellSlot, setSellSlot] = useState<number | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const canShop = d.canShop || d.dead;

  useEffect(() => {
    searchRef.current?.focus();
    return hideTip;
  }, []);

  const reco = useMemo(() => recommended(d.cls), [d.cls]);
  const list = query ? filterItems(SHOP_LIST, query, 'all') : tab === 'reco' ? reco : filterItems(SHOP_LIST, '', tab);
  const selected = sel ? ITEMS[sel] : undefined;

  const buy = (id: string) => {
    const it = ITEMS[id];
    if (it && canBuy(it, d.items, d.gold, canShop, ITEMS) === 'ok') net.send({ t: 'buy', itemId: id });
  };
  const sell = () => {
    if (sellSlot !== null && d.items[sellSlot]) {
      net.send({ t: 'sell', slot: sellSlot });
      setSellSlot(null);
    }
  };
  const close = () => store.set({ shopOpen: false });

  return (
    <div class="lov-modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div class="lov-shop lov-frame" onContextMenu={(e) => e.preventDefault()}>
        <header class="lov-shop-head">
          <h2>Boutique</h2>
          <label class="lov-search">
            <Glyph name="search" size={14} />
            <input
              ref={searchRef}
              placeholder="Rechercher un objet…"
              value={query}
              onInput={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === 'Escape') (query ? setQuery('') : close());
                if (e.key === 'Enter' && list[0]) setSel(list[0].id);
              }}
            />
          </label>
          {!canShop && <span class="lov-shop-closed">Retourne à la fontaine pour acheter</span>}
          <span class="lov-shop-gold">
            <Glyph name="gold" size={16} /> {Math.floor(d.gold)}
          </span>
          <button class="lov-icon-btn" onClick={close} title="Fermer (P / Échap)">
            <Glyph name="close" size={16} />
          </button>
        </header>

        <nav class="lov-shop-tabs">
          {(['reco', 'all', ...CATEGORY_ORDER] as Tab[]).map((t) => (
            <button
              key={t}
              class={!query && tab === t ? 'on' : ''}
              onClick={() => {
                setTab(t);
                setQuery('');
              }}
            >
              {t === 'reco' ? 'Conseillés' : t === 'all' ? 'Tous' : CATEGORY_LABEL[t]}
            </button>
          ))}
        </nav>

        <section class="lov-shop-grid">
          {list.length === 0 && <p class="lov-muted">Aucun objet trouvé.</p>}
          {groupByCategory(list).map(([cat, items]) => (
            <div key={cat} class="lov-shop-group">
              <h3>{CATEGORY_LABEL[cat]}</h3>
              <div class="items">
                {items.map((it) => {
                  const q = quoteItem(it.id, d.items, ITEMS);
                  const v = canBuy(it, d.items, d.gold, canShop, ITEMS);
                  return (
                    <button
                      key={it.id}
                      class={`lov-shop-item ${sel === it.id ? 'sel' : ''} ${v === 'ok' ? 'ok' : 'no'} ${d.items.includes(it.id) ? 'owned' : ''}`}
                      onClick={() => setSel(it.id)}
                      onDblClick={() => buy(it.id)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        buy(it.id);
                      }}
                      {...tip(itemTip(it.id, q.price))}
                    >
                      <ItemIcon itemId={it.id} size={44} />
                      <span class={`price ${q.price < it.cost ? 'disc' : ''}`}>{q.price}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </section>

        <aside class="lov-shop-detail">
          {selected ? <Detail item={selected} inv={d.items} gold={d.gold} canShop={canShop} onBuy={buy} onPick={setSel} /> : <p class="lov-muted">Sélectionne un objet pour voir sa recette. Double-clic ou clic droit pour acheter.</p>}
        </aside>

        <footer class="lov-shop-inv">
          <div class="slots">
            {Array.from({ length: 7 }, (_, i) => {
              const id = d.items[i] ?? null;
              return (
                <button
                  key={i}
                  class={`slot ${sellSlot === i ? 'sel' : ''} ${i === 6 ? 'trinket' : ''}`}
                  disabled={!id}
                  onClick={() => setSellSlot(sellSlot === i ? null : i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (id && canShop && i !== 6) net.send({ t: 'sell', slot: i });
                  }}
                  {...(id && ITEMS[id] ? tip(itemTip(id)) : {})}
                >
                  <ItemIcon itemId={id} size={40} />
                </button>
              );
            })}
          </div>
          <div class="actions">
            <button class="lov-btn small" disabled={!canShop} onClick={() => net.send({ t: 'undoShop' })} title="Annule le dernier achat ou la dernière vente">
              <Glyph name="undo" size={14} /> Annuler
            </button>
            <button class="lov-btn small danger" disabled={!canShop || sellSlot === null || !d.items[sellSlot] || sellSlot === 6} onClick={sell}>
              Vendre{sellSlot !== null && d.items[sellSlot] ? ` (+${Math.floor((ITEMS[d.items[sellSlot]!]?.cost ?? 0) * 0.7)})` : ''}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

function groupByCategory(list: ItemDef[]): [ItemCategory, ItemDef[]][] {
  const m = new Map<ItemCategory, ItemDef[]>();
  for (const it of list) {
    const arr = m.get(it.category) ?? [];
    arr.push(it);
    m.set(it.category, arr);
  }
  return [...m.entries()].sort((a, b) => CATEGORY_ORDER.indexOf(a[0]) - CATEGORY_ORDER.indexOf(b[0]));
}

function Detail({ item, inv, gold, canShop, onBuy, onPick }: { item: ItemDef; inv: (string | null)[]; gold: number; canShop: boolean; onBuy: (id: string) => void; onPick: (id: string) => void }) {
  const q = quoteItem(item.id, inv, ITEMS);
  const v = canBuy(item, inv, gold, canShop, ITEMS);
  const into = buildsInto(item.id, SHOP_LIST);
  return (
    <div class="lov-shop-det">
      {into.length > 0 && (
        <div class="into">
          <small>Utilisé dans</small>
          <div>
            {into.map((i) => (
              <button key={i.id} onClick={() => onPick(i.id)} {...tip(itemTip(i.id))}>
                <ItemIcon itemId={i.id} size={30} />
              </button>
            ))}
          </div>
        </div>
      )}
      <RecipeTree id={item.id} inv={inv} onPick={onPick} root />
      <ItemCard item={item} price={q.price} />
      <button class={`lov-btn primary big ${v === 'ok' ? 'glow' : ''}`} disabled={v !== 'ok'} onClick={() => onBuy(item.id)}>
        {v === 'ok' ? (
          <>
            Acheter · <Glyph name="gold" size={14} /> {q.price}
          </>
        ) : (
          VERDICT_TEXT[v]
        )}
      </button>
    </div>
  );
}

function RecipeTree({ id, inv, onPick, root }: { id: string; inv: (string | null)[]; onPick: (id: string) => void; root?: boolean }) {
  const it = ITEMS[id];
  if (!it) return null;
  const owned = !root && inv.includes(id);
  return (
    <div class={`lov-recipe ${root ? 'root' : ''}`}>
      <button class={`node ${owned ? 'owned' : ''}`} onClick={() => onPick(id)} {...tip(itemTip(id))}>
        <ItemIcon itemId={id} size={root ? 46 : 34} />
        <span>{it.cost}</span>
        {owned && (
          <i class="ok">
            <Glyph name="check" size={10} />
          </i>
        )}
      </button>
      {it.buildsFrom.length > 0 && (
        <div class="children">
          {it.buildsFrom.map((c, i) => (
            <RecipeTree key={`${c}-${i}`} id={c} inv={inv} onPick={onPick} />
          ))}
        </div>
      )}
    </div>
  );
}
