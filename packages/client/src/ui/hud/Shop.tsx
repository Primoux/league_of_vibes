import { CHAMPIONS, ITEM_LIST, ITEMS, type ChampionClass, type ItemCategory, type ItemDef, type Stats } from '@lov/shared';
import { Fragment } from 'preact';
import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { hideTip, itemTip, tip } from '../components.tsx';
import { selfEntity, useWorld } from '../game/world.ts';
import { Glyph, ItemIcon } from '../icons/Icons.tsx';
import { formatStat, STAT_LABEL } from '../logic/format.ts';
import { buildsInto, canBuy, CATEGORY_LABEL, CATEGORY_ORDER, filterItems, quoteItem, sortItems, upgradesFor, type ShopSort, type ShopVerdict } from '../logic/shop.ts';
import { useSelect } from '../useStore.ts';

type Tab = ItemCategory | 'all' | 'reco' | 'up';

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

const TAB_LABEL: Partial<Record<Tab, string>> = { reco: 'Conseillés', up: 'Améliorations', all: 'Tous' };

const SORT_LABEL: Record<ShopSort, string> = {
  category: 'Par catégorie',
  priceAsc: 'Prix croissant',
  priceDesc: 'Prix décroissant',
  name: 'Nom (A → Z)',
};

/** Préférences conservées entre deux ouvertures de la boutique (durée de la partie). */
const prefs: { tab: Tab; sort: ShopSort; stats: string[]; affordable: boolean } = { tab: 'reco', sort: 'category', stats: [], affordable: false };

