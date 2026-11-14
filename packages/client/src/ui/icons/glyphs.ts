/** Glyphes des icônes de sorts (pur, testé). Propriétaire : mistral-ui. */
import type { AbilityDef } from '@lov/shared';

export const TARGET_GLYPH: Record<string, string> = {
  skillshot: 'M3 13 L15 13 L15 9 L22 12 L15 15 L15 11 L3 11 Z M3 8 L9 8 L9 9.4 L3 9.4 Z M3 14.6 L9 14.6 L9 16 L3 16 Z',
  targeted: 'M11 2 H13 V7 H11 Z M11 17 H13 V22 H11 Z M2 11 H7 V13 H2 Z M17 11 H22 V13 H17 Z M12 8.5 A3.5 3.5 0 1 1 11.99 8.5 Z',
  ground: 'M12 4 A8 8 0 1 1 11.99 4 Z M12 6.5 A5.5 5.5 0 1 0 12.01 6.5 Z M12 9.5 A2.5 2.5 0 1 1 11.99 9.5 Z',
  cone: 'M3 12 L21 3 C23 9 23 15 21 21 Z M8 12 L18 7.5 C19 10.5 19 13.5 18 16.5 Z',
  self: 'M12 3 A9 9 0 1 1 11.99 3 Z M12 5 A7 7 0 1 0 12.01 5 Z M12 8 A4 4 0 1 1 11.99 8 Z',
  dash: 'M2 5 L9 12 L2 19 L5 19 L12 12 L5 5 Z M10 5 L17 12 L10 19 L13 19 L20 12 L13 5 Z',
  targetedDash: 'M2 6 L8 12 L2 18 L4.5 18 L10.5 12 L4.5 6 Z M9 6 L15 12 L9 18 L11.5 18 L17.5 12 L11.5 6 Z M20 9.5 A2.5 2.5 0 1 1 19.99 9.5 Z',
  global: 'M12 2 A10 10 0 1 1 11.99 2 Z M12 4 A8 8 0 1 0 12.01 4 Z M11 3 H13 V21 H11 Z M3 11 H21 V13 H3 Z M12 4 C7 8 7 16 12 20 C9 16 9 8 12 4 Z M12 4 C17 8 17 16 12 20 C15 16 15 8 12 4 Z',
  passive: 'M12 2 L20.7 7 V17 L12 22 L3.3 17 V7 Z M12 5 L18 8.5 V15.5 L12 19 L6 15.5 V8.5 Z M12 8 L15.5 10 V14 L12 16 L8.5 14 V10 Z',
};

