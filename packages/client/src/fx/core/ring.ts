/**
 * Allocateur circulaire d'emplacements de particules + suivi des plages modifiées
 * (pour n'envoyer au GPU que ce qui a été écrit depuis la dernière frame).
 */
export class RingAllocator {
  private head = 0;
  private dirtyStart = 0;
  private dirtyCount = 0;
  /** Plages modifiées [début, longueur], valides après `flush()` pour i < valeur renvoyée. Réutilisées (pas d'allocation). */
  readonly rangeStart = new Int32Array(2);
  readonly rangeCount = new Int32Array(2);

  constructor(readonly capacity: number) {}

  /** Réserve un emplacement (écrase le plus ancien quand le tampon est plein). */
  next(): number {
    const i = this.head;
    this.head = (this.head + 1) % this.capacity;
    if (this.dirtyCount === 0) this.dirtyStart = i;
    if (this.dirtyCount < this.capacity) this.dirtyCount++;
    return i;
  }

  /** Calcule les plages modifiées depuis le dernier appel (0, 1 ou 2 si le tampon a bouclé) et remet le suivi à zéro. */
  flush(): number {
    const n = this.dirtyCount;
    this.dirtyCount = 0;
    if (n === 0) return 0;
    if (n >= this.capacity) return this.set(0, 0, this.capacity), 1;
    const end = this.dirtyStart + n;
    if (end <= this.capacity) return this.set(0, this.dirtyStart, n), 1;
    this.set(0, this.dirtyStart, this.capacity - this.dirtyStart);
    this.set(1, 0, end - this.capacity);
    return 2;
  }

  /** Repart de zéro (tous les emplacements sont libres : rien de vivant). */
  reset(): void {
    this.head = 0;
    this.dirtyCount = 0;
  }

  private set(i: number, start: number, count: number): void {
    this.rangeStart[i] = start;
    this.rangeCount[i] = count;
  }
}
