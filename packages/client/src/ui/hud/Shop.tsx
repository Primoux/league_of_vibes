import { CHAMPIONS, ITEM_LIST, ITEMS, type ChampionClass, type ItemCategory, type ItemDef, type Stats } from '@lov/shared';
import { Fragment } from 'preact';
import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { hideTip, itemTip, tip } from '../components.tsx';
import { selfEntity, useWorld } from '../game/world.ts';
import { Glyph, ItemIcon } from '../icons/Icons.tsx';
import { formatStat, STAT_LABEL } from '../logic/format.ts';
import { buildsInto, canBuy, CATEGORY_LABEL, CATEGORY_ORDER, filterItems, quoteItem, type ShopVerdict } from '../logic/shop.ts';
import { useSelect } from '../useStore.ts';

type Tab = ItemCategory | 'all' | 'reco';

const CLASS_STATS: Record<ChampionClass, (keyof Stats)[]> = {
  tank: ['armor', 'mr', 'maxHp', 'hpRegen'],
  fighter: ['ad', 'maxHp', 'abilityHaste', 'armor', 'lifesteal'],
  mage: ['ap', 'magicPen', 'maxMana', 'abilityHaste'],
  marksman: ['ad', 'critChance', 'attackSpeed', 'lifesteal'],
  assassin: ['ad', 'armorPen', 'abilityHaste'],
  support: ['abilityHaste', 'maxMana', 'manaRegen', 'maxHp', 'ap'],
};

/** Objets achetables (`shop: false` = donné par un objectif, ex. Œil du colosse). */
const SHOP_LIST = ITEM_LIST.filter((i) => i.shop !== false);

function recommended(cls: ChampionClass | undefined): ItemDef[] {
  if (!cls) return [];
  const wanted = CLASS_STATS[cls];
  const score = (i: ItemDef) => Object.keys(i.stats).filter((k) => wanted.includes(k as keyof Stats)).length;
  return SHOP_LIST.filter((i) => i.category !== 'trinket' && (score(i) > 0 || i.category === 'consumable' || i.category === 'boots'))
    .sort((a, b) => score(b) - score(a) || b.cost - a.cost)
    .slice(0, 20)
    .sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.cost - b.cost);
}

const VERDICT_TEXT: Record<ShopVerdict, string> = {
  ok: 'Acheter',
  gold: 'Or insuffisant',
  full: 'Inventaire plein',
  unique: 'Déjà possédé',
  closed: 'Retourne à la fontaine',
};

const TAB_GLYPH: Record<Tab, string> = {
  reco: 'crown',
  all: 'nexus',
  starter: 'sword',
  consumable: 'heart',
  boots: 'bolt',
  basic: 'inhibitor',
  epic: 'dragon',
  legendary: 'crown',
  trinket: 'gold',
};

/** Filtres par statistique (combinables : l'objet doit avoir toutes les stats cochées). */
const STAT_FILTERS: { id: string; label: string; stats: (keyof Stats)[] }[] = [
  { id: 'ad', label: 'Attaque', stats: ['ad'] },
  { id: 'as', label: 'Vit. d’attaque', stats: ['attackSpeed'] },
  { id: 'crit', label: 'Critique', stats: ['critChance'] },
  { id: 'leth', label: 'Létalité', stats: ['armorPen'] },
  { id: 'ls', label: 'Vol de vie', stats: ['lifesteal'] },
  { id: 'ap', label: 'Puissance', stats: ['ap'] },
  { id: 'mpen', label: 'Pén. magique', stats: ['magicPen'] },
  { id: 'mana', label: 'Mana', stats: ['maxMana', 'manaRegen'] },
  { id: 'haste', label: 'Accélération', stats: ['abilityHaste'] },
  { id: 'hp', label: 'PV', stats: ['maxHp', 'hpRegen'] },
  { id: 'armor', label: 'Armure', stats: ['armor'] },
  { id: 'mr', label: 'Rés. magique', stats: ['mr'] },
  { id: 'ms', label: 'Vitesse', stats: ['moveSpeed'] },
];

