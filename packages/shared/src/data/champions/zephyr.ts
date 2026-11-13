import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'zephyr',
  name: 'Zephyr',
  title: "l'Arcaniste Instable",
  class: 'mage',
  roles: ['mid'],
  difficulty: 3,
  ranged: true,
  resource: 'mana',
  baseStats: stats({ maxHp: 550, maxMana: 400, manaRegen: 8, armor: 21, mr: 30, ad: 50, moveSpeed: 330, attackRange: 525, attackSpeed: 0.62 }),
  perLevel: { maxHp: 92, armor: 4.2, mr: 1.3, ad: 3, attackSpeed: 0.018, maxMana: 32, manaRegen: 0.8 },
  attackWindup: 0.3,
  projectileSpeed: 1500,
  palette: { primary: '#b067ff', secondary: '#1a0f2e', accent: '#53f2ff' },
  lore: "Un étudiant de l'académie arcanique qui a ouvert une brèche qu'il ne sait pas refermer.",
  passive: ability({ id: 'zephyr_p', slot: 'P', name: 'Surcharge', description: "Tous les 4 sorts lancés, le prochain sort inflige 25 % de dégâts en plus et rend 30 de mana.", targeting: 'self', range: 0, fxKey: 'zephyr_p', params: { every: 4, amp: 0.25, mana: 30 } }),
  abilities: {
    Q: ability({ id: 'zephyr_q', slot: 'Q', name: 'Orbe boomerang', description: "Lance un orbe qui revient vers Zephyr ; il inflige des dégâts magiques à l'aller et au retour (dégâts bruts au retour).", targeting: 'skillshot', range: 900, width: 90, speed: 1400, castTime: 0.25, cooldown: [7, 6.5, 6, 5.5, 5], cost: [60, 65, 70, 75, 80], damage: { type: 'magic', base: [60, 95, 130, 165, 200], ap: 0.5 }, fxKey: 'zephyr_q', params: { returns: 1, returnTrue: 1 } }),
    W: ability({ id: 'zephyr_w', slot: 'W', name: 'Brèche', description: "Se téléporte sur une courte distance, laissant une brèche qui explose après 1 s.", targeting: 'dash', range: 450, radius: 250, speed: 99999, cooldown: [18, 16.5, 15, 13.5, 12], cost: [80], damage: { type: 'magic', base: [70, 110, 150, 190, 230], ap: 0.6 }, fxKey: 'zephyr_w', params: { delay: 1 } }),
    E: ability({ id: 'zephyr_e', slot: 'E', name: 'Chaîne arcanique', description: "Enchaîne un ennemi : s'il reste à portée pendant 1,5 s, il est immobilisé 1,5 s et subit des dégâts magiques.", targeting: 'targeted', range: 650, cooldown: [14, 13, 12, 11, 10], cost: [70], damage: { type: 'magic', base: [60, 100, 140, 180, 220], ap: 0.5 }, fxKey: 'zephyr_e', params: { tetherTime: 1.5, breakRange: 900, root: 1.5 } }),
    R: ability({ id: 'zephyr_r', slot: 'R', name: 'Barrage arcanique', description: "Pendant 8 s, peut lancer jusqu'à 3 rayons à très longue portée qui infligent des dégâts magiques au premier champion touché.", targeting: 'skillshot', range: 2500, width: 160, speed: 3000, castTime: 0.5, cooldown: [120, 100, 80], cost: [100], damage: { type: 'magic', base: [120, 180, 240], ap: 0.45 }, fxKey: 'zephyr_r', params: { charges: 3, window: 8 } }),
  },
});
