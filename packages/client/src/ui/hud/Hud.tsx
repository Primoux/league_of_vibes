import { Glyph } from '../icons/Icons.tsx';
import { store } from '../../state.ts';
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
