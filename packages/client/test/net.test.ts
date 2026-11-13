import { describe, expect, it } from 'vitest';
import type { EntitySnapshot, Snapshot } from '@lov/shared';
import { ServerClock } from '../src/net/clock.ts';
import { MovePredictor, SNAP_DIST } from '../src/net/prediction.ts';
import { MAX_EXTRAPOLATE_MS, SnapshotBuffer, lerpAngle, type Interpolated } from '../src/net/snapshots.ts';

const ent = (id: number, x: number, y: number, extra: Partial<EntitySnapshot> = {}): EntitySnapshot => ({
  id,
  kind: 'minion',
  team: 'blue',
  x,
  y,
  facing: 0,
  hp: 100,
  maxHp: 100,
  ...extra,
});

const snap = (tick: number, serverTime: number, entities: EntitySnapshot[], removed?: number[]): Snapshot => ({
  tick,
  time: tick / 30,
  serverTime,
  entities,
  removed,
  events: [],
  score: { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } },
});

describe('SnapshotBuffer', () => {
  it('interpole linéairement entre deux snapshots', () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 1000, [ent(1, 0, 0)]));
    b.push(snap(2, 1050, [ent(1, 100, 50)]));
    const out: Interpolated = { x: 0, y: 0, facing: 0 };
    expect(b.sample(1, 1025, out)).toBe(true);
    expect(out.x).toBeCloseTo(50);
    expect(out.y).toBeCloseTo(25);
    b.sample(1, 900, out);
    expect(out.x).toBe(0);
  });

  it("extrapole de façon bornée au-delà du dernier échantillon", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 1000, [ent(1, 0, 0)]));
    b.push(snap(2, 1050, [ent(1, 100, 0)]));
    const out: Interpolated = { x: 0, y: 0, facing: 0 };
    b.sample(1, 1075, out);
    expect(out.x).toBeCloseTo(150);
    b.sample(1, 5000, out);
    expect(out.x).toBeCloseTo(100 + (100 * MAX_EXTRAPOLATE_MS) / 50);
  });

  it("ignore les snapshots obsolètes ou dupliqués", () => {
    const b = new SnapshotBuffer();
    expect(b.push(snap(5, 1000, [ent(1, 0, 0)]))).toBe(true);
    expect(b.push(snap(4, 990, [ent(1, 999, 0)]))).toBe(false);
    expect(b.push(snap(5, 1000, [ent(1, 999, 0)]))).toBe(false);
    expect(b.entities.get(1)!.x).toBe(0);
  });

  it('snapshot complet : les entités absentes sont supprimées', () => {
    const b = new SnapshotBuffer();
    const removed: number[] = [];
    b.push(snap(1, 1000, [ent(1, 0, 0), ent(2, 0, 0)]));
    b.push(snap(2, 1050, [ent(1, 0, 0)]), (id) => removed.push(id));
    expect([...b.entities.keys()]).toEqual([1]);
    expect(removed).toEqual([2]);
  });

  it('snapshot delta : conserve les absents et applique removed', () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 1000, [ent(1, 0, 0), ent(2, 0, 0), ent(3, 0, 0)]));
    b.push(snap(2, 1050, [ent(1, 10, 0)], [3]));
    expect([...b.entities.keys()].sort()).toEqual([1, 2]);
    expect(b.entities.get(1)!.x).toBe(10);
  });

  it("n'interpole pas les téléportations (Flash, respawn)", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 1000, [ent(1, 0, 0)]));
    b.push(snap(2, 1050, [ent(1, 400, 0)]));
    const out: Interpolated = { x: 0, y: 0, facing: 0 };
    b.sample(1, 1025, out);
    expect(out.x).toBe(400);
    // respawn : dead -> vivant
    b.push(snap(3, 1100, [ent(1, 410, 0, { dead: true })]));
    b.push(snap(4, 1150, [ent(1, 420, 0)]));
    b.sample(1, 1120, out);
    expect(out.x).toBe(420);
  });

  it('calcule la vitesse serveur récente', () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 1000, [ent(1, 0, 0)]));
    b.push(snap(2, 1100, [ent(1, 33.5, 0)]));
    const v = { x: 0, y: 0 };
    expect(b.velocity(1, v)).toBe(true);
    expect(v.x).toBeCloseTo(335);
  });

  it("garde un historique borné (ring buffer)", () => {
    const b = new SnapshotBuffer();
    for (let i = 0; i < 50; i++) b.push(snap(i + 1, 1000 + i * 50, [ent(1, i * 10, 0)]));
    const out: Interpolated = { x: 0, y: 0, facing: 0 };
    b.sample(1, 1000 + 48 * 50 + 25, out);
    expect(out.x).toBeCloseTo(485);
  });
});

