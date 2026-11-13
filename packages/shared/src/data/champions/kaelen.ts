import { ability, defineChampion, stats } from './_helpers.ts';

export default defineChampion({
  id: 'kaelen',
  name: 'Kaelen',
  title: "la Lame de l'Aube",
  class: 'fighter',
  roles: ['top'],
  difficulty: 2,
  ranged: false,
  resource: 'none',
  baseStats: stats({ maxHp: 630, maxMana: 0, manaRegen: 0, armor: 34, mr: 32, ad: 66, moveSpeed: 345, attackRange: 175, attackSpeed: 0.68 }),
  perLevel: { maxHp: 102, armor: 4.2, mr: 2.05, ad: 3.8, attackSpeed: 0.03 },
  attackWindup: 0.25,
  palette: { primary: '#f2d16b', secondary: '#2c3e75', accent: '#ffffff' },
  lore: "Ancienne garde du temple solaire, Kaelen combat avec une lame qui brille plus fort à chaque coup porté.",
  passive: ability({ id: 'kaelen_p', slot: 'P', name: 'Lame aiguisée', description: "Chaque 3e attaque inflige 40 % de dégâts bonus et soigne Kaelen de 30 % des dégâts infligés.", targeting: 'self', range: 0, fxKey: 'kaelen_p', params: { everyN: 3, bonus: 0.4, heal: 0.3 } }),
  abilities: {
    Q: ability({ id: 'kaelen_q', slot: 'Q', name: 'Taille ascendante', description: "Se fend vers l'avant en infligeant des dégâts. Peut être relancé 2 fois en 4 s ; la 3e frappe projette.", targeting: 'dash', range: 275, radius: 220, speed: 1100, cooldown: [12, 11, 10, 9, 8], damage: { type: 'physical', base: [20, 45, 70, 95, 120], ad: 0.6 }, fxKey: 'kaelen_q', params: { recasts: 2, window: 4, knockup: 0.75 } }),
    W: ability({ id: 'kaelen_w', slot: 'W', name: 'Parade', description: "Pendant 1,25 s, bloque le prochain sort ou attaque de champion et étourdit l'attaquant 1 s s'il est au corps à corps.", targeting: 'self', range: 0, cooldown: [20, 18, 16, 14, 12], fxKey: 'kaelen_w', params: { window: 1.25, stun: 1 } }),
    E: ability({ id: 'kaelen_e', slot: 'E', name: 'Fente', description: "Bondit à travers les unités sur une courte distance et gagne un bouclier pendant 2 s.", targeting: 'dash', range: 350, speed: 1500, cooldown: [10, 9, 8, 7, 6], damage: { type: 'physical', base: [0] }, fxKey: 'kaelen_e', params: { shield: [60, 90, 120, 150, 180], shieldBonusAd: 1, shieldDuration: 2 } }),
    R: ability({ id: 'kaelen_r', slot: 'R', name: 'Jugement solaire', description: "Abat sa lame sur un champion : dégâts physiques augmentés de 1 % par 1 % de PV manquants de la cible.", targeting: 'targeted', range: 400, castTime: 0.4, cooldown: [100, 80, 60], damage: { type: 'physical', base: [150, 250, 350], bonusAd: 0.9 }, fxKey: 'kaelen_r', params: { missingHpAmp: 1 } }),
  },
});
