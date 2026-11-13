/**
 * Effets persistants attachés à une entité : coque de bouclier (fresnel), étoiles d'étourdissement, brume.
 * Annulables par clé (`rt.endAura(key)`, typiquement sur buffRemoved).
 */
import * as THREE from 'three';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { createFresnelMaterial } from '../shaders/materials.ts';

let shellGeo: THREE.BufferGeometry | null = null;

export interface ShellOptions {
  color: THREE.Color;
  radius?: number;
  height?: number;
  bands?: number;
  power?: number;
  /** Appelé chaque frame (éclats en orbite…), avec la position du centre et le temps écoulé. */
  onStep?: (center: THREE.Vector3, t: number, dt: number) => void;
  onEnd?: (center: THREE.Vector3) => void;
}

/** Coque de bouclier qui suit `entityId` pendant `duration` s (ou jusqu'à `rt.endAura(key)`). */
export function shieldShell(rt: FxRuntime, entityId: number, duration: number, key: string, o: ShellOptions): void {
  shellGeo ??= new THREE.IcosahedronGeometry(1, 3);
  const mat = createFresnelMaterial({ color: o.color, power: o.power ?? 2.2, base: 0.03, bands: o.bands ?? 0.15, opacity: 0 });
  const mesh = new THREE.Mesh(shellGeo, mat);
  const r = o.radius ?? 0.95;
  mesh.scale.set(r, (o.height ?? 1.1) * r, r);
  mesh.renderOrder = 14;
  rt.root.add(mesh);
  const center = new THREE.Vector3();
  let t = 0;
  let ending = -1;
  const finish = (): void => {
    if (ending < 0) {
      ending = 0;
      o.onEnd?.(center);
    }
  };
  rt.setAura(key, finish);
  rt.add({
    update(dt) {
      t += dt;
      if (t >= duration) finish();
      if (rt.entities.pos(entityId, center, r * 0.9)) mesh.position.copy(center);
      mat.uniforms.uTime!.value = t;
      if (ending >= 0) {
        ending += dt;
        mat.uniforms.uOpacity!.value = Math.max(0, 1 - ending / 0.2);
        mesh.scale.multiplyScalar(1 + dt * 1.5);
        return ending < 0.2;
      }
      mat.uniforms.uOpacity!.value = Math.min(1, t / 0.15);
      o.onStep?.(center, t, dt);
      return true;
    },
    end() {
      rt.forgetAura(key, finish);
      mesh.removeFromParent();
      mat.dispose();
    },
  });
}

/** Étoiles qui tournent au-dessus de la tête (étourdissement). */
export function stunStars(rt: FxRuntime, entityId: number, duration: number, height = 2.3): void {
  const pos = new THREE.Vector3();
  const p = new THREE.Vector3();
  const color = new THREE.Color(2.4, 2.1, 0.8);
  let t = 0;
  let acc = 0;
  rt.add({
    update(dt) {
      t += dt;
      acc += dt;
      if (acc > 0.05 && rt.entities.pos(entityId, pos, height)) {
        acc = 0;
        for (let i = 0; i < 3; i++) {
          const a = t * 5 + (i / 3) * Math.PI * 2;
          p.set(pos.x + Math.cos(a) * 0.45, pos.y, pos.z + Math.sin(a) * 0.45);
          rt.ps.emit(rt.s.star, p, { count: 1, shape: 'point', life: 0.12, size: 0.22, tint: color });
        }
      }
      return t < duration;
    },
  });
}

/** Brume basse (normal blending) qui s'étale autour d'un point. */
export function groundMist(rt: FxRuntime, at: THREE.Vector3, radius: number, color: THREE.Color, count = 10): void {
  rt.ps.emit(rt.s.smoke, at, { count, shape: 'disc', radius: radius * 0.6, speed: [0.3, 1], up: [0.05, 0.3], life: [0.9, 1.5], size: [radius * 0.5, radius * 0.9], tint: color, drag: 1.5 });
}
