/** Moulinets magiques et étoiles burlesques ; effets liés aux impacts et buffs réels du serveur. */
import * as THREE from 'three';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { ProjectileFx, dashTrail, flash, shockwave, telegraph, timed } from '../effects/primitives.ts';
import { onAbilityHit, registerFx } from '../registry.ts';

const mint = new THREE.Color('#8ef4d2'), brass = new THREE.Color('#d29a49');

export function moulinetGeometry(): THREE.BufferGeometry {
  const points: THREE.Vector3[] = [];
  for (let i = 0; i <= 32; i++) {
    const t = i / 32, a = t * Math.PI * 4.5, r = .04 + t * .35;
    points.push(new THREE.Vector3(Math.cos(a) * r, .04 * t, Math.sin(a) * r));
  }
  return new THREE.TubeGeometry(new THREE.CatmullRomCurve3(points), 40, .018, 5, false);
}

function spiral(rt: FxRuntime, at: THREE.Vector3, scale = 1): { mesh: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>; dispose(): void } {
  const geometry = moulinetGeometry(), material = new THREE.MeshBasicMaterial({ color: mint.clone().multiplyScalar(1.65), transparent: true, depthWrite: false });
  const mesh = new THREE.Mesh(geometry, material); mesh.position.copy(at); mesh.scale.setScalar(scale); rt.root.add(mesh);
  let disposed = false;
  return { mesh, dispose() { if (disposed) return; disposed = true; mesh.removeFromParent(); geometry.dispose(); material.dispose(); } };
}

registerFx('touilleur_q', {
  projectile(rt, e) {
    const from = rt.world(e.from, 1), to = rt.world(e.to), fx = spiral(rt, from, .85);
    const projectile = new ProjectileFx(rt, from, to, rt.scale(e.speed), { color: mint, size: .18, height: 1, ribbonWidth: .09,
      onStep(pos) { fx.mesh.position.copy(pos); fx.mesh.rotation.y = rt.time * 15; }, onEnd: fx.dispose });
    rt.add(projectile); return projectile;
  },
  hit(rt, e) { flash(rt, rt.world(e.pos, 1), .8, mint); shockwave(rt, rt.world(e.pos), .7, .3, mint); },
});
registerFx('touilleur_w_telegraph', { area(rt, e) { telegraph(rt, rt.world(e.pos), rt.scale(e.radius), e.duration, mint, { fill: .07 }); } });
registerFx('touilleur_w', { area(rt, e) {
  const at = rt.world(e.pos, .08), fx = spiral(rt, at, rt.scale(e.radius) * 2);
  shockwave(rt, at, rt.scale(e.radius), .55, brass);
  timed(rt, .65, t => { fx.mesh.rotation.y = t * 13; fx.mesh.material.opacity = 1 - t; }, fx.dispose);
} });
registerFx('touilleur_e', { dash(rt, e) { dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), .15, mint, false); flash(rt, rt.world(e.to, .7), 1, brass); } });
registerFx('touilleur_r', { dash(rt, e) { dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), .18, brass, false); } });

registerFx('touilleur_r_stun', {
  buff(rt, e) {
    const key = `touilleur:r:${e.target}`, position = new THREE.Vector3(), fx = spiral(rt, position, 1.25);
    let ended = false, age = 0;
    const end = (): void => { if (ended) return; ended = true; fx.dispose(); rt.forgetAura(key, end); };
    rt.setAura(key, end);
    rt.add({ update(dt) {
      if (ended || (age += dt) >= e.duration || rt.entities.info(e.target)?.dead || !rt.entities.pos(e.target, position, 1.4)) return false;
      const facing = rt.entities.info(e.target)?.facing ?? 0;
      position.x -= Math.cos(facing) * .35; position.z += Math.sin(facing) * .35;
      fx.mesh.position.copy(position); fx.mesh.rotation.set(Math.PI / 2, age * 12, -facing);
      return true;
    }, end });
  },
  buffEnd(rt, e) { rt.endAura(`touilleur:r:${e.target}`); },
});
registerFx('touilleur_r_channel', { buff() { /* L'index du modèle est déjà animé. */ }, buffEnd() { /* Le buff de cible possède le moulinet. */ } });
onAbilityHit('touilleur_p', (rt, e) => { const at = new THREE.Vector3(); if (rt.entities.pos(e.target, at, 1.4)) { flash(rt, at, 1.2, brass); rt.ps.emit(rt.s.swirl, at, { count: 3, life: [.3, .5], size: [.4, .7], spin: 9, tint: mint }); } });
