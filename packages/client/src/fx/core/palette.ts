/**
 * Couleurs des FX : palette du champion (`palette` dans ses données) ou couleur propre au sort d'invocateur.
 */
import { CHAMPIONS } from '@lov/shared';
import { championOfKey } from '../keys.ts';

export interface FxPalette {
  primary: string;
  secondary: string;
  accent: string;
}

const SUMMONER_PALETTES: Record<string, FxPalette> = {
  sum_flash: { primary: '#ffe27a', secondary: '#c98a1a', accent: '#ffffff' },
  sum_ignite: { primary: '#ff7a1a', secondary: '#8a1c00', accent: '#ffd36b' },
  sum_heal: { primary: '#7dffa0', secondary: '#1f8a4c', accent: '#e8fff0' },
  sum_barrier: { primary: '#ffd970', secondary: '#a6761d', accent: '#fff6d6' },
  sum_exhaust: { primary: '#9a6bff', secondary: '#3a2266', accent: '#d8c9ff' },
  sum_teleport: { primary: '#c46bff', secondary: '#4b1f7a', accent: '#f0d9ff' },
  sum_smite: { primary: '#ffd34d', secondary: '#7a5a10', accent: '#fffbe6' },
  sum_ghost: { primary: '#8ff2ff', secondary: '#2a6b7a', accent: '#e6fdff' },
  sum_cleanse: { primary: '#a8f0ff', secondary: '#3b7f99', accent: '#ffffff' },
};

export const DEFAULT_PALETTE: FxPalette = { primary: '#ffe9b0', secondary: '#7a5a2a', accent: '#ffffff' };

export function paletteForKey(key: string): FxPalette {
  const champ = championOfKey(key);
  if (champ) return CHAMPIONS[champ]!.palette;
  return SUMMONER_PALETTES[key] ?? DEFAULT_PALETTE;
}

/** '#rrggbb' → [r, g, b] en espace sRGB 0..1. */
export function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', ''), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/** Conversion sRGB → linéaire (les shaders et le bloom travaillent en linéaire). */
export const srgbToLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
