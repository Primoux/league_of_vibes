import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'ilyra',
  name: 'Ilyra',
  title: 'la Sentinelle du Givre',
  class: 'mage',
  roles: ['mid'],
  difficulty: 2,
  ranged: true,
  resource: 'mana',
  baseStats: stats({ maxHp: 560, maxMana: 420, manaRegen: 9, armor: 22, mr: 30, ad: 52, moveSpeed: 330, attackRange: 550, attackSpeed: 0.64 }),
  perLevel: { maxHp: 95, armor: 4.4, mr: 1.3, ad: 3, attackSpeed: 0.015, maxMana: 30, manaRegen: 0.8 },
  attackWindup: 0.3,
  projectileSpeed: 1600,
  palette: { primary: '#8fd3ff', secondary: '#1e3a5f', accent: '#e9fbff' },
  lore: "Gardienne d'un glacier vivant, Ilyra fige le temps autour d'elle.",
  passive: ability({ id: 'ilyra_p', slot: 'P', name: 'Morsure du givre', description: "Les sorts appliquent un cumul de Givre (4 s). Au 3e cumul, la cible est étourdie 1 s (une fois toutes les 8 s par cible).", targeting: 'self', range: 0, fxKey: 'ilyra_p', params: { stacks: 3, stun: 1, perTargetCooldown: 8 } }),
  abilities: {
    Q: ability({ id: 'ilyra_q', slot: 'Q', name: 'Éclat de glace', description: "Lance un éclat qui inflige des dégâts magiques au premier ennemi touché et le ralentit de 30 %.", targeting: 'skillshot', range: 950, width: 100, speed: 1700, castTime: 0.25, cooldown: [6, 5.5, 5, 4.5, 4], cost: [50, 55, 60, 65, 70], damage: { type: 'magic', base: [80, 120, 160, 200, 240], ap: 0.7 }, fxKey: 'ilyra_q', params: { slow: 0.3, slowDuration: 1.5 } }),
    W: ability({ id: 'ilyra_w', slot: 'W', name: 'Nova glaciale', description: "Une nova éclate à l'emplacement ciblé après 0,6 s : dégâts magiques et ralentissement de 50 %.", targeting: 'ground', range: 850, radius: 280, castTime: 0.25, cooldown: [10, 9.5, 9, 8.5, 8], cost: [70], damage: { type: 'magic', base: [70, 110, 150, 190, 230], ap: 0.6 }, fxKey: 'ilyra_w', params: { delay: 0.6, slow: 0.5, slowDuration: 2 } }),
    E: ability({ id: 'ilyra_e', slot: 'E', name: 'Égide de givre', description: "Protège un allié (ou elle-même) avec un bouclier pendant 2,5 s. Les ennemis qui le frappent sont ralentis.", targeting: 'targeted', range: 700, cooldown: [16, 15, 14, 13, 12], cost: [60], fxKey: 'ilyra_e', params: { shield: [70, 105, 140, 175, 210], shieldAp: 0.5, duration: 2.5, slow: 0.2 } }),
    R: ability({ id: 'ilyra_r', slot: 'R', name: 'Blizzard', description: "Invoque un blizzard pendant 4 s dans une large zone : dégâts magiques par seconde et ralentissement croissant (jusqu'à 60 %).", targeting: 'ground', range: 900, radius: 500, castTime: 0.25, cooldown: [120, 100, 80], cost: [100], damage: { type: 'magic', base: [60, 90, 120], ap: 0.25 }, fxKey: 'ilyra_r', params: { duration: 4, tickRate: 1, maxSlow: 0.6 } }),
  },
});
