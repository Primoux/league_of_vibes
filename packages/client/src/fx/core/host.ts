/**
 * Ce dont les FX ont besoin du moteur de rendu. Le `GameRenderer` de claude-client le satisfait structurellement
 * (scene, camera, renderer) ; les membres optionnels sont utilisés dès qu'ils existent :
 *  - `onFrame(cb)` : sinon les FX tournent dans leur propre requestAnimationFrame ;
 *  - `getEntityObject(id)` : position interpolée affichée ; sinon EntityTracker lisse les snapshots du store ;
 *  - `setRenderPipeline(fn)` : permet à PostFx de remplacer `renderer.render` (bloom, FXAA…) ;
 *  - `shake(intensité, durée)` : tremblement de caméra des gros impacts.
 */
import type * as THREE from 'three';

export interface FxHost {
  scene: THREE.Scene;
  camera: THREE.Camera;
  renderer?: THREE.WebGLRenderer;
  onFrame?(cb: (dt: number) => void): unknown;
  getEntityObject?(id: number): THREE.Object3D | undefined;
  setRenderPipeline?(render: () => void): unknown;
  /** Tremblement de caméra (amplitude en unités de jeu, durée en s). */
  shake?(intensity: number, duration: number): unknown;
}
