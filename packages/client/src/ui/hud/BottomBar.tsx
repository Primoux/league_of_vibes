import { CHAMPIONS, RECALL_SECONDS, SUMMONERS, type AbilitySlot, type ChampionDef, type EntitySnapshot, type SelfState, type SummonerSlot } from '@lov/shared';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { AbilityCard, ItemCard, SummonerCard, tip } from '../components.tsx';
import { selfEntity, useWorld } from '../game/world.ts';
import { AbilityIcon, ChampionPortrait, Glyph, ItemIcon, SummonerIcon } from '../icons/Icons.tsx';
import { formatCooldown, formatInt } from '../logic/format.ts';
import { canRankUp, cooldownFraction, hastedCooldown, rankValue } from '../logic/game.ts';
import { ITEMS } from '@lov/shared';

interface BarData {
  self: SelfState;
  ent: EntitySnapshot;
  champ: ChampionDef;
}

function useBarData(): BarData | null {
  return useWorld(
    () => {
      const self = store.state.snapshot?.self;
      const ent = selfEntity();
      const champ = ent?.subtype ? CHAMPIONS[ent.subtype] : undefined;
      if (!self || !ent || !champ) return null;
      return { self, ent: { ...ent }, champ };
    },
    (a, b) => a === b || (!!a && !!b && a.self === b.self && a.ent.hp === b.ent.hp && a.ent.mana === b.ent.mana && a.ent.shield === b.ent.shield && a.ent.buffs === b.ent.buffs && a.ent.champ === b.ent.champ && a.ent.dead === b.ent.dead),
  );
}

export function BottomBar() {
  const d = useBarData();
  if (!d) return null;
  const { self, ent, champ } = d;
  const items = ent.champ?.items ?? [];
  return (
    <div class="lov-bottom">
      <BuffBar ent={ent} />
      {self.recalling !== undefined && self.recalling > 0 && <RecallBar remaining={self.recalling} />}
      <div class="lov-bottom-frame">
        <Portrait self={self} ent={ent} champ={champ} />
        <div class="lov-bottom-core">
          <div class="lov-abilities">
            <div class="lov-passive" {...tip(() => <AbilityCard ability={champ.passive} championId={champ.id} stats={self.stats} />)}>
              <AbilityIcon ability={champ.passive} championId={champ.id} size="100%" />
            </div>
            {(['Q', 'W', 'E', 'R'] as const).map((slot) => (
              <AbilitySlotView key={slot} slot={slot} self={self} ent={ent} champ={champ} />
            ))}
            <div class="lov-sum-gap" />
            {(['D', 'F'] as const).map((slot) => (
              <SummonerSlotView key={slot} slot={slot} self={self} />
            ))}
          </div>
          <Bars ent={ent} champ={champ} self={self} />
        </div>
        <Inventory items={items} gold={self.gold} canShop={self.canShop} />
      </div>
    </div>
  );
}

function Portrait({ self, ent, champ }: BarData) {
  const xpFrac = self.xpToNext > 0 ? Math.min(1, self.xp / self.xpToNext) : 1;
  const r = 47;
  const c = 2 * Math.PI * r;
  return (
    <div class={`lov-hud-portrait ${ent.dead ? 'dead' : ''}`} {...tip(() => <span>Expérience : {Math.floor(self.xp)} / {Math.floor(self.xpToNext)}</span>)}>
      <ChampionPortrait id={champ.id} size="100%" variant="round" grayscale={ent.dead} />
      <svg class="xp" viewBox="0 0 100 100">
        <circle cx="50" cy="50" r={r} class="track" />
        <circle cx="50" cy="50" r={r} class="bar" stroke-dasharray={c} stroke-dashoffset={c * (1 - xpFrac)} transform="rotate(90 50 50)" />
      </svg>
      <span class="lvl">{self.level}</span>
      {self.skillPoints > 0 && <span class="pts">+{self.skillPoints}</span>}
    </div>
  );
}

