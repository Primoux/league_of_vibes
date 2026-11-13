import { useEffect } from 'preact/hooks';
import { Glyph } from '../icons/Icons.tsx';
import { ui } from '../game/world.ts';
import { BottomBar } from './BottomBar.tsx';
import { Minimap } from './Minimap.tsx';
import { Announcer, DeathOverlay, KillFeed, SelfStatsStrip, TeamFrames, TopBar, useTabTitle } from './Overlays.tsx';
import { Scoreboard, SettingsPanel, StatsPanel } from './Panels.tsx';
import { Shop } from './Shop.tsx';

/**
 * Raccourcis propres à l'UI (P, Tab, C, Échap). claude-client gère le reste des contrôles.
 * Si claude-client finit par transmettre ces touches via le store, ce handler pourra être retiré.
 */
function useHudKeys(): void {
  useEffect(() => {
    const isTyping = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      return !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA');
    };
    const down = (e: KeyboardEvent) => {
      if (e.key === 'Tab') {
        e.preventDefault();
        if (!ui.state.scoreboard) ui.set({ scoreboard: true });
        return;
      }
      if (isTyping(e) || e.ctrlKey || e.metaKey) return;
      const k = e.key.toLowerCase();
      if (k === 'p') ui.set({ shopOpen: !ui.state.shopOpen });
      else if (k === 'c') ui.set({ stats: !ui.state.stats });
      else if (e.key === 'Escape') {
        if (ui.state.shopOpen) ui.set({ shopOpen: false });
        else if (ui.state.settings) ui.set({ settings: false });
        else ui.set({ settings: true });
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === 'Tab') ui.set({ scoreboard: false });
    };
    const blur = () => ui.state.scoreboard && ui.set({ scoreboard: false });
    addEventListener('keydown', down);
    addEventListener('keyup', up);
    addEventListener('blur', blur);
    return () => {
      removeEventListener('keydown', down);
      removeEventListener('keyup', up);
      removeEventListener('blur', blur);
    };
  }, []);
}

export function Hud() {
  useHudKeys();
  useTabTitle();
  return (
    <div class="lov-hud">
      <TopBar />
      <div class="lov-topright">
        <SelfStatsStrip />
        <button class="lov-icon-btn" onClick={() => ui.set({ settings: true })} title="Paramètres (Échap)">
          <Glyph name="gear" size={16} />
        </button>
      </div>
      <KillFeed />
      <TeamFrames />
      <Announcer />
      <DeathOverlay />
      <StatsPanel />
      <BottomBar />
      <Minimap />
      <Scoreboard />
      <Shop />
      <SettingsPanel />
    </div>
  );
}
