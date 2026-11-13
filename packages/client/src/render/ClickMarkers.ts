/**
 * Marqueurs de clic au sol (déplacement vert, attaque rouge). Propriétaire : claude-client.
 * Pool fixe, aucune allocation après construction.
 */
import * as THREE from 'three';
import type { Vec2 } from '@lov/shared';
import { S } from './coords.ts';

const POOL = 6;
const LIFE = 0.45;

interface Marker {
  mesh: THREE.Mesh;
  mat: THREE.MeshBasicMaterial;
  t: number;
}

export class ClickMarkers {
  private pool: Marker[] = [];
  private next = 0;

  constructor(scene: THREE.Scene) {
    const geo = new THREE.RingGeometry(0.25, 0.36, 24).rotateX(-Math.PI / 2);
    for (let i = 0; i < POOL; i++) {
      const mat = new THREE.MeshBasicMaterial({ color: 0x5cff8a, transparent: true, depthWrite: false, opacity: 0 });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.visible = false;
      mesh.renderOrder = 5;
      scene.add(mesh);
      this.pool.push({ mesh, mat, t: LIFE });
    }
  }

  spawn(pos: Vec2, color: number): void {
    const m = this.pool[this.next]!;
    this.next = (this.next + 1) % POOL;
    m.t = 0;
    m.mat.color.setHex(color);
    m.mesh.position.set(pos.x * S, 0.06, -pos.y * S);
    m.mesh.visible = true;
  }

  update(dt: number): void {
    for (const m of this.pool) {
      if (!m.mesh.visible) continue;
      m.t += dt;
      const k = m.t / LIFE;
      if (k >= 1) {
        m.mesh.visible = false;
        continue;
      }
      m.mesh.scale.setScalar(1.6 - k);
      m.mat.opacity = 1 - k * k;
    }
  }
}
