/**
 * Styles de particules standard, partagés par tous les FX.
 * La plupart sont blancs : la couleur vient de la teinte passée à `emit` (palette du champion).
 */
import { CURVES } from '../core/curves.ts';
import type { ParticleStyle, ParticleSystem } from './ParticleSystem.ts';

export interface StdStyles {
  glow: ParticleStyle;
  glowPop: ParticleStyle;
  spark: ParticleStyle;
  ember: ParticleStyle;
  smoke: ParticleStyle;
  dust: ParticleStyle;
  ring: ParticleStyle;
  star: ParticleStyle;
  snow: ParticleStyle;
  feather: ParticleStyle;
  rune: ParticleStyle;
  shard: ParticleStyle;
  debris: ParticleStyle;
  cross: ParticleStyle;
  coin: ParticleStyle;
  flame: ParticleStyle;
  swirl: ParticleStyle;
  sparkles: ParticleStyle;
  darkWisp: ParticleStyle;
}

export function createStdStyles(ps: ParticleSystem): StdStyles {
  return {
    glow: ps.defineStyle({ texture: 'glow', alpha: CURVES.fadeOut }),
    glowPop: ps.defineStyle({ texture: 'glow', alpha: CURVES.quickInSlowOut, size: CURVES.popShrink }),
    spark: ps.defineStyle({ texture: 'streak', alpha: [[0, 1], [0.7, 0.8], [1, 0]], size: CURVES.shrink }),
    ember: ps.defineStyle({ texture: 'dot', alpha: CURVES.fadeInOut, size: CURVES.shrink }),
    smoke: ps.defineStyle({
      texture: 'smoke',
      blend: 'normal',
      color: [[0, 0.5, 0.5, 0.52], [1, 0.25, 0.25, 0.27]],
      alpha: [[0, 0], [0.15, 0.55], [1, 0]],
      size: CURVES.smokeGrow,
    }),
    dust: ps.defineStyle({
      texture: 'smoke',
      blend: 'normal',
      color: [[0, 0.45, 0.36, 0.25], [1, 0.3, 0.25, 0.18]],
      alpha: [[0, 0], [0.1, 0.6], [1, 0]],
      size: CURVES.smokeGrow,
    }),
    ring: ps.defineStyle({ texture: 'ring', alpha: CURVES.fadeOut, size: [[0, 0.2], [1, 1]] }),
    star: ps.defineStyle({ texture: 'star', alpha: CURVES.fadeInOut, size: CURVES.one }),
    snow: ps.defineStyle({ texture: 'snowflake', alpha: CURVES.fadeInOut, size: CURVES.one }),
    feather: ps.defineStyle({ texture: 'feather', blend: 'normal', alpha: CURVES.fadeInOut, size: CURVES.one }),
    rune: ps.defineStyle({ texture: 'rune', alpha: CURVES.fadeInOut, size: CURVES.grow }),
    shard: ps.defineStyle({ texture: 'shard', alpha: [[0, 1], [0.8, 1], [1, 0]], size: CURVES.one }),
    debris: ps.defineStyle({ texture: 'shard', blend: 'normal', alpha: [[0, 1], [0.8, 1], [1, 0]], size: CURVES.one }),
    cross: ps.defineStyle({ texture: 'cross', alpha: CURVES.fadeInOut, size: CURVES.popShrink }),
    coin: ps.defineStyle({ texture: 'coin', alpha: [[0, 1], [0.75, 1], [1, 0]], size: CURVES.one }),
    flame: ps.defineStyle({
      texture: 'flame',
      color: [[0, 3, 2.6, 1.6], [0.3, 2.4, 1.1, 0.25], [0.7, 1, 0.25, 0.05], [1, 0.2, 0.05, 0.02]],
      alpha: CURVES.fadeInOut,
      size: [[0, 0.6], [0.3, 1], [1, 0.3]],
    }),
    swirl: ps.defineStyle({ texture: 'swirl', alpha: CURVES.fadeInOut, size: CURVES.grow }),
    sparkles: ps.defineStyle({ texture: 'sparkles', alpha: CURVES.fadeInOut, size: CURVES.one }),
    darkWisp: ps.defineStyle({
      texture: 'smoke',
      blend: 'normal',
      color: [[0, 0.08, 0.04, 0.12], [1, 0.02, 0.01, 0.04]],
      alpha: [[0, 0], [0.2, 0.75], [1, 0]],
      size: CURVES.smokeGrow,
    }),
  };
}
