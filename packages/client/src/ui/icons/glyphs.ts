/** Glyphes des icônes de sorts (pur, testé). Propriétaire : mistral-ui. */
import type { AbilityDef, ItemDef } from '@lov/shared';
import { hashString } from '../logic/format.ts';

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


// ------------------------------------------------------------------ objets
export const ITEM_GLYPH: Record<string, string> = {
  amulet: 'M6 2 C6 8 9 11 12 11 C15 11 18 8 18 2 L16.5 2 C16.5 7 14.5 9.5 12 9.5 C9.5 9.5 7.5 7 7.5 2 Z M12 12 L16 16.5 L12 22 L8 16.5 Z',
  belt: 'M2 9 H22 V15 H2 Z M9 7.5 H15 V16.5 H9 Z M11 10 H13 V14 H11 Z',
  gloves: 'M6 22 V12 L4 8 L5.5 7 L8 10 V4 H10 V10 H11 V3 H13 V10 H14 V4 H16 V11 H17 V7 H19 V14 C19 18 17 22 14 22 Z',
  helm: 'M4 14 C4 7 7.6 3 12 3 C16.4 3 20 7 20 14 V20 H15 V14 H9 V20 H4 Z M11 4 H13 V14 H11 Z',
  scythe: 'M3 22 L15 4 L16.5 5 L4.5 23 Z M15 4 C19 3 22 5 23 9 C20 7 17.5 7 15.5 8 Z',
  thorn: 'M11 2 H13 V22 H11 Z M13 6 L18 3 L16 7 Z M11 10 L6 7 L8 11 Z M13 14 L19 12 L16 16 Z M11 18 L5 16 L8 20 Z',
  sun: 'M12 6.5 A5.5 5.5 0 1 1 11.99 6.5 Z M11 0.5 H13 V4.5 H11 Z M11 19.5 H13 V23.5 H11 Z M0.5 11 H4.5 V13 H0.5 Z M19.5 11 H23.5 V13 H19.5 Z M3.4 4.8 L4.8 3.4 L7.6 6.2 L6.2 7.6 Z M16.4 17.8 L17.8 16.4 L20.6 19.2 L19.2 20.6 Z M19.2 3.4 L20.6 4.8 L17.8 7.6 L16.4 6.2 Z M6.2 16.4 L7.6 17.8 L4.8 20.6 L3.4 19.2 Z',
  serpent: 'M5 20 C5 16 10 16 10 13 C10 10 5 10 5 7 C5 4 8 2 11 2 C15 2 18 4 18 6.5 L16 6.5 C16 5 14 4 11 4 C9 4 7 5 7 7 C7 9 12 9 12 13 C12 17 7 17 7 20 C7 21 8 22 10 22 H20 V20 H10 Z M15.5 4.6 A1 1 0 1 0 15.51 4.6 Z',
  crossbow: 'M2 6 C8 4 16 4 22 6 L21.5 7.5 C16 6 8 6 2.5 7.5 Z M11 5 H13 V22 H11 Z M8 18 L12 22 L16 18 Z',
  cyclone: 'M3 5 H21 V7 H3 Z M5 9 H19 V11 H5 Z M7 13 H17 V15 H7 Z M9 17 H15 V19 H9 Z M11 21 H13 V23 H11 Z',
  mask: 'M3 6 C7 4 17 4 21 6 C21 13 17 19 12 19 C7 19 3 13 3 6 Z M6.5 9 C7.5 8 9.5 8 10.5 9.5 C9.5 11 7.5 11 6.5 9 Z M13.5 9.5 C14.5 8 16.5 8 17.5 9 C16.5 11 14.5 11 13.5 9.5 Z',
  infinity: 'M7 7 C10 7 11 10 12 12 C13 10 14 7 17 7 C20 7 22 9.2 22 12 C22 14.8 20 17 17 17 C14 17 13 14 12 12 C11 14 10 17 7 17 C4 17 2 14.8 2 12 C2 9.2 4 7 7 7 Z M7 9 C5.2 9 4 10.3 4 12 C4 13.7 5.2 15 7 15 C9 15 10 13 11 12 C10 11 9 9 7 9 Z M17 9 C15 9 14 11 13 12 C14 13 15 15 17 15 C18.8 15 20 13.7 20 12 C20 10.3 18.8 9 17 9 Z',
  flask: 'M9 2 H15 V4 H14 V9 L20 19 C20.8 20.5 19.8 22 18 22 H6 C4.2 22 3.2 20.5 4 19 L10 9 V4 H9 Z M7 17 L5.8 19.5 C5.6 20 5.9 20.5 6.5 20.5 H17.5 C18.1 20.5 18.4 20 18.2 19.5 L17 17 Z',
  gauntlet: 'M5 9 C5 7.9 5.9 7 7 7 H17 C18.7 7 20 8.3 20 10 V15 C20 18.9 16.9 22 13 22 H11 C7.7 22 5 19.3 5 16 Z M7 3 H10 V7 H7 Z M11 2 H14 V7 H11 Z M15 3 H18 V7 H15 Z',
  mountain: 'M2 21 L9 7 L13 14 L16 9 L22 21 Z M9 7 L11 11 L9 10 L7 11 Z',
  moon: 'M15 2 A10 10 0 1 0 15 22 A12 12 0 0 1 15 2 Z',
  ghost: 'M12 2 C17 2 20 6 20 11 V22 L17.3 19.5 L14.7 22 L12 19.5 L9.3 22 L6.7 19.5 L4 22 V11 C4 6 7 2 12 2 Z M9 9 A1.6 1.6 0 1 0 9.01 9 Z M15 9 A1.6 1.6 0 1 0 15.01 9 Z',
  drop: 'M12 2 C15 7 19 11 19 15 C19 19 15.9 22 12 22 C8.1 22 5 19 5 15 C5 11 9 7 12 2 Z M9 14 C9 17 10.5 19 13 19 C11 18 10.5 16 10.5 14 Z',
  fang: 'M4 3 H20 L18 9 L15 8 L13.5 17 L12 9.5 L10.5 17 L9 8 L6 9 Z',
  skull: 'M12 2 C17 2 21 5.8 21 10.5 C21 13.4 19.5 15.5 17.5 16.8 V20 C17.5 21.1 16.6 22 15.5 22 H8.5 C7.4 22 6.5 21.1 6.5 20 V16.8 C4.5 15.5 3 13.4 3 10.5 C3 5.8 7 2 12 2 Z M8.5 9 A2.2 2.2 0 1 0 8.51 9 Z M15.5 9 A2.2 2.2 0 1 0 15.51 9 Z',
  spear: 'M22 2 L19 9 L16.5 6.5 Z M17.5 7.5 L18.9 8.9 L4.4 23.4 L3 22 Z M15 4 L20 9 L19 10 L14 5 Z',
  compass: 'M12 2 A10 10 0 1 1 11.99 2 Z M12 4 A8 8 0 1 0 12.01 4 Z M12 5 L14.5 12 L12 19 L9.5 12 Z',
  sword: 'M18 2 L22 2 L22 6 L10 18 L12 20 L10.5 21.5 L8 19 L5 22 L2 19 L5 16 L2.5 13.5 L4 12 L6 14 Z',
  blade: 'M20 2 C21 8 16 15 9 18 L11 20 L9.5 21.5 L7 19 L4 22 L2 20 L5 17 L2.5 14.5 L4 13 L6 15 C9 8 14 4 20 2 Z',
  tome: 'M4 3 H17 C18.7 3 20 4.3 20 6 V21 H6.5 C5.1 21 4 19.9 4 18.5 Z M6.5 17.5 C5.9 17.5 5.5 18 5.5 18.5 C5.5 19 5.9 19.5 6.5 19.5 H18.5 V17.5 Z M9 7 H16 V8.5 H9 Z M9 10 H14 V11.5 H9 Z',
  gem: 'M7 3 H17 L22 9 L12 22 L2 9 Z M8 4.5 L4.6 8.8 H9.4 Z M16 4.5 L14.6 8.8 H19.4 Z M10 4.5 L12 8.5 L14 4.5 Z M5 10.3 L11 18.5 L9.6 10.3 Z M19 10.3 L14.4 10.3 L13 18.5 Z',
  boots: 'M6 2 H13 V12 L20 15 C21.5 15.7 22 17 22 18.5 V21 H4 V16 L6 13 Z M4 19 H22 V21 H4 Z',
  potion: 'M9 2 H15 V4 H14 V8 C17.5 9.3 20 12.4 20 16 C20 19.9 16.4 23 12 23 C7.6 23 4 19.9 4 16 C4 12.4 6.5 9.3 10 8 V4 H9 Z M6 15.5 C6.4 19 9 21 12 21 C15 21 17.6 19 18 15.5 Z',
  ring: 'M12 7 A8 8 0 1 1 11.99 7 Z M12 10 A5 5 0 1 0 12.01 10 Z M9 2 H15 L16.5 5 L12 8.5 L7.5 5 Z',
  shield: 'M12 2 L21 5.5 V11 C21 16.5 17 20.8 12 22.5 C7 20.8 3 16.5 3 11 V5.5 Z M12 5 V19.5 C15.5 18 18.5 15 18.5 11 V7.2 Z',
  eye: 'M12 5 C17 5 21 9 23 12 C21 15 17 19 12 19 C7 19 3 15 1 12 C3 9 7 5 12 5 Z M12 8 A4 4 0 1 0 12.01 8 Z M12 10.3 A1.7 1.7 0 1 1 11.99 10.3 Z',
  dagger: 'M12 1 L14.5 6 V14 H17 V16 H13.2 V19 L14 20 L12 23 L10 20 L10.8 19 V16 H7 V14 H9.5 V6 Z',
  cloak: 'M8 2 H16 L17 5 C20 9 21.5 15 22 22 H15 L12 16 L9 22 H2 C2.5 15 4 9 7 5 Z M10 3.5 L12 7 L14 3.5 Z',
  wand: 'M3 20 L15 8 L16.5 9.5 L4.5 21.5 Z M18 2 L19.2 4.8 L22 6 L19.2 7.2 L18 10 L16.8 7.2 L14 6 L16.8 4.8 Z',
  hammer: 'M4 4 H15 L18 7 V11 H15 L13.5 9.5 H12 V22 H9 V9.5 H4 Z',
  hourglass: 'M5 2 H19 V4 H17.5 C17.5 8 14.5 10.5 13 12 C14.5 13.5 17.5 16 17.5 20 H19 V22 H5 V20 H6.5 C6.5 16 9.5 13.5 11 12 C9.5 10.5 6.5 8 6.5 4 H5 Z M8.6 20 H15.4 C15 17.4 13 15.5 12 14.6 C11 15.5 9 17.4 8.6 20 Z',
  chest: 'M3 4 L8 2 C9 4 10.5 5 12 5 C13.5 5 15 4 16 2 L21 4 L19.5 10 L18 9.5 V22 H6 V9.5 L4.5 10 Z M9 11 H15 V12.5 H9 Z M9 14.5 H15 V16 H9 Z',
  orb: 'M12 3 A9 9 0 1 1 11.99 3 Z M9 7.5 A3 3 0 1 0 9.01 7.5 Z',
  heart: 'M12 21 C5 16 2 12.5 2 8.5 C2 5.5 4.5 3 7.5 3 C9.5 3 11 4 12 5.5 C13 4 14.5 3 16.5 3 C19.5 3 22 5.5 22 8.5 C22 12.5 19 16 12 21 Z',
  claw: 'M5 21 C5 13 7 6 12 2 C10 7 10 12 11 15 C12 10 14 6 18 3 C16 8 15.5 13 16 17 C17 14 19 12 22 11 C19 15 18 19 17 22 Z',
  crown: 'M2 7 L7 11 L12 3 L17 11 L22 7 L20 19 H4 Z M4 20.5 H20 V22 H4 Z',
  axe: 'M11 2 H13 V22 H11 Z M13 3 C18 3 21 6 21 10 C21 13 19 15 17 15 C17 12 15.5 10 13 10 Z M11 5 C8 5 6 6.5 6 8.5 C7.5 8 9.5 8 11 9 Z',
  censer: 'M11 1 H13 V4 H11 Z M12 4 L16 7 H8 Z M6 8 H18 C18 13 15.5 16 12 16 C8.5 16 6 13 6 8 Z M9 17 H15 L16 20 H8 Z M7 21 H17 V23 H7 Z',
  bow: 'M5 2 C17 4 20 10 20 12 C20 14 17 20 5 22 L6.5 20.2 C14 18.5 17.5 15 17.5 12 C17.5 9 14 5.5 6.5 3.8 Z M4 11 H19 V13 H4 Z M2 9 L5 12 L2 15 Z',
};

