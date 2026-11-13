/**
 * Conversion coordonnées jeu <-> monde Three.js. Propriétaire : claude-client.
 * Jeu (x, y) -> monde (x * S, h, -y * S) : le y du jeu correspond au -z du monde.
 */
import * as THREE from 'three';
import { RENDER_SCALE, type Vec2 } from '@lov/shared';

export const S = RENDER_SCALE;

export const toWorld = (p: Vec2, h = 0): THREE.Vector3 => new THREE.Vector3(p.x * S, h, -p.y * S);
export const toGame = (v: { x: number; z: number }): Vec2 => ({ x: v.x / S, y: -v.z / S });

/** Variante sans allocation. */
export function toWorldInto(out: THREE.Vector3, x: number, y: number, h = 0): THREE.Vector3 {
  return out.set(x * S, h, -y * S);
}
export function toGameInto(out: Vec2, v: { x: number; z: number }): Vec2 {
  out.x = v.x / S;
  out.y = -v.z / S;
  return out;
}

/** Orientation jeu (radians, 0 = +x, sens trigo dans le plan jeu) -> rotation.y Three.js d'un objet qui regarde +x. */
export const facingToRotY = (facing: number): number => facing;
