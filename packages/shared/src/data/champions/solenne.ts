import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'solenne',
  name: 'Solenne',
  title: "la Prêtresse de l'Aube",
  class: 'support',
  roles: ['bot', 'mid'],
  difficulty: 1,
  ranged: true,
  resource: 'mana',
  baseStats: stats({ maxHp: 560, maxMana: 380, manaRegen: 11, armor: 26, mr: 30, ad: 50, moveSpeed: 335, attackRange: 550, attackSpeed: 0.63 }),
  perLevel: { maxHp: 92, armor: 4.2, mr: 1.3, ad: 2.8, attackSpeed: 0.015, maxMana: 40, manaRegen: 0.6 },
  attackWindup: 0.3,
  projectileSpeed: 1500,
  palette: { primary: '#fff1c1', secondary: '#e0a96d', accent: '#7ee8fa' },
  lore: "Dernière prêtresse d'un ordre solaire, Solenne porte la lumière là où ses alliés en ont besoin.",
  passive: ability({ id: 'solenne_p', slot: 'P', name: 'Grâce', description: "Les soins et boucliers de Solenne sont augmentés de 1 % par tranche de 2 % de PV manquants de la cible.", targeting: 'self', range: 0, fxKey: 'solenne_p', params: { ampPerMissing: 0.5 } }),
  abilities: {
    Q: ability({ id: 'solenne_q', slot: 'Q', name: 'Lien lumineux', description: "Lance un rayon de lumière : le premier ennemi touché subit des dégâts magiques et est immobilisé 1,5 s.", targeting: 'skillshot', range: 1100, width: 120, speed: 1200, castTime: 0.25, cooldown: [13, 12, 11, 10, 9], cost: [60], damage: { type: 'magic', base: [70, 115, 160, 205, 250], ap: 0.6 }, fxKey: 'solenne_q', params: { root: [1.25, 1.35, 1.45, 1.55, 1.65] } }),
    W: ability({ id: 'solenne_w', slot: 'W', name: 'Bénédiction', description: "Soigne un allié et Solenne.", targeting: 'targeted', range: 750, cooldown: [12, 11, 10, 9, 8], cost: [70, 75, 80, 85, 90], fxKey: 'solenne_w', params: { heal: [70, 95, 120, 145, 170], healAp: 0.35, selfPercent: 0.6 } }),
    E: ability({ id: 'solenne_e', slot: 'E', name: 'Bouclier sacré', description: "Protège un allié avec un bouclier pendant 2,5 s ; il gagne 25 % de vitesse d'attaque.", targeting: 'targeted', range: 800, cooldown: [14, 13, 12, 11, 10], cost: [60], fxKey: 'solenne_e', params: { shield: [60, 90, 120, 150, 180], shieldAp: 0.4, duration: 2.5, attackSpeed: 0.25 } }),
    R: ability({ id: 'solenne_r', slot: 'R', name: 'Aube', description: "Après 0,5 s, une colonne de lumière soigne tous les alliés dans une large zone et leur octroie un bouclier pendant 3 s.", targeting: 'ground', range: 900, radius: 600, castTime: 0.25, cooldown: [130, 115, 100], cost: [100], fxKey: 'solenne_r', params: { delay: 0.5, heal: [150, 250, 350], healAp: 0.5, shield: [100, 175, 250], duration: 3 } }),
  },
});
