/**
 * Pool d'objets générique : évite de recréer meshes/matériaux/effets à chaque sort.
 */
export class Pool<T> {
  private free: T[] = [];
  private created = 0;

  constructor(
    private readonly create: () => T,
    private readonly reset?: (item: T) => void,
    private readonly max = Infinity,
  ) {}

  /** Objet libre, ou un nouveau ; `null` si la limite `max` est atteinte. */
  acquire(): T | null {
    const item = this.free.pop();
    if (item !== undefined) return item;
    if (this.created >= this.max) return null;
    this.created++;
    return this.create();
  }

  release(item: T): void {
    this.reset?.(item);
    this.free.push(item);
  }

  /** Nombre total d'objets créés (libres + utilisés). */
  get size(): number {
    return this.created;
  }

  get available(): number {
    return this.free.length;
  }
}
