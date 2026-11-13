/**
 * Racine UI (Preact, overlay DOM au-dessus du canvas). Propriétaire : mistral-ui.
 * BOOTSTRAP : écran d'accueil, lobby minimal, HUD minimal. Tout est à refaire proprement (voir docs/plans/mistral-ui.md).
 */
import { useState } from 'preact/hooks';
import { net } from '../net/client.ts';
import { useStore } from './useStore.ts';

export function App() {
  const s = useStore();
  if (s.game) return <Hud />;
  if (s.lobby) return <Lobby />;
  return <Home />;
}

function Home() {
  const s = useStore();
  const [name, setName] = useState(s.name);
  const [code, setCode] = useState('');
  const dev = new URLSearchParams(location.search).has('dev');
  const go = (fn: () => void) => {
    try {
      localStorage.setItem('lov.name', name);
    } catch {
      /* ignore */
    }
    if (!s.connected) net.connect(name || 'Invocateur');
    fn();
  };
  return (
    <div class="panel center">
      <h1>League of Vibes</h1>
      <input placeholder="Nom d'invocateur" value={name} onInput={(e) => setName(e.currentTarget.value)} maxLength={16} />
      <button onClick={() => go(() => net.send({ t: 'createRoom', devMode: dev }))}>Créer une salle{dev ? ' (dev)' : ''}</button>
      <div class="row">
        <input placeholder="CODE" value={code} onInput={(e) => setCode(e.currentTarget.value.toUpperCase())} maxLength={5} />
        <button onClick={() => go(() => net.send({ t: 'joinRoom', code }))}>Rejoindre</button>
      </div>
      {s.error && <p class="error">{s.error}</p>}
    </div>
  );
}

function Lobby() {
  const s = useStore();
  const lobby = s.lobby!;
  const me = lobby.players.find((p) => p.id === s.playerId);
  return (
    <div class="panel center">
      <h2>Salle {lobby.code}{lobby.devMode ? ' — mode dev' : ''}</h2>
      <div class="teams">
        {(['blue', 'red'] as const).map((team) => (
          <div class={`team ${team}`} key={team}>
            <h3>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</h3>
            {lobby.players.filter((p) => p.team === team).map((p) => (
              <div key={p.id}>
                {p.isHost ? '★ ' : ''}{p.name} {p.ready ? '✔' : ''}
              </div>
            ))}
            {me?.team !== team && <button onClick={() => net.send({ t: 'setTeam', team })}>Rejoindre</button>}
          </div>
        ))}
      </div>
      <div class="row">
        <button onClick={() => net.send({ t: 'setReady', ready: !me?.ready })}>{me?.ready ? 'Pas prêt' : 'Prêt'}</button>
        {me?.isHost && <button onClick={() => net.send({ t: 'startGame' })}>Lancer</button>}
      </div>
      {s.error && <p class="error">{s.error}</p>}
    </div>
  );
}

function Hud() {
  const s = useStore();
  const snap = s.snapshot;
  const t = snap?.time ?? 0;
  return (
    <div class="hud">
      <div>
        {Math.floor(t / 60)}:{String(Math.floor(t % 60)).padStart(2, '0')} — Or : {snap?.self?.gold ?? 0}
      </div>
      <div class="hint">Clic droit : se déplacer · QWER : sorts · D/F : invocateur</div>
    </div>
  );
}
