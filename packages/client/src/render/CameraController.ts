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
  /** Tremblement : amplitude (unités de jeu) et temps restant / durée totale (s). */
  private shakeAmp = 0;
  private shakeLeft = 0;
  private shakeDur = 0;
  private shakeT = 0;
  /** Décalage de tremblement courant (unités de jeu), pour les tests. */
  readonly shakeOffset = { x: 0, y: 0 };

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

  /**
   * Fait trembler la caméra : `intensity` = amplitude en unités de jeu (≈ 15 léger, 40 fort, max 80),
   * `duration` en secondes. Un nouveau tremblement plus fort remplace le courant, un plus faible est ignoré
   * s'il reste plus d'énergie en cours. Décroissance quadratique. N'affecte pas `center`.
   */
  shake(intensity: number, duration: number): void {
    const amp = clamp(intensity, 0, 80);
    const current = this.shakeDur > 0 ? this.shakeAmp * (this.shakeLeft / this.shakeDur) ** 2 : 0;
    if (amp < current || duration <= 0) return;
    this.shakeAmp = amp;
    this.shakeLeft = this.shakeDur = duration;
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
    this.shakeT += dt;
    if (this.shakeLeft > 0) {
      this.shakeLeft = Math.max(0, this.shakeLeft - dt);
      // / √2 : la norme du décalage (x, y) reste ≤ amplitude
      const k = this.shakeAmp * (this.shakeLeft / this.shakeDur) ** 2 * Math.SQRT1_2;
      // bruit pseudo-aléatoire lisse (somme de sinus incommensurables)
      this.shakeOffset.x = k * (Math.sin(this.shakeT * 47.3) * 0.6 + Math.sin(this.shakeT * 83.1) * 0.4);
      this.shakeOffset.y = k * (Math.sin(this.shakeT * 53.7 + 1.3) * 0.6 + Math.sin(this.shakeT * 91.9 + 0.7) * 0.4);
    } else {
      this.shakeOffset.x = 0;
      this.shakeOffset.y = 0;
    }
    this.apply();
    return manual;
  }

  /** Place la caméra : au sud du centre (z+), élevée selon le pitch. */
  apply(): void {
    const d = this.distance * S;
    const cx = (this.center.x + this.shakeOffset.x) * S;
    const cz = -(this.center.y + this.shakeOffset.y) * S;
    this.camera.position.set(cx, Math.sin(CAMERA_PITCH) * d, cz + Math.cos(CAMERA_PITCH) * d);
    this.lookAt.set(cx, 0, cz);
    this.camera.lookAt(this.lookAt);
  }

  resize(aspect: number): void {
    this.camera.aspect = aspect;
    this.camera.updateProjectionMatrix();
  }
}
