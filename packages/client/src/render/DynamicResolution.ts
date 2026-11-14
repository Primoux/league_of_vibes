/**
 * Résolution dynamique (qualité haute). Propriétaire : claude-client (optimisation claude-fx).
 *
 * Le coût GPU croît avec le carré du pixel ratio : en HiDPI (ratio 2), le post-process et les shaders du décor
 * coûtent ~3× plus cher qu'en ratio 1. Si les frames restent lentes, on descend le ratio par paliers ; on ne
 * le remonte qu'après une longue période fluide, et une remontée suivie d'une rechute bloque les essais.
 * Logique pure (testée), aucune allocation.
 */
export interface DynResOptions {
  /** Frame plus lente que ce seuil (s) : candidate à la baisse (défaut 1/48 ≈ 48 fps). */
  slow?: number;
  /** Frame plus rapide que ce seuil (s) : candidate à la hausse (défaut 1/57). */
  fast?: number;
  /** Pas de ratio. */
  step?: number;
}

export class DynamicResolution {
  /** Ratio courant. */
  ratio: number;
  private ema = 1 / 60;
  private slowFor = 0;
  private fastFor = 0;
  private cooldown = 0;
  private sinceUpgrade = Infinity;
  private upgradeLock = 0;
  private readonly slow: number;
  private readonly fast: number;
  private readonly step: number;

  constructor(
    private max: number,
    private readonly min = 1,
    o: DynResOptions = {},
  ) {
    this.ratio = max;
    this.slow = o.slow ?? 1 / 48;
    this.fast = o.fast ?? 1 / 57;
    this.step = o.step ?? 0.25;
  }

  /** Repart d'un nouveau maximum (changement de qualité, d'écran). */
  reset(max: number): void {
    this.max = max;
    this.ratio = max;
    this.ema = 1 / 60;
    this.slowFor = this.fastFor = this.cooldown = this.upgradeLock = 0;
    this.sinceUpgrade = Infinity;
  }

  /** À appeler chaque frame avec sa durée (s). Retourne le nouveau ratio s'il change, sinon null. */
  update(dt: number): number | null {
    // onglet en arrière-plan, chargement : un pic isolé ne doit ni compter ni fausser la moyenne
    if (!(dt > 0) || dt > 0.25) return null;
    this.ema += (dt - this.ema) * 0.08;
    this.cooldown = Math.max(0, this.cooldown - dt);
    this.upgradeLock = Math.max(0, this.upgradeLock - dt);
    this.sinceUpgrade += dt;
    this.slowFor = this.ema > this.slow ? this.slowFor + dt : 0;
    this.fastFor = this.ema < this.fast ? this.fastFor + dt : 0;
    if (this.slowFor > 1 && this.cooldown === 0 && this.ratio > this.min) {
      // remontée ratée : on n'essaie plus avant une minute
      if (this.sinceUpgrade < 5) this.upgradeLock = 60;
      return this.apply(Math.max(this.min, this.ratio - this.step));
    }
    if (this.fastFor > 8 && this.cooldown === 0 && this.upgradeLock === 0 && this.ratio < this.max) {
      this.sinceUpgrade = 0;
      return this.apply(Math.min(this.max, this.ratio + this.step));
    }
    return null;
  }

  private apply(r: number): number {
    this.ratio = r;
    this.cooldown = 2;
    this.slowFor = this.fastFor = 0;
    // la moyenne repart d'une valeur neutre : sinon l'ancien régime déclencherait un second palier aussitôt
    this.ema = (this.slow + this.fast) / 2;
    return r;
  }
}
