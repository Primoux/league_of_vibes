import { Glyph } from '../icons/Icons.tsx';
import { fullscreenSupported, keyboardLockSupported, toggleGameMode } from '../../input/gameMode.ts';
import { store } from '../../state.ts';
import { useSelect } from '../useStore.ts';
import { BottomBar } from './BottomBar.tsx';
import { Minimap } from './Minimap.tsx';
import { Chat } from './Chat.tsx';
import { Announcer, DeathOverlay, KillFeed, LowHpVignette, SelfStatsStrip, TeamFrames, TopBar, useTabTitle } from './Overlays.tsx';
import { Scoreboard, SettingsPanel, StatsPanel } from './Panels.tsx';
import { Shop } from './Shop.tsx';

export function Hud() {
  useTabTitle();
  return (
    <div class="lov-hud">
      <LowHpVignette />
      <TopBar />
      <div class="lov-topright">
        <SelfStatsStrip />
        <GameModeButton />
        <button class="lov-icon-btn" onClick={() => store.set({ settingsOpen: true })} title="Paramètres (Échap)">
          <Glyph name="gear" size={16} />
        </button>
      </div>
      <KillFeed />
      <TeamFrames />
      <Announcer />
      <DeathOverlay />
      <StatsPanel />
      <Chat />
      <BottomBar />
      <Minimap />
      <Scoreboard />
      <Shop />
      <SettingsPanel />
    </div>
  );
}

/** Bouton plein écran (mode jeu) ; rappelle F11 et, sans Keyboard Lock, que Ctrl+W ferme l'onglet. */
function GameModeButton() {
  const fs = useSelect((s) => s.fullscreen);
  if (!fullscreenSupported()) return null;
  const title = fs
    ? 'Quitter le mode jeu (F11)'
    : 'Mode jeu : plein écran (F11)' + (keyboardLockSupported() ? '' : ' — sur ce navigateur, Ctrl+W reste réservé : monte tes sorts avec Alt+QWER');
  return (
    <button class={`lov-icon-btn lov-gamemode-btn${fs ? '' : ' off'}`} onClick={toggleGameMode} title={title}>
      <svg viewBox="0 0 24 24" width={16} height={16} aria-hidden="true">
        <path
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          d={fs ? 'M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5' : 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5'}
        />
      </svg>
    </button>
  );
}