function AbilitySlotView({ slot, self, ent, champ }: { slot: AbilitySlot } & BarData) {
  const ab = champ.abilities[slot];
  const rank = self.abilityLevels[slot] ?? 0;
  const cd = self.cooldowns[slot] ?? 0;
  const total = rank > 0 ? hastedCooldown(rankValue(ab.cooldown, rank), self.stats.abilityHaste) : 0;
  const cost = rank > 0 ? rankValue(ab.cost, rank) : 0;
  const noMana = champ.resource !== 'none' && (ent.mana ?? Infinity) < cost;
  const up = canRankUp(slot, self.level, rank, self.skillPoints);
  const frac = cooldownFraction(cd, Math.max(total, cd));
  const state = rank === 0 ? 'locked' : cd > 0 ? 'cd' : noMana ? 'nomana' : 'ready';
  return (
    <div class={`lov-ability ${state} ${slot === 'R' ? 'ult' : ''}`}>
      {up && (
        <button class="lov-rankup" onClick={() => net.send({ t: 'levelAbility', slot })} title={`Améliorer (Ctrl+${slot})`}>
          <Glyph name="plus" size={12} />
        </button>
      )}
      <div class="icon" {...tip(() => <AbilityCard ability={ab} championId={champ.id} rank={Math.max(rank, 1)} stats={self.stats} hotkey={slot} />)}>
        <AbilityIcon ability={ab} championId={champ.id} size="100%" />
        {cd > 0 && <div class="cd-sweep" style={{ '--f': frac } as Record<string, number>} />}
        {cd > 0 && <span class="cd-num">{formatCooldown(cd)}</span>}
        <kbd>{slot}</kbd>
        {cost > 0 && <span class="cost">{cost}</span>}
      </div>
      <div class="ranks">
        {Array.from({ length: ab.maxRank }, (_, i) => (
          <i key={i} class={i < rank ? 'on' : ''} />
        ))}
      </div>
    </div>
  );
}

function SummonerSlotView({ slot, self }: { slot: SummonerSlot; self: SelfState }) {
  const id = self.summoners[slot];
  const cd = self.cooldowns[slot] ?? 0;
  const total = SUMMONERS[id]?.cooldown ?? 1;
  return (
    <div class={`lov-ability sum ${cd > 0 ? 'cd' : 'ready'}`}>
      <div class="icon" {...tip(() => <SummonerCard id={id} />)}>
        <SummonerIcon id={id} size="100%" />
        {cd > 0 && <div class="cd-sweep" style={{ '--f': cooldownFraction(cd, Math.max(total, cd)) } as Record<string, number>} />}
        {cd > 0 && <span class="cd-num">{formatCooldown(cd)}</span>}
        <kbd>{slot}</kbd>
      </div>
    </div>
  );
}

function Bars({ ent, champ, self }: BarData) {
  const maxHp = Math.max(1, ent.maxHp);
  const shield = ent.dead ? 0 : (ent.shield ?? 0);
  const denom = Math.max(maxHp, ent.hp + shield);
  const hpPct = (ent.hp / denom) * 100;
  const shPct = (shield / denom) * 100;
  const maxMana = ent.maxMana ?? 0;
  const resClass = champ.resource === 'energy' ? 'energy' : champ.resource === 'none' ? 'none' : 'mana';
  return (
    <div class="lov-bars">
      <div class="lov-bar hp">
        <i class="fill" style={{ width: `${hpPct}%` }} />
        {shield > 0 && <i class="shield" style={{ left: `${hpPct}%`, width: `${shPct}%` }} />}
        <i class="ticks" style={{ '--seg': `${(100 * 100) / denom}%` } as Record<string, string>} />
        <span>
          {formatInt(ent.hp)} / {formatInt(maxHp)}
          {shield > 0 && <em> (+{formatInt(shield)})</em>}
        </span>
        <small>+{(self.stats.hpRegen / 5).toFixed(1).replace('.', ',')}</small>
      </div>
      {resClass !== 'none' && (
        <div class={`lov-bar ${resClass}`}>
          <i class="fill" style={{ width: `${maxMana > 0 ? ((ent.mana ?? 0) / maxMana) * 100 : 0}%` }} />
          <span>
            {formatInt(ent.mana ?? 0)} / {formatInt(maxMana)}
          </span>
          <small>+{(self.stats.manaRegen / 5).toFixed(1).replace('.', ',')}</small>
        </div>
      )}
    </div>
  );
}

