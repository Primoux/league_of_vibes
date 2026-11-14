import * as THREE from 'three';
import { describe, expect, it, vi } from 'vitest';
import type { EntitySnapshot } from '@lov/shared';
import { createChampionView, ProceduralChampionView } from '../src/render/champions/index.ts';
import { getFx } from '../src/fx/registry.ts';
import { moulinetGeometry } from '../src/fx/champions/touilleur.ts';
import type { Effect, FxRuntime } from '../src/fx/core/FxRuntime.ts';

describe('index du Touilleur', () => {
  it('reste vers le haut et décrit un moulinet dans tous les états vivants', () => {
    const view = createChampionView({ id: 301, subtype: 'touilleur', team: 'blue' })!;
    expect(view).toBeInstanceOf(ProceduralChampionView);
    const model = view as ProceduralChampionView, direction = new THREE.Vector3(), tip = new THREE.Vector3();
    const snap: EntitySnapshot = { id: 301, kind: 'champion', subtype: 'touilleur', team: 'blue', x: 0, y: 0, facing: 0, hp: 100, maxHp: 100 };
    try {
      for (const name of ['idle', 'run', 'strike', 'cast_Q', 'cast_W', 'cast_E', 'cast_R', 'recall']) {
        view.playAnim(name);
        const xs: number[] = [], zs: number[] = [];
        for (let i = 0; i < 24; i++) {
          if (name === 'run') view.object3d.position.x += .17;
          view.update(snap, .05); view.object3d.updateMatrixWorld(true);
          const finger = model.rig.joint('prop');
          direction.set(0, 1, 0).transformDirection(finger.matrixWorld);
          expect(direction.y, name).toBeGreaterThan(.9);
          // Direction de l'index : indépendante des déplacements de tout le corps.
          tip.copy(direction).multiplyScalar(.3); xs.push(tip.x); zs.push(tip.z);
        }
        expect(Math.max(...xs) - Math.min(...xs), name).toBeGreaterThan(.06);
        expect(Math.max(...zs) - Math.min(...zs), name).toBeGreaterThan(.06);
      }
    } finally { view.dispose(); }
  });

  it('reste dans cinq lots et reprend après la mort', () => {
    const view = createChampionView({ id: 302, subtype: 'touilleur', team: 'red' })!;
    const snap: EntitySnapshot = { id: 302, kind: 'champion', subtype: 'touilleur', team: 'red', x: 0, y: 0, facing: 0, hp: 100, maxHp: 100 };
    let meshes = 0, triangles = 0;
    view.object3d.traverse(o => { if (o instanceof THREE.Mesh) { meshes++; triangles += (o.geometry.index?.count ?? o.geometry.getAttribute('position').count) / 3; } });
    expect(meshes).toBeLessThanOrEqual(5); expect(triangles).toBeLessThan(16000);
    view.update({ ...snap, dead: true }, 2.5); view.update(snap, .2);
    expect(view.object3d.children[0]!.visible).toBe(true); view.dispose();
  });
});

describe('moulinets FX', () => {
  it('dispose de handlers pour les vrais événements de chaque sort', () => {
    for (const [key, phase] of [['touilleur_q', 'projectile'], ['touilleur_q', 'hit'], ['touilleur_w_telegraph', 'area'], ['touilleur_w', 'area'], ['touilleur_e', 'dash'], ['touilleur_r', 'dash'], ['touilleur_r_stun', 'buff'], ['touilleur_r_stun', 'buffEnd']] as const) expect(getFx(key)?.[phase]).toBeTypeOf('function');
    const g = moulinetGeometry();
    expect(Array.from(g.getAttribute('position').array).every(Number.isFinite)).toBe(true);
    expect(g.index!.count / 3).toBeLessThan(500); g.dispose();
  });

  it('arrête le moulinet à la suppression du stun et libère ses ressources une seule fois', () => {
    const root = new THREE.Group(), effects: Effect[] = [], auras = new Map<string, () => void>();
    // Faux runtime structurel limité aux dépendances du handler, sans WebGL ni canvas.
    const rt = { root, entities: { pos(_id: number, out: THREE.Vector3, h: number) { out.set(0, h, 0); return true; }, info() { return { dead: false, facing: 0 }; } },
      add(e: Effect) { effects.push(e); }, setAura(k: string, end: () => void) { auras.get(k)?.(); auras.set(k, end); },
      forgetAura(k: string, end: () => void) { if (auras.get(k) === end) auras.delete(k); }, endAura(k: string) { const end = auras.get(k); auras.delete(k); end?.(); },
    } as unknown as FxRuntime;
    const handler = getFx('touilleur_r_stun')!;
    handler.buff!(rt, { type: 'buffApplied', target: 1, source: 2, buffId: 'stun', duration: 2, fxKey: 'touilleur_r_stun' }, {} as never);
    const mesh = root.children[0] as THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>;
    const geo = vi.spyOn(mesh.geometry, 'dispose'), mat = vi.spyOn(mesh.material, 'dispose');
    expect(effects[0]!.update(.1)).toBe(true); expect(root.children).toHaveLength(1);
    handler.buffEnd!(rt, { type: 'buffRemoved', target: 1, buffId: 'stun', fxKey: 'touilleur_r_stun' }, {} as never);
    expect(effects[0]!.update(.1)).toBe(false); effects[0]!.end!();
    expect(root.children).toHaveLength(0); expect(auras.size).toBe(0);
    expect(geo).toHaveBeenCalledTimes(1); expect(mat).toHaveBeenCalledTimes(1);
  });
});
