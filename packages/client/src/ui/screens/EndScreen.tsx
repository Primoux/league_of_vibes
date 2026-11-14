import { CHAMPIONS, ITEMS, type GameOver, type Team } from '@lov/shared';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { itemTip, tip } from '../components.tsx';
import { ChampionPortrait, Glyph, ItemIcon } from '../icons/Icons.tsx';
import { formatClock, formatGoldShort, formatInt, formatKdaRatio } from '../logic/format.ts';
import { mvpScore } from '../logic/game.ts';
import { RiftBackground } from '../stage/RiftBackground.tsx';
import { useSelect } from '../useStore.ts';

export function EndScreen() {
  const { result, game, me, lobby } = useSelect((s) => ({ result: s.result!, game: s.game, me: s.playerId, lobby: s.lobby }));
  const myTeam: Team = game?.team ?? result.stats.find((r) => r.playerId === me)?.team ?? 'blue';
  const won = result.winner === myTeam;
  const names = new Map<string, string>([...(lobby?.players.map((p) => [p.id, p.name] as const) ?? []), ...(game?.players.map((p) => [p.id, p.name] as const) ?? [])]);
  const maxDmg = Math.max(1, ...result.stats.map((r) => r.damageToChampions));
  const mvp = [...result.stats].sort((a, b) => mvpScore(b) - mvpScore(a))[0];

  const back = () => {
    net.send({ t: 'returnToLobby' });
    store.set({ game: null, snapshot: null, result: null, draft: null });
  };
  const leave = () => {
    net.send({ t: 'leaveRoom' });
    store.set({ game: null, snapshot: null, result: null, draft: null, lobby: null });
  };

  return (
    <div class={`lov-screen lov-end ${won ? 'win' : 'lose'}`}>
      <RiftBackground tint={won ? '#f6dc94' : '#ff5a5a'} intensity={1.2} />
      <div class="lov-end-burst" />
      <h1 class="lov-end-title">{won ? 'Victoire' : 'Défaite'}</h1>
      <p class="lov-end-sub">
        {won ? 'Le nexus ennemi s’est effondré.' : 'Votre nexus a été détruit.'} · Durée {formatClock(result.duration)}
      </p>
      <div class="lov-end-tables">
        {(['blue', 'red'] as const).map((team) => {
          const rows = result.stats.filter((r) => r.team === team);
          const kills = rows.reduce((s, r) => s + r.kills, 0);
          const gold = rows.reduce((s, r) => s + r.gold, 0);
          return (
            <section key={team} class={`lov-end-team ${team} ${result.winner === team ? 'winner' : ''}`}>
              <header>
                <h2>
                  {team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'} {result.winner === team && <Glyph name="crown" size={18} />}
                </h2>
                <span>
                  {kills} éliminations · {formatGoldShort(gold)} or
                </span>
              </header>
              <table>
                <thead>
                  <tr>
                    <th>Champion</th>
                    <th>K / D / A</th>
                    <th>CS</th>
                    <th>Or</th>
                    <th class="dmg">Dégâts aux champions</th>
                    <th>Objets</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r) => (
                    <EndRow key={r.playerId} r={r} name={names.get(r.playerId) ?? 'Invocateur'} me={r.playerId === me} mvp={r === mvp} maxDmg={maxDmg} />
                  ))}
                </tbody>
              </table>
            </section>
          );
        })}
      </div>
      <div class="lov-end-actions">
        <button class="lov-btn ghost big" onClick={leave}>
          Quitter la salle
        </button>
        <button class="lov-btn primary big glow" onClick={back}>
          Retour au salon
        </button>
      </div>
    </div>
  );
}

function EndRow({ r, name, me, mvp, maxDmg }: { r: GameOver['stats'][number]; name: string; me: boolean; mvp: boolean; maxDmg: number }) {
  return (
    <tr class={me ? 'me' : ''}>
      <td class="who">
        <ChampionPortrait id={r.championId} size={36} variant="round" />
        <div>
          <b>
            {name} {mvp && <span class="lov-badge mvp">MVP</span>}
          </b>
          <small>{CHAMPIONS[r.championId]?.name ?? r.championId}</small>
        </div>
      </td>
      <td class="kda">
        {r.kills} / <span class="d">{r.deaths}</span> / {r.assists}
        <small>{formatKdaRatio(r.kills, r.deaths, r.assists)}</small>
      </td>
      <td>{r.cs}</td>
      <td class="gold">{formatGoldShort(r.gold)}</td>
      <td class="dmg">
        <div class="lov-dmg-bar">
          <i style={{ width: `${(r.damageToChampions / maxDmg) * 100}%` }} />
          <span>{formatInt(r.damageToChampions)}</span>
        </div>
      </td>
      <td class="items">
        {r.items.slice(0, 7).map((id, i) => (
          <span key={i} {...(id && ITEMS[id] ? tip(itemTip(id)) : {})}>
            <ItemIcon itemId={id} size={26} />
          </span>
        ))}
      </td>
    </tr>
  );
}
