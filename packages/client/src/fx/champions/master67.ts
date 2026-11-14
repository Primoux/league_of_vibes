/** Chiffres de lumière en géométrie, sans texture ni asset tiers. */
import * as THREE from 'three';
import { onAbilityHit, registerFx } from '../registry.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { ProjectileFx, dashTrail, flash, shockwave, telegraph, timed } from '../effects/primitives.ts';

const cyan = new THREE.Color('#35e6e0'), violet = new THREE.Color('#ad77ff');
const paths = {
  6: [[.19, .38], [-.04, .42], [-.26, .16], [-.25, -.2], [-.08, -.34], [.17, -.27], [.22, -.08], [.06, .06], [-.23, -.02]],
  7: [[-.25, .35], [.25, .35], [-.12, -.35]],
} as const;

/** Renvoie une destruction idempotente, y compris quand un projectile est arrêté à l'impact. */
function glyph(rt: FxRuntime, digit: 6 | 7, position: THREE.Vector3, color: THREE.Color, scale = 1): { mesh: THREE.Mesh; dispose(): void } {
  const points = paths[digit].map(([x, y]) => new THREE.Vector3(x, y, 0));
  const curve = digit === 6 ? new THREE.CatmullRomCurve3(points) : new THREE.CurvePath<THREE.Vector3>();
  if (curve instanceof THREE.CurvePath) for (let i = 1; i < points.length; i++) curve.add(new THREE.LineCurve3(points[i - 1]!, points[i]!));
  const geometry = new THREE.TubeGeometry(curve, digit === 6 ? 26 : 10, .042, 5, false);
  const material = new THREE.MeshBasicMaterial({ color: color.clone().multiplyScalar(1.8), transparent: true, depthWrite: false });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.copy(position); mesh.scale.setScalar(scale); mesh.quaternion.copy(rt.camera.quaternion);
  rt.root.add(mesh);
  let disposed = false;
  return { mesh, dispose() { if (disposed) return; disposed = true; mesh.removeFromParent(); geometry.dispose(); material.dispose(); } };
}

function risingDigit(rt: FxRuntime, digit: 6 | 7, at: THREE.Vector3, color: THREE.Color, size = 1.7): void {
  const fx = glyph(rt, digit, at, color, size);
  const y = at.y;
  timed(rt, .8, (t) => { fx.mesh.position.y = y + t; fx.mesh.quaternion.copy(rt.camera.quaternion); (fx.mesh.material as THREE.MeshBasicMaterial).opacity = 1 - t * t; }, fx.dispose);
}

registerFx('master67_q', {
  projectile(rt, e) {
    const from = rt.world(e.from, 1.05), to = rt.world(e.to);
    const digit = glyph(rt, 6, from, cyan, 1.1);
    const fx = new ProjectileFx(rt, from, to, rt.scale(e.speed), { color: cyan, size: .22, height: 1.05, ribbonWidth: .12,
      onStep(pos) { digit.mesh.position.copy(pos); digit.mesh.quaternion.copy(rt.camera.quaternion); }, onEnd: digit.dispose });
    rt.add(fx); return fx;
  },
  hit(rt, e) { flash(rt, rt.world(e.pos, 1), 1, cyan); shockwave(rt, rt.world(e.pos), .9, .35, cyan); },
});
registerFx('master67_w_telegraph', {
  area(rt, e) {
    const at = rt.world(e.pos);
    telegraph(rt, at, rt.scale(e.radius), e.duration, violet);
    const fx = glyph(rt, 7, at, violet, 2.3);
    timed(rt, e.duration, (t) => { fx.mesh.position.y = at.y + 3.5 * (1 - t) + .3; fx.mesh.quaternion.copy(rt.camera.quaternion); }, fx.dispose);
  },
});
registerFx('master67_w', {
  area(rt, e) { const at = rt.world(e.pos); shockwave(rt, at, rt.scale(e.radius), .55, violet); flash(rt, at, 2.7, violet); },
});
registerFx('master67_e', {
  dash(rt, e) { dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), .1, cyan, false); risingDigit(rt, 6, rt.world(e.from, .8), cyan); risingDigit(rt, 7, rt.world(e.to, .8), violet); },
});
for (const [name, digit, color] of [['six', 6, cyan], ['seven', 7, violet]] as const) {
  registerFx(`master67_r_${name}`, {
    area(rt, e) {
      const at = rt.world(e.pos);
      shockwave(rt, at, rt.scale(e.radius), .67, color, .08);
      risingDigit(rt, digit, rt.world(e.pos, 2.1), color, 3.5);
      rt.ps.emit(rt.s.spark, at, { count: 36, shape: 'disc', radius: rt.scale(e.radius), size: [.12, .28], life: [.4, .8], up: [1, 2.5], tint: color });
    },
  });
}
registerFx('master67_r', {
  cast(rt, e) { const at = new THREE.Vector3(); if (rt.entities.pos(e.source, at)) telegraph(rt, at, rt.scale(467), .67, cyan, { color2: violet, fill: .06 }); },
});
onAbilityHit('master67_p', (rt, e) => {
  const at = new THREE.Vector3();
  if (rt.entities.pos(e.target, at, 1.5)) risingDigit(rt, 7, at, violet);
});
