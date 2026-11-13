/** Easings des clips (t quelconque, borné à 0..1). */
export const clamp01 = (t: number): number => (t < 0 ? 0 : t > 1 ? 1 : t);
export const easeOut = (t: number): number => 1 - (1 - clamp01(t)) ** 3;
export const easeIn = (t: number): number => clamp01(t) ** 2;
/** Cloche 0 → 1 → 0 sur [a, b]. */
export const bell = (t: number, a: number, b: number): number => Math.sin(clamp01((t - a) / (b - a)) * Math.PI);
