import { CHAMPION_LIST, CHAMPIONS, DRAFT_TURN_SECONDS, SUMMONER_IDS, SUMMONERS, type ChampionDef, type DraftPick, type Role, type SummonerId, type Team } from '@lov/shared';
import { useEffect, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { AbilityCard, SummonerCard, TimerRing, tip, useCountdown } from '../components.tsx';
import { AbilityIcon, ChampionPortrait, ClassEmblem, Glyph, RoleIcon, SummonerIcon } from '../icons/Icons.tsx';
import { CLASS_LABEL, ROLE_LABEL } from '../logic/format.ts';
import { useSelect } from '../useStore.ts';

const FINALIZE_SECONDS = 15;

export function Draft() {
  const { draft, me, lobby } = useSelect((s) => ({ draft: s.draft!, me: s.playerId, lobby: s.lobby }));
  const myPick = draft.picks.find((p) => p.playerId === me);
  const myTeam: Team = myPick?.team ?? 'blue';
  const finalizing = draft.currentTurn >= draft.turns.length;
  const turnPlayers = draft.turns[draft.currentTurn] ?? [];
  const myTurn = !finalizing && !!me && turnPlayers.includes(me) && !myPick?.locked;
  const turnTeam = draft.picks.find((p) => turnPlayers.includes(p.playerId))?.team;
  const remaining = useCountdown(finalizing ? (draft.finalizeEndsAt ?? null) : draft.turnEndsAt);
  const total = finalizing ? FINALIZE_SECONDS : DRAFT_TURN_SECONDS;

  const [selected, setSelected] = useState<string | null>(myPick?.hoverChampionId ?? myPick?.championId ?? null);
  const [roleFilter, setRoleFilter] = useState<Role | 'all'>('all');
  useEffect(() => {
    if (myPick?.locked && myPick.championId) setSelected(myPick.championId);
  }, [myPick?.locked, myPick?.championId]);

  const takenBy = new Map<string, DraftPick>();
  for (const p of draft.picks) if (p.locked && p.championId) takenBy.set(p.championId, p);
  const allyHover = new Map<string, DraftPick>();
  for (const p of draft.picks) if (p.team === myTeam && !p.locked && p.hoverChampionId && p.playerId !== me) allyHover.set(p.hoverChampionId, p);

  const pick = (id: string) => {
    if (takenBy.has(id)) return;
    setSelected(id);
    if (myTurn) net.send({ t: 'draftHover', championId: id });
  };
  const lock = () => selected && myTurn && !takenBy.has(selected) && net.send({ t: 'draftLock', championId: selected });

  const names = new Map(lobby?.players.map((p) => [p.id, p.name]) ?? []);
  const champ = selected ? CHAMPIONS[selected] : undefined;
  const list = CHAMPION_LIST.filter((c) => roleFilter === 'all' || c.roles.includes(roleFilter));

  let phaseText: string;
  if (finalizing) phaseText = 'Finalisation — choisis tes sorts d’invocateur';
  else if (myTurn) phaseText = 'À toi de choisir !';
  else if (turnTeam === myTeam) phaseText = 'Ton équipe choisit…';
  else phaseText = 'L’équipe adverse choisit…';

  return (
    <div class={`lov-screen lov-draft ${myTurn ? 'my-turn' : ''}`}>
      <div class="lov-aurora dim" />
      <TeamPanel team="blue" picks={draft.picks} myTeam={myTeam} turnPlayers={finalizing ? [] : turnPlayers} me={me} names={names} />

      <main class="lov-draft-center">
        <header class="lov-draft-head">
          <div class="lov-draft-phase">
            <small>
              {finalizing ? 'Finalisation' : `Tour ${draft.currentTurn + 1} / ${draft.turns.length}`}
            </small>
            <h1 class={myTurn ? 'pulse' : ''}>{phaseText}</h1>
            <TurnTrack turns={draft.turns} current={draft.currentTurn} picks={draft.picks} />
          </div>
          <TimerRing remaining={remaining} total={total} size={78} />
        </header>

        <div class="lov-draft-filters">
          <button class={`lov-chip ${roleFilter === 'all' ? 'on' : ''}`} onClick={() => setRoleFilter('all')}>
            Tous
          </button>
          {(['top', 'jungle', 'mid', 'bot'] as const).map((r) => (
            <button key={r} class={`lov-chip ${roleFilter === r ? 'on' : ''}`} onClick={() => setRoleFilter(r)}>
              <RoleIcon role={r} size={14} /> {ROLE_LABEL[r]}
            </button>
          ))}
        </div>

        <div class="lov-champ-grid">
          {list.map((c) => {
            const taken = takenBy.get(c.id);
            const hov = allyHover.get(c.id);
            return (
              <button
                key={c.id}
                class={`lov-champ-tile ${taken ? 'taken' : ''} ${selected === c.id ? 'sel' : ''} ${taken?.team ?? ''}`}
                onClick={() => pick(c.id)}
                onDblClick={() => {
                  pick(c.id);
                  if (myTurn && !taken) net.send({ t: 'draftLock', championId: c.id });
                }}
                disabled={!!taken}
              >
                <ChampionPortrait id={c.id} size="100%" grayscale={!!taken} />
                <span class="name">{c.name}</span>
                <span class="cls">
                  <ClassEmblem cls={c.class} size={12} />
                </span>
                {taken && (
                  <span class="lock">
                    <Glyph name="lock" size={18} />
                  </span>
                )}
                {hov && <span class="hover-tag">{names.get(hov.playerId) ?? 'Allié'}</span>}
              </button>
            );
          })}
        </div>

        {finalizing || myPick?.locked ? (
          <SummonerPicker pick={myPick} />
        ) : (
          <div class="lov-draft-lockbar">
            <button class="lov-btn primary big glow lock" disabled={!myTurn || !selected || takenBy.has(selected)} onClick={lock}>
              {myTurn ? (selected ? `Verrouiller ${CHAMPIONS[selected]?.name ?? ''}` : 'Choisis un champion') : 'En attente de ton tour'}
            </button>
          </div>
        )}
        {myPick?.locked && !finalizing && <p class="lov-muted center">Champion verrouillé. Prépare tes sorts d’invocateur pendant que les autres choisissent.</p>}
      </main>

      <aside class="lov-draft-side">
        {champ ? <ChampionInfo champ={champ} /> : <div class="lov-draft-empty">Survole ou clique un champion pour voir son kit.</div>}
      </aside>

      <TeamPanel team="red" picks={draft.picks} myTeam={myTeam} turnPlayers={finalizing ? [] : turnPlayers} me={me} names={names} />
    </div>
  );
}

function TurnTrack({ turns, current, picks }: { turns: string[][]; current: number; picks: DraftPick[] }) {
  return (
    <div class="lov-turn-track">
      {turns.map((t, i) => {
        const team = picks.find((p) => t.includes(p.playerId))?.team ?? 'blue';
        return (
          <span key={i} class={`step ${team} ${i < current ? 'done' : i === current ? 'now' : ''}`}>
            {t.map((_, j) => (
              <i key={j} />
            ))}
          </span>
        );
      })}
    </div>
  );
}

function TeamPanel({ team, picks, myTeam, turnPlayers, me, names }: { team: Team; picks: DraftPick[]; myTeam: Team; turnPlayers: string[]; me: string | null; names: Map<string, string> }) {
  const tp = picks.filter((p) => p.team === team);
  const ally = team === myTeam;
  return (
    <aside class={`lov-draft-team ${team}`}>
      <h2>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</h2>
      {tp.map((p) => {
        const showId = p.locked ? p.championId : ally ? p.hoverChampionId : null;
        const c = showId ? CHAMPIONS[showId] : undefined;
        const active = turnPlayers.includes(p.playerId) && !p.locked;
        return (
          <div key={p.playerId} class={`lov-pick ${p.locked ? 'locked' : ''} ${active ? 'active' : ''} ${p.playerId === me ? 'me' : ''}`}>
            <div class="lov-pick-art">
              {showId ? <ChampionPortrait id={showId} size="100%" class={p.locked ? '' : 'ghost'} /> : <div class="lov-pick-empty">?</div>}
            </div>
            <div class="lov-pick-info">
              <b>{c ? c.name : active ? 'Choisit…' : '—'}</b>
              <span>{names.get(p.playerId) ?? 'Invocateur'}</span>
            </div>
            {ally && (
              <div class="lov-pick-sums">
                <SummonerIcon id={p.summoners[0]} size={20} />
                <SummonerIcon id={p.summoners[1]} size={20} />
              </div>
            )}
          </div>
        );
      })}
    </aside>
  );
}

function SummonerPicker({ pick }: { pick: DraftPick | undefined }) {
  const [slot, setSlot] = useState<0 | 1>(0);
  if (!pick) return null;
  const cur = pick.summoners;
  const choose = (id: SummonerId) => {
    const next: [SummonerId, SummonerId] = [cur[0], cur[1]];
    const other = slot === 0 ? 1 : 0;
    if (next[other] === id) next[other] = next[slot];
    next[slot] = id;
    net.send({ t: 'draftSummoners', summoners: next });
    setSlot(other);
  };
  return (
    <div class="lov-sum-picker">
      <div class="lov-sum-slots">
        {(['D', 'F'] as const).map((k, i) => (
          <button key={k} class={`lov-sum-slot ${slot === i ? 'on' : ''}`} onClick={() => setSlot(i as 0 | 1)} {...tip(() => <SummonerCard id={cur[i]!} />)}>
            <SummonerIcon id={cur[i]!} size={44} />
            <kbd>{k}</kbd>
          </button>
        ))}
      </div>
      <div class="lov-sum-grid">
        {SUMMONER_IDS.map((id) => (
          <button key={id} class={`lov-sum-opt ${cur.includes(id) ? 'used' : ''}`} onClick={() => choose(id)} {...tip(() => <SummonerCard id={id} />)}>
            <SummonerIcon id={id} size={36} />
            <span>{SUMMONERS[id].name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

const STAT_BARS: { label: string; get: (c: ChampionDef) => number; max: number }[] = [
  { label: 'Dégâts', get: (c) => c.baseStats.ad + (c.perLevel.ad ?? 0) * 10, max: 120 },
  { label: 'Résistance', get: (c) => c.baseStats.maxHp / 10 + c.baseStats.armor + (c.perLevel.maxHp ?? 0) / 3, max: 160 },
  { label: 'Portée', get: (c) => c.baseStats.attackRange, max: 650 },
  { label: 'Mobilité', get: (c) => c.baseStats.moveSpeed - 300 + Object.values(c.abilities).filter((a) => a.targeting === 'dash' || a.targeting === 'targetedDash').length * 25, max: 100 },
];

function ChampionInfo({ champ }: { champ: ChampionDef }) {
  const slots = ['P', 'Q', 'W', 'E', 'R'] as const;
  const [focus, setFocus] = useState<(typeof slots)[number]>('P');
  const ab = focus === 'P' ? champ.passive : champ.abilities[focus];
  return (
    <div class="lov-champ-info" key={champ.id}>
      <div class="lov-champ-hero">
        <ChampionPortrait id={champ.id} variant="splash" size="100%" />
        <div class="lov-champ-hero-text">
          <h2>{champ.name}</h2>
          <p>{champ.title}</p>
        </div>
      </div>
      <div class="lov-champ-tags">
        <span class="lov-tag">
          <ClassEmblem cls={champ.class} size={13} /> {CLASS_LABEL[champ.class]}
        </span>
        {champ.roles.map((r) => (
          <span class="lov-tag" key={r}>
            <RoleIcon role={r} size={13} /> {ROLE_LABEL[r]}
          </span>
        ))}
        <span class="lov-tag diff" title="Difficulté">
          {'◆'.repeat(champ.difficulty)}
          <i>{'◆'.repeat(3 - champ.difficulty)}</i>
        </span>
        <span class="lov-tag">{champ.ranged ? 'Distance' : 'Mêlée'}</span>
      </div>
      <p class="lov-lore">{champ.lore}</p>
      <div class="lov-stat-bars">
        {STAT_BARS.map((b) => (
          <div key={b.label}>
            <span>{b.label}</span>
            <i style={{ '--v': Math.max(0.08, Math.min(1, b.get(champ) / b.max)) } as Record<string, number>} />
          </div>
        ))}
      </div>
      <div class="lov-kit-row">
        {slots.map((s) => {
          const a = s === 'P' ? champ.passive : champ.abilities[s];
          return (
            <button key={s} class={`lov-kit-btn ${focus === s ? 'on' : ''}`} onMouseEnter={() => setFocus(s)} onClick={() => setFocus(s)}>
              <AbilityIcon ability={a} championId={champ.id} size={44} />
              <kbd>{s === 'P' ? '•' : s}</kbd>
            </button>
          );
        })}
      </div>
      <AbilityCard ability={ab} championId={champ.id} />
    </div>
  );
}
