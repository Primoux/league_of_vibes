/**
 * Synchronisation d'horloge client/serveur. Propriétaire : claude-client.
 *
 * Estimation de `offset` tel que `serverNow ≈ localNow + offset`, à partir des timestamps `serverTime`
 * des snapshots. Chaque échantillon vaut `serverTime - localRecv = offset - latenceAller`, donc le MAX
 * sur une fenêtre glissante est l'échantillon le moins retardé (le plus proche du vrai offset).
 * Le résultat est lissé pour éviter les sauts visibles dans l'interpolation.
 * Si un RTT est connu (pong), on ajoute RTT/2 pour compenser la latence minimale.
 */
export class ServerClock {
  private samples: number[] = [];
  private head = 0;
  private target = 0;
  private smoothed = 0;
  private initialized = false;
  private rttMs = 0;

  constructor(private readonly windowSize = 40) {}

  /** À appeler à chaque snapshot reçu. */
  sample(serverTime: number, localRecv: number): void {
    const s = serverTime - localRecv;
    if (this.samples.length < this.windowSize) this.samples.push(s);
    else {
      this.samples[this.head] = s;
      this.head = (this.head + 1) % this.windowSize;
    }
    let max = -Infinity;
    for (const v of this.samples) if (v > max) max = v;
    // Base = temps des données REÇUES (pas le temps serveur réel) : l'interpolation doit garder
    // INTERP_DELAY_MS de marge sur le dernier snapshot arrivé. La latence est compensée par la prédiction.
    this.target = max;
    if (!this.initialized) {
      this.smoothed = this.target;
      this.initialized = true;
    }
  }

  setRtt(rttMs: number): void {
    this.rttMs = rttMs;
  }

  /** Lissage exponentiel vers la cible ; gros écart (> 250 ms) = recalage immédiat. */
  update(dtMs: number): void {
    const err = this.target - this.smoothed;
    if (Math.abs(err) > 250) this.smoothed = this.target;
    else this.smoothed += err * Math.min(1, dtMs / 1000);
  }

  get ready(): boolean {
    return this.initialized;
  }

  get offset(): number {
    return this.smoothed;
  }

  serverNow(localNow: number): number {
    return localNow + this.smoothed;
  }

  reset(): void {
    this.samples.length = 0;
    this.head = 0;
    this.initialized = false;
  }
}
