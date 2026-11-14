/** Export des véritables meshes déformés pour contrôle hors WebGL. Usage : node --import tsx …/exportChampionModels.ts sortie.json */
import * as THREE from 'three';
import { writeFileSync } from 'node:fs';
import { CHAMPION_LIST } from '@lov/shared';
import { createChampionView } from '../src/render/champions/index.ts';

const models = CHAMPION_LIST.map((def, i) => {
  const view = createChampionView({ id: i + 1, subtype: def.id, team: 'blue' })!;
  const snap = { id: i + 1, kind: 'champion' as const, subtype: def.id, team: 'blue' as const, x: 0, y: 0, facing: 0, hp: 100, maxHp: 100 };
  for (let frame = 0; frame < 5; frame++) view.update(snap, .05);
  view.object3d.updateMatrixWorld(true);
  const meshes: { positions: number[]; normals: number[]; colors: number[]; roughness: number; metalness: number; emission: boolean; smooth: boolean }[] = [];
  view.object3d.traverse(object => {
    if (!(object instanceof THREE.Mesh) || !object.visible) return;
    for (let parent = object.parent; parent; parent = parent.parent) if (!parent.visible) return;
    const material = object.material as THREE.MeshStandardMaterial;
    if (!material.color) return; // Le contour GPU est une coque de shader, pas un volume physique.
    const g = object.geometry, position = g.getAttribute('position'), normal = g.getAttribute('normal'), color = g.getAttribute('color');
    if (!position?.count) return;
    const positions: number[] = [], normals: number[] = [], colors: number[] = [];
    const baseNormal = new THREE.Matrix3().getNormalMatrix(object.matrixWorld), bones: THREE.Matrix3[] = [];
    if (object instanceof THREE.SkinnedMesh) object.skeleton.bones.forEach((bone, j) => {
      bones[j] = new THREE.Matrix3().getNormalMatrix(object.matrixWorld.clone().multiply(object.bindMatrixInverse).multiply(bone.matrixWorld).multiply(object.skeleton.boneInverses[j]!).multiply(object.bindMatrix));
    });
    for (let a = 0; a < (g.index?.count ?? position.count); a++) {
      const k = g.index ? g.index.getX(a) : a, v = new THREE.Vector3().fromBufferAttribute(position, k);
      if (object instanceof THREE.SkinnedMesh) object.applyBoneTransform(k, v);
      v.applyMatrix4(object.matrixWorld); positions.push(v.x, v.y, v.z);
      const n = normal ? new THREE.Vector3().fromBufferAttribute(normal, k) : new THREE.Vector3(0, 1, 0);
      n.applyMatrix3(bones.length ? bones[g.getAttribute('skinIndex').getX(k)]! : baseNormal).normalize(); normals.push(n.x, n.y, n.z);
      const c = material.color.clone(); if (color) c.multiply(new THREE.Color().fromBufferAttribute(color, k)); colors.push(c.r, c.g, c.b, 1);
    }
    meshes.push({ positions, normals, colors, roughness: material.roughness ?? .7, metalness: material.metalness ?? 0, emission: material.type === 'MeshBasicMaterial', smooth: !material.flatShading });
  });
  const result = { id: def.id, name: def.name, height: view.height, meshes };
  view.dispose(); return result;
});
writeFileSync(process.argv[2] ?? 'champion-models.json', JSON.stringify(models));
console.log(models.map(m => ({ id: m.id, batches: m.meshes.length, triangles: m.meshes.reduce((sum, g) => sum + g.positions.length / 9, 0) })));
