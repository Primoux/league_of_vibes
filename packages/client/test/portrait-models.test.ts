import * as THREE from 'three';
import { describe, expect, it, vi } from 'vitest';
import { CHAMPION_LIST, type EntitySnapshot } from '@lov/shared';
import { createChampionView } from '../src/render/champions/index.ts';
import { ModelBuilder } from '../src/render/champions/builder.ts';
import { Rig } from '../src/render/champions/rig.ts';
import { master67Model } from '../src/render/champions/models/master67.ts';

function visibleMeshes(root: THREE.Object3D): THREE.Mesh[] {
  const meshes: THREE.Mesh[] = [];
  root.traverseVisible(o => { if (o instanceof THREE.Mesh && o.geometry.getAttribute('position')?.count) meshes.push(o); });
  return meshes;
}

describe('modèles sculptés', () => {
  it.each(CHAMPION_LIST)('$id garde une géométrie valide, un budget borné et des articulations finies pendant les sorts', def => {
    const snap: EntitySnapshot = { id: 200, kind: 'champion', subtype: def.id, team: 'red', x: 0, y: 0, facing: 0, hp: 100, maxHp: 100 };
    const view = createChampionView(snap)!;
    try {
      view.update(snap, .05); view.object3d.updateMatrixWorld(true);
      const meshes = visibleMeshes(view.object3d);
      const triangles = meshes.reduce((sum, mesh) => sum + (mesh.geometry.index?.count ?? mesh.geometry.getAttribute('position').count) / 3, 0);
      // Contours inclus ; effets de Zephyr/Solenne inclus. Mélenchon utilise un autre rig.
      expect(triangles).toBeLessThan(def.id === 'melenchon' ? 14000 : 25000);
      expect(meshes.length).toBeLessThanOrEqual(def.id === 'melenchon' ? 30 : 9);
      for (const mesh of meshes) {
        const g = mesh.geometry, position = g.getAttribute('position');
        expect(position.count).toBeGreaterThan(0);
        for (const attribute of Object.values(g.attributes)) expect(Array.from(attribute.array).every(Number.isFinite)).toBe(true);
        expect(g.getAttribute('normal').count).toBe(position.count);
        if (g.index) expect(Array.from(g.index.array).every(n => Number.isInteger(n) && n >= 0 && n < position.count)).toBe(true);
        if (mesh instanceof THREE.SkinnedMesh) {
          const weights = g.getAttribute('skinWeight'), indices = g.getAttribute('skinIndex');
          expect(weights.count).toBe(position.count); expect(indices.count).toBe(position.count);
          let valid = true;
          for (let i = 0; i < position.count; i++) {
            valid &&= Math.abs(weights.getX(i) + weights.getY(i) + weights.getZ(i) + weights.getW(i) - 1) < 1e-5;
            valid &&= Number.isInteger(indices.getX(i)) && indices.getX(i) < mesh.skeleton.bones.length;
          }
          expect(valid).toBe(true);
        }
      }
      const vertex = new THREE.Vector3();
      for (const slot of ['Q', 'W', 'E', 'R']) {
        view.cast(slot);
        for (let frame = 0; frame < 18; frame++) {
          view.update(snap, 1 / 30); view.object3d.updateMatrixWorld(true);
          let finiteAndBounded = true;
          for (const mesh of meshes) {
            const p = mesh.geometry.getAttribute('position');
            for (let i = 0; i < p.count; i += Math.max(1, Math.floor(p.count / 32))) {
              vertex.fromBufferAttribute(p, i);
              if (mesh instanceof THREE.SkinnedMesh) mesh.applyBoneTransform(i, vertex);
              vertex.applyMatrix4(mesh.matrixWorld);
              finiteAndBounded &&= [vertex.x, vertex.y, vertex.z].every(Number.isFinite) && vertex.length() < 10;
            }
          }
          expect(finiteAndBounded).toBe(true);
        }
      }
    } finally { view.dispose(); }
  });

  it('libère aussi les nouveaux lots fusionnés de Mélenchon, une seule fois', () => {
    const view = createChampionView({ id: 201, subtype: 'melenchon', team: 'blue' })!;
    const meshes = visibleMeshes(view.object3d);
    const geometries = [...new Set(meshes.map(m => m.geometry))];
    const materials = [...new Set(meshes.flatMap(m => Array.isArray(m.material) ? m.material : [m.material]))];
    const spies = [...geometries, ...materials].map(resource => vi.spyOn(resource, 'dispose'));
    view.dispose(); view.dispose();
    for (const spy of spies) expect(spy).toHaveBeenCalledTimes(1);
  });

  it('rend aussi les dagues métalliques de Nyx translucides pendant son invisibilité', () => {
    const snap: EntitySnapshot = { id: 202, kind: 'champion', subtype: 'nyx', team: 'blue', x: 0, y: 0, facing: 0, hp: 100, maxHp: 100 };
    const view = createChampionView(snap)!;
    try {
      view.update({ ...snap, buffs: [{ id: 'invisible', remaining: 2 }] }, .1);
      const materials = visibleMeshes(view.object3d).map(m => m.material).filter((m): m is THREE.MeshStandardMaterial | THREE.MeshBasicMaterial => m instanceof THREE.MeshStandardMaterial || m instanceof THREE.MeshBasicMaterial);
      expect(materials.some(m => m instanceof THREE.MeshStandardMaterial && m.metalness > .6)).toBe(true);
      for (const m of materials) { expect(m.transparent).toBe(true); expect(m.opacity).toBe(.3); expect(m.depthWrite).toBe(false); }
      view.update(snap, .1);
      for (const m of materials) { expect(m.transparent).toBe(false); expect(m.opacity).toBe(1); expect(m.depthWrite).toBe(true); }
    } finally { view.dispose(); }
  });
});

