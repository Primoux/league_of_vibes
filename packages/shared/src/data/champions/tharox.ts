import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'tharox',
  name: 'Tharox',
  title: 'la Charge Sauvage',
  class: 'tank',
  roles: ['jungle', 'top'],
  difficulty: 1,
  ranged: false,
  resource: 'mana',
  baseStats: stats({ maxHp: 650, armor: 36, mr: 32, ad: 62, moveSpeed: 345, attackRange: 150, attackSpeed: 0.65 }),
  perLevel: { maxHp: 105, armor: 4.5, mr: 2.05, ad: 3.3, attackSpeed: 0.025, maxMana: 40 },
  attackWindup: 0.28,
  palette: { primary: '#5b3a29', secondary: '#c9a66b', accent: '#ff5533' },
  lore: "Sanglier cuirassé des forêts profondes. Quand Tharox prend de l'élan, rien ne l'arrête.",
  passive: ability({ id: 'tharox_p', slot: 'P', name: 'Élan', description: "Se déplacer accumule de l'élan (jusqu'à 100). À 100, la prochaine attaque inflige 8 % des PV max de la cible et ralentit de 50 %.", targeting: 'self', range: 0, fxKey: 'tharox_p', params: { perUnit: 0.03, bonusTargetMaxHp: 0.08, slow: 0.5, slowDuration: 1.5 } }),
  abilities: {
    Q: ability({ id: 'tharox_q', slot: 'Q', name: 'Défenses', description: "Fonce sur une courte distance ; le premier ennemi touché subit des dégâts et est repoussé.", targeting: 'dash', range: 450, speed: 1200, width: 140, cooldown: [10, 9.5, 9, 8.5, 8], cost: [50], damage: { type: 'physical', base: [60, 100, 140, 180, 220], bonusAd: 0.7 }, fxKey: 'tharox_q', params: { knockback: 250 } }),
    W: ability({ id: 'tharox_w', slot: 'W', name: 'Écorce de fer', description: "Gagne armure et résistance magique pendant 4 s et inflige des dégâts magiques autour de lui chaque seconde.", targeting: 'self', range: 0, radius: 300, cooldown: [14], cost: [40], damage: { type: 'magic', base: [15, 25, 35, 45, 55], bonusHp: 0.01 }, fxKey: 'tharox_w', params: { resists: [15, 20, 25, 30, 35], duration: 4 } }),
    E: ability({ id: 'tharox_e', slot: 'E', name: 'Piétinement', description: "Frappe le sol : dégâts magiques et ralentissement de 35 % autour de lui.", targeting: 'self', range: 0, radius: 375, cooldown: [8, 7.5, 7, 6.5, 6], cost: [45], damage: { type: 'magic', base: [55, 90, 125, 160, 195], ap: 0.4 }, fxKey: 'tharox_e', params: { slow: 0.35, slowDuration: 2 } }),
    R: ability({ id: 'tharox_r', slot: 'R', name: 'Ruée sauvage', description: "Après 1 s de préparation, charge sur une très longue distance. Les champions percutés sont étourdis 1,5 s.", targeting: 'dash', range: 2500, speed: 1800, width: 200, castTime: 1, cooldown: [140, 120, 100], cost: [100], damage: { type: 'physical', base: [150, 250, 350], bonusAd: 1 }, fxKey: 'tharox_r', params: { stun: 1.5 } }),
  },
});
