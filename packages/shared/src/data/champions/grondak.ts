import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'grondak',
  name: 'Grondak',
  title: 'le Rempart de Granit',
  class: 'tank',
  roles: ['top', 'jungle'],
  difficulty: 1,
  ranged: false,
  resource: 'mana',
  baseStats: stats({ maxHp: 680, armor: 38, mr: 32, ad: 64, moveSpeed: 335, attackRange: 150, attackSpeed: 0.62 }),
  perLevel: { maxHp: 110, armor: 4.8, mr: 2.05, ad: 3.5, attackSpeed: 0.02, maxMana: 40 },
  attackWindup: 0.3,
  palette: { primary: '#7a6a58', secondary: '#3d3a36', accent: '#ffb347' },
  lore: "Un golem taillé dans la roche du Rift, réveillé par la magie de la rivière. Il protège ceux qui n'ont pas de murs.",
  passive: ability({ id: 'grondak_p', slot: 'P', name: 'Peau de granit', description: "Après 8 s sans subir de dégâts de champion, Grondak gagne un bouclier égal à 10 % de ses PV max.", targeting: 'self', range: 0, fxKey: 'grondak_p', params: { outOfCombat: 8, shieldMaxHp: 0.1 } }),
  abilities: {
    Q: ability({ id: 'grondak_q', slot: 'Q', name: 'Poing sismique', description: "Frappe le sol en ligne : dégâts physiques et ralentissement de 40 % pendant 1,5 s.", targeting: 'skillshot', range: 650, width: 160, speed: 1800, cooldown: [9, 8.5, 8, 7.5, 7], cost: [50, 55, 60, 65, 70], damage: { type: 'physical', base: [70, 115, 160, 205, 250], bonusAd: 0.8 }, fxKey: 'grondak_q', params: { slow: 0.4, slowDuration: 1.5 } }),
    W: ability({ id: 'grondak_w', slot: 'W', name: 'Rempart', description: "Réduit les dégâts subis de 30 % pendant 3 s et provoque les ennemis proches pendant 1 s.", targeting: 'self', range: 0, radius: 325, cooldown: [16, 15, 14, 13, 12], cost: [60], fxKey: 'grondak_w', params: { damageReduction: 0.3, duration: 3, taunt: 1 } }),
    E: ability({ id: 'grondak_e', slot: 'E', name: 'Charge de roche', description: "Charge vers un ennemi, lui inflige des dégâts magiques et l'étourdit 1 s.", targeting: 'targetedDash', range: 550, speed: 1400, cooldown: [14, 13, 12, 11, 10], cost: [70], damage: { type: 'magic', base: [60, 95, 130, 165, 200], bonusHp: 0.05 }, fxKey: 'grondak_e', params: { stun: 1 } }),
    R: ability({ id: 'grondak_r', slot: 'R', name: 'Effondrement', description: "Après 0,75 s, la terre s'effondre dans une large zone : dégâts magiques et projection 1,25 s.", targeting: 'ground', range: 700, radius: 420, castTime: 0.25, cooldown: [130, 110, 90], cost: [100], damage: { type: 'magic', base: [200, 325, 450], ap: 0.7, bonusHp: 0.06 }, fxKey: 'grondak_r', params: { delay: 0.75, knockup: 1.25 } }),
  },
});
