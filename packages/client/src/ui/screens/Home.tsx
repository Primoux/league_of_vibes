import { CHAMPION_LIST } from '@lov/shared';
import { useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { ChampionPortrait } from '../icons/Icons.tsx';
import { useSelect } from '../useStore.ts';

export function Home() {
  const s = useSelect((st) => ({ name: st.name, connected: st.connected, error: st.error }));
  const [name, setName] = useState(s.name);
  const [code, setCode] = useState('');
  const [dev, setDev] = useState(new URLSearchParams(location.search).has('dev'));
  const [busy, setBusy] = useState(false);

  const go = (fn: () => void) => {
    const n = name.trim() || 'Invocateur';
    try {
      localStorage.setItem('lov.name', n);
    } catch {
      /* stockage indisponible */
    }
    store.set({ name: n, error: null });
    if (!store.state.connected) net.connect(n);
    setBusy(true);
    setTimeout(() => setBusy(false), 2500);
    fn();
  };
  const join = () => code.length === 5 && go(() => net.send({ t: 'joinRoom', code }));

  return (
    <div class="lov-screen lov-home">
      <div class="lov-aurora" />
      <div class="lov-home-parade" aria-hidden="true">
        {CHAMPION_LIST.map((c, i) => (
          <div class="lov-home-parade-card" style={{ '--i': i } as Record<string, number>} key={c.id}>
            <ChampionPortrait id={c.id} variant="splash" size="100%" />
          </div>
        ))}
      </div>
      <div class="lov-home-panel lov-frame">
        <div class="lov-logo">
          <span class="lov-logo-top">League of</span>
          <span class="lov-logo-main">Vibes</span>
          <span class="lov-logo-sub">Faille des Vibes · 4 contre 4</span>
        </div>
        <label class="lov-field">
          <span>Nom d’invocateur</span>
          <input
            class="lov-input"
            placeholder="Ton pseudo"
            value={name}
            maxLength={16}
            autoFocus
            onInput={(e) => setName(e.currentTarget.value)}
            onKeyDown={(e) => e.key === 'Enter' && go(() => net.send({ t: 'createRoom', devMode: dev }))}
          />
        </label>
        <button class="lov-btn primary big" disabled={busy} onClick={() => go(() => net.send({ t: 'createRoom', devMode: dev }))}>
          Créer une salle
        </button>
        <div class="lov-sep">
          <span>ou rejoindre</span>
        </div>
        <div class="lov-join">
          <input
            class="lov-input code"
            placeholder="CODE"
            value={code}
            maxLength={5}
            onInput={(e) => setCode(e.currentTarget.value.toUpperCase().replace(/[^A-Z0-9]/g, ''))}
            onKeyDown={(e) => e.key === 'Enter' && join()}
          />
          <button class="lov-btn" disabled={code.length !== 5 || busy} onClick={join}>
            Rejoindre
          </button>
        </div>
        <label class="lov-check">
          <input type="checkbox" checked={dev} onChange={(e) => setDev(e.currentTarget.checked)} />
          <span>Mode dev (partie à moins de 8 joueurs)</span>
        </label>
        {s.error && <p class="lov-error">{s.error}</p>}
        <div class={`lov-conn ${s.connected ? 'on' : ''}`}>{s.connected ? 'Connecté au serveur' : 'Hors ligne'}</div>
      </div>
      <footer class="lov-home-foot">Un MOBA fait aux vibes · 10 champions · 3 voies · 1 Léviathan</footer>
    </div>
  );
}
