import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'sable',
  name: 'Sable',
  title: 'la Tireuse des Dunes',
  class: 'marksman',
  roles: ['bot'],
  difficulty: 2,
  ranged: true,
  resource: 'mana',
  baseStats: stats({ maxHp: 580, maxMana: 300, armor: 26, mr: 30, ad: 60, moveSpeed: 330, attackRange: 575, attackSpeed: 0.67 }),
  perLevel: { maxHp: 96, armor: 4.2, mr: 1.3, ad: 3, attackSpeed: 0.03, maxMana: 35 },
  attackWindup: 0.18,
  projectileSpeed: 2000,
  palette: { primary: '#d9a441', secondary: '#4a2f1b', accent: '#ff7a1a' },
  lore: "Chasseuse de primes du désert, Sable ne rate jamais deux fois la même cible.",
  passive: ability({ id: 'sable_p', slot: 'P', name: 'Œil du désert', description: "Les coups critiques infligent 10 % de dégâts en plus. Toucher un champion octroie 15 % de vitesse de déplacement pendant 1 s.", targeting: 'self', range: 0, fxKey: 'sable_p', params: { critBonus: 0.1, moveSpeed: 0.15, duration: 1 } }),
  abilities: {
    Q: ability({ id: 'sable_q', slot: 'Q', name: 'Tir explosif', description: "La prochaine attaque explose et inflige des dégâts physiques bonus aux ennemis autour de la cible.", targeting: 'self', range: 0, radius: 200, cooldown: [8, 7, 6, 5, 4], cost: [30], damage: { type: 'physical', base: [20, 45, 70, 95, 120], ad: 1.1 }, fxKey: 'sable_q' }),
    W: ability({ id: 'sable_w', slot: 'W', name: 'Volée', description: "Tire 7 balles en cône ; chaque ennemi n'est touché qu'une fois et ralenti de 20 %.", targeting: 'cone', range: 750, radius: 50, castTime: 0.25, cooldown: [12, 10.5, 9, 7.5, 6], cost: [60], damage: { type: 'physical', base: [50, 85, 120, 155, 190], bonusAd: 1 }, fxKey: 'sable_w', params: { bullets: 7, coneAngle: 50, slow: 0.2, slowDuration: 1.5 } }),
    E: ability({ id: 'sable_e', slot: 'E', name: 'Roulade', description: "Roule vers une position ; la prochaine attaque dans les 3 s est un coup critique garanti.", targeting: 'dash', range: 400, speed: 1200, cooldown: [18, 16, 14, 12, 10], cost: [50], fxKey: 'sable_e', params: { empowerWindow: 3 } }),
    R: ability({ id: 'sable_r', slot: 'R', name: 'Balle du jugement', description: "Après 1 s de visée, tire une balle globale qui transperce les sbires et touche le premier champion.", targeting: 'skillshot', range: 12000, width: 160, speed: 2500, castTime: 1, cooldown: [100, 85, 70], cost: [100], damage: { type: 'physical', base: [250, 400, 550], bonusAd: 1.5 }, fxKey: 'sable_r', params: { piercesMinions: 1 } }),
  },
});