export function matchesStatFilters(item: ItemDef, active: readonly string[]): boolean {
  return active.every((id) => {
    const f = STAT_FILTERS.find((x) => x.id === id);
    return !f || f.stats.some((k) => (item.stats[k] ?? 0) !== 0);
  });
}

const SHORT_STAT: Partial<Record<keyof Stats, string>> = {
  ad: 'AD',
  ap: 'AP',
  maxHp: 'PV',
  maxMana: 'mana',
  armor: 'armure',
  mr: 'RM',
  attackSpeed: 'VA',
  critChance: 'crit',
  moveSpeed: 'vitesse',
  abilityHaste: 'accél.',
  lifesteal: 'vol de vie',
  armorPen: 'létalité',
  magicPen: 'pén. mag.',
  hpRegen: 'régén. PV',
  manaRegen: 'régén. mana',
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
  const [flash, setFlash] = useState(0);
  const [statFilter, setStatFilter] = useState<string[]>([]);
  const searchRef = useRef<HTMLInputElement>(null);
  const canShop = d.canShop || d.dead;

  useEffect(() => {
    searchRef.current?.focus();
    return hideTip;
  }, []);

  const reco = useMemo(() => recommended(d.cls), [d.cls]);
  const base = query ? filterItems(SHOP_LIST, query, 'all') : tab === 'reco' ? reco : filterItems(SHOP_LIST, '', tab);
  const list = base.filter((i) => matchesStatFilters(i, statFilter));
  const grouped = !!query || tab === 'all' || tab === 'reco';
  const toggleStat = (id: string) => setStatFilter((f) => (f.includes(id) ? f.filter((x) => x !== id) : [...f, id]));
  const selected = (sel ? ITEMS[sel] : undefined) ?? list[0];

  const buy = (id: string) => {
    const it = ITEMS[id];
    if (it && canBuy(it, d.items, d.gold, canShop, ITEMS) === 'ok') {
      net.send({ t: 'buy', itemId: id });
      setFlash((f) => f + 1);
    }
  };
  const sell = () => {
    if (sellSlot !== null && d.items[sellSlot]) {
      net.send({ t: 'sell', slot: sellSlot });
      setSellSlot(null);
    }
  };
  const close = () => store.set({ shopOpen: false });
  const renderCard = (it: ItemDef, i: number) => {
            const q = quoteItem(it.id, d.items, ITEMS);
            const v = canBuy(it, d.items, d.gold, canShop, ITEMS);
            const stats = Object.entries(it.stats).slice(0, 2) as [keyof Stats, number][];
            const fx = it.active ?? it.passive;
            return (
              <button
                key={it.id}
                class={`lov-item-card cat-${it.category} ${selected?.id === it.id ? 'sel' : ''} ${v === 'ok' ? 'ok' : 'no'} ${d.items.includes(it.id) ? 'owned' : ''}`}
                style={{ '--i': Math.min(i, 20) } as Record<string, number>}
                onClick={() => setSel(it.id)}
                onDblClick={() => buy(it.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  buy(it.id);
                }}
              >
                <span class="ic">
                  <ItemIcon itemId={it.id} size={46} />
                </span>
                <span class="body">
                  <b>{it.name}</b>
                  <span class="chips">
                    {stats.map(([k, val]) => (
                      <em key={k}>
                        {formatStat(k, val, true)} {SHORT_STAT[k] ?? STAT_LABEL[k]}
                      </em>
                    ))}
                    {stats.length === 0 && fx && <em>{fx.name}</em>}
                  </span>
                </span>
                <span class={`price ${q.price < it.cost ? 'disc' : ''}`}>
                  <span class="coin" />
                  {q.price}
                </span>
                {d.items.includes(it.id) && <span class="owned-tag">Possédé</span>}
              </button>
            );
          };

  const count = (t: Tab) => (t === 'reco' ? reco.length : t === 'all' ? SHOP_LIST.length : SHOP_LIST.filter((i) => i.category === t).length);

  return (
    <div class="lov-modal-backdrop lov-shop2-back" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div class="lov-shop2" onContextMenu={(e) => e.preventDefault()}>
        <header class="lov-shop2-head">
          <div class="title">
            <Glyph name="gold" size={20} />
            <h2>Boutique</h2>
          </div>
          <label class="lov-search2">
            <Glyph name="search" size={15} />
            <input
              ref={searchRef}
              placeholder="Rechercher (nom, stat, effet…)"
              value={query}
              onInput={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === 'Escape') (query ? setQuery('') : close());
                if (e.key === 'Enter' && list[0]) setSel(list[0].id);
              }}
            />
          </label>
          {!canShop && (
            <span class="closed">
              <Glyph name="lock" size={13} /> Hors de la fontaine
            </span>
          )}
          <span class="wallet" key={flash}>
            <span class="coin" />
            <b>{Math.floor(d.gold)}</b>
          </span>
          <button class="lov-icon-btn" onClick={close} title="Fermer (P / Échap)">
            <Glyph name="close" size={16} />
          </button>
        </header>

        <nav class="lov-shop2-nav">
          {(['reco', 'all', ...CATEGORY_ORDER] as Tab[]).filter((t) => count(t) > 0).map((t) => (
            <button
              key={t}
              class={!query && tab === t ? 'on' : ''}
              onClick={() => {
                setTab(t);
                setQuery('');
                setSel(null);
              }}
            >
              <Glyph name={TAB_GLYPH[t]} size={15} />
              <span>{t === 'reco' ? 'Conseillés' : t === 'all' ? 'Tous' : CATEGORY_LABEL[t]}</span>
              <small>{count(t)}</small>
            </button>
          ))}
        </nav>

        <section class="lov-shop2-grid">
          <div class="lov-stat-filters">
            {STAT_FILTERS.map((f) => (
              <button key={f.id} class={`lov-chip ${statFilter.includes(f.id) ? 'on' : ''}`} onClick={() => toggleStat(f.id)}>
                {f.label}
              </button>
            ))}
            {statFilter.length > 0 && (
              <button class="lov-chip clear" onClick={() => setStatFilter([])}>
                <Glyph name="close" size={10} /> Effacer
              </button>
            )}
          </div>
          {list.length === 0 && <p class="lov-muted">Aucun objet ne correspond.</p>}
          {grouped
            ? groupByCategory(list).map(([cat, items]) => (
                <Fragment key={cat}>
                  <h3 class="lov-shop2-group">
                    {CATEGORY_LABEL[cat]} <small>{items.length}</small>
                  </h3>
                  {items.map((it, i) => renderCard(it, i))}
                </Fragment>
              ))
            : list.map((it, i) => renderCard(it, i))}
        </section>

        <aside class="lov-shop2-detail">{selected && <Detail item={selected} inv={d.items} gold={d.gold} canShop={canShop} onBuy={buy} onPick={setSel} />}</aside>

        <footer class="lov-shop2-inv">
          <span class="lbl">Inventaire</span>
          <div class="slots">
            {Array.from({ length: 7 }, (_, i) => {
              const id = d.items[i] ?? null;
              return (
                <button
                  key={i}
                  class={`slot ${sellSlot === i ? 'sel' : ''} ${i === 6 ? 'trinket' : ''} ${id ? 'full' : ''}`}
                  disabled={!id}
                  onClick={() => setSellSlot(sellSlot === i ? null : i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (id && canShop && i !== 6) net.send({ t: 'sell', slot: i });
                  }}
                  {...(id && ITEMS[id] ? tip(itemTip(id)) : {})}
                >
                  <ItemIcon itemId={id} size={42} />
                </button>
              );
            })}
          </div>
          <div class="actions">
            <button class="lov-btn small" disabled={!canShop} onClick={() => net.send({ t: 'undoShop' })} title="Annule le dernier achat ou la dernière vente">
              <Glyph name="undo" size={14} /> Annuler
            </button>
            <button class="lov-btn small danger" disabled={!canShop || sellSlot === null || !d.items[sellSlot] || sellSlot === 6} onClick={sell}>
              Vendre{sellSlot !== null && d.items[sellSlot] ? ` · +${Math.floor((ITEMS[d.items[sellSlot]!]?.cost ?? 0) * 0.7)}` : ''}
            </button>
          </div>
          <span class="hint">Double-clic / clic droit : acheter · clic droit sur l’inventaire : vendre</span>
        </footer>
      </div>
    </div>
  );
}

