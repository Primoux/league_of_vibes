/** Générateur pseudo-aléatoire rapide (mulberry32), déterministe si on fixe la graine (tests, fxlab). */
export class Rng {
  private s: number;
  constructor(seed = (Math.random() * 2 ** 32) >>> 0) {
    this.s = seed >>> 0;
  }
  next(): number {
    let t = (this.s = (this.s + 0x6d2b79f5) >>> 0);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }
  range(min: number, max: number): number {
    return min + (max - min) * this.next();
  }
  /** Valeur dans une plage [min, max] (ou valeur fixe). */
  pick(r: number | readonly [number, number]): number {
    return typeof r === 'number' ? r : this.range(r[0], r[1]);
  }
  sign(): number {
    return this.next() < 0.5 ? -1 : 1;
  }
}

export const rng = new Rng();
