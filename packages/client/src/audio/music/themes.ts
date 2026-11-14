/**
 * Thèmes musicaux originaux de League of Vibes (compositions écrites pour le jeu, en Ré mineur).
 * Un thème = tempo + boucle de N mesures de 16 doubles-croches + fonction qui joue un pas.
 * `intensity` (0..1) ajoute des couches (percussions, cuivres, cordes) pendant les combats.
 */
import { bass, bell, brass, choir, crash, flute, kalimba, pad, pluck, snare, softPad, strings, swell, taiko, type MusicBus } from './instruments.ts';

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

// ------------------------------------------------------------------ PARTIE : « Sous la canopée » (ambiance douce, générative)
// Ré dorien, tempo lent, harmonie qui change toutes les 2 mesures. Chaque boucle tire une progression
// et des motifs différents (aléatoire déterministe par boucle/mesure) : rien ne se répète à l'identique.
// Le combat ajoute seulement de légères couches (pulsation sourde, arpège feutré) : jamais de cuivres ni de caisse claire.

/** Accords ouverts et doux (nappes) + fondamentale grave. */
const AMB: Record<string, { notes: number[]; root: number }> = {
  Dm9: { notes: [57, 60, 64, 65], root: 38 },
  Fmaj9: { notes: [57, 60, 64, 67], root: 41 },
  G69: { notes: [57, 59, 62, 64], root: 43 },
  Am7: { notes: [55, 60, 64, 69], root: 45 },
  Cadd9: { notes: [55, 62, 64, 67], root: 48 },
  Bbmaj7: { notes: [57, 62, 65, 69], root: 46 },
};
const AMB_PROGS: string[][] = [
  ['Dm9', 'Fmaj9', 'Cadd9', 'G69'],
  ['Dm9', 'Bbmaj7', 'Fmaj9', 'Am7'],
  ['Bbmaj7', 'Cadd9', 'Dm9', 'Dm9'],
  ['Dm9', 'G69', 'Am7', 'Fmaj9'],
  ['Fmaj9', 'Am7', 'Bbmaj7', 'Cadd9'],
];
/** Pentatonique de Ré mineur (consonante sur tous les accords ci-dessus). */
const PENTA = [62, 65, 67, 69, 72, 74, 77, 79, 81, 84];
const MOTIF_STEPS = [0, 2, 4, 6, 8, 10, 12];

/** Aléatoire déterministe dans [0, 1) (pur, testable). */
export function hash3(a: number, b: number, c: number): number {
  let h = (a * 374761393 + b * 668265263 + c * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

/** Leitmotiv rare (une boucle sur quatre, au calme) : [pas, note, durée]. */
const GAME_MELODY: Phrase = [
  [0, 74, 6], [8, 72, 4], [12, 69, 8],
  [32, 67, 6], [40, 69, 10],
  [64, 74, 4], [70, 77, 4], [76, 74, 8],
  [96, 72, 6], [104, 69, 16],
];

export interface AmbientBar {
  chord: string;
  /** notes du motif : [pas dans la mesure, note MIDI, durée en pas] */
  motif: [number, number, number][];
}

/** Plan d'une mesure du thème de partie : accord + motif généré (pur, testé). */
export function gameBar(loop: number, bar: number): AmbientBar {
  const prog = AMB_PROGS[Math.floor(hash3(loop, 0, 7) * AMB_PROGS.length)]!;
  const chord = prog[Math.floor(bar / 2)]!;
  const motif: [number, number, number][] = [];
  // une boucle sur quatre (à partir de la 3e) : respiration, nappe seule
  const breath = loop % 4 === 2;
  // le motif apparaît dans ~40 % des mesures, jamais deux mesures de suite
  const prevPlayed = bar > 0 && hash3(loop, bar - 1, 1) < 0.4;
  if (breath || prevPlayed || hash3(loop, bar, 1) >= 0.4) return { chord, motif };
  const count = 2 + Math.floor(hash3(loop, bar, 2) * 3); // 2 à 4 notes
  let idx = 2 + Math.floor(hash3(loop, bar, 3) * 5);
  let pos = Math.floor(hash3(loop, bar, 4) * 3) * 2;
  for (let i = 0; i < count && pos < 16; i++) {
    const next = MOTIF_STEPS.find((p) => p > pos + 1 + Math.floor(hash3(loop, bar, 10 + i) * 4)) ?? 16;
    motif.push([pos, PENTA[idx]!, Math.max(2, next - pos)]);
    // petit pas mélodique (-2..+2), tendance descendante pour rester apaisé
    idx = Math.max(0, Math.min(PENTA.length - 1, idx + Math.floor(hash3(loop, bar, 20 + i) * 5) - 2 - (i % 2)));
    pos = next;
  }
  return { chord, motif };
}

const game: Theme = {
  id: 'game',
  bpm: 66,
  bars: 8,
  play(bus, step, loop, t, intensity, sd) {
    const bar = Math.floor(step / 16);
    const s = step % 16;
    const plan = gameBar(loop, bar);
    const ch = AMB[plan.chord]!;
    // nappe et basse : une par accord (toutes les 2 mesures), qui se chevauchent en fondu
    if (s === 0 && bar % 2 === 0) {
      softPad(bus, ch.notes, t, sd * 34, 0.022 + 0.008 * intensity, 0.8 + intensity * 0.5);
      bass(bus, ch.root, t, sd * 30, 0.05 + 0.02 * intensity);
    }
    // de temps en temps, une quinte grave au milieu de l'accord
    if (s === 0 && bar % 2 === 1 && hash3(loop, bar, 30) < 0.35) bass(bus, ch.root + 7, t, sd * 12, 0.035);
    // motif généré (kalimba ou flûte selon la boucle), atténué en combat
    if (intensity < 0.7) {
      const useFlute = hash3(loop, 0, 40) < 0.35;
      for (const [at, note, len] of plan.motif)
        if (at === s) {
          if (useFlute) flute(bus, note, t, len * sd, 0.022 * (1 - intensity));
          else kalimba(bus, note, t, 0.03 * (1 - intensity * 0.8));
        }
    }
    // leitmotiv, rarement et seulement au calme
    if (loop % 4 === 3 && intensity < 0.3) for (const [at, note, len] of GAME_MELODY) if (at === step) flute(bus, note, t, len * sd, 0.02);
    // étincelles aiguës très rares
    if (s === 6 && hash3(loop, bar, 50) < 0.12) bell(bus, PENTA[6 + Math.floor(hash3(loop, bar, 51) * 4)]! + 12, t, 0.012, 2.5);
    // combat 1 : pulsation sourde et lointaine
    if (intensity > 0.3 && (s === 0 || s === 8)) taiko(bus, t, 0.04 + 0.07 * intensity, 0.75);
    // combat 2 : arpège feutré en croches sur l'accord
    if (intensity > 0.55 && s % 2 === 0) pluck(bus, ch.notes[(s / 2) % ch.notes.length]! + 12, t, 0.008 + 0.01 * intensity);
    // combat 3 : souffle qui monte avant chaque nouvelle boucle
    if (intensity > 0.8 && step === 7 * 16) swell(bus, t, sd * 16, 0.025);
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