describe('matières et normales du builder', () => {
  it('préserve une normale sculptée après une échelle non uniforme et une rotation, avec métal séparé', () => {
    const rig = new Rig(master67Model.rig), b = new ModelBuilder().sculptedSurfaces();
    const source = new THREE.SphereGeometry(1, 8, 6), input = source.toNonIndexed();
    const normal = new THREE.Vector3().fromBufferAttribute(input.getAttribute('normal'), 30);
    const matrix = new THREE.Matrix4().compose(new THREE.Vector3(.1, .2, 0), new THREE.Quaternion().setFromEuler(new THREE.Euler(.2, .4, .1)), new THREE.Vector3(.1, .2, .3));
    normal.applyMatrix3(new THREE.Matrix3().getNormalMatrix(matrix)).normalize();
    b.add('head', source, '#bf9c80', { pos: [.1, .2, 0], rot: [.2, .4, .1], scale: [.1, .2, .3], smooth: true });
    b.metal('spine', new THREE.BoxGeometry(.1, .1, .1), '#cba14f');
    const built = b.build(rig);
    const actual = new THREE.Vector3().fromBufferAttribute(built.meshes[0]!.geometry.getAttribute('normal'), 30);
    expect(actual.distanceTo(normal)).toBeLessThan(1e-5);
    expect(built.material.flatShading).toBe(false);
    const metal = built.meshes[1]!.material as THREE.MeshStandardMaterial;
    expect(metal.metalness).toBeGreaterThan(.6); expect(metal.roughness).toBeLessThan(built.material.roughness);
    expect(built.outlines).toHaveLength(2);
    input.dispose();
    for (const mesh of [...built.meshes, ...built.outlines]) mesh.geometry.dispose();
    built.material.dispose(); metal.dispose(); rig.skeleton.dispose();
  });

  it('conserve les facettes historiques pour les modèles sans option sculptée', () => {
    const rig = new Rig(master67Model.rig), b = new ModelBuilder();
    b.add('head', new THREE.SphereGeometry(1, 8, 6), '#ffffff');
    const built = b.build(rig, { outline: false });
    expect(built.material.flatShading).toBe(true); expect(built.meshes).toHaveLength(1);
    built.meshes[0]!.geometry.dispose(); built.material.dispose(); rig.skeleton.dispose();
  });
});
