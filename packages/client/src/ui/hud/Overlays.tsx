import { CHAMPIONS, type Team } from '@lov/shared';
import { useEffect } from 'preact/hooks';
import { store } from '../../state.ts';
import { champions, myTeam, selfEntity, ui, useWorld } from '../game/world.ts';
import { ChampionPortrait, Glyph, UnitGlyph } from '../icons/Icons.tsx';
import { DRAGON_COLOR, DRAGON_LABEL, formatClock } from '../logic/format.ts';
import type { FeedEntry, UnitRef } from '../logic/feed.ts';
import { useMini, useSelect } from '../useStore.ts';
import type { DragonElement } from '@lov/shared';

// ------------------------------------------------------------------ score + chrono
export function TopBar() {
  const d = useSelect((s) => ({ score: s.snapshot?.score, time: Math.floor(s.snapshot?.time ?? 0), team: s.game?.team ?? 'blue' }), (a, b) => a.score === b.score && a.time === b.time && a.team === b.team);
  const score = d.score ?? { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } };
  const side = (t: Team) => {
    const sc = score[t];
    return (
      <div class={`lov-score-side ${t} ${t === d.team ? 'mine' : ''}`}>
        <span class="towers" title="Tours détruites">
          <Glyph name="tower" size={14} />
          {sc.towers}
        </span>
        <span class="drakes" title="Drakes">
          {sc.dragons.map((el, i) => (
            <i key={i} style={{ background: DRAGON_COLOR[el as DragonElement] ?? '#aaa' }} title={DRAGON_LABEL[el as DragonElement] ?? el} />
          ))}
          {sc.dragons.length === 0 && <em>—</em>}
          {sc.dragons.length >= 4 && (
            <span class="soul" title="Âme du drake" style={{ '--c': DRAGON_COLOR[(sc.dragons[2] ?? sc.dragons[0]) as DragonElement] ?? '#fff' } as Record<string, string>}>
              Âme
            </span>
          )}
        </span>
        {sc.grubs > 0 && (
          <span class="grubs" title="Larves">
            <Glyph name="grub" size={14} />
            {sc.grubs}
          </span>
        )}
        <b class="kills">{sc.kills}</b>
      </div>
    );
  };
  return (
    <div class="lov-topbar">
      {side('blue')}
      <div class="lov-clock">
        <Glyph name="sword" size={12} />
        {formatClock(d.time)}
      </div>
      {side('red')}
    </div>
  );
}

/** KDA / CS du joueur, en haut à droite. */
export function SelfStatsStrip() {
  const d = useWorld(() => {
    const c = selfEntity()?.champ;
    return c ? { k: c.kills, d: c.deaths, a: c.assists, cs: c.cs } : null;
  });
  if (!d) return null;
  return (
    <div class="lov-selfstrip">
      <span title="Éliminations / Morts / Assistances">
        <Glyph name="skull" size={13} /> {d.k}/{d.d}/{d.a}
      </span>
      <span title="Sbires tués">
        <Glyph name="minion" size={13} /> {d.cs}
      </span>
    </div>
  );
}

// ------------------------------------------------------------------ kill feed
function UnitBadge({ u, team }: { u?: UnitRef; team?: Team | 'neutral' }) {
  if (!u) return <span class="lov-feed-unit unknown"><UnitGlyph kind="minion" /></span>;
  const t = u.team === 'neutral' ? (team ?? 'neutral') : u.team;
  if (u.kind === 'champion' && u.subtype) {
    return (
      <span class={`lov-feed-unit ${t}`}>
        <ChampionPortrait id={u.subtype} size={30} variant="round" />
      </span>
    );
  }
  return (
    <span class={`lov-feed-unit glyph ${t}`}>
      <UnitGlyph kind={u.kind} subtype={u.subtype} />
    </span>
  );
}

export function KillFeed() {
  const feed = useMini(ui, (s) => s.feed);
  const mine = useSelect((s) => s.game?.team ?? 'blue');
  return (
    <div class="lov-killfeed">
      {feed.map((f) => (
        <FeedRow key={f.key} f={f} mine={mine} />
      ))}
    </div>
  );
}

function FeedRow({ f, mine }: { f: FeedEntry; mine: Team }) {
  const tone = f.team === 'neutral' ? 'neutral' : f.team === mine ? 'ally' : 'enemy';
  return (
    <div class={`lov-feed-row ${tone} ${f.kind}`}>
      {f.kind === 'objective' ? (
        <>
          <UnitBadge u={f.killer} />
          <span class="arrow">⟶</span>
          <span class="lov-feed-unit glyph neutral" style={f.element ? { color: DRAGON_COLOR[f.element] } : undefined}>
            <UnitGlyph kind="monster" subtype={f.monster} />
          </span>
          <span class="label">{f.label}</span>
        </>
      ) : f.kind === 'structure' ? (
        <>
          <span class="label">{f.label} détruite</span>
          <UnitBadge u={f.victim} />
        </>
      ) : (
        <>
          <UnitBadge u={f.killer} team={f.team} />
          {f.assists.length > 0 && (
            <span class="assists">
              {f.assists.slice(0, 3).map((a) => (a.subtype ? <ChampionPortrait key={a.id} id={a.subtype} size={16} variant="round" /> : null))}
            </span>
          )}
          <span class="arrow">
            <Glyph name="sword" size={14} />
          </span>
          <UnitBadge u={f.victim} />
        </>
      )}
    </div>
  );
}

