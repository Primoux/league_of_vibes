/**
 * Clips procéduraux génériques (humanoïde). Les champions peuvent remplacer n'importe quel clip.
 * Voir les conventions d'axes dans pose.ts (avant = +x, membre +z = vers l'avant).
 */
import { J, addRot, setOffset, setRot, type Clip, type ClipCtx, type ClipSet, type Pose } from './pose.ts';

const TAU = Math.PI * 2;
const clamp01 = (t: number): number => (t < 0 ? 0 : t > 1 ? 1 : t);
/** Cloche 0 → 1 → 0 sur [a, b]. */
const bell = (t: number, a: number, b: number): number => {
  const k = clamp01((t - a) / (b - a));
  return Math.sin(k * Math.PI);
};
const easeOut = (t: number): number => 1 - (1 - clamp01(t)) ** 3;
const easeIn = (t: number): number => clamp01(t) ** 2;

/** Bras légèrement écartés, coudes un peu pliés : posture de base. */
function rest(p: Pose, spread = 0.12): void {
  setRot(p, J.armL, spread, 0, 0.05);
  setRot(p, J.armR, -spread, 0, 0.05);
  setRot(p, J.foreL, 0, 0, 0.25);
  setRot(p, J.foreR, 0, 0, 0.25);
}

export const idle: Clip = {
  loop: true,
  duration: 2.6,
  sample(t, p) {
    rest(p);
    const b = Math.sin((t / 2.6) * TAU);
    addRot(p, J.spine, 0, 0, -0.03 + b * 0.02);
    addRot(p, J.armL, b * 0.03, 0, 0);
    addRot(p, J.armR, -b * 0.03, 0, 0);
    setRot(p, J.head, 0, Math.sin((t / 5.2) * TAU) * 0.12, 0);
    setOffset(p, 0, b * 0.02, 0);
  },
};

export const run: Clip = {
  loop: true,
  duration: 0.62,
  sample(t, p, ctx) {
    const ph = (t / 0.62) * TAU;
    const s = Math.sin(ph);
    const e = ctx.energy;
    setRot(p, J.legL, 0, 0, s * 0.75 * e);
    setRot(p, J.legR, 0, 0, -s * 0.75 * e);
    // le genou plie quand la jambe repasse vers l'avant
    setRot(p, J.shinL, 0, 0, -0.15 - Math.max(0, -Math.cos(ph)) * 1.1 * e);
    setRot(p, J.shinR, 0, 0, -0.15 - Math.max(0, Math.cos(ph)) * 1.1 * e);
    setRot(p, J.armL, 0.15, 0, -s * 0.6 * e);
    setRot(p, J.armR, -0.15, 0, s * 0.6 * e);
    setRot(p, J.foreL, 0, 0, 0.7);
    setRot(p, J.foreR, 0, 0, 0.7);
    setRot(p, J.spine, 0, s * 0.12, -0.18);
    setRot(p, J.head, 0, -s * 0.08, 0.1);
    setOffset(p, 0, Math.abs(Math.cos(ph)) * 0.07 * e - 0.03, 0);
  },
};

/** Armement d'une auto-attaque, tenu en attendant l'impact (`strike`). */
export const attackReady: Clip = {
  loop: true,
  duration: 1,
  sample(t, p, ctx) {
    rest(p);
    const k = easeOut(t / 0.25);
    const arm = ctx.hand === 1 ? J.armR : J.armL;
    const fore = ctx.hand === 1 ? J.foreR : J.foreL;
    addRot(p, arm, 0, 0, -0.5 * k);
    addRot(p, fore, 0, 0, 0.9 * k);
    setRot(p, J.spine, 0, ctx.hand * 0.35 * k, 0.05 * k);
    setRot(p, J.legL, 0, 0, 0.25);
    setRot(p, J.legR, 0, 0, -0.2);
    setRot(p, J.shinR, 0, 0, -0.25);
  },
};

/** Frappe (au moment de l'événement autoAttack) puis retour. */
export const strike: Clip = {
  loop: false,
  duration: 0.35,
  sample(t, p, ctx) {
    rest(p);
    const hit = easeOut(t / 0.08);
    const back = easeIn((t - 0.12) / 0.23);
    const k = hit * (1 - back);
    const arm = ctx.hand === 1 ? J.armR : J.armL;
    const fore = ctx.hand === 1 ? J.foreR : J.foreL;
    addRot(p, arm, 0, 0, -0.5 + 2.0 * k);
    addRot(p, fore, 0, 0, 0.9 - 0.8 * k);
    setRot(p, J.spine, 0, ctx.hand * (0.35 - 0.7 * k), -0.2 * k);
    setRot(p, J.legL, 0, 0, 0.25);
    setRot(p, J.legR, 0, 0, -0.2);
    setOffset(p, 0.08 * k, -0.04 * k, 0);
  },
};