const TAB_GLYPH: Record<Tab, string> = {
  reco: 'crown',
  up: 'plus',
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

type Group = { key: string; label: string; items: ItemDef[] };

function ShopPanel() {
  const d = useWorld(() => {
    const self = store.state.snapshot?.self;
    const e = selfEntity();
    return { gold: self?.gold ?? 0, canShop: self?.canShop ?? false, items: e?.champ?.items ?? [], cls: e?.subtype ? CHAMPIONS[e.subtype]?.class : undefined, dead: !!e?.dead };
  });
  const [tab, setTabState] = useState<Tab>(prefs.tab);
  const [query, setQuery] = useState('');
  const [sel, setSel] = useState<string | null>(null);
  const [hover, setHover] = useState<string | null>(null);
  const [sellSlot, setSellSlot] = useState<number | null>(null);
  const [flash, setFlash] = useState(0);
  const [bought, setBought] = useState<{ id: string; n: number } | null>(null);
  const [statFilter, setStatFilterState] = useState<string[]>(prefs.stats);
  const [sort, setSortState] = useState<ShopSort>(prefs.sort);
  const [affordable, setAffordableState] = useState(prefs.affordable);
  const searchRef = useRef<HTMLInputElement>(null);
  const gridRef = useRef<HTMLElement>(null);
  const canShop = d.canShop || d.dead;

  const setTab = (t: Tab) => setTabState((prefs.tab = t));
  const setStatFilter = (f: string[]) => setStatFilterState((prefs.stats = f));
  const setSort = (m: ShopSort) => setSortState((prefs.sort = m));
  const setAffordable = (v: boolean) => setAffordableState((prefs.affordable = v));

  useEffect(() => {
    searchRef.current?.focus();
    return hideTip;
  }, []);

  const invKey = d.items.join(',');
  const reco = useMemo(() => recommended(d.cls), [d.cls]);
  const upgrades = useMemo(() => upgradesFor(d.items, SHOP_LIST, ITEMS), [invKey]);
  const price = (it: ItemDef) => quoteItem(it.id, d.items, ITEMS).price;
  const activeTab: Tab = tab === 'up' && upgrades.length === 0 ? 'reco' : tab;

  const groups = useMemo<Group[]>(() => {
    const base = query ? filterItems(SHOP_LIST, query, 'all') : activeTab === 'reco' ? reco : activeTab === 'up' ? upgrades : filterItems(SHOP_LIST, '', activeTab);
    const filtered = base.filter((i) => matchesStatFilters(i, statFilter) && (!affordable || price(i) <= d.gold));
    const byCategory = sort === 'category' && activeTab !== 'up';
    const list = sort === 'category' ? filtered : sortItems(filtered, sort, price);
    const out: Group[] = [];
    // Dans « Conseillés », les améliorations de l'inventaire passent en premier.
    if (!query && activeTab === 'reco') {
      const ups = upgrades.filter((i) => matchesStatFilters(i, statFilter) && (!affordable || price(i) <= d.gold)).slice(0, 6);
      if (sort !== 'category') ups.splice(0, ups.length, ...sortItems(ups, sort, price));
      if (ups.length) out.push({ key: 'up', label: 'Pour ton inventaire', items: ups });
    }
    if (byCategory && (query || activeTab === 'all' || activeTab === 'reco')) {
      for (const [cat, items] of groupByCategory(list)) out.push({ key: cat, label: CATEGORY_LABEL[cat], items });
    } else if (list.length) out.push({ key: 'list', label: '', items: list });
    return out;
  }, [query, activeTab, reco, upgrades, statFilter, affordable, sort, invKey, Math.floor(d.gold)]);

  /** Ordre de navigation au clavier (doublons possibles entre groupes : on garde la première occurrence). */
  const flat = useMemo(() => [...new Set(groups.flatMap((g) => g.items.map((i) => i.id)))], [groups]);
  const pinned = (sel ? ITEMS[sel] : undefined) ?? (flat[0] ? ITEMS[flat[0]] : undefined);
  const shown = (hover ? ITEMS[hover] : undefined) ?? pinned;
  const filtersActive = statFilter.length > 0 || affordable || !!query;

  const buy = (id: string) => {
    const it = ITEMS[id];
    if (it && canBuy(it, d.items, d.gold, canShop, ITEMS) === 'ok') {
      net.send({ t: 'buy', itemId: id });
      setFlash((f) => f + 1);
      setBought((b) => ({ id, n: (b?.n ?? 0) + 1 }));
    }
  };
  const sell = () => {
    if (sellSlot !== null && d.items[sellSlot]) {
      net.send({ t: 'sell', slot: sellSlot });
      setSellSlot(null);
    }
  };
  const close = () => store.set({ shopOpen: false });
  const pick = (id: string) => {
    setSel(id);
    setHover(null);
  };
  const toggleStat = (id: string) => setStatFilter(statFilter.includes(id) ? statFilter.filter((x) => x !== id) : [...statFilter, id]);
  const resetFilters = () => {
    setStatFilter([]);
    setAffordable(false);
    setQuery('');
  };

  /** Flèches : déplace la sélection dans la grille ; Entrée : achète l'objet sélectionné. */
  const navigate = (e: KeyboardEvent): boolean => {
    if (e.key === 'Enter') {
      if (pinned) buy(pinned.id);
      return true;
    }
    const dirs: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns(gridRef.current), ArrowDown: columns(gridRef.current) };
    const step = dirs[e.key];
    if (step === undefined || flat.length === 0) return false;
    const cur = pinned ? flat.indexOf(pinned.id) : -1;
    const next = flat[Math.max(0, Math.min(flat.length - 1, cur < 0 ? 0 : cur + step))]!;
    pick(next);
    gridRef.current?.querySelector(`[data-item="${next}"]`)?.scrollIntoView({ block: 'nearest' });
    return true;
  };

  const renderCard = (it: ItemDef, i: number) => {
    const q = quoteItem(it.id, d.items, ITEMS);
    const v = canBuy(it, d.items, d.gold, canShop, ITEMS);
    const owned = d.items.filter((x) => x === it.id).length;
    const stats = Object.entries(it.stats).slice(0, 3) as [keyof Stats, number][];
    const fx = it.active ?? it.passive;
    const missing = Math.ceil(q.price - d.gold);
    return (
      <div
        key={it.id}
        data-item={it.id}
        role="button"
        tabIndex={-1}
        class={`lov-item-card cat-${it.category} ${pinned?.id === it.id ? 'sel' : ''} ${v === 'ok' ? 'ok' : 'no'} ${missing > 0 ? 'poor' : ''} ${owned ? 'owned' : ''} ${bought?.id === it.id ? 'bought' : ''}`}
        style={{ '--i': Math.min(i, 20) } as Record<string, number>}
        onMouseEnter={() => setHover(it.id)}
        onClick={() => pick(it.id)}
        onDblClick={() => buy(it.id)}
        onContextMenu={(e) => {
          e.preventDefault();
          buy(it.id);
        }}
      >
        {bought?.id === it.id && <span class="buy-burst" key={bought.n} />}
        <span class="ic">
          <ItemIcon itemId={it.id} size={46} />
          {owned > 0 && <span class="owned-tag">{owned > 1 ? `×${owned}` : <Glyph name="check" size={10} />}</span>}
        </span>
        <span class="body">
          <b>{it.name}</b>
          <span class="chips">
            {stats.map(([k, val]) => (
              <em key={k}>
                {formatStat(k, val, true)} {SHORT_STAT[k] ?? STAT_LABEL[k]}
              </em>
            ))}
            {stats.length === 0 && fx && <em class="fx">{fx.name}</em>}
          </span>
        </span>
        <span class="foot">
          <span class={`price ${q.price < it.cost ? 'disc' : ''}`}>
            <span class="coin" />
            {q.price}
            {q.price < it.cost && <s>{it.cost}</s>}
          </span>
          {missing > 0 && <span class="missing" title={`Il te manque ${missing} or`}>manque {missing}</span>}
        </span>
        {v === 'ok' && (
          <button
            class="quick"
            title="Acheter"
            onClick={(e) => {
              e.stopPropagation();
              buy(it.id);
            }}
            onDblClick={(e) => e.stopPropagation()}
          >
            <Glyph name="plus" size={12} />
          </button>
        )}
      </div>
    );
  };

  const count = (t: Tab) =>
    t === 'reco' ? reco.length : t === 'up' ? upgrades.length : t === 'all' ? SHOP_LIST.length : SHOP_LIST.filter((i) => i.category === t).length;

  let cardIndex = 0;
  const sellItem = sellSlot !== null ? d.items[sellSlot] : null;

  return (
    <div class="lov-modal-backdrop lov-shop2-back" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div
        class="lov-shop2"
        onContextMenu={(e) => e.preventDefault()}
        onKeyDown={(e) => {
          if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
          if (navigate(e)) {
            e.preventDefault();
            e.stopPropagation();
          }
        }}
      >
        <header class="lov-shop2-head">
          <div class="title">
            <Glyph name="gold" size={20} />
            <h2>Boutique</h2>
          </div>
          <label class="lov-search2">
            <Glyph name="search" size={15} />
            <input
              ref={searchRef}
              placeholder="Rechercher un objet, une stat, un effet…"
              value={query}
              onInput={(e) => {
                setQuery(e.currentTarget.value);
                setSel(null);
              }}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === 'Escape') {
                  if (query) setQuery('');
                  else close();
                  return;
                }
                if (navigate(e)) e.preventDefault();
              }}
            />
            {query && (
              <button class="clear" title="Effacer la recherche" onClick={() => (setQuery(''), searchRef.current?.focus())}>
                <Glyph name="close" size={11} />
              </button>
            )}
          </label>
          {!canShop && (
            <span class="closed" title="Les achats ne sont possibles qu’à la fontaine (ou mort)">
              <Glyph name="lock" size={13} /> Hors de la fontaine — consultation seule
            </span>
          )}
          <span class="wallet" key={flash} title="Or disponible">
            <span class="coin" />
            <b>{Math.floor(d.gold)}</b>
          </span>
          <button class="lov-icon-btn" onClick={close} title="Fermer (P / Échap)">
            <Glyph name="close" size={16} />
          </button>
        </header>

        <nav class="lov-shop2-nav">
          {(['reco', 'up', 'all', ...CATEGORY_ORDER] as Tab[])
            .filter((t) => count(t) > 0)
            .map((t, i) => (
              <Fragment key={t}>
                {i > 0 && (t === 'all' || t === CATEGORY_ORDER[0]) && <hr />}
                <button
                  class={`${!query && activeTab === t ? 'on' : ''} ${t === 'up' ? 'up' : ''}`}
                  onClick={() => {
                    setTab(t);
                    setQuery('');
                    setSel(null);
                    gridRef.current?.scrollTo({ top: 0 });
                  }}
                >
                  <Glyph name={TAB_GLYPH[t]} size={15} />
                  <span>{TAB_LABEL[t] ?? CATEGORY_LABEL[t as ItemCategory]}</span>
                  <small>{count(t)}</small>
                </button>
              </Fragment>
            ))}
          <div class="legend">
            <p>
              <kbd>Clic</kbd> détails
            </p>
            <p>
              <kbd>Double-clic</kbd> / <kbd>Clic droit</kbd> acheter
            </p>
            <p>
              <kbd>←↑↓→</kbd> naviguer · <kbd>Entrée</kbd> acheter
            </p>
          </div>
        </nav>

        <section class="lov-shop2-grid" ref={gridRef} onMouseLeave={() => setHover(null)}>
          <div class="lov-shop2-tools">
            <div class="lov-stat-filters">
              {STAT_FILTERS.map((f) => (
                <button key={f.id} class={`lov-chip ${statFilter.includes(f.id) ? 'on' : ''}`} onClick={() => toggleStat(f.id)}>
                  {f.label}
                </button>
              ))}
            </div>
            <div class="row">
              <button class={`lov-chip toggle ${affordable ? 'on' : ''}`} onClick={() => setAffordable(!affordable)} title="Masquer les objets trop chers pour ton or actuel">
                <span class="coin" /> Abordables
              </button>
              <select value={sort} onChange={(e) => setSort(e.currentTarget.value as ShopSort)} title="Trier">
                {(Object.keys(SORT_LABEL) as ShopSort[]).map((m) => (
                  <option key={m} value={m}>
                    {SORT_LABEL[m]}
                  </option>
                ))}
              </select>
              {query && (
                <span class="ctx">
                  Résultats pour « {query} » · {flat.length}
                </span>
              )}
              {filtersActive && (
                <button class="lov-chip clear" onClick={resetFilters}>
                  <Glyph name="close" size={10} /> Réinitialiser
                </button>
              )}
            </div>
          </div>
          {flat.length === 0 && (
            <div class="lov-shop2-empty">
              <Glyph name="search" size={28} />
              <p>Aucun objet ne correspond{affordable ? ' à ton or actuel' : ''}.</p>
              {filtersActive && (
                <button class="lov-btn small" onClick={resetFilters}>
                  Réinitialiser les filtres
                </button>
              )}
            </div>
          )}
          {groups.map((g) => (
            <Fragment key={g.key}>
              {g.label && (
                <h3 class={`lov-shop2-group ${g.key === 'up' ? 'up' : ''}`}>
                  {g.label} <small>{g.items.length}</small>
                </h3>
              )}
              {g.items.map((it) => renderCard(it, cardIndex++))}
            </Fragment>
          ))}
        </section>

        <aside class="lov-shop2-detail">
          {shown && <Detail item={shown} preview={!!hover && hover !== pinned?.id} inv={d.items} gold={d.gold} canShop={canShop} onBuy={buy} onPick={pick} />}
        </aside>

        <footer class="lov-shop2-inv">
          <span class="lbl">Inventaire</span>
          <div class="slots">
            {Array.from({ length: 7 }, (_, i) => {
              const id = d.items[i] ?? null;
              const usedInShown = shown ? quoteItem(shown.id, d.items, ITEMS).consumedSlots.includes(i) : false;
              return (
                <button
                  key={i}
                  class={`slot ${sellSlot === i ? 'sel' : ''} ${i === 6 ? 'trinket' : ''} ${id ? 'full' : ''} ${usedInShown ? 'used' : ''}`}
                  disabled={!id}
                  onClick={() => {
                    setSellSlot(sellSlot === i ? null : i);
                    if (id && ITEMS[id]?.shop !== false) pick(id);
                  }}
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
            <button class="lov-btn small danger" disabled={!canShop || !sellItem || sellSlot === 6} onClick={sell} title={sellItem ? undefined : 'Clique un objet de l’inventaire pour le vendre'}>
              {sellItem ? `Vendre ${ITEMS[sellItem]?.name ?? ''} · +${Math.floor((ITEMS[sellItem]?.cost ?? 0) * 0.7)}` : 'Vendre'}
            </button>
          </div>
          <span class="hint">
            {shown && quoteItem(shown.id, d.items, ITEMS).consumedSlots.length > 0
              ? 'Les objets surlignés seront utilisés pour la recette'
              : 'Clic droit sur un objet de l’inventaire : vendre (70 %)'}
          </span>
        </footer>
      </div>
    </div>
  );
}

/** Nombre de colonnes réellement affichées par la grille (pour ↑/↓). */
function columns(grid: HTMLElement | null): number {
  if (!grid) return 1;
  return Math.max(1, getComputedStyle(grid).gridTemplateColumns.split(' ').filter(Boolean).length);
}

function Detail({ item, preview, inv, gold, canShop, onBuy, onPick }: { item: ItemDef; preview: boolean; inv: (string | null)[]; gold: number; canShop: boolean; onBuy: (id: string) => void; onPick: (id: string) => void }) {
  const q = quoteItem(item.id, inv, ITEMS);
  const v = canBuy(item, inv, gold, canShop, ITEMS);
  const into = buildsInto(item.id, SHOP_LIST);
  const stats = Object.entries(item.stats) as [keyof Stats, number][];
  const missing = Math.max(0, Math.ceil(q.price - gold));
  const progress = q.price > 0 ? Math.min(1, gold / q.price) : 1;
  return (
    <div class={`lov-shop2-det cat-${item.category} ${preview ? 'preview' : ''}`}>
      {preview && <span class="peek">Aperçu · clic pour épingler</span>}
      <div class="hero">
        <span class="halo" />
        <ItemIcon itemId={item.id} size={76} />
      </div>
      <h3>{item.name}</h3>
      <span class="cat">{CATEGORY_LABEL[item.category]}</span>

      <button class="lov-cta lov-buy" disabled={v !== 'ok'} onClick={() => onBuy(item.id)}>
        <span class="lov-cta-shine" />
        <span class="lov-cta-label">{v === 'ok' ? 'Acheter' : v === 'gold' ? `Il manque ${missing} or` : VERDICT_TEXT[v]}</span>
        <small>
          {q.price < item.cost ? (
            <>
              <s>{item.cost}</s> {q.price} or · composants déduits
            </>
          ) : (
            <>{q.price} or</>
          )}
        </small>
        {v === 'gold' && <span class="lov-buy-progress" style={{ '--p': progress } as Record<string, number>} />}
      </button>

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
          <small>Améliorable en</small>
          <div>
            {into.map((i) => (
              <button key={i.id} onClick={() => onPick(i.id)} {...tip(itemTip(i.id))}>
                <ItemIcon itemId={i.id} size={34} />
              </button>
            ))}
          </div>
        </div>
      )}
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
