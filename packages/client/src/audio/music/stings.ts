/**
 * Jingles d'élimination (originaux, synthétisés) qui remplacent les voix : plus la série est longue,
 * plus le jingle est riche (plus aigu, plus de couches : cloches → cuivres → tambours → chœur → cymbales).
 * Allié = majeur et lumineux ; ennemi = mineur et sombre ; ton propre kill = plus fort et scintillant.
 */
import { bass, bell, brass, choir, crash, pad, pluck, swell, taiko, type MusicBus } from './instruments.ts';

export type StingSide = 'self' | 'ally' | 'enemy';

/** Arpège montant (demi-tons depuis la tonique) : majeur pour nous, mineur pour l'ennemi. */
const MAJOR = [0, 4, 7, 12, 16, 19, 24];
const MINOR = [0, 3, 7, 12, 15, 19, 24];

/**
 * `level` 0..9 (voir stingLevel) : nombre de notes, couches et hauteur montent avec le niveau.
 * Tonique : La4 (69) pour un kill simple, +1 demi-ton par niveau (jusqu'à Fa#5).
 */
export function killSting(bus: MusicBus, t: number, level: number, side: StingSide): void {
  const enemy = side === 'enemy';
  const scale = enemy ? MINOR : MAJOR;
  const root = (enemy ? 57 : 69) + Math.min(9, level);
  const vol = side === 'self' ? 1.25 : enemy ? 0.75 : 1;
  const step = Math.max(0.055, 0.1 - level * 0.005); // l'arpège s'accélère avec la série
  const notes = Math.min(scale.length, 2 + Math.ceil(level / 2));

  // 1. arpège de cloches (toujours) — signature du jingle
  for (let i = 0; i < notes; i++) bell(bus, root + scale[i]!, t + i * step, 0.045 * vol, 1.6 + level * 0.15);
  const top = t + (notes - 1) * step;

  // 2. coup sourd : présent dès le kill simple, plus lourd ensuite
  taiko(bus, t, (0.16 + level * 0.025) * vol, enemy ? 0.8 : 1);

  // 3. cuivres sur la note d'arrivée (dès le double)
  if (level >= 2) brass(bus, [root, root + scale[2]!, root + 12], top, 0.5 + level * 0.06, (0.026 + level * 0.003) * vol);

  // 4. basse + nappe (triple et plus)
  if (level >= 4) {
    bass(bus, root - 24, top, 1.2, 0.1 * vol);
    pad(bus, [root, root + scale[1]!, root + scale[2]!], top, 1.8, 0.03 * vol, 1.3);
    taiko(bus, t + step, 0.14 * vol, 1.25);
  }

  // 5. chœur + roulement (quadra et plus)
  if (level >= 6) {
    choir(bus, [root + 12, root + 12 + scale[2]!], top, 2.2, 0.03 * vol);
    for (let i = 0; i < 4; i++) taiko(bus, t + i * step * 0.5, (0.1 + i * 0.03) * vol, 1.15);
  }

  // 6. apothéose : cymbales, second accord de cuivres, cascade de cloches (penta / série légendaire)
  if (level >= 8) {
    crash(bus, top, 0.06 * vol);
    brass(bus, [root + 5, root + 9, root + 12 + 5], top + 0.45, 0.9, 0.034 * vol);
    brass(bus, [root + 7, root + 11, root + 12 + 7], top + 0.8, 1.4, 0.036 * vol);
    for (let i = 0; i < 5; i++) bell(bus, root + 24 + scale[i % scale.length]!, top + 0.5 + i * 0.07, 0.03 * vol, 2.5);
  }

  // scintillement de ton propre kill
  if (side === 'self') for (let i = 0; i < 3; i++) pluck(bus, root + 24 + scale[(i * 2) % scale.length]!, top + 0.06 + i * 0.05, 0.02);
}

/** Premier sang : quinte ascendante solennelle, tambour grave, souffle de cymbale. */
export function firstBloodSting(bus: MusicBus, t: number, side: StingSide): void {
  const enemy = side === 'enemy';
  const r = enemy ? 50 : 62;
  swell(bus, t, 0.5, 0.05);
  taiko(bus, t + 0.5, 0.36, 0.75);
  brass(bus, [r, r + 7], t + 0.5, 0.7, 0.04);
  brass(bus, [r + 5, r + 12], t + 0.85, 0.5, 0.036);
  brass(bus, enemy ? [r + 3, r + 7, r + 12] : [r + 4, r + 7, r + 12], t + 1.15, 1.6, 0.045);
  bass(bus, r - 24, t + 1.15, 1.6, 0.11);
  bell(bus, r + 24, t + 1.15, 0.05, 2.8);
  crash(bus, t + 1.15, 0.05);
}
