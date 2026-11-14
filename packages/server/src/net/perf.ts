/**
 * Mesures de performance de la boucle serveur (activées par LOV_PERF=1). Propriétaire : claude-server.
 * Log toutes les 10 s : durée des ticks, coût des snapshots, régularité réelle des envois, débit par client,
 * retard de la boucle d'événements Node (un tick qui dépasse son budget = lag ressenti par tout le monde).
 */
import { monitorEventLoopDelay } from 'node:perf_hooks';

type IntervalHistogram = ReturnType<typeof monitorEventLoopDelay>;

const pct = (a: number[], p: number): number => {
  if (!a.length) return 0;
  const s = [...a].sort((x, y) => x - y);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]!;
};

export class LoopPerf {
  private ticks: number[] = [];
  private snaps: number[] = [];
  private gaps: number[] = [];
  private bytes = 0;
  private clients = 0;
  private lastSnapAt = 0;
  private since = performance.now();
  private loopDelay: IntervalHistogram;

  constructor(
    private label: string,
    private log: (m: string) => void = console.log,
  ) {
    this.loopDelay = monitorEventLoopDelay({ resolution: 5 });
    this.loopDelay.enable();
  }

  tick(ms: number): void {
    this.ticks.push(ms);
  }

  snapshot(ms: number, bytes: number, clients: number): void {
    const now = performance.now();
    if (this.lastSnapAt) this.gaps.push(now - this.lastSnapAt);
    this.lastSnapAt = now;
    this.snaps.push(ms);
    this.bytes += bytes;
    this.clients = clients;
    if (now - this.since >= 10_000) this.flush(now);
  }

  private flush(now: number): void {
    const secs = (now - this.since) / 1000;
    const avg = this.ticks.reduce((a, b) => a + b, 0) / Math.max(1, this.ticks.length);
    this.log(
      `[perf ${this.label}] tick moy ${avg.toFixed(2)} ms p99 ${pct(this.ticks, 99).toFixed(2)} max ${pct(this.ticks, 100).toFixed(1)}` +
        ` | snapshot p99 ${pct(this.snaps, 99).toFixed(2)} ms | intervalle envois p50 ${pct(this.gaps, 50).toFixed(0)} p95 ${pct(this.gaps, 95).toFixed(0)} max ${pct(this.gaps, 100).toFixed(0)} ms` +
        ` | ${(this.ticks.length / secs).toFixed(1)} ticks/s | ${(this.bytes / 1024 / secs / Math.max(1, this.clients)).toFixed(1)} Ko/s/client (brut)` +
        ` | boucle Node p99 ${(this.loopDelay.percentile(99) / 1e6).toFixed(1)} ms max ${(this.loopDelay.max / 1e6).toFixed(1)} ms`,
    );
    this.ticks = [];
    this.snaps = [];
    this.gaps = [];
    this.bytes = 0;
    this.since = now;
    this.loopDelay.reset();
  }

  dispose(): void {
    this.loopDelay.disable();
  }
}
