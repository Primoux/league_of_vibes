/**
 * Ambiance de la carte : lucioles dans la jungle, pollen au-dessus des voies, brume et scintillements sur la
 * rivière, étincelles qui montent des fontaines.
 *
 * Système de particules **dédié** et petit (1 024) : une émission continue dans le système principal
 * l'empêcherait de se remettre à zéro entre deux combats (son lot resterait plein et le vertex shader
 * traiterait toutes les instances à chaque frame). Émission seulement dans l'emprise de la caméra
 * (`store.cameraView`), proportionnelle à la surface de chaque zone visible ; aucune allocation par frame.
 */
import * as THREE from 'three';
import { FOUNTAINS, LANE_HALF_WIDTH, MAP_SIZE, RIVER_HALF_WIDTH, RENDER_SCALE as S } from '@lov/shared';
import { store } from '../../state.ts';
import { distToLane, distToRiver, terrainHeight } from '../../render/map/geometry.ts';
import { ParticleSystem, type ParticleStyle } from '../particles/ParticleSystem.ts';
import { CURVES } from '../core/curves.ts';
import { rng } from '../core/rng.ts';

/** Particules par seconde et par million d'unités² de jeu visibles. */
const RATE = { firefly: 0.9, pollen: 0.7, mist: 0.12, sparkle: 1.4 };
const TICK = 0.1;

type Zone = 'jungle' | 'lane' | 'river' | 'none';

function zoneAt(x: number, y: number): Zone {
  if (x < 0 || y < 0 || x > MAP_SIZE || y > MAP_SIZE) return 'none';
  if (Math.hypot(x - FOUNTAINS.blue.x, y - FOUNTAINS.blue.y) < 2300 || Math.hypot(x - FOUNTAINS.red.x, y - FOUNTAINS.red.y) < 2300) return 'none';
  if (distToRiver(x, y) < RIVER_HALF_WIDTH - 80 && Math.abs(x - y) * Math.SQRT1_2 < 5800) return 'river';
  if (distToLane(x, y) < LANE_HALF_WIDTH + 150) return 'lane';
  return 'jungle';
}

export class AmbientFx {
  readonly ps = new ParticleSystem(1024);
  private readonly firefly: ParticleStyle;
  private readonly mote: ParticleStyle;
  private readonly mist: ParticleStyle;
  private readonly sparkle: ParticleStyle;
  private readonly p = new THREE.Vector3();
  private readonly cFirefly = new THREE.Color(0.75, 1, 0.35);
  private readonly cPollen = new THREE.Color(1, 0.95, 0.75);
  private readonly cMist = new THREE.Color(0.85, 0.95, 1);
  private readonly cSpark = new THREE.Color(0.85, 0.97, 1);
  private readonly cTeam = { blue: new THREE.Color(0.35, 0.65, 1), red: new THREE.Color(1, 0.4, 0.3) };
  private acc = 0;
  private carry = { firefly: 0, pollen: 0, mist: 0, sparkle: 0 };
  enabled = true;

  constructor(parent: THREE.Object3D) {
    const ps = this.ps;
    // luciole : clignote (alpha en dents de scie douces), taille constante
    this.firefly = ps.defineStyle({ texture: 'glow', alpha: [[0, 0], [0.15, 1], [0.3, 0.35], [0.45, 1], [0.6, 0.4], [0.75, 0.9], [1, 0]], size: CURVES.one });
    this.mote = ps.defineStyle({ texture: 'dot', alpha: CURVES.fadeInOut, size: CURVES.one });
    this.mist = ps.defineStyle({ texture: 'glow', blend: 'normal', alpha: [[0, 0], [0.35, 0.07], [0.7, 0.05], [1, 0]], size: [[0, 0.7], [1, 1.3]] });
    this.sparkle = ps.defineStyle({ texture: 'sparkles', alpha: [[0, 0], [0.4, 1], [1, 0]], size: CURVES.popShrink });
    ps.group.name = 'fx-ambient';
    parent.add(ps.group);
  }

