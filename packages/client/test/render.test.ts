import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import type { EntitySnapshot } from '@lov/shared';
import { isAttackable, needsTarget } from '../src/input/rules.ts';
import { CameraController, CAMERA_PITCH, ZOOM_MAX, ZOOM_MIN } from '../src/render/CameraController.ts';
import { toGame, toWorld } from '../src/render/coords.ts';
import { hitboxFor, rayCylinder } from '../src/render/picking.ts';

describe('coordonnées', () => {
  it('jeu -> monde -> jeu est une identité, y du jeu = -z', () => {
    const w = toWorld({ x: 1234, y: 5678 }, 2);
    expect(w.x).toBeCloseTo(12.34);
    expect(w.y).toBe(2);
    expect(w.z).toBeCloseTo(-56.78);
    const g = toGame(w);
    expect(g.x).toBeCloseTo(1234);
    expect(g.y).toBeCloseTo(5678);
  });

  it("l'orientation jeu correspond à rotation.y Three.js", () => {
    const o = new THREE.Object3D();
    const facing = Math.PI / 3;
    o.rotation.y = facing;
    const dir = new THREE.Vector3(1, 0, 0).applyQuaternion(o.quaternion);
    const g = toGame(dir);
    expect(Math.atan2(g.y, g.x)).toBeCloseTo(facing);
  });
});

describe('picking', () => {
  const down = { ox: 0, oy: 10, oz: 0, dx: 0, dy: -1, dz: 0 };
  it('touche un cylindre par le dessus', () => {
    expect(rayCylinder(down, 0.2, 0, 0.5, 2)).toBeCloseTo(8);
    expect(rayCylinder(down, 1, 0, 0.5, 2)).toBe(-1);
  });

  it('touche un cylindre par le flanc', () => {
    const side = { ox: -5, oy: 1, oz: 0, dx: 1, dy: 0, dz: 0 };
    expect(rayCylinder(side, 0, 0, 0.5, 2)).toBeCloseTo(4.5);
    const above = { ox: -5, oy: 3, oz: 0, dx: 1, dy: 0, dz: 0 };
    expect(rayCylinder(above, 0, 0, 0.5, 2)).toBe(-1);
  });

  it('hitbox : projectiles et zones non cliquables', () => {
    const e = (kind: EntitySnapshot['kind']): EntitySnapshot => ({ id: 1, kind, team: 'red', x: 0, y: 0, facing: 0, hp: 1, maxHp: 1 });
    expect(hitboxFor(e('projectile'))).toBeNull();
    expect(hitboxFor(e('zone'))).toBeNull();
    expect(hitboxFor(e('champion'))).not.toBeNull();
  });
});

describe('caméra', () => {
  it('regarde son centre avec le bon angle et reste dans les limites', () => {
    const c = new CameraController(16 / 9);
    c.jumpTo({ x: 6000, y: 6000 });
    c.update(0.016, null, false, false, 3000, 1920, 1080);
    const dir = new THREE.Vector3();
    c.camera.getWorldDirection(dir);
    expect(Math.asin(-dir.y)).toBeCloseTo(CAMERA_PITCH, 3);
    c.jumpTo({ x: -99999, y: 99999 });
    expect(c.center.x).toBeGreaterThan(-1000);
    expect(c.center.y).toBeLessThan(13000);
  });

  it('suit le champion quand elle est verrouillée, défile librement sinon', () => {
    const c = new CameraController(1);
    c.jumpTo({ x: 1000, y: 1000 });
    for (let i = 0; i < 60; i++) c.update(1 / 60, { x: 2000, y: 2000 }, true, true, 3000, 800, 600);
    expect(c.center.x).toBeCloseTo(2000, 0);
    c.setMouse(799, 300); // bord droit
    const manual = c.update(0.1, { x: 2000, y: 2000 }, false, true, 3000, 800, 600);
    expect(manual).toBe(true);
    expect(c.center.x).toBeGreaterThan(2200);
  });

  it('borne le zoom', () => {
    const c = new CameraController(1);
    c.zoomBy(100000);
    for (let i = 0; i < 100; i++) c.update(0.05, null, false, false, 0, 1, 1);
    expect(c.distance).toBeCloseTo(ZOOM_MAX, 0);
    c.zoomBy(-100000);
    for (let i = 0; i < 100; i++) c.update(0.05, null, false, false, 0, 1, 1);
    expect(c.distance).toBeCloseTo(ZOOM_MIN, 0);
  });
});

describe('règles input', () => {
  const e = (team: EntitySnapshot['team'], extra: Partial<EntitySnapshot> = {}): EntitySnapshot => ({ id: 1, kind: 'minion', team, x: 0, y: 0, facing: 0, hp: 1, maxHp: 1, ...extra });
  it('attaque les ennemis et neutres vivants seulement', () => {
    expect(isAttackable(e('red'), 'blue')).toBe(true);
    expect(isAttackable(e('neutral', { kind: 'monster' }), 'blue')).toBe(true);
    expect(isAttackable(e('blue'), 'blue')).toBe(false);
    expect(isAttackable(e('red', { dead: true }), 'blue')).toBe(false);
    expect(isAttackable(e('red', { kind: 'projectile' }), 'blue')).toBe(false);
  });
  it('sorts ciblés', () => {
    expect(needsTarget('targeted')).toBe(true);
    expect(needsTarget('targetedDash')).toBe(true);
    expect(needsTarget('skillshot')).toBe(false);
  });
});
