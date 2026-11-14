import { TEAM_SIZE, type LobbyPlayer, type Role, type Team } from '@lov/shared';
import { useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { Glyph, RoleIcon } from '../icons/Icons.tsx';
import { ROLE_LABEL } from '../logic/format.ts';
import { RiftBackground } from '../stage/RiftBackground.tsx';
import { useSelect } from '../useStore.ts';

const ROLES: Role[] = ['top', 'jungle', 'mid', 'bot'];
const ROLE_HINT: Record<Role, string> = { top: 'Duelliste solitaire', jungle: 'Maître des camps', mid: 'Cœur de la Faille', bot: 'Tireur ou soutien' };

export function Lobby() {
  const { lobby, me, error } = useSelect((s) => ({ lobby: s.lobby!, me: s.playerId, error: s.error }));
  const [copied, setCopied] = useState(false);
  const self = lobby.players.find((p) => p.id === me);
  const readyCount = lobby.players.filter((p) => p.ready).length;
  const allReady = lobby.players.length > 0 && lobby.players.every((p) => p.ready || !p.connected);
  const full = lobby.players.length === TEAM_SIZE * 2;
  const canStart = !!self?.isHost && allReady && (full || lobby.devMode);

  const copy = () => {
    const done = () => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
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

  let status: string;
  if (canStart) status = 'Tout le monde est prêt !';
  else if (!full && !lobby.devMode) status = `En attente de joueurs… ${lobby.players.length}/${TEAM_SIZE * 2}`;
  else if (!allReady) status = `${readyCount}/${lobby.players.length} joueurs prêts`;
  else status = 'En attente de l’hôte';

  return (
    <div class="lov-screen lov-lobby2">
      <RiftBackground tint="#4ea2ff" intensity={0.7} />
      <div class="lov-lobby2-sides" aria-hidden="true">
        <i class="blue" />
        <i class="red" />
      </div>

      <header class="lov-lobby2-head">
        <div class="lov-lobby2-title">
          <small>Salon de partie {lobby.devMode && <span class="lov-badge dev">ENTRAÎNEMENT</span>}</small>
          <h1>La Faille t’attend</h1>
        </div>
        <button class={`lov-seal ${copied ? 'copied' : ''}`} onClick={copy} title="Copier le code pour inviter tes potes">
          <svg viewBox="0 0 200 64" class="lov-seal-frame" aria-hidden="true">
            <path d="M12 2 H188 L198 32 L188 62 H12 L2 32 Z" />
          </svg>
          <small>Code d’invitation</small>
          <b>
            {lobby.code.split('').map((ch, i) => (
              <span key={i} style={{ '--i': i } as Record<string, number>}>
                {ch}
              </span>
            ))}
          </b>
          <em>
            <Glyph name={copied ? 'check' : 'copy'} size={12} /> {copied ? 'Copié !' : 'Cliquer pour copier'}
          </em>
        </button>
        <button class="lov-btn ghost" onClick={leave}>
          Quitter
        </button>
      </header>

      <main class="lov-lobby2-arena">
        <TeamBanners team="blue" players={lobby.players.filter((p) => p.team === 'blue')} me={me} />
        <div class="lov-vs">
          <svg viewBox="0 0 120 120" aria-hidden="true">
            <circle cx="60" cy="60" r="56" class="r1" />
            <circle cx="60" cy="60" r="46" class="r2" />
            <path d="M60 8 L64 56 L112 60 L64 64 L60 112 L56 64 L8 60 L56 56 Z" class="star" />
          </svg>
          <b>VS</b>
        </div>
        <TeamBanners team="red" players={lobby.players.filter((p) => p.team === 'red')} me={me} />
      </main>

      <footer class="lov-lobby2-foot">
        <div class="lov-role-pick">
          <small>Ton rôle préféré</small>
          <div>
            {ROLES.map((r) => (
              <button key={r} class={`lov-role-tile ${self?.role === r ? 'on' : ''}`} onClick={() => net.send({ t: 'setRole', role: r })}>
                <RoleIcon role={r} size={26} />
                <b>{ROLE_LABEL[r]}</b>
                <span>{ROLE_HINT[r]}</span>
              </button>
            ))}
          </div>
        </div>

        <div class="lov-ready-zone">
          <div class="lov-ready-meter" title={status}>
            {Array.from({ length: TEAM_SIZE * 2 }, (_, i) => {
              const p = lobby.players[i];
              return <i key={i} class={p ? (p.ready ? 'ready' : 'here') : ''} />;
            })}
          </div>
          <p class={canStart ? 'go' : ''}>{status}</p>
          <div class="lov-ready-actions">
            <button class={`lov-ready-btn ${self?.ready ? 'on' : ''}`} onClick={() => net.send({ t: 'setReady', ready: !self?.ready })}>
              <span class="ring" />
              <Glyph name="check" size={22} />
              <b>{self?.ready ? 'Prêt !' : 'Prêt ?'}</b>
            </button>
            {self?.isHost && (
              <button class="lov-cta lov-launch" disabled={!canStart} onClick={() => net.send({ t: 'startGame' })}>
                <span class="lov-cta-shine" />
                <span class="lov-cta-label">Lancer</span>
                <small>{canStart ? 'Direction la sélection des champions' : lobby.devMode ? 'Tous les joueurs doivent être prêts' : '8 joueurs prêts requis'}</small>
              </button>
            )}
          </div>
        </div>
      </footer>
      {error && <p class="lov-error center">{error}</p>}
    </div>
  );
}

function TeamBanners({ team, players, me }: { team: Team; players: LobbyPlayer[]; me: string | null }) {
  const mine = players.some((p) => p.id === me);
  const slots: (LobbyPlayer | null)[] = [...players, ...Array<null>(Math.max(0, TEAM_SIZE - players.length)).fill(null)];
  return (
    <section class={`lov-banners ${team}`}>
      <h2>
        <span>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</span>
        <small>
          {players.length}/{TEAM_SIZE}
        </small>
      </h2>
      <div class="row">
        {slots.map((p, i) =>
          p ? (
            <div key={p.id} class={`lov-banner ${p.id === me ? 'me' : ''} ${p.ready ? 'ready' : ''} ${p.connected ? '' : 'offline'}`} style={{ '--i': i } as Record<string, number>}>
              <div class="lov-banner-cloth">
                <div class="crest">
                  <span class="initial">{p.name.charAt(0).toUpperCase()}</span>
                  {p.role && (
                    <span class="role">
                      <RoleIcon role={p.role} size={16} />
                    </span>
                  )}
                </div>
                <b class="name">
                  {p.isHost && <Glyph name="crown" size={13} class="host" title="Hôte" />}
                  {p.name}
                </b>
                <span class="sub">{p.role ? ROLE_LABEL[p.role] : 'Rôle libre'}</span>
                <span class="state">{!p.connected ? 'Déconnecté' : p.ready ? 'Prêt' : 'En attente'}</span>
              </div>
            </div>
          ) : (
            <button key={`empty-${i}`} class="lov-banner empty" disabled={mine} onClick={() => net.send({ t: 'setTeam', team })} style={{ '--i': i } as Record<string, number>}>
              <div class="lov-banner-cloth">
                <span class="plus">+</span>
                <span class="sub">{mine ? 'Place libre' : 'Rejoindre'}</span>
              </div>
            </button>
          ),
        )}
      </div>
    </section>
  );
}