// ------------------------------------------------------------------ annonces centrales
export function Announcer() {
  const a = useMini(ui, (s) => s.announce);
  if (!a) return null;
  return (
    <div class={`lov-announce ${a.tone}`} key={a.key}>
      {(a.killer?.subtype || a.victim?.subtype) && (
        <div class="faces">
          {a.killer?.subtype && a.killer.kind === 'champion' && <ChampionPortrait id={a.killer.subtype} size={56} variant="round" />}
          {a.victim?.subtype && a.victim.kind === 'champion' && (
            <>
              <span class="x">
                <Glyph name="sword" size={20} />
              </span>
              <ChampionPortrait id={a.victim.subtype} size={56} variant="round" grayscale />
            </>
          )}
        </div>
      )}
      <h2 style={a.element ? { color: DRAGON_COLOR[a.element] } : undefined}>{a.title}</h2>
      {a.subtitle && <p>{a.subtitle}</p>}
    </div>
  );
}

// ------------------------------------------------------------------ cadres des alliés
export function TeamFrames() {
  const list = useWorld(() => {
    const team = myTeam();
    const me = selfEntity()?.id;
    return champions()
      .filter((c) => c.team === team && c.id !== me)
      .map((c) => ({ id: c.id, champ: c.subtype ?? '', hp: c.hp / Math.max(1, c.maxHp), mana: c.maxMana ? (c.mana ?? 0) / c.maxMana : 0, lvl: c.level ?? 1, dead: !!c.dead, respawn: c.champ?.respawnIn, name: c.champ?.name ?? '' }));
  }, (a, b) => JSON.stringify(a) === JSON.stringify(b));
  if (!list.length) return null;
  return (
    <div class="lov-teamframes">
      {list.map((c) => (
        <div key={c.id} class={`lov-tf ${c.dead ? 'dead' : ''}`} title={c.name}>
          <div class="pic">
            <ChampionPortrait id={c.champ} size={44} variant="round" grayscale={c.dead} />
            <span class="lvl">{c.lvl}</span>
            {c.dead && c.respawn !== undefined && <span class="respawn">{Math.ceil(c.respawn)}</span>}
          </div>
          <div class="bars">
            <i class="hp" style={{ width: `${c.hp * 100}%` }} />
            <i class="mp" style={{ width: `${c.mana * 100}%` }} />
          </div>
        </div>
      ))}
    </div>
  );
}

// ------------------------------------------------------------------ PV bas
/** Vignette rouge qui pulse sur les bords de l'écran sous 30 % de PV (plus intense sous 15 %). */
export function LowHpVignette() {
  const level = useWorld(() => {
    const e = selfEntity();
    if (!e || e.dead || e.maxHp <= 0) return 0;
    const r = e.hp / e.maxHp;
    return r < 0.15 ? 2 : r < 0.3 ? 1 : 0;
  });
  if (!level) return null;
  return <div class={`lov-lowhp l${level}`} aria-hidden="true" />;
}

// ------------------------------------------------------------------ mort
export function DeathOverlay() {
  const d = useWorld(() => {
    const e = selfEntity();
    return e?.dead ? { respawn: e.champ?.respawnIn ?? 0, champ: e.subtype ?? '' } : null;
  });
  useEffect(() => {
    document.body.classList.toggle('lov-dead', !!d);
    return () => document.body.classList.remove('lov-dead');
  }, [!!d]);
  if (!d) return null;
  const name = CHAMPIONS[d.champ]?.name ?? '';
  return (
    <div class="lov-death">
      <small>{name} est tombé·e au combat</small>
      <h2>Réapparition dans</h2>
      <b>{Math.ceil(d.respawn)}</b>
      <p>Tu peux acheter en boutique (P) pendant ce temps.</p>
    </div>
  );
}

/** Rafraîchit le titre de l'onglet (utile quand on alt-tab). */
export function useTabTitle(): void {
  useEffect(() => {
    const off = store.subscribe((s) => {
      const t = s.snapshot?.time;
      document.title = t !== undefined ? `${formatClock(t)} · League of Vibes` : 'League of Vibes';
    });
    return () => {
      off();
      document.title = 'League of Vibes';
    };
  }, []);
}
