/**
 * Courbes et dégradés pour les particules (logique pure, testée).
 * Les styles de particules sont « cuits » dans une texture (CURVE_RES échantillons par ligne) lue par le GPU.
 */

/** Clé de courbe : [t ∈ 0..1, valeur]. Les clés doivent être triées par t croissant. */
export type CurveKey = readonly [t: number, v: number];
export type Curve = readonly CurveKey[];
/** Arrêt de dégradé : [t, r, g, b] en couleur linéaire (peut dépasser 1 pour le bloom HDR). */
export type ColorStop = readonly [t: number, r: number, g: number, b: number];

export const CURVE_RES = 64;

/** Échantillonne une courbe par interpolation linéaire (constante hors des bornes). */
export function sampleCurve(c: Curve, t: number): number {
  const n = c.length;
  if (n === 0) return 1;
  const first = c[0]!;
  if (t <= first[0]) return first[1];
  for (let i = 1; i < n; i++) {
    const b = c[i]!;
    if (t <= b[0]) {
      const a = c[i - 1]!;
      const span = b[0] - a[0];
      const k = span > 0 ? (t - a[0]) / span : 1;
      return a[1] + (b[1] - a[1]) * k;
    }
  }
  return c[n - 1]![1];
}

/** Échantillonne un dégradé dans `out` (r, g, b) sans allocation. */
export function sampleGradient(stops: readonly ColorStop[], t: number, out: { r: number; g: number; b: number }): void {
  const n = stops.length;
  if (n === 0) {
    out.r = out.g = out.b = 1;
    return;
  }
  let a = stops[0]!;
  let b = a;
  if (t > a[0]) {
    b = stops[n - 1]!;
    for (let i = 1; i < n; i++) {
      if (t <= stops[i]![0]) {
        a = stops[i - 1]!;
        b = stops[i]!;
        break;
      }
      a = stops[i]!;
    }
  }
  const span = b[0] - a[0];
  const k = span > 0 ? Math.min(1, Math.max(0, (t - a[0]) / span)) : 0;
  out.r = a[1] + (b[1] - a[1]) * k;
  out.g = a[2] + (b[2] - a[2]) * k;
  out.b = a[3] + (b[3] - a[3]) * k;
}

/**
 * Écrit 2 lignes RGBA de CURVE_RES texels dans `out` à partir de `row` :
 * ligne `row` = couleur (rgb) + alpha, ligne `row + 1` = taille dans le canal r.
 */
export function bakeStyle(color: readonly ColorStop[], alpha: Curve, size: Curve, out: Float32Array, row: number): void {
  const tmp = { r: 0, g: 0, b: 0 };
  const stride = CURVE_RES * 4;
  const c0 = row * stride;
  const s0 = (row + 1) * stride;
  for (let i = 0; i < CURVE_RES; i++) {
    const t = i / (CURVE_RES - 1);
    sampleGradient(color, t, tmp);
    out[c0 + i * 4] = tmp.r;
    out[c0 + i * 4 + 1] = tmp.g;
    out[c0 + i * 4 + 2] = tmp.b;
    out[c0 + i * 4 + 3] = sampleCurve(alpha, t);
    out[s0 + i * 4] = sampleCurve(size, t);
    out[s0 + i * 4 + 1] = 0;
    out[s0 + i * 4 + 2] = 0;
    out[s0 + i * 4 + 3] = 1;
  }
}

/** Courbes usuelles. */
export const CURVES = {
  one: [[0, 1]] as Curve,
  fadeOut: [[0, 1], [1, 0]] as Curve,
  fadeInOut: [[0, 0], [0.15, 1], [0.7, 0.8], [1, 0]] as Curve,
  quickInSlowOut: [[0, 0], [0.05, 1], [1, 0]] as Curve,
  grow: [[0, 0.3], [1, 1]] as Curve,
  shrink: [[0, 1], [1, 0]] as Curve,
  popShrink: [[0, 0.4], [0.1, 1], [1, 0.1]] as Curve,
  smokeGrow: [[0, 0.4], [1, 1.6]] as Curve,
} as const;

/** Easings (t ∈ 0..1). */
export const ease = {
  outCubic: (t: number): number => 1 - (1 - t) ** 3,
  inCubic: (t: number): number => t * t * t,
  inOutSine: (t: number): number => -(Math.cos(Math.PI * t) - 1) / 2,
  outBack: (t: number): number => {
    const c1 = 1.70158;
    const c3 = c1 + 1;
    return 1 + c3 * (t - 1) ** 3 + c1 * (t - 1) ** 2;
  },
};
