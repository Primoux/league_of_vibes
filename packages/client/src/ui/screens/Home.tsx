import { CHAMPION_LIST } from '@lov/shared';
import { useEffect, useState } from 'preact/hooks';
import { net } from '../../net/client.ts';
import { store } from '../../state.ts';
import { ChampionPortrait, ClassEmblem, RoleIcon } from '../icons/Icons.tsx';
import { CLASS_LABEL, ROLE_LABEL } from '../logic/format.ts';
import { ChampionStage } from '../stage/ChampionStage.tsx';
import { RiftBackground } from '../stage/RiftBackground.tsx';
import { useSelect } from '../useStore.ts';

const FEATURE_MS = 7000;

export function Home() {
  const s = useSelect((st) => ({ name: st.name, connected: st.connected, error: st.error }));
  const [name, setName] = useState(s.name);
  const [code, setCode] = useState('');
  const [mode, setMode] = useState<'create' | 'join'>('create');
  const [dev, setDev] = useState(new URLSearchParams(location.search).has('dev'));
  const [busy, setBusy] = useState(false);
  const [featured, setFeatured] = useState(() => Math.floor(Math.random() * CHAMPION_LIST.length));
  const [paused, setPaused] = useState(false);
  const [pulse, setPulse] = useState(0);

  useEffect(() => {
    if (paused) return;
    const id = setInterval(() => setFeatured((f) => (f + 1) % CHAMPION_LIST.length), FEATURE_MS);
    return () => clearInterval(id);
  }, [paused]);

  const champ = CHAMPION_LIST[featured]!;

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
  const create = () => go(() => net.send({ t: 'createRoom', devMode: dev }));
  const join = () => code.length === 5 && go(() => net.send({ t: 'joinRoom', code }));
  const pick = (i: number) => {
    setFeatured(i);
    setPaused(true);
    setPulse((p) => p + 1);
  };

  return (
    <div class="lov-screen lov-home2" style={{ '--champ': champ.palette.primary, '--champ2': champ.palette.accent } as Record<string, string>}>
      <RiftBackground tint={champ.palette.primary} />
      <div class="lov-home2-glow" />

      <section class="lov-home2-left">
        <div class="lov-logo2">
          <span class="lov-logo2-top">League of</span>
          <span class="lov-logo2-main" data-text="Vibes">
            Vibes
          </span>
          <span class="lov-logo2-sub">
            <i />
            La Faille des Vibes · 4 contre 4
            <i />
          </span>
        </div>

        <div class="lov-play-card">
          <div class="lov-play-tabs" role="tablist">
            <button role="tab" class={mode === 'create' ? 'on' : ''} onClick={() => setMode('create')}>
              Créer une partie
            </button>
            <button role="tab" class={mode === 'join' ? 'on' : ''} onClick={() => setMode('join')}>
              Rejoindre
            </button>
            <span class="lov-play-tabs-ink" style={{ transform: `translateX(${mode === 'create' ? 0 : 100}%)` }} />
          </div>

          <label class="lov-field2">
            <span class="lov-avatar2">{(name.trim() || '?').charAt(0).toUpperCase()}</span>
            <input
              class="lov-input2"
              placeholder="Ton nom d’invocateur"
              value={name}
              maxLength={16}
              autoFocus
              onInput={(e) => setName(e.currentTarget.value)}
              onKeyDown={(e) => e.key === 'Enter' && (mode === 'create' ? create() : join())}
            />
          </label>

          {mode === 'create' ? (
            <>
              <button class="lov-cta" disabled={busy} onClick={create}>
                <span class="lov-cta-shine" />
                <span class="lov-cta-label">Jouer</span>
                <small>Créer une salle et inviter tes potes</small>
              </button>
              <label class="lov-check">
                <input type="checkbox" checked={dev} onChange={(e) => setDev(e.currentTarget.checked)} />
                <span>Mode entraînement (moins de 8 joueurs)</span>
              </label>
            </>
          ) : (
            <>
              <div class="lov-code-boxes" onClick={(e) => (e.currentTarget.querySelector('input') as HTMLInputElement | null)?.focus()}>
                {Array.from({ length: 5 }, (_, i) => (
                  <span key={i} class={`${code[i] ? 'on' : ''} ${i === code.length ? 'cursor' : ''}`}>
                    {code[i] ?? ''}
                  </span>
                ))}
                <input
                  aria-label="Code de la salle"
                  value={code}
                  maxLength={5}
                  onInput={(e) => setCode(e.currentTarget.value.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 5))}
                  onKeyDown={(e) => e.key === 'Enter' && join()}
                />
              </div>
              <button class="lov-cta" disabled={code.length !== 5 || busy} onClick={join}>
                <span class="lov-cta-shine" />
                <span class="lov-cta-label">Rejoindre</span>
                <small>{code.length === 5 ? `Salle ${code}` : 'Entre le code à 5 caractères'}</small>
              </button>
            </>
          )}

          {s.error && <p class="lov-error">{s.error}</p>}
          <div class={`lov-conn ${s.connected ? 'on' : ''}`}>{s.connected ? 'Connecté au serveur' : 'Prêt à se connecter'}</div>
        </div>
      </section>

      <section class="lov-home2-right" onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)}>
        <div class="lov-showcase">
          <ChampionStage championId={champ.id} pulse={pulse} class="lov-showcase-stage" spin={0.3} />
          <div class="lov-showcase-plate" key={champ.id}>
            <span class="cls">
              <ClassEmblem cls={champ.class} size={14} /> {CLASS_LABEL[champ.class]}
            </span>
            <h2>{champ.name}</h2>
            <p>{champ.title}</p>
            <div class="roles">
              {champ.roles.map((r) => (
                <span key={r}>
                  <RoleIcon role={r} size={13} /> {ROLE_LABEL[r]}
                </span>
              ))}
            </div>
          </div>
          <button class="lov-showcase-arrow left" onClick={() => pick((featured + CHAMPION_LIST.length - 1) % CHAMPION_LIST.length)} aria-label="Champion précédent">
            ‹
          </button>
          <button class="lov-showcase-arrow right" onClick={() => pick((featured + 1) % CHAMPION_LIST.length)} aria-label="Champion suivant">
            ›
          </button>
        </div>
        <div class="lov-roster">
          {CHAMPION_LIST.map((c, i) => (
            <button key={c.id} class={`lov-roster-chip ${i === featured ? 'on' : ''}`} onClick={() => pick(i)} title={`${c.name}, ${c.title}`} style={{ '--c': c.palette.primary } as Record<string, string>}>
              <ChampionPortrait id={c.id} size="100%" variant="round" />
            </button>
          ))}
        </div>
      </section>

      <footer class="lov-home2-foot">
        <span>11 champions</span>
        <i />
        <span>3 voies</span>
        <i />
        <span>Drakes · Colosse · Léviathan</span>
        <i />
        <span>Fait aux vibes, ce soir</span>
      </footer>
    </div>
  );
}
