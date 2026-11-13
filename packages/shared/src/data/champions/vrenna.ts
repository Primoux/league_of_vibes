import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'vrenna',
  name: 'Vrenna',
  title: 'la Lance des Steppes',
  class: 'fighter',
  roles: ['jungle', 'top'],
  difficulty: 2,
  ranged: false,
  resource: 'mana',
  baseStats: stats({ maxHp: 620, armor: 33, mr: 32, ad: 64, moveSpeed: 345, attackRange: 200, attackSpeed: 0.66 }),
  perLevel: { maxHp: 100, armor: 4.2, mr: 2.05, ad: 3.6, attackSpeed: 0.028, maxMana: 45 },
  attackWindup: 0.27,
  palette: { primary: '#a33b3b', secondary: '#e8d9b0', accent: '#ffcc00' },
  lore: "Cheffe de guerre nomade, Vrenna traque ses proies à travers les steppes avec une lance d'os de dragon.",
  passive: ability({ id: 'vrenna_p', slot: 'P', name: 'Saignée', description: "Les attaques et sorts appliquent Saignée (5 cumuls max) : dégâts physiques sur 3 s. À 5 cumuls, Vrenna gagne 20 % de vitesse d'attaque.", targeting: 'self', range: 0, fxKey: 'vrenna_p', params: { maxStacks: 5, dps: 6, duration: 3, attackSpeedAtMax: 0.2 } }),
  abilities: {
    Q: ability({ id: 'vrenna_q', slot: 'Q', name: 'Coup de lance', description: "Frappe en ligne : dégâts physiques. Les ennemis touchés par la pointe sont attirés vers Vrenna.", targeting: 'skillshot', range: 600, width: 120, speed: 2500, castTime: 0.25, cooldown: [9, 8.5, 8, 7.5, 7], cost: [40], damage: { type: 'physical', base: [60, 95, 130, 165, 200], bonusAd: 1 }, fxKey: 'vrenna_q', params: { pull: 300, tipLength: 150 } }),
    W: ability({ id: 'vrenna_w', slot: 'W', name: 'Cri de guerre', description: "Gagne de 30 à 50 % de vitesse d'attaque pendant 4 s et se soigne de 8 % de ses PV manquants.", targeting: 'self', range: 0, cooldown: [15, 14, 13, 12, 11], cost: [50], fxKey: 'vrenna_w', params: { attackSpeed: [0.3, 0.35, 0.4, 0.45, 0.5], duration: 4, healMissing: 0.08 } }),
    E: ability({ id: 'vrenna_e', slot: 'E', name: 'Bond des steppes', description: "Saute vers une zone : dégâts physiques et ralentissement de 40 % à l'atterrissage.", targeting: 'dash', range: 500, radius: 250, speed: 1200, cooldown: [13, 12, 11, 10, 9], cost: [60], damage: { type: 'physical', base: [50, 85, 120, 155, 190], bonusAd: 0.6 }, fxKey: 'vrenna_e', params: { slow: 0.4, slowDuration: 1.5 } }),
    R: ability({ id: 'vrenna_r', slot: 'R', name: 'Lance céleste', description: "Bondit sur un champion ennemi, inflige de lourds dégâts physiques et le projette 1 s.", targeting: 'targetedDash', range: 650, speed: 1800, cooldown: [110, 90, 70], cost: [100], damage: { type: 'physical', base: [175, 275, 375], bonusAd: 1.2 }, fxKey: 'vrenna_r', params: { knockup: 1 } }),
  },
});
