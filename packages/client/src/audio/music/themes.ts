/**
 * Thèmes musicaux originaux de League of Vibes (compositions écrites pour le jeu, en Ré mineur).
 * Un thème = tempo + boucle de N mesures de 16 doubles-croches + fonction qui joue un pas.
 * `intensity` (0..1) ajoute des couches (percussions, cuivres, cordes) pendant les combats.
 */
import { bass, bell, brass, choir, crash, flute, pad, pluck, snare, strings, swell, taiko, type MusicBus } from './instruments.ts';

export type ThemeId = 'menu' | 'draft' | 'game';

export interface Theme {
  id: ThemeId;
  bpm: number;
  bars: number;
  /** Joue le pas `step` (0..bars*16-1) de la boucle numéro `loop` à l'instant `t`. */
  play(bus: MusicBus, step: number, loop: number, t: number, intensity: number, stepDur: number): void;
}

// notes MIDI utiles (Ré mineur)
const D2 = 38, F2 = 41, G2 = 43, A2 = 45, Bb2 = 46, C3 = 48, D3 = 50;

/** Accords (voicings serrés, registre médium). */
const CH = {
  Dm: [62, 65, 69],
  Bb: [58, 62, 65],
  F: [60, 65, 69],
  C: [60, 64, 67],
  Gm: [58, 62, 67],
  A: [61, 64, 69],
  Am: [60, 64, 69],
  Dsus: [62, 67, 69],
} as const;
type ChordName = keyof typeof CH;
const ROOT: Record<ChordName, number> = { Dm: D2, Bb: Bb2, F: F2, C: C3 - 12, Gm: G2, A: A2, Am: A2, Dsus: D2 };

/** Mélodie : [pas dans la boucle, note MIDI, durée en pas]. */
type Phrase = [number, number, number][];

// ------------------------------------------------------------------ MENU : « La Faille des Vibes » (héroïque, ample)
const MENU_PROG: ChordName[] = ['Dm', 'Bb', 'F', 'C', 'Gm', 'Bb', 'A', 'A'];
const MENU_THEME: Phrase = [
  [0, 69, 4], [4, 74, 4], [8, 77, 6], [14, 76, 2],
  [16, 74, 8], [24, 77, 4], [28, 82, 4],
  [32, 81, 6], [38, 79, 2], [40, 77, 4], [44, 72, 4],
  [48, 76, 12], [60, 79, 4],
  [64, 82, 4], [68, 81, 4], [72, 79, 4], [76, 74, 4],
  [80, 77, 8], [88, 79, 4], [92, 81, 4],
  [96, 85, 6], [102, 81, 2], [104, 76, 8],
  [112, 81, 16],
];
const MENU_ARP = [0, 1, 2, 1, 2, 0, 1, 2]; // indices dans l'accord, en croches

const menu: Theme = {
  id: 'menu',
  bpm: 76,
  bars: 8,
  play(bus, step, loop, t, _i, sd) {
    const bar = Math.floor(step / 16);
    const s = step % 16;
    const chord = CH[MENU_PROG[bar]!];
    if (s === 0) {
      pad(bus, [...chord], t, sd * 16, 0.035, 0.9);
      bass(bus, ROOT[MENU_PROG[bar]!], t, sd * 14, 0.1);
      if (bar >= 4 && loop % 2 === 1) choir(bus, [chord[0] + 12, chord[2] + 12], t, sd * 16, 0.022);
    }
    // arpège en croches, plus présent à partir de la 2e boucle
    if (s % 2 === 0 && loop > 0) {
      const n = chord[MENU_ARP[s / 2]!]! + 12;
      pluck(bus, n, t, 0.022);
    }
    // percussions lentes et solennelles
    if (s === 0) taiko(bus, t, bar === 0 ? 0.3 : 0.18, 0.9);
    if (s === 10 && bar % 2 === 1) taiko(bus, t, 0.12, 1.2);
    if (step === 7 * 16) swell(bus, t, sd * 16, 0.05);
    if (step === 0 && loop > 0) crash(bus, t, 0.045);
    // le thème à la cloche, puis à la flûte une boucle sur deux
    for (const [at, note, len] of MENU_THEME)
      if (at === step) {
        if (loop % 2 === 0) bell(bus, note, t, 0.045, 2.2);
        else flute(bus, note, t, len * sd, 0.045);
      }
  },
};

