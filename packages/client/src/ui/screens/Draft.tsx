import { CHAMPION_LIST, CHAMPIONS, DRAFT_TURN_SECONDS, SUMMONER_IDS, SUMMONERS, type ChampionDef, type DraftPick, type Role, type SummonerId, type Team } from '@lov/shared';
import { useEffect, useRef, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { AbilityCard, SummonerCard, TimerRing, tip, useCountdown } from '../components.tsx';
import { AbilityIcon, ChampionPortrait, ClassEmblem, Glyph, RoleIcon, SummonerIcon } from '../icons/Icons.tsx';
import { CLASS_LABEL, ROLE_LABEL } from '../logic/format.ts';
import { ChampionStage } from '../stage/ChampionStage.tsx';
import { RiftBackground } from '../stage/RiftBackground.tsx';
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

  const [selected, setSelected] = useState<string | null>(myPick?.championId ?? myPick?.hoverChampionId ?? null);
  const [roleFilter, setRoleFilter] = useState<Role | 'all'>('all');
  const [pulse, setPulse] = useState(0);
  const wasLocked = useRef(!!myPick?.locked);
  useEffect(() => {
    if (myPick?.locked && myPick.championId) {
      setSelected(myPick.championId);
      if (!wasLocked.current) setPulse((p) => p + 1);
    }
    wasLocked.current = !!myPick?.locked;
  }, [myPick?.locked, myPick?.championId]);

  const takenBy = new Map<string, DraftPick>();
  for (const p of draft.picks) if (p.locked && p.championId) takenBy.set(p.championId, p);
  const allyHover = new Map<string, DraftPick>();
  for (const p of draft.picks) if (p.team === myTeam && !p.locked && p.hoverChampionId && p.playerId !== me) allyHover.set(p.hoverChampionId, p);

  const pick = (id: string) => {
    if (takenBy.has(id) && takenBy.get(id)!.playerId !== me) return;
    setSelected(id);
    if (myTurn) net.send({ t: 'draftHover', championId: id });
  };
  const lock = () => selected && myTurn && !takenBy.has(selected) && net.send({ t: 'draftLock', championId: selected });

  const names = new Map(lobby?.players.map((p) => [p.id, p.name]) ?? []);
  const champ = selected ? CHAMPIONS[selected] : undefined;
  const list = CHAMPION_LIST.filter((c) => roleFilter === 'all' || c.roles.includes(roleFilter));

  let phaseText: string;
  if (finalizing) phaseText = 'Prépare-toi au combat';
  else if (myTurn) phaseText = 'À toi de choisir !';
  else if (turnTeam === myTeam) phaseText = 'Ton équipe choisit…';
  else phaseText = 'L’équipe adverse choisit…';

  return (
    <div class={`lov-screen lov-draft2 ${myTurn ? 'my-turn' : ''} ${turnTeam ?? ''}-turn`} style={{ '--champ': champ?.palette.primary ?? '#3ee6c8', '--champ2': champ?.palette.accent ?? '#f6dc94' } as Record<string, string>}>
      <RiftBackground tint={champ?.palette.primary ?? '#3ee6c8'} intensity={0.8} />
      <div class="lov-draft2-glow" />

      <TeamPanel team="blue" picks={draft.picks} myTeam={myTeam} turnPlayers={finalizing ? [] : turnPlayers} me={me} names={names} />

      <main class="lov-draft2-center">
        <header class="lov-draft2-head">
          <div class="phase">
            <small>{finalizing ? 'Finalisation' : `Phase de sélection · tour ${draft.currentTurn + 1}/${draft.turns.length}`}</small>
            <h1 class={myTurn ? 'pulse' : ''}>{phaseText}</h1>
            <TurnTrack turns={draft.turns} current={draft.currentTurn} picks={draft.picks} />
          </div>
          <TimerRing remaining={remaining} total={total} size={84} />
        </header>

        <section class="lov-draft2-stage">
          {champ ? (
            <>
              <ChampionStage championId={champ.id} pulse={pulse} class="stage" spin={0.25} />
              <ChampionInfo champ={champ} key={champ.id} />
              <div class="lov-draft2-name" key={`n-${champ.id}`}>
                <span>
                  <ClassEmblem cls={champ.class} size={14} /> {CLASS_LABEL[champ.class]}
                </span>
                <h2>{champ.name}</h2>
                <p>{champ.title}</p>
              </div>
            </>
          ) : (
            <div class="lov-draft2-empty">
              <Glyph name="nexus" size={42} />
              <p>Choisis un champion dans la galerie</p>
            </div>
          )}
        </section>

        <section class="lov-draft2-dock">
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
          <div class="lov-gallery">
            {list.map((c, i) => {
              const taken = takenBy.get(c.id);
              const hov = allyHover.get(c.id);
              const mine = taken?.playerId === me;
              return (
                <button
                  key={c.id}
                  class={`lov-gal-tile ${taken && !mine ? 'taken' : ''} ${selected === c.id ? 'sel' : ''} ${taken?.team ?? ''}`}
                  style={{ '--c': c.palette.primary, '--i': i } as Record<string, string | number>}
                  onClick={() => pick(c.id)}
                  onDblClick={() => {
                    pick(c.id);
                    if (myTurn && !taken) net.send({ t: 'draftLock', championId: c.id });
                  }}
                  disabled={!!taken && !mine}
                  title={`${c.name}, ${c.title}`}
                >
                  <ChampionPortrait id={c.id} size="100%" grayscale={!!taken && !mine} />
                  <span class="name">{c.name}</span>
                  {taken && !mine && (
                    <span class="lock">
                      <Glyph name="lock" size={16} />
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
            <button class="lov-cta lov-lock-cta" disabled={!myTurn || !selected || takenBy.has(selected)} onClick={lock}>
              <span class="lov-cta-shine" />
              <span class="lov-cta-label">{myTurn ? 'Verrouiller' : 'Patience…'}</span>
              <small>{myTurn ? (selected ? `${CHAMPIONS[selected]?.name ?? ''} entre dans la Faille` : 'Choisis un champion') : 'Ce n’est pas encore ton tour'}</small>
            </button>
          )}
        </section>
      </main>

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
    <aside class={`lov-draft2-team ${team}`}>
      <h2>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</h2>
      {tp.map((p, i) => {
        const showId = p.locked ? p.championId : ally ? p.hoverChampionId : null;
        const c = showId ? CHAMPIONS[showId] : undefined;
        const active = turnPlayers.includes(p.playerId) && !p.locked;
        return (
          <div key={p.playerId} class={`lov-pick2 ${p.locked ? 'locked' : ''} ${active ? 'active' : ''} ${p.playerId === me ? 'me' : ''}`} style={{ '--c': c?.palette.primary ?? '#334', '--i': i } as Record<string, string | number>}>
            <div class="art">{showId ? <ChampionPortrait id={showId} size="100%" class={p.locked ? '' : 'ghost'} /> : <span class="q">?</span>}</div>
            <div class="info">
              <small>{names.get(p.playerId) ?? 'Invocateur'}</small>
              <b>{c ? c.name : active ? 'Choisit…' : '—'}</b>
              {c && <span>{CLASS_LABEL[c.class]}</span>}
            </div>
            {ally && (
              <div class="sums">
                <SummonerIcon id={p.summoners[0]} size={18} />
                <SummonerIcon id={p.summoners[1]} size={18} />
              </div>
            )}
            {active && <i class="sweep" />}
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
    <div class="lov-sum-picker2">
      <div class="slots">
        {(['D', 'F'] as const).map((k, i) => (
          <button key={k} class={`slot ${slot === i ? 'on' : ''}`} onClick={() => setSlot(i as 0 | 1)} {...tip(() => <SummonerCard id={cur[i]!} />)}>
            <SummonerIcon id={cur[i]!} size={46} />
            <kbd>{k}</kbd>
          </button>
        ))}
      </div>
      <div class="opts">
        {SUMMONER_IDS.map((id) => (
          <button key={id} class={`opt ${cur.includes(id) ? 'used' : ''}`} onClick={() => choose(id)} {...tip(() => <SummonerCard id={id} />)}>
            <SummonerIcon id={id} size={34} />
            <span>{SUMMONERS[id].name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

const STAT_BARS: { label: string; get: (c: ChampionDef) => number; max: number }[] = [
  { label: 'Dégâts', get: (c) => c.baseStats.ad + (c.perLevel.ad ?? 0) * 10, max: 120 },
  { label: 'Robustesse', get: (c) => c.baseStats.maxHp / 10 + c.baseStats.armor + (c.perLevel.maxHp ?? 0) / 3, max: 160 },
  { label: 'Portée', get: (c) => c.baseStats.attackRange, max: 650 },
  { label: 'Mobilité', get: (c) => c.baseStats.moveSpeed - 300 + Object.values(c.abilities).filter((a) => a.targeting === 'dash' || a.targeting === 'targetedDash').length * 25, max: 100 },
];

function ChampionInfo({ champ }: { champ: ChampionDef }) {
  const slots = ['P', 'Q', 'W', 'E', 'R'] as const;
  const [focus, setFocus] = useState<(typeof slots)[number]>('P');
  const ab = focus === 'P' ? champ.passive : champ.abilities[focus];
  return (
    <div class="lov-draft2-info">
      <div class="tags">
        {champ.roles.map((r) => (
          <span class="lov-tag" key={r}>
            <RoleIcon role={r} size={13} /> {ROLE_LABEL[r]}
          </span>
        ))}
        <span class="lov-tag">{champ.ranged ? 'Distance' : 'Mêlée'}</span>
        <span class="lov-tag diff" title="Difficulté">
          {'◆'.repeat(champ.difficulty)}
          <i>{'◆'.repeat(3 - champ.difficulty)}</i>
        </span>
      </div>
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
              <AbilityIcon ability={a} championId={champ.id} size={42} />
              <kbd>{s === 'P' ? '•' : s}</kbd>
            </button>
          );
        })}
      </div>
      <AbilityCard ability={ab} championId={champ.id} />
      <p class="lov-lore">{champ.lore}</p>
    </div>
  );
}