/** Incantation générique : bras levés vers l'avant. */
export const cast: Clip = {
  loop: false,
  duration: 0.5,
  sample(t, p) {
    rest(p);
    const k = bell(t, 0, 0.5);
    addRot(p, J.armR, -0.2 * k, 0, 1.5 * k);
    addRot(p, J.armL, 0.2 * k, 0, 1.2 * k);
    addRot(p, J.foreR, 0, 0, -0.2 * k);
    setRot(p, J.spine, 0, 0, 0.12 * k);
    setRot(p, J.head, 0, 0, 0.1 * k);
  },
};

/** Ultime : bras au ciel puis abattus. */
export const castR: Clip = {
  loop: false,
  duration: 0.8,
  sample(t, p) {
    rest(p);
    const up = easeOut(t / 0.35);
    const down = easeIn((t - 0.4) / 0.2);
    const k = up * (1 - down);
    addRot(p, J.armR, -0.3, 0, 2.8 * k + 1.0 * down * (1 - easeIn((t - 0.6) / 0.2)));
    addRot(p, J.armL, 0.3, 0, 2.8 * k + 1.0 * down * (1 - easeIn((t - 0.6) / 0.2)));
    setRot(p, J.spine, 0, 0, 0.25 * k - 0.3 * down * (1 - easeIn((t - 0.6) / 0.2)));
    setRot(p, J.head, 0, 0, 0.25 * k);
    setOffset(p, 0, 0.06 * k - 0.08 * down, 0);
  },
};

export const dash: Clip = {
  loop: true,
  duration: 0.4,
  sample(_t, p) {
    setRot(p, J.body, 0, 0, -0.35);
    setRot(p, J.armL, 0.4, 0, -0.9);
    setRot(p, J.armR, -0.4, 0, -0.9);
    setRot(p, J.legL, 0, 0, 0.7);
    setRot(p, J.shinL, 0, 0, -1.2);
    setRot(p, J.legR, 0, 0, -0.5);
    setRot(p, J.shinR, 0, 0, -0.3);
    setOffset(p, 0, 0.05, 0);
  },
};

export const death: Clip = {
  loop: false,
  duration: 0.9,
  sample(t, p) {
    const k = easeIn(t / 0.7);
    const fall = Math.min(1, k * 1.1);
    setRot(p, J.body, 0, 0, 1.35 * fall);
    setRot(p, J.armL, 0.9 * fall, 0, 0.6 * fall);
    setRot(p, J.armR, -0.9 * fall, 0, 0.6 * fall);
    setRot(p, J.legL, 0, 0, 0.5 * fall);
    setRot(p, J.shinL, 0, 0, -0.9 * fall);
    setRot(p, J.head, 0, 0.3 * fall, 0.4 * fall);
    setOffset(p, -0.2 * fall, -0.55 * fall, 0);
  },
};

/** Rappel : genou à terre, mains jointes. */
export const recall: Clip = {
  loop: true,
  duration: 3,
  sample(t, p) {
    const k = easeOut(t / 0.5);
    const b = Math.sin((t / 1.5) * TAU) * 0.03;
    setRot(p, J.legL, 0, 0, 1.2 * k);
    setRot(p, J.shinL, 0, 0, -1.5 * k);
    setRot(p, J.legR, 0, 0, -0.3 * k);
    setRot(p, J.shinR, 0, 0, -1.7 * k);
    setRot(p, J.armL, 0.5 * k, 0, 0.9 * k);
    setRot(p, J.armR, -0.5 * k, 0, 0.9 * k);
    setRot(p, J.foreL, 0, 0, 1.1 * k);
    setRot(p, J.foreR, 0, 0, 1.1 * k);
    setRot(p, J.spine, 0, 0, -0.15 * k + b);
    setRot(p, J.head, 0, 0, -0.25 * k);
    setOffset(p, 0, -0.42 * k, 0);
  },
};

/** Étourdi : vacille. */
export const stunned: Clip = {
  loop: true,
  duration: 1.2,
  sample(t, p) {
    rest(p, 0.3);
    const s = Math.sin((t / 1.2) * TAU);
    setRot(p, J.body, s * 0.08, 0, 0.1);
    setRot(p, J.head, 0, s * 0.3, 0.25);
    setOffset(p, 0, -0.05, 0);
  },
};

export const GENERIC_CLIPS: ClipSet = { idle, run, attackReady, strike, cast, cast_R: castR, dash, death, recall, stunned };

/** Contexte par défaut. */
export const DEFAULT_CTX: ClipCtx = { hand: 1, energy: 1 };