// ------------------------------------------------------------------ DRAFT : « Le Choix » (tendu, ostinato)
const DRAFT_PROG: ChordName[] = ['Dm', 'Dm', 'Bb', 'C', 'Dm', 'Dm', 'Gm', 'A'];
const DRAFT_OSTINATO = [0, 0, 7, 0, 12, 0, 7, 10]; // demi-tons au-dessus de la fondamentale, en doubles-croches (x2)
const DRAFT_COUNTER: Phrase = [
  [64, 74, 4], [68, 76, 4], [72, 77, 8],
  [80, 79, 4], [84, 77, 4], [88, 76, 8],
  [96, 77, 4], [100, 79, 4], [104, 81, 12],
  [112, 85, 8], [120, 81, 8],
];

const draft: Theme = {
  id: 'draft',
  bpm: 112,
  bars: 8,
  play(bus, step, loop, t, _i, sd) {
    const bar = Math.floor(step / 16);
    const s = step % 16;
    const name = DRAFT_PROG[bar]!;
    const root = ROOT[name] + 24;
    // ostinato de cordes en doubles-croches
    if (s % 2 === 0) strings(bus, root + DRAFT_OSTINATO[(s / 2) % 8]!, t, sd * 1.8, 0.028);
    if (s === 0) {
      pad(bus, [...CH[name]], t, sd * 16, 0.025, 0.7);
      bass(bus, ROOT[name], t, sd * 7, 0.11);
    }
    if (s === 8) bass(bus, ROOT[name], t, sd * 7, 0.09);
    // tambours qui montent
    if (s === 0 || s === 6 || s === 10) taiko(bus, t, s === 0 ? 0.28 : 0.17, s === 0 ? 1 : 1.25);
    if (bar >= 4 && (s === 4 || s === 12)) snare(bus, t, 0.07);
    if (bar >= 6 && s % 2 === 1) snare(bus, t, 0.035);
    // cuivres sur les temps forts de la 2e moitié
    if (bar >= 4 && s === 0) brass(bus, [...CH[name]], t, sd * 3, 0.03);
    if (step === 7 * 16) swell(bus, t, sd * 16, 0.06);
    if (step === 0 && loop > 0) crash(bus, t, 0.05);
    for (const [at, note, len] of DRAFT_COUNTER) if (at === step && loop % 2 === 1) brass(bus, [note], t, len * sd, 0.026);
  },
};

// ------------------------------------------------------------------ PARTIE : « Sous la canopée » (exploration → combat)
const GAME_PROG: ChordName[] = ['Dm', 'F', 'C', 'Gm', 'Dm', 'Bb', 'C', 'A'];
const GAME_MELODY: Phrase = [
  [0, 74, 6], [6, 76, 2], [8, 77, 8],
  [16, 81, 6], [22, 79, 2], [24, 77, 4], [28, 76, 4],
  [32, 72, 8], [40, 76, 4], [44, 79, 4],
  [48, 77, 12], [60, 74, 4],
  [64, 74, 4], [68, 77, 4], [72, 81, 8],
  [80, 82, 6], [86, 81, 2], [88, 77, 8],
  [96, 79, 4], [100, 76, 4], [104, 72, 8],
  [112, 73, 8], [120, 76, 8],
];
const GAME_ARP = [0, 2, 1, 2];

