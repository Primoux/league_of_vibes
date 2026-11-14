/**
 * Vitrine des matériaux FX dans le fxlab : bouclier fresnel, cristal de givre, eau, distorsion de chaleur,
 * dissolve en boucle. Sert à vérifier visuellement (et à la compilation GPU) chaque shader.
 */
import * as THREE from 'three';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { DISTORTION_LAYER, distortionUsers, applyDissolve, createDissolveUniforms, createFresnelMaterial, createHeatMaterial, createIceMaterial, createWaterMaterial } from '../shaders/materials.ts';

export function createShowcase(rt: FxRuntime, origin: THREE.Vector3): { group: THREE.Group; update(dt: number): void } {
  const group = new THREE.Group();
  group.position.copy(origin);
  const mats: THREE.ShaderMaterial[] = [];
  let time = 0;

  // Bouclier : sphère fresnel avec bandes animées
  const shieldMat = createFresnelMaterial({ color: new THREE.Color(0.45, 0.75, 1.3), power: 2.4, base: 0.03, bands: 0.2 });
  const shield = new THREE.Mesh(new THREE.IcosahedronGeometry(1, 3), shieldMat);
  shield.position.set(-4, 1, 0);
  mats.push(shieldMat);

  // Cristal de givre : facettes low-poly
  const ice = new THREE.Group();
  const iceMat = createIceMaterial();
  for (let i = 0; i < 7; i++) {
    const shard = new THREE.Mesh(new THREE.ConeGeometry(0.22 + (i % 3) * 0.06, 1.2 + (i % 4) * 0.35, 5), iceMat);
    const a = (i / 7) * Math.PI * 2;
    shard.position.set(Math.cos(a) * 0.35, 0.5, Math.sin(a) * 0.35);
    shard.rotation.set(Math.sin(a) * 0.5, a, Math.cos(a) * 0.5);
    ice.add(shard);
  }
  ice.position.set(-1.5, 0, 0);
  mats.push(iceMat);

  // Eau : disque animé
  const waterMat = createWaterMaterial();
  const water = new THREE.Mesh(new THREE.PlaneGeometry(2.6, 2.6), waterMat);
  water.rotation.x = -Math.PI / 2;
  water.position.set(1.2, 0.04, 0);
  mats.push(waterMat);

  // Chaleur : visible uniquement via PostFx (couche de distorsion), au-dessus d'un feu de particules
  const heatMat = createHeatMaterial(0.6);
  const heat = new THREE.Mesh(new THREE.PlaneGeometry(1.6, 2.4), heatMat);
  heat.position.set(4, 1.3, 0);
  heat.layers.set(DISTORTION_LAYER);
  distortionUsers.count++;
  mats.push(heatMat);
  const firePos = new THREE.Vector3();

  // Dissolve : mannequin standard qui se dissout en boucle
  const du = createDissolveUniforms();
  const body = new THREE.Mesh(new THREE.CapsuleGeometry(0.45, 1, 4, 12), new THREE.MeshStandardMaterial({ color: 0xc9a66b, roughness: 0.5 }));
  applyDissolve(body, du);
  body.position.set(6.5, 0.95, 0);
  body.castShadow = true;

  group.add(shield, ice, water, heat, body);
  rt.root.add(group);

  let fireAcc = 0;
  return {
    group,
    update(dt) {
      time += dt;
      for (const m of mats) if (m.uniforms.uTime) m.uniforms.uTime.value = time;
      shield.rotation.y += dt * 0.4;
      ice.rotation.y += dt * 0.25;
      du.uDissolve.value = Math.max(0, Math.sin(time * 0.8)) * 1.05;
      fireAcc += dt * 40;
      const n = Math.floor(fireAcc);
      if (n > 0) {
        fireAcc -= n;
        body.updateWorldMatrix(true, false);
        group.localToWorld(firePos.set(4, 0.1, 0));
        rt.ps.emit(rt.s.flame, firePos, { count: n, shape: 'disc', radius: 0.35, up: [1.2, 2.2], life: [0.5, 0.9], size: [0.5, 0.9] });
        rt.ps.emit(rt.s.smoke, firePos.setY(1.4), { count: n * 0.3, shape: 'disc', radius: 0.3, up: [0.6, 1.2], life: [1, 1.6], size: [0.6, 1] });
      }
    },
  };
}