function Detail({ item, inv, gold, canShop, onBuy, onPick }: { item: ItemDef; inv: (string | null)[]; gold: number; canShop: boolean; onBuy: (id: string) => void; onPick: (id: string) => void }) {
  const q = quoteItem(item.id, inv, ITEMS);
  const v = canBuy(item, inv, gold, canShop, ITEMS);
  const into = buildsInto(item.id, SHOP_LIST);
  const stats = Object.entries(item.stats) as [keyof Stats, number][];
  return (
    <div class={`lov-shop2-det cat-${item.category}`} key={item.id}>
      <div class="hero">
        <span class="halo" />
        <ItemIcon itemId={item.id} size={84} />
      </div>
      <h3>{item.name}</h3>
      <span class="cat">{CATEGORY_LABEL[item.category]}</span>
      {stats.length > 0 && (
        <ul class="stats">
          {stats.map(([k, val]) => (
            <li key={k}>
              <b>{formatStat(k, val, true)}</b> {STAT_LABEL[k]}
            </li>
          ))}
        </ul>
      )}
      {item.passive && (
        <p class="fx">
          <b class="pas">Passif · {item.passive.name}</b>
          {item.passive.description}
        </p>
      )}
      {item.active && (
        <p class="fx">
          <b class="act">
            Actif · {item.active.name}
            {item.active.cooldown > 0 && <em> ({item.active.cooldown} s)</em>}
          </b>
          {item.active.description}
        </p>
      )}
      {!item.passive && !item.active && stats.length === 0 && <p class="fx">{item.description}</p>}

      {item.buildsFrom.length > 0 && (
        <div class="tree">
          <small>Recette</small>
          <RecipeTree id={item.id} inv={inv} onPick={onPick} root />
        </div>
      )}
      {into.length > 0 && (
        <div class="into">
          <small>Utilisé dans</small>
          <div>
            {into.map((i) => (
              <button key={i.id} onClick={() => onPick(i.id)} {...tip(itemTip(i.id))}>
                <ItemIcon itemId={i.id} size={34} />
              </button>
            ))}
          </div>
        </div>
      )}

      <button class="lov-cta lov-buy" disabled={v !== 'ok'} onClick={() => onBuy(item.id)}>
        <span class="lov-cta-shine" />
        <span class="lov-cta-label">{v === 'ok' ? 'Acheter' : VERDICT_TEXT[v]}</span>
        <small>
          {q.price < item.cost ? (
            <>
              <s>{item.cost}</s> {q.price} or · composants déduits
            </>
          ) : (
            <>{q.price} or</>
          )}
        </small>
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
        <ItemIcon itemId={id} size={root ? 44 : 34} />
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

function groupByCategory(list: ItemDef[]): [ItemCategory, ItemDef[]][] {
  const m = new Map<ItemCategory, ItemDef[]>();
  for (const it of list) {
    const arr = m.get(it.category) ?? [];
    arr.push(it);
    m.set(it.category, arr);
  }
  return [...m.entries()].sort((a, b) => CATEGORY_ORDER.indexOf(a[0]) - CATEGORY_ORDER.indexOf(b[0]));
}
