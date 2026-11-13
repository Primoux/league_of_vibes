/**
 * Caméra isométrique. Propriétaire : claude-client.
 *
 * - Vue en perspective inclinée (~56°), regardant le centre `center` (coordonnées jeu).
 * - Mode verrouillé (Y) : suit le champion local. Mode libre : bords d'écran, flèches, clic minimap.
 * - Espace maintenu : recentre sur le champion. Molette : zoom borné. Limites de carte.
 */
import * as THREE from 'three';
import { MAP_SIZE, clamp, type Vec2 } from '@lov/shared';
import { S } from './coords.ts';

export const CAMERA_PITCH = (56 * Math.PI) / 180;
export const ZOOM_MIN = 1500; // distance caméra-cible (unités de jeu)
export const ZOOM_MAX = 2800;
const EDGE_PX = 12;
/** Marge de déplacement autorisée autour de la carte (unités de jeu). */
const BOUNDS_MARGIN = 400;

export class CameraController {
  readonly camera: THREE.PerspectiveCamera;
  /** Centre visé, coordonnées jeu. */
  readonly center: Vec2 = { x: 1500, y: 1500 };
  distance = 2300;
  private targetDistance = 2300;
  /** Position de souris (pixels) pour le défilement par les bords ; null = souris hors fenêtre. */
  private mouse: { x: number; y: number } | null = null;
  private keys = { left: false, right: false, up: false, down: false };
  centerHeld = false;
  private lookAt = new THREE.Vector3();

  constructor(aspect: number) {
    this.camera = new THREE.PerspectiveCamera(38, aspect, 0.5, 200);
  }

  setMouse(x: number, y: number): void {
    if (!this.mouse) this.mouse = { x, y };
    else {
      this.mouse.x = x;
      this.mouse.y = y;
    }
  }
  clearMouse(): void {
    this.mouse = null;
  }
  setKey(k: keyof CameraController['keys'], down: boolean): void {
    this.keys[k] = down;
  }

  zoomBy(deltaY: number): void {
    this.targetDistance = clamp(this.targetDistance + deltaY * 1.2, ZOOM_MIN, ZOOM_MAX);
  }

  jumpTo(p: Vec2): void {
    this.center.x = clamp(p.x, -BOUNDS_MARGIN, MAP_SIZE + BOUNDS_MARGIN);
    this.center.y = clamp(p.y, -BOUNDS_MARGIN, MAP_SIZE + BOUNDS_MARGIN);
  }

  /**
   * @param follow position du champion à suivre (si verrouillé ou Espace), sinon null.
   * @param edgePan défilement par les bords activé.
   * @param speed vitesse de défilement libre (unités jeu / s).
   * @returns true si l'utilisateur a déplacé la caméra manuellement (bords/flèches).
   */
  update(dt: number, follow: Vec2 | null, locked: boolean, edgePan: boolean, speed: number, viewW: number, viewH: number): boolean {
    let manual = false;
    if (follow && (locked || this.centerHeld)) {
      // suivi serré mais lissé
      const k = this.centerHeld ? 1 : Math.min(1, dt * 12);
      this.center.x += (follow.x - this.center.x) * k;
      this.center.y += (follow.y - this.center.y) * k;
    } else {
      let dx = 0;
      let dy = 0;
      if (edgePan && this.mouse) {
        if (this.mouse.x <= EDGE_PX) dx -= 1;
        else if (this.mouse.x >= viewW - EDGE_PX) dx += 1;
        if (this.mouse.y <= EDGE_PX) dy += 1;
        else if (this.mouse.y >= viewH - EDGE_PX) dy -= 1;
      }
      if (this.keys.left) dx -= 1;
      if (this.keys.right) dx += 1;
      if (this.keys.up) dy += 1;
      if (this.keys.down) dy -= 1;
      if (dx || dy) {
        const len = Math.hypot(dx, dy);
        this.center.x += (dx / len) * speed * dt;
        this.center.y += (dy / len) * speed * dt;
        manual = true;
      }
    }
    this.center.x = clamp(this.center.x, -BOUNDS_MARGIN, MAP_SIZE + BOUNDS_MARGIN);
    this.center.y = clamp(this.center.y, -BOUNDS_MARGIN, MAP_SIZE + BOUNDS_MARGIN);

    this.distance += (this.targetDistance - this.distance) * Math.min(1, dt * 10);
    this.apply();
    return manual;
  }

  /** Place la caméra : au sud du centre (z+), élevée selon le pitch. */
  apply(): void {
    const d = this.distance * S;
    const cx = this.center.x * S;
    const cz = -this.center.y * S;
    this.camera.position.set(cx, Math.sin(CAMERA_PITCH) * d, cz + Math.cos(CAMERA_PITCH) * d);
    this.lookAt.set(cx, 0, cz);
    this.camera.lookAt(this.lookAt);
  }

  resize(aspect: number): void {
    this.camera.aspect = aspect;
    this.camera.updateProjectionMatrix();
  }
}
