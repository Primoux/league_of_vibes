import { CHAMPIONS, ITEMS, type Stats, type Team } from '@lov/shared';
import type { ComponentChildren } from 'preact';
import { useState } from 'preact/hooks';
import { exitGameMode, enterGameMode, fullscreenSupported, keyboardLockSupported } from '../../input/gameMode.ts';
import { DEFAULT_GAME_SETTINGS, store, type Quality } from '../../state.ts';
import { itemTip, tip } from '../components.tsx';
import { champMemo, useWorld } from '../game/world.ts';
import { ChampionPortrait, Glyph, ItemIcon, RoleIcon, SummonerIcon } from '../icons/Icons.tsx';
import { formatGoldShort, formatStat, STAT_LABEL } from '../logic/format.ts';
import { sortScoreRows, type ScoreRow } from '../logic/game.ts';
import { inventoryValue } from '../logic/shop.ts';
import { DEFAULT_SETTINGS, settings, type AudioChannel } from '../settings.ts';
import { useMini, useSelect } from '../useStore.ts';

// ------------------------------------------------------------------ tableau des scores (Tab)
export function Scoreboard() {
  const open = useSelect((s) => s.scoreboardOpen);
  if (!open) return null;
  return <ScoreboardPanel />;
}

function ScoreboardPanel() {
  const rows = useWorld(() => buildRows(), (a, b) => JSON.stringify(a) === JSON.stringify(b));
  const time = store.state.snapshot?.time ?? 0;
  return (
    <div class="lov-scoreboard lov-frame">
      {(['blue', 'red'] as Team[]).map((team) => {
        const tr = sortScoreRows(rows.filter((r) => r.team === team));
        const kills = tr.reduce((s, r) => s + r.kills, 0);
        const value = tr.reduce((s, r) => s + inventoryValue(r.items, ITEMS), 0);
        return (
          <section key={team} class={`lov-sb-team ${team}`}>
            <header>
              <b>{team === 'blue' ? 'Équipe bleue' : 'Équipe rouge'}</b>
              <span>
                <Glyph name="skull" size={13} /> {kills}
              </span>
              <span title="Valeur des objets">
                <Glyph name="gold" size={13} /> {formatGoldShort(value)}
              </span>
            </header>
            {tr.map((r) => (
              <div key={r.playerId} class={`lov-sb-row ${r.isMe ? 'me' : ''} ${r.dead ? 'dead' : ''}`}>
                <div class="pic">
                  <ChampionPortrait id={r.championId} size={40} variant="round" grayscale={r.dead} />
                  <span class="lvl">{r.level}</span>
                  {r.dead && r.respawnIn !== undefined && <span class="respawn">{Math.ceil(r.respawnIn)}</span>}
                </div>
                <div class="sums">
                  <SummonerIcon id={r.summoners[0]} size={18} />
                  <SummonerIcon id={r.summoners[1]} size={18} />
                </div>
                <div class="who">
                  <b>{r.name}</b>
                  <small>
                    {r.role && <RoleIcon role={r.role} size={11} />} {CHAMPIONS[r.championId]?.name ?? r.championId}
                  </small>
                </div>
                <div class="items">
                  {Array.from({ length: 7 }, (_, i) => {
                    const id = r.items[i] ?? null;
                    return (
                      <span key={i} class={i === 6 ? 'trinket' : ''} {...(id && ITEMS[id] ? tip(itemTip(id)) : {})}>
                        <ItemIcon itemId={id} size={28} />
                      </span>
                    );
                  })}
                </div>
                <div class="kda">
                  {r.kills} / <span class="d">{r.deaths}</span> / {r.assists}
                </div>
                <div class="cs">{r.cs}</div>
              </div>
            ))}
          </section>
        );
      })}
      <footer class="lov-muted">Maintiens Tab · {Math.floor(time / 60)} min de jeu</footer>
    </div>
  );
}

function buildRows(): ScoreRow[] {
  const s = store.state;
  const me = s.playerId;
  const roles = new Map(s.lobby?.players.map((p) => [p.id, p.role]) ?? []);
  const players = s.game?.players ?? [];
  return players.map((p) => {
    const m = champMemo.get(p.id);
    return {
      playerId: p.id,
      name: p.name,
      team: p.team,
      championId: p.championId,
      role: roles.get(p.id),
      level: m?.level ?? 1,
      kills: m?.kills ?? 0,
      deaths: m?.deaths ?? 0,
      assists: m?.assists ?? 0,
      cs: m?.cs ?? 0,
      items: m?.items ?? [],
      summoners: p.summoners,
      dead: m?.dead ?? false,
      respawnIn: m?.respawnIn,
      isMe: p.id === me,
    };
  });
}