const game: Theme = {
  id: 'game',
  bpm: 92,
  bars: 8,
  play(bus, step, loop, t, intensity, sd) {
    const bar = Math.floor(step / 16);
    const s = step % 16;
    const name = GAME_PROG[bar]!;
    const chord = CH[name];
    const calm = 1 - intensity;
    if (s === 0) {
      pad(bus, [...chord], t, sd * 16, 0.03 + 0.012 * intensity, 0.7 + intensity * 0.6);
      bass(bus, ROOT[name], t, sd * (intensity > 0.4 ? 3.5 : 14), 0.1);
    }
    // basse pulsée en combat
    if (intensity > 0.4 && (s === 4 || s === 8 || s === 12)) bass(bus, ROOT[name] + (s === 8 ? 12 : 0), t, sd * 3.5, 0.08);
    // arpège doux (exploration)
    if (s % 4 === 0 && calm > 0.2) pluck(bus, chord[GAME_ARP[s / 4]!]! + 12, t, 0.02 * calm + 0.008);
    // mélodie à la flûte, une boucle sur deux, seulement au calme
    if (loop % 2 === 1 && intensity < 0.5) for (const [at, note, len] of GAME_MELODY) if (at === step) flute(bus, note, t, len * sd, 0.038);
    // couche combat 1 : tambours
    if (intensity > 0.25) {
      if (s === 0 || s === 8) taiko(bus, t, 0.14 + 0.18 * intensity);
      if (s === 6 || s === 14) taiko(bus, t, 0.08 + 0.1 * intensity, 1.3);
    }
    // couche combat 2 : cordes en ostinato + caisse claire
    if (intensity > 0.5) {
      if (s % 2 === 0) strings(bus, ROOT[name] + 24 + (s % 8 === 6 ? 7 : 0), t, sd * 1.7, 0.02 + 0.015 * intensity);
      if (s === 4 || s === 12) snare(bus, t, 0.05 + 0.05 * intensity);
    }
    // couche combat 3 : cuivres héroïques et cymbales
    if (intensity > 0.75) {
      if (s === 0 || s === 10) brass(bus, [...chord], t, sd * (s === 0 ? 5 : 3), 0.028 + 0.02 * intensity);
      if (s === 0 && bar % 4 === 0) crash(bus, t, 0.04);
    }
    if (step === 7 * 16 + 8 && intensity > 0.3) swell(bus, t, sd * 8, 0.04);
  },
};

export const THEMES: Record<ThemeId, Theme> = { menu, draft, game };

// ------------------------------------------------------------------ jingles de fin
/** Fanfare de victoire (≈ 6 s) : montée de cuivres, timbales, accord majeur final. */
export function victory(bus: MusicBus, t: number): void {
  const b = 60 / 96 / 4;
  const seq: [number, number[]][] = [
    [0, [62, 66, 69]], // Ré majeur
    [6, [62, 67, 71]], // Sol
    [10, [64, 69, 73]], // La
    [16, [66, 69, 74]], // Ré majeur, aigu
  ];
  for (let i = 0; i < 12; i++) taiko(bus, t + i * b * 1.2, 0.08 + i * 0.012, 1.1);
  for (const [at, ch] of seq) {
    brass(bus, ch, t + at * b * 2, at === 16 ? 3.5 : b * 8, 0.05);
    bass(bus, ch[0]! - 24, t + at * b * 2, at === 16 ? 3.5 : b * 8, 0.12);
  }
  pad(bus, [62, 66, 69, 74], t + 32 * b, 4, 0.04, 1.2);
  choir(bus, [74, 78, 81], t + 32 * b, 4, 0.03);
  bell(bus, 86, t + 32 * b, 0.05, 3);
  bell(bus, 90, t + 34 * b, 0.04, 3);
  bell(bus, 93, t + 36 * b, 0.04, 3.5);
  crash(bus, t + 32 * b, 0.07);
  taiko(bus, t + 32 * b, 0.4, 0.8);
}

/** Complainte de défaite (≈ 6 s) : cordes graves descendantes, chœur sombre. */
export function defeat(bus: MusicBus, t: number): void {
  const b = 0.55;
  const seq: [number, number[]][] = [
    [0, [62, 65, 69]], // Ré m
    [2, [58, 62, 65]], // Si♭
    [4, [55, 58, 62]], // Sol m
    [6, [57, 61, 64]], // La
  ];
  for (const [at, ch] of seq) {
    pad(bus, ch, t + at * b, b * 2.6, 0.035, 0.6);
    bass(bus, ch[0]! - 24, t + at * b, b * 2, 0.1);
  }
  choir(bus, [57, 62], t + 6 * b, 3.5, 0.03);
  taiko(bus, t, 0.3, 0.7);
  taiko(bus, t + 6 * b, 0.25, 0.6);
  flute(bus, 74, t + 1 * b, b * 1.5, 0.035);
  flute(bus, 72, t + 2.5 * b, b * 1.5, 0.035);
  flute(bus, 70, t + 4 * b, b * 1.5, 0.035);
  flute(bus, 69, t + 5.5 * b, b * 3, 0.035);
}

/** Vérifie la cohérence d'une phrase (tests). */
export function phraseInBounds(phrase: Phrase, bars: number): boolean {
  return phrase.every(([at, note, len]) => at >= 0 && at < bars * 16 && len > 0 && note >= 36 && note <= 100);
}
export const PHRASES = { MENU_THEME, DRAFT_COUNTER, GAME_MELODY };
