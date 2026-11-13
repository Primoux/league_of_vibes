import { CHAMPIONS, LOADING_SECONDS, type SummonerId, type Team } from '@lov/shared';
import { useEffect, useState } from 'preact/hooks';
import { useCountdown } from '../components.tsx';
import { ui } from '../game/world.ts';
import { ChampionPortrait, ClassEmblem, SummonerIcon } from '../icons/Icons.tsx';
import { CLASS_LABEL } from '../logic/format.ts';
import { useMini, useSelect } from '../useStore.ts';

const TIPS = [
  'Le Léviathan apparaît à 15:00 : son buff renforce le rappel et les sbires proches.',
  'Les tours externes sont fortifiées pendant les 3 premières minutes.',
  'Une balise révèle une zone de 900 unités pendant 90 secondes. Pose-les aux entrées de la rivière !',
  'Le 3e drake détermine le type de la Faille. À 4 drakes, ton équipe obtient l’âme.',
  'Châtiment est indispensable en jungle : sans lui, les camps rapportent moins.',
  'Ctrl + Q/W/E/R monte un sort. Le bouton + apparaît quand un point est disponible.',
  'Tab affiche le tableau des scores, P la boutique, C tes statistiques détaillées.',
  'Trois larves tuées : des larvelettes t’aident à démolir les tours.',
];

interface Card {
  playerId: string;
  name: string;
  team: Team;
  championId: string | null;
  summoners: [SummonerId, SummonerId];
}

export function Loading() {
  const { draft, lobby, me } = useSelect((s) => ({ draft: s.draft, lobby: s.lobby, me: s.playerId }));
  const endsAt = useMini(ui, (s) => s.loadingEndsAt);
  const remaining = useCountdown(endsAt);
  const [tip, setTip] = useState(() => Math.floor(Math.random() * TIPS.length));
  useEffect(() => {
    const id = setInterval(() => setTip((t) => (t + 1) % TIPS.length), 4000);
    return () => clearInterval(id);
  }, []);

  const names = new Map(lobby?.players.map((p) => [p.id, p.name]) ?? []);
  const cards: Card[] = (draft?.picks ?? []).map((p) => ({ playerId: p.playerId, name: names.get(p.playerId) ?? 'Invocateur', team: p.team, championId: p.championId, summoners: p.summoners }));
  const progress = endsAt ? 1 - remaining / LOADING_SECONDS : 0.5;

  return (
    <div class="lov-screen lov-loading">
      <div class="lov-aurora dim" />
      {(['blue', 'red'] as const).map((team) => (
        <div class={`lov-load-row ${team}`} key={team}>
          {cards
            .filter((c) => c.team === team)
            .map((c, i) => {
              const champ = c.championId ? CHAMPIONS[c.championId] : undefined;
              return (
                <div class={`lov-load-card ${c.playerId === me ? 'me' : ''}`} key={c.playerId} style={{ '--i': i + (team === 'red' ? 4 : 0) } as Record<string, number>}>
                  <div class="art">{c.championId ? <ChampionPortrait id={c.championId} variant="splash" size="100%" /> : null}</div>
                  <div class="meta">
                    <b>{champ?.name ?? '???'}</b>
                    <small>{champ?.title}</small>
                    <span class="player">{c.name}</span>
                    <span class="row">
                      {champ && (
                        <span class="cls">
                          <ClassEmblem cls={champ.class} size={12} /> {CLASS_LABEL[champ.class]}
                        </span>
                      )}
                      <span class="sums">
                        <SummonerIcon id={c.summoners[0]} size={18} />
                        <SummonerIcon id={c.summoners[1]} size={18} />
                      </span>
                    </span>
                  </div>
                </div>
              );
            })}
        </div>
      ))}
      <div class="lov-load-foot">
        <div class="lov-load-title">Faille des Vibes</div>
        <div class="lov-progress">
          <i style={{ width: `${Math.round(Math.min(1, Math.max(0, progress)) * 100)}%` }} />
        </div>
        <p class="lov-tip" key={tip}>
          <b>Astuce :</b> {TIPS[tip]}
        </p>
      </div>
    </div>
  );
}