/** Glyphes thématiques (viewBox 24) choisis d'après le nom et la description du sort. */
export const THEME_GLYPH: Record<string, string> = {
  snow: 'M11 1 H13 V8.3 L18.6 4.5 L19.7 6.2 L14.1 10 L21 10 V12 H14.1 L19.7 15.8 L18.6 17.5 L13 13.7 V23 H11 V13.7 L5.4 17.5 L4.3 15.8 L9.9 12 L3 12 V10 L9.9 10 L4.3 6.2 L5.4 4.5 L11 8.3 Z',
  shield: 'M12 1.5 L21 5 V11 C21 16.5 17 21 12 22.5 C7 21 3 16.5 3 11 V5 Z M12 5 L6.5 7.2 V11 C6.5 14.6 8.8 17.6 12 19 Z',
  heal: 'M9 2 H15 V9 H22 V15 H15 V22 H9 V15 H2 V9 H9 Z',
  bolt: 'M13 1 L4 14 H11 L9 23 L20 9 H13 Z',
  flame: 'M12 1 C14 6 19 8 19 14.5 C19 19 15.9 22.5 12 22.5 C8.1 22.5 5 19 5 14.5 C5 11 7 9 8.5 7.5 C8.5 10 9.5 11.5 11 12 C10 8 11 4 12 1 Z M12 14 C13.5 16 15 17 15 18.8 C15 20.4 13.7 21.3 12 21.3 C10.3 21.3 9 20.4 9 18.8 C9 17 10.5 16 12 14 Z',
  bird: 'M2 12 C6 8 9 8 12 10 C14 6 18 4 23 4 C20 6 18 8 17 11 C19 11 21 12 22 14 C18 13 15 14 12 17 C10 15 7 14 2 12 Z M15.5 7.5 A1 1 0 1 0 15.51 7.5 Z',
  arrow: 'M2 11 H16 V7 L23 12 L16 17 V13 H2 Z M2 8 L5 11 L2 11 Z M2 16 L5 13 L2 13 Z',
  spear: 'M22 2 L19 9 L16.5 6.5 Z M17.5 7.5 L18.9 8.9 L4.4 23.4 L3 22 Z M15 4 L20 9 L19 10 L14 5 Z',
  blade: 'M20 2 C21 8 16 15 9 18 L11 20 L9.5 21.5 L7 19 L4 22 L2 20 L5 17 L2.5 14.5 L4 13 L6 15 C9 8 14 4 20 2 Z',
  dash: 'M2 5 L9 12 L2 19 L5 19 L12 12 L5 5 Z M10 5 L17 12 L10 19 L13 19 L20 12 L13 5 Z',
  burst: 'M12 1 L14 8 L20 4 L16 10 L23 12 L16 14 L20 20 L14 16 L12 23 L10 16 L4 20 L8 14 L1 12 L8 10 L4 4 L10 8 Z',
  beam: 'M2 9 H22 V15 H2 Z M2 11 H22 V13 H2 Z M5 6 L7 8 M17 16 L19 18',
  trap: 'M3 13 L5 9 L7 13 L9 9 L11 13 L13 9 L15 13 L17 9 L19 13 L21 9 V15 H3 Z M3 17 H21 V19 H3 Z',
  orb: 'M12 3 A9 9 0 1 1 11.99 3 Z M9 7 A3 3 0 1 0 9.01 7 Z',
  crescent: 'M15 2 A10 10 0 1 0 15 22 A12 12 0 0 1 15 2 Z',
  target: 'M12 2 A10 10 0 1 1 11.99 2 Z M12 5 A7 7 0 1 0 12.01 5 Z M12 8 A4 4 0 1 1 11.99 8 Z M12 10.5 A1.5 1.5 0 1 0 12.01 10.5 Z',
  mic: 'M9 2 H15 V12 C15 13.7 13.7 15 12 15 C10.3 15 9 13.7 9 12 Z M6 10 H7.5 V12 C7.5 14.5 9.5 16.5 12 16.5 C14.5 16.5 16.5 14.5 16.5 12 V10 H18 V12 C18 15 15.8 17.6 12.8 18 V21 H16 V22.5 H8 V21 H11.2 V18 C8.2 17.6 6 15 6 12 Z',
  wave: 'M2 12 C5 6 8 6 11 12 C14 18 17 18 20 12 L22 13 C19 20 14 20 11 14 C8 8 6 8 4 13 Z',
  swirl: 'M12 2 C17.5 2 22 6.5 22 12 C22 15.5 19 18 16 18 C13 18 11 16 11 13.5 C11 11.5 12.5 10 14.3 10 C15.7 10 16.8 11 16.8 12.3 C16.8 13.3 16 14 15 14 C15.8 15.5 19 15 19 12 C19 8.1 15.9 5 12 5 C7.6 5 4 8.6 4 13 C4 17.4 7.6 21 12 21 V23 C5.9 23 1 18.1 1 12 C1 6.5 5.9 2 12 2 Z',
  fist: 'M5 9 C5 7.9 5.9 7 7 7 H17 C18.7 7 20 8.3 20 10 V15 C20 18.9 16.9 22 13 22 H11 C7.7 22 5 19.3 5 16 Z M7 3 H10 V7 H7 Z M11 2 H14 V7 H11 Z M15 3 H18 V7 H15 Z',
  sun: 'M12 6.5 A5.5 5.5 0 1 1 11.99 6.5 Z M11 0.5 H13 V4.5 H11 Z M11 19.5 H13 V23.5 H11 Z M0.5 11 H4.5 V13 H0.5 Z M19.5 11 H23.5 V13 H19.5 Z M3.4 4.8 L4.8 3.4 L7.6 6.2 L6.2 7.6 Z M16.4 17.8 L17.8 16.4 L20.6 19.2 L19.2 20.6 Z M19.2 3.4 L20.6 4.8 L17.8 7.6 L16.4 6.2 Z M6.2 16.4 L7.6 17.8 L4.8 20.6 L3.4 19.2 Z',
  claw: 'M5 21 C5 13 7 6 12 2 C10 7 10 12 11 15 C12 10 14 6 18 3 C16 8 15.5 13 16 17 C17 14 19 12 22 11 C19 15 18 19 17 22 Z',
};

const THEMES: [RegExp, string][] = [
  [/micro|question au/, 'mic'],
  [/bouclier|egide|parade|barriere|rempart|protege|ecorce/, 'shield'],
  [/solaire|soleil|\baube\b|lumineu|jugement solaire/, 'sun'],
  [/soin|soigne|benedict|priere|regenere/, 'heal'],
  [/piege/, 'trap'],
  [/corbeau|nuee/, 'bird'],
  [/rayon|faisceau|barrage|lien/, 'beam'],
  [/orbe/, 'orb'],
  [/nova|explos|onde|seisme|fracas|deflagr|effondr|pietin|censure/, 'burst'],
  [/blizzard|tempete|tourbillon|hurlement|cyclone/, 'swirl'],
  [/charge|bond|saut|ruee|roulade|teleport|elan|fonce|blink|fente|breche|envol/, 'dash'],
  [/lance|percant|empal|harpon/, 'spear'],
  [/fleche|balle|volee|tir |carreau|projectile/, 'arrow'],
  [/lame|tranch|epee|execution|taille/, 'blade'],
  [/marque|cible|traque|mort certaine/, 'target'],
  [/invisib|ombre|furtiv|tenebre/, 'crescent'],
  [/givre|glace|glaci|gel\b|froid/, 'snow'],
  [/eclair|foudre|orage|chaine|electr/, 'bolt'],
  [/feu|flamme|brul|embras|incendi|dune/, 'flame'],
  [/griffe|saignement|morsure|defenses/, 'claw'],
  [/poing|coup|ecrase|impact|frappe/, 'fist'],
];

const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

/** Nom du glyphe d'un sort : thème trouvé dans le nom, puis la description, sinon d'après le ciblage. */
export function abilityGlyphName(ability: Pick<AbilityDef, 'name' | 'description' | 'slot' | 'targeting'>): string {
  for (const text of [norm(ability.name), norm(ability.description)]) for (const [re, g] of THEMES) if (re.test(text)) return g;
  if (ability.slot === 'P') return 'passive';
  return ability.targeting in TARGET_GLYPH ? ability.targeting : 'self';
}

export function abilityGlyph(ability: Pick<AbilityDef, 'name' | 'description' | 'slot' | 'targeting'>): string {
  const n = abilityGlyphName(ability);
  return THEME_GLYPH[n] ?? TARGET_GLYPH[n] ?? TARGET_GLYPH.self!;
}

