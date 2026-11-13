/**
 * Minuteur d'effets : `after(0.6, fn)` pour enchaîner anticipation → impact → persistance.
 * Temps en secondes, avancé par `update(dt)` (donc figé si l'onglet est en pause, comme le rendu).
 */
interface Task {
  at: number;
  fn: () => void;
}

export class Scheduler {
  private tasks: Task[] = [];
  private spare: Task[] = [];
  private now = 0;

  after(delay: number, fn: () => void): void {
    const t = this.spare.pop() ?? { at: 0, fn };
    t.at = this.now + Math.max(0, delay);
    t.fn = fn;
    this.tasks.push(t);
  }

  update(dt: number): void {
    this.now += dt;
    // Parcours à l'envers avec retrait par échange : O(1) par retrait, l'ordre d'exécution n'importe pas à l'intérieur d'une frame.
    for (let i = this.tasks.length - 1; i >= 0; i--) {
      const t = this.tasks[i]!;
      if (t.at > this.now) continue;
      const last = this.tasks.pop()!;
      if (i < this.tasks.length) this.tasks[i] = last;
      t.fn();
      this.spare.push(t);
    }
  }

  get pending(): number {
    return this.tasks.length;
  }

  clear(): void {
    this.spare.push(...this.tasks);
    this.tasks.length = 0;
  }
}