  update(dt: number): void {
    this.ps.update(dt);
    if (!this.enabled) return;
    this.acc += dt;
    if (this.acc < TICK) return;
    const step = this.acc;
    this.acc = 0;
    const v = store.state.cameraView;
    if (!(v.w > 0 && v.h > 0)) return;
    // tirages : nombre attendu par zone = taux × surface vue × durée ; chaque tirage teste la zone du point
    const area = (v.w * v.h) / 1e6;
    this.spawn('firefly', area * RATE.firefly * step, v, 'jungle');
    this.spawn('pollen', area * RATE.pollen * step, v, 'lane');
    this.spawn('mist', area * RATE.mist * step, v, 'river');
    this.spawn('sparkle', area * RATE.sparkle * step, v, 'river');
    this.fountains(step, v);
  }

  private spawn(kind: keyof AmbientFx['carry'], expected: number, v: { minX: number; minY: number; w: number; h: number }, zone: Zone): void {
    let n = expected + this.carry[kind];
    const whole = Math.floor(n);
    this.carry[kind] = n - whole;
    for (n = whole; n > 0; n--) {
      const x = v.minX + rng.next() * v.w;
      const y = v.minY + rng.next() * v.h;
      if (zoneAt(x, y) !== zone) continue;
      const ground = terrainHeight(x, y) * S;
      const p = this.p;
      switch (kind) {
        case 'firefly':
          p.set(x * S, ground + 0.25 + rng.next() * 1.1, -y * S);
          this.ps.emit(this.firefly, p, { count: 1, life: [3, 5.5], size: [0.07, 0.12], speed: [0.08, 0.22], drag: 0.15, up: [-0.03, 0.06], tint: this.cFirefly, intensity: 2.6 });
          break;
        case 'pollen':
          p.set(x * S, ground + 0.15 + rng.next() * 1.2, -y * S);
          this.ps.emit(this.mote, p, { count: 1, life: [4, 7], size: [0.025, 0.045], speed: [0.04, 0.12], up: [0.02, 0.08], tint: this.cPollen, intensity: 0.9 });
          break;
        case 'mist':
          // la rivière s'écoule vers le bas-droite : jeu (1, -1) → monde (1, 0, 1)
          p.set(x * S, -0.12, -y * S);
          this.ps.emit(this.mist, p, { count: 1, life: [6, 9], size: [3, 4.5], speed: 0, up: [0.0, 0.02], inherit: FLOW, tint: this.cMist });
          break;
        case 'sparkle':
          p.set(x * S, -0.12, -y * S);
          this.ps.emit(this.sparkle, p, { count: 1, life: [0.5, 0.9], size: [0.08, 0.16], speed: 0, inherit: FLOW, tint: this.cSpark, intensity: 2.2 });
          break;
      }
    }
  }

  /** Étincelles aux couleurs d'équipe qui montent des fontaines (si visibles). */
  private fountains(step: number, v: { minX: number; minY: number; maxX: number; maxY: number }): void {
    for (const team of ['blue', 'red'] as const) {
      const f = FOUNTAINS[team];
      if (f.x < v.minX - 600 || f.x > v.maxX + 600 || f.y < v.minY - 600 || f.y > v.maxY + 600) continue;
      const p = this.p.set(f.x * S, terrainHeight(f.x, f.y) * S + 0.3, -f.y * S);
      this.ps.emit(this.mote, p, { count: Math.max(1, Math.round(step * 30)), shape: 'ring', radius: 3.4, life: [1.6, 2.6], size: [0.05, 0.09], speed: [-0.05, 0.05], up: [0.5, 1.1], drag: 0.3, tint: this.cTeam[team], intensity: 2.4 });
      this.ps.emit(this.firefly, p.setY(p.y + 2.2), { count: Math.max(1, Math.round(step * 6)), shape: 'sphere', radius: 0.9, life: [1, 1.8], size: [0.1, 0.18], speed: [0.1, 0.3], up: [0.2, 0.4], tint: this.cTeam[team], intensity: 2 });
    }
  }
}

const FLOW = new THREE.Vector3(1, 0, 1).normalize().multiplyScalar(0.18);