export const CATEGORY_COLOR: Record<string, [string, string]> = {
  starter: ['#5b8a5a', '#1d2b1c'],
  consumable: ['#b7434a', '#2a0f13'],
  boots: ['#8a6a46', '#2a1c10'],
  basic: ['#7f8ea3', '#1a1f29'],
  epic: ['#8b5bd6', '#1d1233'],
  legendary: ['#e2b64a', '#3a2408'],
  trinket: ['#d7c24a', '#2c2709'],
};

export function itemGlyphKey(item: ItemDef): string {
  const k = `${item.icon} ${item.id}`.toLowerCase();
  const table: [RegExp, string][] = [
    [/hourglass|sablier|stasis/, 'hourglass'],
    [/amulet|collier|pendentif/, 'amulet'],
    [/belt|ceinture|harness|harnais/, 'belt'],
    [/glove|gants?\b/, 'gloves'],
    [/gauntlet|titan|poing/, 'gauntlet'],
    [/helm|heaume|casque|warden|gardien/, 'helm'],
    [/reaper|faucheu|scythe|faux/, 'scythe'],
    [/thorn|epine|ronce/, 'thorn'],
    [/sunfire|sun|soleil|solaire/, 'sun'],
    [/serpent|vipere/, 'serpent'],
    [/crossbow|arbalete/, 'crossbow'],
    [/hurricane|ouragan|cyclone|tempete/, 'cyclone'],
    [/guise|masque|mask/, 'mask'],
    [/infinity|infini/, 'infinity'],
    [/vial|catalyst|fiole|catalyseur/, 'flask'],
    [/mountain|montagne|_mont\b|du_mont/, 'mountain'],
    [/nightfall|crepuscule|lune\b/, 'moon'],
    [/spectral|spectre|fantome/, 'ghost'],
    [/venom|venin|poison/, 'drop'],
    [/vampir|vampaxe|sanguin/, 'fang'],
    [/cursed|maudit/, 'skull'],
    [/piercing|percant/, 'spear'],
    [/traveler|voyageur/, 'compass'],
    [/royal|queen|reine/, 'crown'],
    [/berserker|waraxe/, 'axe'],
    [/buckler/, 'shield'],
    [/dirk|shiv|assassin|poignard/, 'dagger'],
    [/recurve/, 'bow'],
    [/mageband|lifering/, 'ring'],
    [/crown|couronne/, 'crown'],
    [/executioner|bourreau|axe|hache/, 'axe'],
    [/censer|encensoir|lantern/, 'censer'],
    [/veil|voile|negatron/, 'cloak'],
    [/rafale|gust/, 'blade'],
    [/colossus|colosse_masse|masse/, 'hammer'],
    [/cloth|rockplate/, 'chest'],
    [/potion|flask|elixir|biscuit/, 'potion'],
    [/ward|eye|trinket|totem/, 'eye'],
    [/boot|greave|tabi|shoe/, 'boots'],
    [/machete|claw|griffe/, 'claw'],
    [/bow|arc/, 'bow'],
    [/dagger|dague/, 'dagger'],
    [/blade|lame|edge|doran_blade/, 'blade'],
    [/sword|epee|saber/, 'sword'],
    [/tome|book|codex|grimoire/, 'tome'],
    [/ruby|crystal|gem|sapphire|saphir|jewel/, 'gem'],
    [/ring|anneau/, 'ring'],
    [/shield|bouclier|aegis|egide/, 'shield'],
    [/cloak|cape|mantle|negation/, 'cloak'],
    [/wand|rod|staff|baton|baguette|sceptre/, 'wand'],
    [/hammer|marteau|mace/, 'hammer'],
    [/armor|vest|plate|cuirasse|tissu|chest/, 'chest'],
    [/orb|pearl|perle/, 'orb'],
    [/heart|coeur|warmog/, 'heart'],
  ];
  for (const [re, g] of table) if (re.test(k)) return g;
  const s = item.stats;
  if (s.critChance) return 'cloak';
  if (s.attackSpeed) return 'dagger';
  if (s.ad || s.armorPen) return 'sword';
  if (s.ap || s.magicPen) return 'wand';
  if (s.armor) return 'chest';
  if (s.mr) return 'cloak';
  if (s.maxHp) return 'heart';
  if (s.maxMana) return 'orb';
  return 'gem';
}