// ------------------------------------------------------------------ statistiques détaillées (C)
const STAT_ORDER: (keyof Stats)[] = ['ad', 'ap', 'armor', 'mr', 'attackSpeed', 'abilityHaste', 'critChance', 'moveSpeed', 'attackRange', 'lifesteal', 'armorPen', 'magicPen', 'hpRegen', 'manaRegen'];
const STAT_ICON: Partial<Record<keyof Stats, string>> = { ad: 'sword', ap: 'bolt', armor: 'tower', mr: 'inhibitor', attackSpeed: 'bolt', moveSpeed: 'check' };

export function StatsPanel() {
  const open = useSelect((s) => s.statsOpen);
  const stats = useWorld(() => store.state.snapshot?.self?.stats ?? null, (a, b) => a === b);
  if (!open || !stats) return null;
  return (
    <div class="lov-stats lov-frame">
      <h3>Statistiques</h3>
      <ul>
        {STAT_ORDER.map((k) => (
          <li key={k} title={STAT_LABEL[k]}>
            <span class={`k k-${k}`}>
              <Glyph name={STAT_ICON[k] ?? 'nexus'} size={12} />
              {STAT_LABEL[k]}
            </span>
            <b>{formatStat(k, stats[k])}</b>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ------------------------------------------------------------------ paramètres
const CHANNEL_LABEL: Record<AudioChannel, string> = { master: 'Général', sfx: 'Effets', voice: 'Annonces', music: 'Musique', ui: 'Interface' };
const KEYS: [string, string][] = [
  ['Clic droit', 'Se déplacer / attaquer'],
  ['A + clic', 'Attaque-déplacement'],
  ['Q W E R', 'Sorts (maintenir pour viser, relâcher pour lancer)'],
  ['Maj + Q/W/E/R', 'Inverser lancement rapide / normal'],
  ['Alt (ou Ctrl) + Q/W/E/R', 'Monter un sort'],
  ['D / F', 'Sorts d’invocateur'],
  ['1-3, 5-7 / 4', 'Objets / balise'],
  ['B', 'Rappel'],
  ['S', 'Stop'],
  ['P', 'Boutique'],
  ['Tab', 'Tableau des scores (maintenir)'],
  ['C', 'Statistiques'],
  ['² / ` (maintenir)', 'Ne cibler que les champions'],
  ['Clic molette (maintenir)', 'Faire glisser la caméra'],
  ['Espace', 'Centrer la caméra'],
  ['Y', 'Verrouiller la caméra'],
  ['Alt + clic', 'Ping'],
  ['Échap', 'Fermer / paramètres'],
  ['F11', 'Mode jeu (plein écran)'],
  ['Échap (maintenir)', 'Quitter le plein écran en mode jeu'],
];

type SetTab = 'audio' | 'game' | 'video' | 'keys';

export function SettingsPanel() {
  const open = useSelect((s) => s.settingsOpen);
  if (!open) return null;
  return <SettingsModal onClose={() => store.set({ settingsOpen: false })} />;
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const s = useMini(settings, (x) => x, (a, b) => a === b);
  const game = useSelect((st) => st.settings, (a, b) => a === b);
  const fullscreen = useSelect((st) => st.fullscreen);
  const [tab, setTab] = useState<SetTab>('audio');
  const row = (label: string, ctrl: ComponentChildren) => (
    <label class="lov-set-row">
      <span>{label}</span>
      {ctrl}
    </label>
  );
  const toggle = (on: boolean, fn: (v: boolean) => void) => <input type="checkbox" class="lov-switch" checked={on} onChange={(e) => fn(e.currentTarget.checked)} />;
  return (
    <div class="lov-modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div class="lov-settings lov-frame">
        <header>
          <h2>Paramètres</h2>
          <button class="lov-icon-btn" onClick={onClose} title="Fermer">
            <Glyph name="close" size={16} />
          </button>
        </header>
        <nav class="lov-shop-tabs">
          {(
            [
              ['audio', 'Son'],
              ['game', 'Jeu'],
              ['video', 'Graphismes'],
              ['keys', 'Raccourcis'],
            ] as [SetTab, string][]
          ).map(([k, l]) => (
            <button key={k} class={tab === k ? 'on' : ''} onClick={() => setTab(k)}>
              {l}
            </button>
          ))}
        </nav>
        <div class="lov-set-body">
          {tab === 'audio' &&
            (Object.keys(CHANNEL_LABEL) as AudioChannel[]).map((ch) =>
              row(
                CHANNEL_LABEL[ch],
                <span class="lov-range">
                  <input type="range" min={0} max={100} value={Math.round(s.volumes[ch] * 100)} onInput={(e) => settings.set({ volumes: { ...s.volumes, [ch]: Number(e.currentTarget.value) / 100 } })} />
                  <b>{Math.round(s.volumes[ch] * 100)}</b>
                </span>,
              ),
            )}
          {tab === 'game' && (
            <>
              {row(
                'Mode jeu : plein écran, barre du navigateur masquée' + (keyboardLockSupported() ? ', Ctrl+W/Ctrl+Q capturés' : ''),
                fullscreenSupported() ? (
                  <button class="lov-btn small" onClick={() => void (fullscreen ? exitGameMode() : enterGameMode())}>
                    {fullscreen ? 'Quitter (F11)' : 'Activer (F11)'}
                  </button>
                ) : (
                  <span class="lov-muted">Non disponible</span>
                ),
              )}
              {row('Mode jeu automatique au premier clic en partie', toggle(game.autoFullscreen, (v) => store.setSettings({ autoFullscreen: v })))}
              {row('Confirmer avant de quitter ou recharger la page en partie', toggle(game.confirmLeave, (v) => store.setSettings({ confirmLeave: v })))}
              {row('Lancement rapide des sorts (dès l’appui, sans viser)', toggle(game.quickCast, (v) => store.setSettings({ quickCast: v })))}
              {row('Attaque-déplacement au clic gauche', toggle(game.leftClickAttackMove, (v) => store.setSettings({ leftClickAttackMove: v })))}
              {row('Défilement aux bords de l’écran', toggle(game.edgePan, (v) => store.setSettings({ edgePan: v })))}
              {row(
                'Vitesse de la caméra',
                <span class="lov-range">
                  <input type="range" min={1500} max={5000} step={250} value={game.cameraSpeed} onInput={(e) => store.setSettings({ cameraSpeed: Number(e.currentTarget.value) })} />
                  <b>{Math.round(game.cameraSpeed / 100)}</b>
                </span>,
              )}
              {row('Timers d’objectifs sur la minimap', toggle(s.showTimers, (v) => settings.set({ showTimers: v })))}
              {row(
                'Taille de l’interface',
                <span class="lov-range">
                  <input type="range" min={80} max={130} step={5} value={Math.round(s.hudScale * 100)} onInput={(e) => settings.set({ hudScale: Number(e.currentTarget.value) / 100 })} />
                  <b>{Math.round(s.hudScale * 100)} %</b>
                </span>,
              )}
              {row('Réduire les animations', toggle(s.reduceMotion, (v) => settings.set({ reduceMotion: v })))}
            </>
          )}
          {tab === 'video' &&
            row(
              'Qualité graphique',
              <span class="lov-seg">
                {(['low', 'high'] as Quality[]).map((g) => (
                  <button key={g} class={game.quality === g ? 'on' : ''} onClick={() => store.setSettings({ quality: g })}>
                    {g === 'low' ? 'Basse' : 'Haute'}
                  </button>
                ))}
              </span>,
            )}
          {tab === 'keys' && (
            <table class="lov-keys">
              <tbody>
                {KEYS.map(([k, v]) => (
                  <tr key={k}>
                    <td>
                      <kbd>{k}</kbd>
                    </td>
                    <td>{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
        <footer>
          <button class="lov-btn ghost small" onClick={() => (settings.set(DEFAULT_SETTINGS), store.setSettings({ ...DEFAULT_GAME_SETTINGS }))}>
            Réinitialiser
          </button>
          <button class="lov-btn primary" onClick={onClose}>
            OK
          </button>
        </footer>
      </div>
    </div>
  );
}