describe('lerpAngle', () => {
  it('prend le chemin le plus court', () => {
    expect(lerpAngle(3, -3, 0.5)).toBeCloseTo(Math.PI, 1);
    expect(lerpAngle(0, 1, 0.5)).toBeCloseTo(0.5);
  });
});

describe('ServerClock', () => {
  it("retient l'échantillon le moins retardé", () => {
    const c = new ServerClock();
    // offset réel : +5000 ms ; latences 30, 80, 20 ms
    c.sample(10_000, 10_000 - 5000 + 30);
    c.sample(10_050, 10_050 - 5000 + 80);
    c.sample(10_100, 10_100 - 5000 + 20);
    c.update(10_000);
    expect(c.offset).toBeCloseTo(5000 - 20);
    expect(c.serverNow(0)).toBeCloseTo(4980);
  });

  it('lisse les petites variations', () => {
    const c = new ServerClock(2);
    c.sample(1000, 0); // offset 1000
    c.update(16);
    c.sample(1100, 0); // nouvel offset 1100 (fenêtre de 2 : max = 1100)
    c.update(100);
    expect(c.offset).toBeGreaterThan(1000);
    expect(c.offset).toBeLessThan(1100);
  });
});

describe('MovePredictor', () => {
  const still = (x: number, y: number) => ({ x, y, vx: 0, vy: 0, lead: 0 });

  it('avance immédiatement vers le point cliqué, sans attendre le serveur', () => {
    const p = new MovePredictor();
    p.reset(0, 0);
    p.command({ x: 1000, y: 0 }, 0.1);
    const pos = p.update(0.1, still(0, 0), 335, true);
    expect(pos.x).toBeGreaterThan(20);
    expect(pos.x).toBeLessThan(33.5);
  });

  it('converge vers la position serveur quand le mouvement est confirmé', () => {
    const p = new MovePredictor();
    p.reset(0, 0);
    p.command({ x: 3000, y: 0 }, 0.1);
    let sx = 0;
    for (let i = 0; i < 120; i++) {
      // le serveur suit avec ~100 ms de retard, vitesse 335 ; lead = 0.1 compense
      if (i >= 6) sx += 335 / 60;
      p.update(1 / 60, { x: sx, y: 0, vx: i >= 6 ? 335 : 0, vy: 0, lead: 0.1 }, 335, true);
    }
    expect(Math.abs(p.pos.x - (sx + 33.5))).toBeLessThan(15);
  });

  it('recale immédiatement sur un gros écart', () => {
    const p = new MovePredictor();
    p.reset(0, 0);
    const pos = p.update(0.016, still(SNAP_DIST + 100, 0), 335, true);
    expect(pos.x).toBe(SNAP_DIST + 100);
  });

  it("abandonne la prédiction si le serveur reste immobile (CC, attaque)", () => {
    const p = new MovePredictor();
    p.reset(0, 0);
    p.command({ x: 2000, y: 0 }, 0.05);
    for (let i = 0; i < 60; i++) p.update(1 / 60, still(0, 0), 335, true);
    expect(p.moving).toBe(false);
    for (let i = 0; i < 120; i++) p.update(1 / 60, still(0, 0), 335, true);
    expect(Math.abs(p.pos.x)).toBeLessThan(2);
  });

  it('ne prédit pas à travers un mur', () => {
    const p = new MovePredictor(() => true);
    p.reset(0, 0);
    p.command({ x: 1000, y: 0 });
    expect(p.moving).toBe(false);
  });

  it('suspend la prédiction pendant un dash et suit le serveur', () => {
    const p = new MovePredictor();
    p.reset(0, 0);
    p.suspend(0.3);
    const pos = p.update(0.016, still(120, 40), 335, true);
    expect(pos).toEqual({ x: 120, y: 40 });
  });
});
