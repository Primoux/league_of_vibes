import { describe, expect, it, vi } from 'vitest';
import * as THREE from 'three';
import type { EntitySnapshot } from '@lov/shared';
import { MelenchonView } from '../src/render/champions/melenchon.ts';
import { defaultViewFor } from '../src/render/views/DefaultViews.ts';

const snap: EntitySnapshot = { id: 1, kind: 'champion', subtype: 'melenchon', team: 'blue', x: 0, y: 0, facing: 0, hp: 1000, maxHp: 1000 };

describe('caricature de Mélenchon', () => {
  it('est utilisée en partie et conserve une silhouette lisible dans le budget géométrique', () => {
    const model = defaultViewFor(snap)!;
    expect(model).toBeInstanceOf(MelenchonView);
    const root = model.object3d;
    for (const name of ['index-accusateur', 'cravate-rouge', 'lunettes-monture', 'meche-coiffee-en-arriere', 'nez']) expect(root.getObjectByName(name)).toBeDefined();
    let triangles = 0;
    root.traverse((o) => { if (o instanceof THREE.Mesh) triangles += (o.geometry.index?.count ?? o.geometry.getAttribute('position').count) / 3; });
    expect(triangles).toBeLessThan(22000);
    root.updateMatrixWorld(true);
    const bounds = new THREE.Box3().setFromObject(root);
    expect(bounds.max.y).toBeGreaterThan(2.4);
    expect(bounds.max.y).toBeLessThan(3);
    model.dispose();
  });

  it('regarde +x, anime son geste et revient vivant après une mort', () => {
    const model = new MelenchonView();
    model.object3d.updateMatrixWorld(true);
    const nose = model.object3d.getObjectByName('nez')!;
    expect(nose.getWorldPosition(new THREE.Vector3()).x).toBeGreaterThan(0.25);
    model.playAnim('cast_R'); model.update(snap, 0.1);
    const mouth = model.object3d.getObjectByName('bouche')!;
    expect(mouth.scale.y).toBeGreaterThan(0.04);
    model.update({ ...snap, dead: true }, 0.1);
    model.update(snap, 2);
    expect(model.object3d.children[0]!.position.y).toBe(0);
    expect(model.object3d.getObjectByName('aureole-republicaine')!.parent!.visible).toBe(false);
    model.update({ ...snap, buffs: [{ id: 'melenchon_r', remaining: 2 }] }, 0.1);
    expect(model.object3d.getObjectByName('aureole-republicaine')!.parent!.visible).toBe(true);
    model.dispose();
  });

  it('libère une seule fois ses ressources, y compris pour les hologrammes', () => {
    const model = new MelenchonView(true);
    const mesh = model.object3d.getObjectByName('visage') as THREE.Mesh<THREE.BufferGeometry, THREE.MeshStandardMaterial>;
    expect(mesh.material.transparent).toBe(true);
    expect(mesh.material.depthWrite).toBe(false);
    const disposeGeo = vi.spyOn(mesh.geometry, 'dispose');
    const disposeMat = vi.spyOn(mesh.material, 'dispose');
    model.dispose(); model.dispose();
    expect(disposeGeo).toHaveBeenCalledTimes(1);
    expect(disposeMat).toHaveBeenCalledTimes(1);
  });
});
