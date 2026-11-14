import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'touilleur', name: 'Le Touilleur', title: 'le Virtuose du Moulinet', class: 'mage', roles: ['mid', 'jungle'],
  difficulty: 2, ranged: false, resource: 'mana', attackWindup: .27,
  baseStats: stats({ maxHp: 615, maxMana: 350, manaRegen: 8, ad: 58, armor: 31, mr: 32, attackRange: 175, attackSpeed: .67, moveSpeed: 345 }),
  perLevel: { maxHp: 98, maxMana: 42, ad: 3, armor: 4.5, mr: 1.3, attackSpeed: .022, manaRegen: .7 },
  palette: { primary: '#17434a', secondary: '#d29a49', accent: '#8ef4d2' },
  lore: "On lui avait demandé de remuer la soupe. Il a découvert les duels. Depuis, son index travaille en CDI et son autre main cherche encore le bouton d'arrêt.",
  passive: ability({ id: 'touilleur_p', slot: 'P', name: 'Ça monte à la tête', targeting: 'self', range: 0, fxKey: 'touilleur_p',
    description: "Chaque impact de sort sur un champion donne un tour de moulinet pendant 5 s (un maximum toutes les 0,4 s). Au troisième : 30 (+20 % AP) dégâts magiques supplémentaires et 20 % de vitesse pendant 1,5 s. Il mélange ses adversaires avec ses priorités.",
    params: { stacks: 3, duration: 5, lockout: .4, damage: 30, ap: .2, speed: .2, speedDuration: 1.5 } }),
  abilities: {
    Q: ability({ id: 'touilleur_q', slot: 'Q', name: 'Coup de doigt', targeting: 'skillshot', range: 825, width: 85, speed: 1550, castTime: .15,
      cooldown: [7, 6.5, 6, 5.5, 5], cost: [45], damage: { type: 'magic', base: [65, 95, 125, 155, 185], ap: .55 }, fxKey: 'touilleur_q',
      description: "Projette un petit moulinet sur le premier ennemi : 65/95/125/155/185 (+55 % AP) dégâts magiques et ralentissement de 20 % pendant 1 s. Un doigt d'explication suffit.", params: { slow: .2, slowDuration: 1 } }),
    W: ability({ id: 'touilleur_w', slot: 'W', name: 'Tourbillon de poche', targeting: 'ground', range: 650, radius: 215, castTime: .15,
      cooldown: [13, 12, 11, 10, 9], cost: [65], damage: { type: 'magic', base: [70, 100, 130, 160, 190], ap: .45 }, fxKey: 'touilleur_w',
      description: "Après 0,45 s, remue une zone : 70/100/130/160/190 (+45 % AP) dégâts magiques et ralentissement de 35 % pendant 1,5 s. La recette ne prévoyait pas autant d'opposants.", params: { delay: .45, slow: .35, slowDuration: 1.5 } }),
    E: ability({ id: 'touilleur_e', slot: 'E', name: 'Pas de côté', targeting: 'dash', range: 325,
      cooldown: [17, 16, 15, 14, 13], cost: [50], fxKey: 'touilleur_e',
      description: "Se téléporte jusqu'à 325 unités et gagne un bouclier de 45/65/85/105/125 (+30 % AP) pendant 2 s. Le corps esquive. L'index n'a pas reçu le mémo.", params: { shield: [45, 65, 85, 105, 125], shieldAp: .3, duration: 2 } }),
    R: ability({ id: 'touilleur_r', slot: 'R', name: 'La grande touille', targeting: 'targetedDash', range: 450, castTime: .25,
      cooldown: [110, 90, 70], cost: [100], damage: { type: 'magic', base: [150, 250, 350], ap: .7 }, fxKey: 'touilleur_r',
      description: "Passe derrière un champion ennemi, inflige 150/250/350 (+70 % AP) dégâts magiques et l'étourdit jusqu'à 2 s pendant un moulinet burlesque derrière son manteau. Le Touilleur canalise sur place. Contrôle du lanceur, purification, mort, stase ou rupture de proximité arrêtent la touille. La ténacité réduit l'étourdissement. Le service était compris.",
      params: { duration: 2, breakRange: 230, offset: 15 } }),
  },
});