export function itemTint(item: ItemDef): string {
  const s = item.stats;
  if (s.ap || s.magicPen) return '#c48bff';
  if (s.critChance || s.attackSpeed) return '#ffd16b';
  if (s.ad || s.armorPen || s.lifesteal) return '#ff9a5c';
  if (s.armor) return '#e8d08c';
  if (s.mr) return '#7fc6ff';
  if (s.maxHp || s.hpRegen) return '#ff6d7a';
  if (s.maxMana || s.manaRegen) return '#6fa8ff';
  if (s.moveSpeed) return '#9ff0c2';
  return '#f2f2f2';
}


const GEM_COLORS = ['#ff5a6e', '#59d8ff', '#7dff9a', '#ffd166', '#c79bff', '#ff9f43', '#e9f1ff'];

/** Variation visuelle déterministe par objet (teinte de fond, inclinaison, anneaux, gemme). */
export function itemVariation(item: Pick<ItemDef, 'id'>): { hue: number; lightX: number; ring: number; tilt: number; gemX: number; gem: string } {
  const h = hashString(item.id);
  return {
    hue: (h % 51) - 25,
    lightX: 25 + ((h >> 6) % 21),
    ring: (h >> 11) % 5,
    tilt: ((h >> 14) % 25) - 12,
    gemX: (h >> 19) % 2 ? 7 : 33,
    gem: GEM_COLORS[(h >> 21) % GEM_COLORS.length]!,
  };
}
