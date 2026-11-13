/**
 * Overlay de performance (`?stats`) : fps, temps de frame, draw calls, triangles, entités. Propriétaire : claude-client.
 * Mis à jour 2 fois par seconde, aucune allocation par frame.
 */
import type * as THREE from 'three';

export class StatsOverlay {
  private el: HTMLDivElement;
  private frames = 0;
  private total = 0;
  private acc = 0;
  private worst = 0;
  private calls = 0;
  private tris = 0;

  /** `?stats=log` : écrit aussi les mesures dans la console (mesures headless). */
  private log = new URLSearchParams(location.search).get('stats') === 'log';

  constructor() {
    this.el = document.createElement('div');
    this.el.style.cssText =
      'position:fixed;right:8px;top:8px;z-index:50;padding:6px 10px;background:rgba(8,10,20,.82);color:#cfe;font:12px/1.45 ui-monospace,monospace;border:1px solid #345;border-radius:4px;pointer-events:none;white-space:pre';
    document.body.appendChild(this.el);
  }

  /** À appeler après le rendu de chaque frame. */
  frame(dt: number, renderer: THREE.WebGLRenderer, entities: number): void {
    if (this.log && entities > 0) console.info(`[stats] entities=${entities} calls=${renderer.info.render.calls} tris=${renderer.info.render.triangles}`);
    this.frames++;
    this.acc += dt;
    if (dt > this.worst) this.worst = dt;
    // renderer.info est remis à zéro à chaque render() : on garde le max de la fenêtre
    this.calls = Math.max(this.calls, renderer.info.render.calls);
    this.tris = Math.max(this.tris, renderer.info.render.triangles);
    if (this.acc < 0.5 && this.frames < 30) return;
    const fps = this.frames / this.acc;
    this.el.textContent =
      `${fps.toFixed(0)} fps  ${((this.acc / this.frames) * 1000).toFixed(1)} ms (pire ${(this.worst * 1000).toFixed(0)})\n` +
      `draw calls ${this.calls}  tris ${(this.tris / 1000).toFixed(0)}k\n` +
      `entités ${entities}  géo ${renderer.info.memory.geometries}  tex ${renderer.info.memory.textures}`;
    (this.el.dataset as DOMStringMap).fps = fps.toFixed(1);
    (this.el.dataset as DOMStringMap).calls = String(this.calls);
    (this.el.dataset as DOMStringMap).tris = String(this.tris);
    this.frames = 0;
    this.acc = 0;
    this.worst = 0;
    this.calls = 0;
    this.tris = 0;
  }
}
