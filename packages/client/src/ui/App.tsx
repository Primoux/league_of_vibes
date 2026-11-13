/**
 * Racine UI (Preact, overlay DOM au-dessus du canvas). Propriétaire : mistral-ui.
 * Routage par état : fin > partie > chargement > draft > lobby > accueil. `?uilab` : démo sans serveur.
 */
import { lazy, Suspense } from 'preact/compat';
import { TooltipLayer } from './components.tsx';
import { installUiWorld } from './game/world.ts';
import { Hud } from './hud/Hud.tsx';
import { Draft } from './screens/Draft.tsx';
import { EndScreen } from './screens/EndScreen.tsx';
import { Home } from './screens/Home.tsx';
import { Loading } from './screens/Loading.tsx';
import { Lobby } from './screens/Lobby.tsx';
import { installSettings } from './settings.ts';
import { useSelect } from './useStore.ts';
import './hud.css';
import './screens.css';

installUiWorld();
installSettings();

const UiLab = lazy(() => import('./lab/UiLab.tsx').then((m) => ({ default: m.UiLab })));
const LAB = typeof location !== 'undefined' && new URLSearchParams(location.search).has('uilab');

export type Screen = 'home' | 'lobby' | 'draft' | 'loading' | 'game' | 'end';

export function App() {
  return (
    <>
      <Router />
      <TooltipLayer />
      {LAB && (
        <Suspense fallback={null}>
          <UiLab />
        </Suspense>
      )}
    </>
  );
}

function Router() {
  const s = useSelect((st) => ({
    result: !!st.result,
    game: !!st.game,
    phase: st.lobby?.phase,
    lobby: !!st.lobby,
    draft: !!st.draft,
    loadingEndsAt: st.loadingEndsAt,
  }));
  const loadingEndsAt = s.loadingEndsAt;
  if (s.result) return <EndScreen />;
  if (s.game) return <Hud />;
  if (s.phase === 'loading' || s.phase === 'game' || (loadingEndsAt && s.draft)) return <Loading />;
  if (s.draft && s.phase !== 'lobby') return <Draft />;
  if (s.lobby) return <Lobby />;
  return <Home />;
}
