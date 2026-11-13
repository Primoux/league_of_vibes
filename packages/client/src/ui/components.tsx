/** Composants UI communs : infobulle globale, chrono circulaire, compte à rebours, fiches de sorts/objets. */
import { CHAMPIONS, ITEMS, SUMMONERS, type AbilityDef, type ChampionDef, type ItemDef, type Scaling, type Stats, type SummonerId } from '@lov/shared';
import type { ComponentChildren, JSX } from 'preact';
import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks';
import { AbilityIcon, ItemIcon, SummonerIcon } from './icons/Icons.tsx';
import { formatRanks, formatStat, STAT_LABEL } from './logic/format.ts';
import { hastedCooldown, rankValue } from './logic/game.ts';
import { clock } from '../net/client.ts';
import { MiniStore, useMini } from './useStore.ts';

// ------------------------------------------------------------------ infobulle globale (un seul nœud DOM)
interface TipState {
  content: ComponentChildren | null;
  x: number;
  y: number;
  anchor: 'above' | 'below';
}
const tipStore = new MiniStore<TipState>({ content: null, x: 0, y: 0, anchor: 'above' });

/** Props à étaler sur un élément pour lui donner une infobulle. */
export function tip(content: () => ComponentChildren, anchor: 'above' | 'below' = 'above') {
  return {
    onMouseEnter: (e: JSX.TargetedMouseEvent<HTMLElement>) => {
      const r = e.currentTarget.getBoundingClientRect();
      tipStore.set({ content: content(), x: r.left + r.width / 2, y: anchor === 'above' ? r.top : r.bottom, anchor });
    },
    onMouseLeave: () => tipStore.set({ content: null }),
  };
}
export const hideTip = () => tipStore.set({ content: null });

export function TooltipLayer() {
  const t = useMini(tipStore, (s) => s);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !t.content) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const left = Math.max(8, Math.min(innerWidth - w - 8, t.x - w / 2));
    let top = t.anchor === 'above' ? t.y - h - 10 : t.y + 10;
    if (top < 8) top = t.y + 10;
    if (top + h > innerHeight - 8) top = innerHeight - h - 8;
    setPos({ left, top });
  }, [t]);
  if (!t.content) return null;
  return (
    <div class="lov-tooltip" ref={ref} style={{ left: pos.left, top: pos.top }}>
      {t.content}
    </div>
  );
}

// ------------------------------------------------------------------ compte à rebours / anneau
/** Secondes restantes avant `endsAt` (timestamp serveur, ms), via l'horloge synchronisée de net/. Rafraîchi 10×/s. */
export function useCountdown(endsAt: number | null | undefined): number {
  const [now, setNow] = useState(() => clock.serverNow(Date.now()));
  useEffect(() => {
    if (!endsAt) return;
    const id = setInterval(() => setNow(clock.serverNow(Date.now())), 100);
    return () => clearInterval(id);
  }, [endsAt]);
  return endsAt ? Math.max(0, (endsAt - now) / 1000) : 0;
}

