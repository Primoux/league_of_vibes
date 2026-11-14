/**
 * Clips procéduraux génériques (humanoïde). Les champions peuvent remplacer n'importe quel clip.
 * Voir les conventions d'axes dans pose.ts (avant = +x, membre +z = vers l'avant).
 */
import { clamp01, easeOut, easeIn } from './ease.ts';
import { J, addRot, setOffset, setRot, type Clip, type ClipCtx, type ClipSet, type Pose } from './pose.ts';

const TAU = Math.PI * 2;

/** Bras légèrement écartés, coudes un peu pliés : posture de base. */
function rest(p: Pose, spread = 0.12): void {
  setRot(p, J.armL, spread, 0, 0.05);
  setRot(p, J.armR, -spread, 0, 0.05);
  setRot(p, J.foreL, 0, 0, 0.25);
  setRot(p, J.foreR, 0, 0, 0.25);
}

/** Repos : respiration, transfert de poids lent d'une jambe à l'autre, regard qui balaie. */
export const idle: Clip = {
  loop: true,
  duration: 5.2,
  sample(t, p) {
    rest(p);
    const breath = Math.sin((t / 2.6) * TAU);
    const shift = Math.sin((t / 5.2) * TAU);
    addRot(p, J.spine, 0, 0, -0.03 + breath * 0.025);
    addRot(p, J.armL, breath * 0.03, 0, 0);
    addRot(p, J.armR, -breath * 0.03, 0, 0);
    // le poids passe d'une jambe à l'autre : bassin qui roule, jambe libre légèrement fléchie
    setRot(p, J.hips, shift * 0.05, 0, 0);
    setRot(p, J.legL, -shift * 0.05, 0, 0.05 + Math.max(0, shift) * 0.08);
    setRot(p, J.legR, -shift * 0.05, 0, 0.05 + Math.max(0, -shift) * 0.08);
    setRot(p, J.shinL, 0, 0, -0.04 - Math.max(0, shift) * 0.16);
    setRot(p, J.shinR, 0, 0, -0.04 - Math.max(0, -shift) * 0.16);
    addRot(p, J.spine, -shift * 0.04, 0, 0);
    setRot(p, J.head, 0, Math.sin((t / 5.2) * TAU * 0.5 + 1) * 0.18, 0.02);
    setOffset(p, 0, breath * 0.012 - Math.abs(shift) * 0.015, shift * 0.02);
  },
};

/**
 * Course : enjambée modérée, genou plié en phase de balancier seulement, bassin qui pivote et épaules en
 * contre-rotation, bras pliés en opposition, tête stabilisée, deux rebonds par cycle.
 * `energy` règle l'amplitude (tanks lourds < 1, assassins > 1).
 */
