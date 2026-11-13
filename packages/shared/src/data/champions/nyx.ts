import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'nyx',
  name: 'Nyx',
  title: "l'Ombre Sans Nom",
  class: 'assassin',
  roles: ['mid', 'jungle'],
  difficulty: 3,
  ranged: false,
  resource: 'energy',
  baseStats: stats({ maxHp: 590, maxMana: 200, manaRegen: 50, armor: 30, mr: 32, ad: 68, moveSpeed: 350, attackRange: 125, attackSpeed: 0.69 }),
  perLevel: { maxHp: 98, armor: 4.5, mr: 2.05, ad: 3.4, attackSpeed: 0.025 },
  attackWindup: 0.22,
  palette: { primary: '#1b1b2f', secondary: '#4b2a6b', accent: '#ff2e63' },
  lore: "Personne ne connaît son vrai nom. Ceux qui l'ont vu ne s'en souviennent plus.",
  passive: ability({ id: 'nyx_p', slot: 'P', name: 'Proie isolée', description: "Les attaques et sorts contre un ennemi sans allié à moins de 500 unités infligent 20 % de dégâts en plus.", targeting: 'self', range: 0, fxKey: 'nyx_p', params: { isolationRadius: 500, amp: 0.2 } }),
  abilities: {
    Q: ability({ id: 'nyx_q', slot: 'Q', name: 'Lame du crépuscule', description: "Lacère une cible : dégâts physiques. Si la cible meurt dans les 3 s, le délai de récupération est réinitialisé.", targeting: 'targeted', range: 325, cooldown: [6, 5.5, 5, 4.5, 4], cost: [40], damage: { type: 'physical', base: [70, 105, 140, 175, 210], bonusAd: 1.1 }, fxKey: 'nyx_q', params: { resetWindow: 3 } }),
    W: ability({ id: 'nyx_w', slot: 'W', name: "Voile d'ombre", description: "Devient invisible pendant 1,5 s et gagne 25 % de vitesse de déplacement. Attaquer ou lancer un sort brise l'invisibilité.", targeting: 'self', range: 0, cooldown: [22, 20, 18, 16, 14], cost: [60], fxKey: 'nyx_w', params: { duration: 1.5, moveSpeed: 0.25 } }),
    E: ability({ id: 'nyx_e', slot: 'E', name: "Pas de l'ombre", description: "Se téléporte derrière une unité (alliée ou ennemie). Sur un ennemi, ralentit de 40 % pendant 1 s.", targeting: 'targetedDash', range: 600, speed: 99999, cooldown: [16, 14.5, 13, 11.5, 10], cost: [50], damage: { type: 'physical', base: [40, 65, 90, 115, 140], bonusAd: 0.5 }, fxKey: 'nyx_e', params: { slow: 0.4, slowDuration: 1 } }),
    R: ability({ id: 'nyx_r', slot: 'R', name: 'Mort certaine', description: "Marque un champion pendant 3 s. À la fin, la marque explose et inflige 30 % des dégâts subis pendant la durée, plus des dégâts de base.", targeting: 'targeted', range: 500, cooldown: [110, 85, 60], damage: { type: 'physical', base: [100, 200, 300], bonusAd: 0.8 }, fxKey: 'nyx_r', params: { duration: 3, storedPercent: 0.3 } }),
  },
});
