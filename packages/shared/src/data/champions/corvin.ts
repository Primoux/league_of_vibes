import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'corvin',
  name: 'Corvin',
  title: "l'Archer des Corbeaux",
  class: 'marksman',
  roles: ['bot'],
  difficulty: 2,
  ranged: true,
  resource: 'mana',
  baseStats: stats({ maxHp: 570, maxMana: 320, armor: 25, mr: 30, ad: 59, moveSpeed: 330, attackRange: 600, attackSpeed: 0.66 }),
  perLevel: { maxHp: 94, armor: 4.2, mr: 1.3, ad: 2.9, attackSpeed: 0.032, maxMana: 35 },
  attackWindup: 0.2,
  projectileSpeed: 2200,
  palette: { primary: '#2b2d42', secondary: '#8d99ae', accent: '#7cff6b' },
  lore: "Corvin parle aux corbeaux, et les corbeaux lui disent où se cachent ses proies.",
  passive: ability({ id: 'corvin_p', slot: 'P', name: 'Marque du corbeau', description: "Les attaques marquent la cible. La 3e marque consomme les marques : 6 % des PV max de la cible en dégâts magiques.", targeting: 'self', range: 0, fxKey: 'corvin_p', params: { marks: 3, targetMaxHp: 0.06, markDuration: 4 } }),
  abilities: {
    Q: ability({ id: 'corvin_q', slot: 'Q', name: 'Flèche perçante', description: "Tire une flèche qui transperce tous les ennemis en ligne et applique une marque.", targeting: 'skillshot', range: 1000, width: 80, speed: 2000, castTime: 0.25, cooldown: [9, 8, 7, 6, 5], cost: [50, 55, 60, 65, 70], damage: { type: 'physical', base: [40, 80, 120, 160, 200], ad: 1 }, fxKey: 'corvin_q', params: { pierce: 1 } }),
    W: ability({ id: 'corvin_w', slot: 'W', name: 'Piège à griffes', description: "Pose un piège invisible (1 min). Le premier ennemi qui marche dessus est immobilisé 1,25 s et révélé.", targeting: 'ground', range: 800, radius: 120, cooldown: [16, 14, 12, 10, 8], cost: [40], damage: { type: 'magic', base: [40, 70, 100, 130, 160], ap: 0.4 }, fxKey: 'corvin_w', params: { root: 1.25, lifetime: 60, maxTraps: 3 } }),
    E: ability({ id: 'corvin_e', slot: 'E', name: 'Envol', description: "Bondit en arrière dans un nuage de plumes et gagne 30 % de vitesse de déplacement pendant 2 s.", targeting: 'dash', range: 425, speed: 1300, cooldown: [20, 18, 16, 14, 12], cost: [60], fxKey: 'corvin_e', params: { moveSpeed: 0.3, duration: 2 } }),
    R: ability({ id: 'corvin_r', slot: 'R', name: 'Nuée', description: "Une nuée de corbeaux s'abat sur une zone pendant 3 s : dégâts physiques par seconde, révèle et réduit les soins de 40 %.", targeting: 'ground', range: 1200, radius: 450, castTime: 0.25, cooldown: [110, 95, 80], cost: [100], damage: { type: 'physical', base: [70, 110, 150], bonusAd: 0.35 }, fxKey: 'corvin_r', params: { duration: 3, grievous: 0.4 } }),
  },
});
