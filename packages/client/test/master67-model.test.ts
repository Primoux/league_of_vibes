import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { createChampionView, ProceduralChampionView } from '../src/render/champions/index.ts';
import { createPose } from '../src/render/champions/anim/pose.ts';
import { Rig } from '../src/render/champions/rig.ts';
import { master67Model, sampleSixSeven } from '../src/render/champions/models/master67.ts';

describe('gestes six-seven', () => {
  it('alterne les mains en gardant les paumes vers le haut', () => {
    const rig = new Rig(master67Model.rig), pose = createPose();
    const heights: number[][] = [];
    for (const time of [.335, 1.005]) {
      pose.fill(0); sampleSixSeven(time, pose); rig.apply(pose); rig.root.updateMatrixWorld(true);
      heights.push(['foreL', 'foreR'].map(name => {
        const joint = rig.joint(name as 'foreL' | 'foreR');
        const normal = new THREE.Vector3(1, 0, 0).transformDirection(joint.matrixWorld);
        expect(normal.y).toBeGreaterThan(.7);
        return joint.localToWorld(new THREE.Vector3(.02, -.36, 0)).y;
      }));
    }
    expect(heights[0]![0]! - heights[0]![1]!).toBeGreaterThan(.15);
    expect(heights[1]![0]! - heights[1]![1]!).toBeLessThan(-.15);
  });
  it('conserve les gestes en course et pendant chaque incantation', () => {
    for (const name of ['idle', 'run', 'strike', 'cast_Q', 'cast_W', 'cast_E', 'cast_R', 'recall']) {
      const clip = master67Model.clips![name]!, a = createPose(), b = createPose();
      clip.sample(.1, a, { energy: 1, hand: 1 }); clip.sample(.4, b, { energy: 1, hand: 1 });
      expect([...a].every(Number.isFinite)).toBe(true);
      expect([...a].some((value, i) => value !== b[i])).toBe(true);
    }
  });
  it('utilise un modèle dédié en trois meshes et reprend après une mort', () => {
    const view = createChampionView({ id: 67, subtype: 'master67', team: 'blue' })!;
    expect(view).toBeInstanceOf(ProceduralChampionView);
    let meshes = 0, triangles = 0;
    view.object3d.traverse(o => { if (o instanceof THREE.Mesh) { meshes++; triangles += (o.geometry.index?.count ?? o.geometry.attributes.position!.count) / 3; } });
    expect(meshes).toBeLessThanOrEqual(3); expect(triangles).toBeLessThan(16000);
    const snap = { id: 67, subtype: 'master67', kind: 'champion' as const, team: 'blue' as const, hp: 100, maxHp: 100, x: 0, y: 0, facing: 0 };
    view.update({ ...snap, dead: true }, 2.5); view.update(snap, .1); view.update(snap, .2);
    expect(view.object3d.children[0]!.visible).toBe(true);
    view.dispose();
  });
});