const ITEM_KEYS = ['1', '2', '3', '5', '6', '7'];

function Inventory({ items, gold, canShop }: { items: (string | null)[]; gold: number; canShop: boolean }) {
  const trinket = items[6] ?? items.find((i) => i && ITEMS[i]?.category === 'trinket') ?? null;
  return (
    <div class="lov-inventory">
      <div class="grid">
        {Array.from({ length: 6 }, (_, i) => {
          const id = items[i] ?? null;
          const def = id ? ITEMS[id] : undefined;
          return (
            <div class={`slot ${id ? 'full' : ''}`} key={i} {...(def ? tip(() => <ItemCard item={def} />) : {})}>
              <ItemIcon itemId={id} size="100%" />
              <kbd>{ITEM_KEYS[i]}</kbd>
            </div>
          );
        })}
      </div>
      <div class="side">
        <div class="slot trinket" {...(trinket && ITEMS[trinket] ? tip(() => <ItemCard item={ITEMS[trinket]!} />) : {})}>
          <ItemIcon itemId={trinket} size="100%" />
          <kbd>4</kbd>
        </div>
        <button class={`lov-gold ${canShop ? 'shop' : ''}`} onClick={() => store.set({ shopOpen: !store.state.shopOpen })} title="Boutique (P)">
          <Glyph name="gold" size={16} />
          <b>{Math.floor(gold)}</b>
        </button>
      </div>
    </div>
  );
}

const CC_WORDS = /stun|root|slow|silence|knock|taunt|suppress|fear|blind|ground|bleed|burn|ignite|mark|frost|givre|poison|exhaust|grievous|anti/i;

function BuffBar({ ent }: { ent: EntitySnapshot }) {
  const buffs = ent.buffs ?? [];
  if (!buffs.length) return null;
  return (
    <div class="lov-buffs">
      {buffs.slice(0, 12).map((b) => (
        <div key={b.id} class={`lov-buff ${CC_WORDS.test(b.id) ? 'debuff' : 'buff'}`} {...tip(() => <span>{prettyBuff(b.id)} — {b.remaining.toFixed(1).replace('.', ',')} s</span>)}>
          <span>{buffInitials(b.id)}</span>
          {b.stacks && b.stacks > 1 && <b>{b.stacks}</b>}
          <i style={{ '--t': `${Math.min(99, b.remaining)}` } as Record<string, string>}>{Math.ceil(b.remaining)}</i>
        </div>
      ))}
    </div>
  );
}

const BUFF_NAMES: Record<string, string> = {
  blue_buff: 'Sentinelle bleue',
  red_buff: 'Brute rouge',
  leviathan: 'Bénédiction du Léviathan',
  leviathan_buff: 'Bénédiction du Léviathan',
  elder: 'Drake Ancien',
  elder_buff: 'Drake Ancien',
  recall: 'Rappel',
};

function prettyBuff(id: string): string {
  const known = BUFF_NAMES[id];
  if (known) return known;
  const s = id.replace(/_/g, ' ');
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function buffInitials(id: string): string {
  const words = prettyBuff(id).split(/\s+/).filter((w) => w.length > 2 || /^[A-Z]/.test(w));
  return (words.length > 1 ? words.slice(0, 2).map((w) => w[0]) : [prettyBuff(id).slice(0, 2)]).join('').toUpperCase();
}

function RecallBar({ remaining }: { remaining: number }) {
  const total = Math.max(RECALL_SECONDS, remaining);
  return (
    <div class="lov-recall">
      <span>Rappel</span>
      <div class="lov-progress">
        <i style={{ width: `${(1 - remaining / total) * 100}%` }} />
      </div>
      <b>{remaining.toFixed(1).replace('.', ',')}</b>
    </div>
  );
}
