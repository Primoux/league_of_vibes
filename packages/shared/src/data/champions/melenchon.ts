import { ability, defineChampion, stats } from './_helpers.ts';

/** Caricature jouable : le personnage et ses textes relèvent de la satire. */
export default defineChampion({
  id: 'melenchon', name: 'Mélenchon', title: 'le Tribun indigné', class: 'mage', roles: ['mid', 'bot'],
  difficulty: 2, ranged: true, resource: 'mana',
  baseStats: stats({ maxHp: 620, maxMana: 400, manaRegen: 9, hpRegen: 7, ad: 54, armor: 28, mr: 30, attackRange: 525, moveSpeed: 325, attackSpeed: .64 }),
  perLevel: { maxHp: 100, maxMana: 42, manaRegen: .7, ad: 2.8, armor: 4.2, mr: 1.3, attackSpeed: .018 },
  attackWindup: .28, projectileSpeed: 1700,
  palette: { primary: '#203452', secondary: '#ef3340', accent: '#5de2ff' },
  lore: "Personnage satirique. Arrivé dans la Faille pour une simple question au gouvernement, il a réquisitionné la voie du milieu. Son micro n'a pas de bouton d'arrêt. Son adversaire aimerait bien en avoir un.",
  passive: ability({ id: 'melenchon_p', slot: 'P', name: 'Indignation permanente', targeting: 'self', range: 0, fxKey: 'melenchon_p',
    description: "Toucher un champion avec un sort donne de l'Indignation (6 s, au plus un cumul toutes les 0,5 s). À 3 cumuls : bouclier de 35 + 5 par niveau (+20 % AP) et +18 % de vitesse pendant 2 s. La cravate aussi monte dans les tours.",
    params: { maxStacks: 3, stackDuration: 6, hitLockout: .5, shield: 35, shieldPerLevel: 5, shieldAp: .2, moveSpeed: .18, duration: 2 } }),
  abilities: {
    Q: ability({ id: 'melenchon_q', slot: 'Q', name: 'Question au gouvernement', targeting: 'skillshot', range: 950, width: 110, speed: 1750, castTime: .2,
      cooldown: [7, 6.5, 6, 5.5, 5], cost: [45, 50, 55, 60, 65], damage: { type: 'magic', base: [80, 120, 160, 200, 240], ap: .65 }, fxKey: 'melenchon_q',
      description: "Projette un micro sur le premier ennemi : 80/120/160/200/240 (+65 % AP) dégâts magiques et ralentissement de 35 % pendant 1,5 s. La question est courte. L'impact, beaucoup moins.",
      params: { slow: .35, slowDuration: 1.5 } }),
    W: ability({ id: 'melenchon_w', slot: 'W', name: 'Motion de censure', targeting: 'ground', range: 800, radius: 280, castTime: .2,
      cooldown: [13, 12, 11, 10, 9], cost: [65], damage: { type: 'magic', base: [70, 105, 140, 175, 210], ap: .5 }, fxKey: 'melenchon_w',
      description: "Après une télégraphie de 0,55 s, les amendements pleuvent : 70/105/140/175/210 (+50 % AP) dégâts magiques et silence pendant 1 s. Pour une fois, quelqu'un d'autre coupe son micro.",
      params: { delay: .55, silence: 1 } }),
    E: ability({ id: 'melenchon_e', slot: 'E', name: 'Hologramme', targeting: 'dash', range: 400,
      cooldown: [18, 17, 16, 15, 14], cost: [60], damage: { type: 'magic', base: [50, 80, 110, 140, 170], ap: .4 }, fxKey: 'melenchon_e', radius: 240,
      description: "Se téléporte de 400 unités et gagne un bouclier de 45/65/85/105/125 (+30 % AP) pendant 2 s. Son hologramme reste au départ, puis explose après 1,2 s. Même absent, il occupe le débat.",
      params: { delay: 1.2, shield: [45, 65, 85, 105, 125], shieldAp: .3, duration: 2 } }),
    R: ability({ id: 'melenchon_r', slot: 'R', name: 'La République, c’est moi !', targeting: 'self', range: 0, radius: 500, castTime: .6,
      cooldown: [120, 105, 90], cost: [100], damage: { type: 'magic', base: [150, 225, 300], ap: .6 }, fxKey: 'melenchon_r',
      description: "Après 0,6 s de déclaration solennelle, une onde tricolore inflige 150/225/300 (+60 % AP) dégâts magiques et provoque les ennemis pendant 1,25 s. Gagne un bouclier de 20 % des PV max (+40 % AP) pendant 4 s. Le teamfight devient une séance extraordinaire.",
      params: { taunt: 1.25, shieldMaxHp: .2, shieldAp: .4, duration: 4 } }),
  },
});