export const run: Clip = {
  loop: true,
  duration: 0.66,
  sample(t, p, ctx) {
    const ph = (t / 0.66) * TAU;
    const s = Math.sin(ph);
    const c = Math.cos(ph);
    const e = ctx.energy;
    // jambes : la gauche est devant quand s > 0 ; elle avance (balancier) quand c > 0
    setRot(p, J.legL, 0.04, 0, 0.12 + s * 0.55 * e);
    setRot(p, J.legR, -0.04, 0, 0.12 - s * 0.55 * e);
    setRot(p, J.shinL, 0, 0, -0.18 - Math.max(0, c) * 0.95 * e);
    setRot(p, J.shinR, 0, 0, -0.18 - Math.max(0, -c) * 0.95 * e);
    // bassin et épaules en contre-rotation
    setRot(p, J.hips, Math.sin(ph * 2) * 0.03, s * 0.12 * e, 0);
    setRot(p, J.spine, 0, -s * 0.2 * e, -0.16 * e);
    setRot(p, J.head, 0, s * 0.08 * e, 0.12 * e); // la tête compense : le regard reste devant
    // bras pliés, en opposition des jambes
    setRot(p, J.armL, 0.14, 0, -s * 0.42 * e);
    setRot(p, J.armR, -0.14, 0, s * 0.42 * e);
    setRot(p, J.foreL, 0, 0, 0.95 + Math.max(0, s) * 0.35);
    setRot(p, J.foreR, 0, 0, 0.95 + Math.max(0, -s) * 0.35);
    // deux rebonds par cycle : point bas au contact du pied
    setOffset(p, 0, (0.5 - 0.5 * Math.cos(ph * 2)) * 0.06 * e - 0.05, 0);
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

/**
 * Frappe (au moment de l'événement autoAttack) : le corps entier participe — pas en avant, torsion du buste
 * qui traverse, bras qui balaie, puis accompagnement et retour.
 */
export const strike: Clip = {
  loop: false,
  duration: 0.4,
  sample(t, p, ctx) {
    rest(p);
    const hit = easeOut(t / 0.09);
    const back = easeIn((t - 0.16) / 0.24);
    const k = hit * (1 - back);
    const h = ctx.hand;
    const arm = h === 1 ? J.armR : J.armL;
    const fore = h === 1 ? J.foreR : J.foreL;
    const other = h === 1 ? J.armL : J.armR;
    addRot(p, arm, 0, 0, -0.5 + 2.1 * k);
    addRot(p, fore, 0, 0, 0.9 - 0.85 * k);
    addRot(p, other, 0, 0, -0.4 * k); // l'autre bras équilibre
    setRot(p, J.spine, 0, h * (0.4 - 0.9 * k), -0.25 * k);
    setRot(p, J.hips, 0, h * (0.12 - 0.25 * k), 0);
    setRot(p, J.head, 0, -h * 0.3 * k, 0.1 * k);
    // pas en avant pendant l'impact
    setRot(p, J.legL, 0, 0, 0.2 + 0.35 * k);
    setRot(p, J.shinL, 0, 0, -0.15 - 0.3 * k);
    setRot(p, J.legR, 0, 0, -0.15 - 0.25 * k);
    setRot(p, J.shinR, 0, 0, -0.2);
    setOffset(p, 0.12 * k, -0.07 * k, 0);
  },
};

/** Incantation générique : petit élan, poussée des deux mains vers l'avant, léger recul. */
export const cast: Clip = {
  loop: false,
  duration: 0.55,
  sample(t, p) {
    rest(p);
    const wind = easeOut(t / 0.12) * (1 - clamp01((t - 0.12) / 0.08));
    const push = easeOut((t - 0.12) / 0.1) * (1 - easeIn((t - 0.32) / 0.23));
    addRot(p, J.armR, -0.2 * push, 0, -0.3 * wind + 1.55 * push);
    addRot(p, J.armL, 0.2 * push, 0, -0.3 * wind + 1.3 * push);
    addRot(p, J.foreR, 0, 0, 0.5 * wind - 0.25 * push);
    addRot(p, J.foreL, 0, 0, 0.5 * wind - 0.2 * push);
    setRot(p, J.spine, 0, 0, 0.12 * wind - 0.18 * push);
    setRot(p, J.head, 0, 0, 0.08 * push);
    setRot(p, J.legL, 0, 0, 0.25 * push);
    setRot(p, J.legR, 0, 0, -0.15 * push);
    setOffset(p, -0.04 * wind + 0.06 * push, -0.03 * push, 0);
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

/** Mort : les genoux cèdent, puis chute en arrière avec un petit rebond au sol. */
export const death: Clip = {
  loop: false,
  duration: 1.0,
  sample(t, p) {
    const buckle = easeOut(t / 0.22);
    const fall = clamp01((t - 0.15) / 0.5);
    const f = fall * fall * (3 - 2 * fall);
    const bounce = t > 0.65 ? Math.sin(clamp01((t - 0.65) / 0.3) * Math.PI) * 0.08 * (1 - clamp01((t - 0.65) / 0.3)) : 0;
    setRot(p, J.shinL, 0, 0, -0.9 * buckle * (1 - f * 0.5));
    setRot(p, J.shinR, 0, 0, -0.7 * buckle * (1 - f * 0.5));
    setRot(p, J.legL, 0, 0, 0.5 * buckle);
    setRot(p, J.legR, 0, 0, 0.35 * buckle + 0.3 * f);
    setRot(p, J.head, 0, 0.3 * f, -0.3 * buckle + 0.5 * f);
    setRot(p, J.body, 0, 0, 1.4 * f - bounce);
    setRot(p, J.armL, 0.3 + 0.8 * f, 0, 0.4 * buckle + 0.4 * f);
    setRot(p, J.armR, -0.3 - 0.8 * f, 0, 0.4 * buckle + 0.4 * f);
    setRot(p, J.foreL, 0, 0, 0.5 * buckle);
    setRot(p, J.foreR, 0, 0, 0.5 * buckle);
    // le pivot du corps est aux pieds : on ne l'abaisse que pendant la flexion des genoux, sinon le corps
    // couché (≈ 80°) finirait sous le sol
    setOffset(p, -0.25 * f, -0.22 * buckle * (1 - f) + 0.04 * f, 0);
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
