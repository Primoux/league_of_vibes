import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import { MeshBatcher } from '../src/render/MeshBatcher.ts';

const geo = new THREE.BoxGeometry(1, 1, 1);
const matA = new THREE.MeshBasicMaterial();
const matB = new THREE.MeshBasicMaterial();

function model(): THREE.Group {
  const root = new THREE.Group();
  const arm = new THREE.Group();
  arm.position.set(1, 2, 3);
  root.add(arm);
  arm.add(new THREE.Mesh(geo, matA));
  root.add(new THREE.Mesh(geo, matA));
  root.add(new THREE.Mesh(geo, matB));
  return root;
}

const batchesOf = (scene: THREE.Scene) => scene.children.filter((o): o is THREE.InstancedMesh => (o as THREE.InstancedMesh).isInstancedMesh);

describe('MeshBatcher', () => {
  it('regroupe les pièces par (géométrie, matériau) et recopie les matrices monde', () => {
    const scene = new THREE.Scene();
    const b = new MeshBatcher(scene);
    const views = [model(), model(), model()];
    for (const v of views) {
      scene.add(v);
      b.adopt(v);
    }
    // les Mesh ont été remplacés par des nœuds vides
    let meshesLeft = 0;
    for (const v of views) v.traverse((o) => ((o as THREE.Mesh).isMesh ? meshesLeft++ : 0));
    expect(meshesLeft).toBe(0);
    expect(b.batchCount).toBe(2);
    views[1]!.position.x = 10;
    scene.updateMatrixWorld();
    b.sync();
    const [a, bb] = batchesOf(scene);
    expect(a!.count + bb!.count).toBe(9);
    const counts = [a!.count, bb!.count].sort();
    expect(counts).toEqual([3, 6]);
    // le bras animé : sa matrice monde est bien utilisée
    const mA = a!.material === matA ? a! : bb!;
    const m = new THREE.Matrix4();
    const xs: number[] = [];
    for (let i = 0; i < mA.count; i++) {
      mA.getMatrixAt(i, m);
      xs.push(new THREE.Vector3().setFromMatrixPosition(m).x);
    }
    expect(xs.sort((p, q) => p - q)).toEqual([0, 0, 1, 1, 10, 11]);
  });

  it('ignore les vues masquées ou détachées, libère et agrandit les lots', () => {
    const scene = new THREE.Scene();
    const b = new MeshBatcher(scene);
    const handles = [];
    const views: THREE.Group[] = [];
    for (let i = 0; i < 40; i++) {
      const v = model();
      scene.add(v);
      views.push(v);
      handles.push(b.adopt(v));
    }
    views[0]!.visible = false;
    views[1]!.removeFromParent();
    b.release(handles[2]!);
    scene.updateMatrixWorld();
    b.sync();
    const total = batchesOf(scene).reduce((n, m) => n + m.count, 0);
    expect(total).toBe(37 * 3);
    expect(batchesOf(scene).every((m) => m.instanceMatrix.count >= m.count)).toBe(true);
  });
});
