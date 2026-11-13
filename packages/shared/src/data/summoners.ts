import type { SummonerDef } from '../schema.ts';
import type { SummonerId } from '../types.ts';

/** Propriétaire : codex-champions. Effets implémentés dans server/src/systems/summoners.ts. */
export const SUMMONERS: Record<SummonerId, SummonerDef> = {
  flash: { id: 'flash', name: 'Saut éclair', description: 'Se téléporte sur une courte distance (400).', cooldown: 300, targeting: 'ground', range: 400, fxKey: 'sum_flash' },
  ignite: { id: 'ignite', name: 'Embrasement', description: 'Brûle un champion ennemi : dégâts bruts sur 5 s et réduction des soins de 40 %.', cooldown: 180, targeting: 'targeted', range: 600, fxKey: 'sum_ignite' },
  heal: { id: 'heal', name: 'Soins', description: 'Soigne le lanceur et l’allié le plus blessé à proximité, et octroie 30 % de vitesse pendant 1 s.', cooldown: 240, targeting: 'self', range: 850, fxKey: 'sum_heal' },
  barrier: { id: 'barrier', name: 'Barrière', description: 'Bouclier temporaire pendant 2,5 s.', cooldown: 180, targeting: 'self', range: 0, fxKey: 'sum_barrier' },
  exhaust: { id: 'exhaust', name: 'Fatigue', description: 'Ralentit un champion ennemi de 30 % et réduit ses dégâts de 40 % pendant 3 s.', cooldown: 210, targeting: 'targeted', range: 650, fxKey: 'sum_exhaust' },
  teleport: { id: 'teleport', name: 'Téléportation', description: 'Après 4 s de canalisation, se téléporte sur une tour, un sbire ou une balise alliée.', cooldown: 360, targeting: 'targeted', range: 99999, fxKey: 'sum_teleport' },
  smite: { id: 'smite', name: 'Châtiment', description: 'Inflige 600 dégâts bruts à un monstre ou un sbire. Requis pour jouer jungle (bonus d’or/xp des camps).', cooldown: 90, targeting: 'targeted', range: 500, fxKey: 'sum_smite' },
  ghost: { id: 'ghost', name: 'Fantôme', description: 'Ignore les collisions et gagne jusqu’à 40 % de vitesse de déplacement pendant 10 s.', cooldown: 210, targeting: 'self', range: 0, fxKey: 'sum_ghost' },
  cleanse: { id: 'cleanse', name: 'Purge', description: 'Retire tous les contrôles (sauf projections et suppressions) et réduit les suivants de 65 % pendant 3 s.', cooldown: 210, targeting: 'self', range: 0, fxKey: 'sum_cleanse' },
};
export const SUMMONER_IDS = Object.keys(SUMMONERS) as SummonerId[];
export const DEFAULT_SUMMONERS: [SummonerId, SummonerId] = ['flash', 'ignite'];