export function TimerRing({ remaining, total, size = 72, label, warnAt = 5 }: { remaining: number; total: number; size?: number; label?: string; warnAt?: number }) {
  const r = 44;
  const c = 2 * Math.PI * r;
  const frac = total > 0 ? Math.max(0, Math.min(1, remaining / total)) : 0;
  const warn = remaining <= warnAt;
  return (
    <div class={`lov-timer-ring ${warn ? 'warn' : ''}`} style={{ width: size, height: size }}>
      <svg viewBox="0 0 100 100" width={size} height={size}>
        <circle cx="50" cy="50" r={r} class="track" />
        <circle cx="50" cy="50" r={r} class="bar" stroke-dasharray={c} stroke-dashoffset={c * (1 - frac)} transform="rotate(-90 50 50)" />
      </svg>
      <div class="lov-timer-ring-text">
        <b>{Math.ceil(remaining)}</b>
        {label && <small>{label}</small>}
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ fiches (contenu d'infobulle)
const SCALING_LABEL: [keyof Scaling, string, string][] = [
  ['ad', 'AD', 'sc-ad'],
  ['bonusAd', 'AD bonus', 'sc-ad'],
  ['ap', 'AP', 'sc-ap'],
  ['maxHp', 'PV max', 'sc-hp'],
  ['bonusHp', 'PV bonus', 'sc-hp'],
  ['armor', 'armure', 'sc-armor'],
  ['targetMaxHp', 'PV max de la cible', 'sc-hp'],
  ['targetMissingHp', 'PV manquants de la cible', 'sc-hp'],
];

function scalingText(d: Scaling, rank: number, stats?: Stats): ComponentChildren[] {
  const out: ComponentChildren[] = [];
  const base = rank > 0 ? rankValue(d.base, rank) : null;
  out.push(<span class="sc-base">{rank > 0 ? base : formatRanks(d.base)}</span>);
  for (const [k, label, cls] of SCALING_LABEL) {
    const v = d[k];
    if (typeof v !== 'number' || v === 0) continue;
    const pct = Math.round(v * 100);
    out.push(
      <span class={cls}>
        {' '}+ {pct} % {label}
      </span>,
    );
  }
  if (stats && rank > 0 && base !== null) {
    const total = base + (d.ad ?? 0) * stats.ad + (d.ap ?? 0) * stats.ap + (d.maxHp ?? 0) * stats.maxHp + (d.armor ?? 0) * stats.armor;
    out.push(<span class="sc-total"> ({Math.round(total)})</span>);
  }
  return out;
}

const DMG_LABEL = { physical: 'physiques', magic: 'magiques', true: 'bruts' } as const;

export function AbilityCard({ ability, championId, rank = 0, stats, hotkey }: { ability: AbilityDef; championId: string; rank?: number; stats?: Stats; hotkey?: string }) {
  const champ: ChampionDef | undefined = CHAMPIONS[championId];
  const resource = champ?.resource === 'energy' ? 'énergie' : 'mana';
  const isPassive = ability.slot === 'P';
  const cd = ability.cooldown.some((v) => v > 0) ? (rank > 0 ? hastedCooldown(rankValue(ability.cooldown, rank), stats?.abilityHaste ?? 0) : null) : null;
  return (
    <div class="lov-card">
      <div class="lov-card-head">
        <AbilityIcon ability={ability} championId={championId} size={40} />
        <div>
          <div class="lov-card-title">
            {ability.name}
            {hotkey && <kbd>{hotkey}</kbd>}
          </div>
          <div class="lov-card-sub">
            {isPassive ? 'Passif' : `Rang ${rank}/${ability.maxRank}`}
            {ability.range > 0 && !isPassive && ability.range < 99999 && <> · Portée {ability.range}</>}
          </div>
        </div>
      </div>
      <p class="lov-card-desc">{ability.description}</p>
      {ability.damage && (
        <p class="lov-card-line">
          Dégâts {DMG_LABEL[ability.damage.type]} : {scalingText(ability.damage, rank, stats)}
        </p>
      )}
      {!isPassive && (
        <div class="lov-card-foot">
          {ability.cost.some((v) => v > 0) && (
            <span class="mana">
              {rank > 0 ? rankValue(ability.cost, rank) : formatRanks(ability.cost)} {resource}
            </span>
          )}
          <span class="cd">⟳ {cd !== null ? `${cd.toFixed(1).replace('.0', '').replace('.', ',')} s` : `${formatRanks(ability.cooldown)} s`}</span>
        </div>
      )}
    </div>
  );
}

export function ItemCard({ item, price }: { item: ItemDef; price?: number }) {
  const stats = Object.entries(item.stats) as [keyof Stats, number][];
  return (
    <div class="lov-card">
      <div class="lov-card-head">
        <ItemIcon itemId={item.id} size={40} />
        <div>
          <div class="lov-card-title">{item.name}</div>
          <div class="lov-card-sub gold">
            {price !== undefined && price !== item.cost ? (
              <>
                <b>{price}</b> <s>{item.cost}</s>
              </>
            ) : (
              <b>{item.cost}</b>
            )}{' '}
            or
          </div>
        </div>
      </div>
      {stats.length > 0 && (
        <ul class="lov-card-stats">
          {stats.map(([k, v]) => (
            <li key={k}>
              <b>{formatStat(k, v, true)}</b> {STAT_LABEL[k]}
            </li>
          ))}
        </ul>
      )}
      {item.passive && (
        <p class="lov-card-line">
          <b class="pas">{item.passive.name}</b> — {item.passive.description}
        </p>
      )}
      {item.active && (
        <p class="lov-card-line">
          <b class="act">Actif : {item.active.name}</b> — {item.active.description}
          {item.active.cooldown > 0 && <> ({item.active.cooldown} s)</>}
        </p>
      )}
      {!item.passive && !item.active && stats.length === 0 && <p class="lov-card-desc">{item.description}</p>}
    </div>
  );
}

export function SummonerCard({ id }: { id: SummonerId }) {
  const s = SUMMONERS[id];
  return (
    <div class="lov-card">
      <div class="lov-card-head">
        <SummonerIcon id={id} size={36} />
        <div>
          <div class="lov-card-title">{s.name}</div>
          <div class="lov-card-sub">Recharge {s.cooldown} s</div>
        </div>
      </div>
      <p class="lov-card-desc">{s.description}</p>
    </div>
  );
}

export const itemTip = (id: string, price?: number) => () => {
  const it = ITEMS[id];
  return it ? <ItemCard item={it} price={price} /> : null;
};
