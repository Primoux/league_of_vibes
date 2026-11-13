import { TEAM_SIZE, type LobbyPlayer, type Role, type Team } from '@lov/shared';
import { useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { Glyph, RoleIcon } from '../icons/Icons.tsx';
import { ROLE_LABEL } from '../logic/format.ts';
import { useSelect } from '../useStore.ts';

const ROLES: Role[] = ['top', 'jungle', 'mid', 'bot'];

export function Lobby() {
  const { lobby, me, error } = useSelect((s) => ({ lobby: s.lobby!, me: s.playerId, error: s.error }));
  const [copied, setCopied] = useState(false);
  const self = lobby.players.find((p) => p.id === me);
  const allReady = lobby.players.length > 0 && lobby.players.every((p) => p.ready || !p.connected);
  const full = lobby.players.length === TEAM_SIZE * 2;
  const canStart = !!self?.isHost && allReady && (full || lobby.devMode);

  const copy = () => {
    const done = () => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    };
    try {
      void navigator.clipboard.writeText(lobby.code).then(done, done);
    } catch {
      done();
    }
  };
  const leave = () => {
    net.send({ t: 'leaveRoom' });
    store.set({ lobby: null, draft: null });
  };

  return (
    <div class="lov-screen lov-lobby">
      <div class="lov-aurora dim" />
      <header class="lov-lobby-head">
        <div>
          <h1 class="lov-title">Salle d’attente</h1>
          <p class="lov-muted">
            {lobby.players.length}/{TEAM_SIZE * 2} invocateurs · choisis ton équipe et ton rôle
            {lobby.devMode && <span class="lov-badge dev">MODE DEV</span>}
          </p>
        </div>
        <button class="lov-room-code" onClick={copy} title="Copier le code">
          <small>Code de la salle</small>
          <b>{lobby.code}</b>
          <span>
            <Glyph name={copied ? 'check' : 'copy'} size={14} /> {copied ? 'Copié !' : 'Copier'}
          </span>
        </button>
      </header>

      <div class="lov-lobby-teams">
        {(['blue', 'red'] as const).map((team) => (
          <TeamColumn key={team} team={team} players={lobby.players.filter((p) => p.team === team)} me={me} />
        ))}
        <div class="lov-lobby-vs">VS</div>
      </div>

      <footer class="lov-lobby-foot">
        <button class="lov-btn ghost" onClick={leave}>
          Quitter
        </button>
        <div class="lov-lobby-roles">
          <span class="lov-muted">Rôle préféré</span>
          {ROLES.map((r) => (
            <button key={r} class={`lov-role-btn ${self?.role === r ? 'on' : ''}`} onClick={() => net.send({ t: 'setRole', role: r })} title={ROLE_LABEL[r]}>
              <RoleIcon role={r} size={20} />
              <span>{ROLE_LABEL[r]}</span>
            </button>
          ))}
        </div>
        <button class={`lov-btn ${self?.ready ? 'ready' : 'primary'} big`} onClick={() => net.send({ t: 'setReady', ready: !self?.ready })}>
          {self?.ready ? 'Prêt ✓' : 'Je suis prêt'}
        </button>
        {self?.isHost && (
          <button class="lov-btn primary big glow" disabled={!canStart} onClick={() => net.send({ t: 'startGame' })} title={canStart ? '' : lobby.devMode ? 'Tous les joueurs doivent être prêts' : '8 joueurs prêts requis'}>
            Lancer la partie
          </button>
        )}
      </footer>
      {error && <p class="lov-error center">{error}</p>}
    </div>
  );
}

function TeamColumn({ team, players, me }: { team: Team; players: LobbyPlayer[]; me: string | null }) {
  const mine = players.some((p) => p.id === me);
  const slots: (LobbyPlayer | null)[] = [...players, ...Array<null>(Math.max(0, TEAM_SIZE - players.length)).fill(null)];
  return (
    <section class={`lov-team-col ${team}`}>
      <h2>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</h2>
      {slots.map((p, i) =>
        p ? (
          <div key={p.id} class={`lov-slot ${p.id === me ? 'me' : ''} ${p.ready ? 'ready' : ''} ${p.connected ? '' : 'offline'}`}>
            <div class="lov-avatar">{p.name.charAt(0).toUpperCase()}</div>
            <div class="lov-slot-name">
              <b>
                {p.name}
                {p.isHost && <Glyph name="crown" size={14} class="host" title="Hôte" />}
              </b>
              <span>{p.role ? ROLE_LABEL[p.role] : 'Rôle libre'}</span>
            </div>
            {p.role && <RoleIcon role={p.role} size={22} />}
            <span class="lov-ready-dot" title={p.ready ? 'Prêt' : 'Pas prêt'} />
          </div>
        ) : (
          <button key={`empty-${i}`} class="lov-slot empty" disabled={mine} onClick={() => net.send({ t: 'setTeam', team })}>
            {mine ? 'Place libre' : '+ Rejoindre cette équipe'}
          </button>
        ),
      )}
    </section>
  );
}
