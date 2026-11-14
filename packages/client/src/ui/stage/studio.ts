/**
 * Studio photo des champions : rend les vrais modèles 3D de claude-fx (`createChampionView`) dans un
 * WebGLRenderer hors écran, avec un éclairage de portrait (clé chaude, contre-jour à la couleur du champion),
 * et met en cache des images (buste et pied) utilisées par tous les portraits de l'UI.
 * Propriétaire : mistral-ui. Repli : si WebGL est indisponible, les portraits gardent leur version SVG.
 */
import * as THREE from 'three';
import { CHAMPIONS, CHAMPION_LIST } from '@lov/shared';
import { createChampionView, type AnimatedChampionView } from '../../render/champions/index.ts';
import { championArtworkUrl } from '../icons/ChampionArtwork.tsx';
import { MiniStore } from '../useStore.ts';

export type ShotKind = 'bust' | 'full';

/** `version` change à chaque nouvelle image prête (les portraits se re-rendent). */
export const studio = new MiniStore<{ version: number }>({ version: 0 });

const cache = new Map<string, string>();
const queue: { id: string; kind: ShotKind }[] = [];
const queued = new Set<string>();
let renderer: THREE.WebGLRenderer | null = null;
let failed = false;
let pumping = false;
let nextViewId = -100_000;

/** Image du champion si elle est prête, sinon null (et la demande est mise en file). */
export function getShot(id: string, kind: ShotKind): string | null {
  const key = `${id}:${kind}`;
  const hit = cache.get(key);
  if (hit) return hit;
  if (failed || !CHAMPIONS[id] || typeof document === 'undefined') return null;
  if (!queued.has(key)) {
    queued.add(key);
    queue.push({ id, kind });
    pump();
  }
  return null;
}

/** Précharge tous les bustes (appelé au démarrage de l'UI). */
export function preloadShots(): void {
  // seuls les champions sans illustration peinte ont besoin d'une photo 3D
  const todo = CHAMPION_LIST.filter((c) => !championArtworkUrl(c.id));
  for (const c of todo) getShot(c.id, 'bust');
  for (const c of todo) getShot(c.id, 'full');
}

function pump(): void {
  if (pumping) return;
  pumping = true;
  const step = () => {
    const job = queue.shift();
    if (!job) {
      pumping = false;
      return;
    }
    try {
      const url = shoot(job.id, job.kind);
      if (url) {
        cache.set(`${job.id}:${job.kind}`, url);
        studio.set({ version: studio.state.version + 1 });
      }
    } catch (err) {
      console.warn('[ui/studio] rendu impossible, portraits SVG conservés', err);
      failed = true;
      queue.length = 0;
      pumping = false;
      return;
    }
    // une image par tâche : l'interface reste fluide
    setTimeout(step, 16);
  };
  setTimeout(step, 0);
}

/** Lumières de studio réutilisées par la scène vivante (ChampionStage). */
export function studioLights(scene: THREE.Scene, rimColor: THREE.ColorRepresentation): { rim: THREE.DirectionalLight; rim2: THREE.DirectionalLight } {
  scene.add(new THREE.HemisphereLight(0xdfe6ff, 0x1a1020, 0.9));
  const key = new THREE.DirectionalLight(0xfff0d8, 2.4);
  key.position.set(2.5, 3.5, 4);
  scene.add(key);
  const fill = new THREE.DirectionalLight(0x8fb4ff, 0.7);
  fill.position.set(-3, 1.5, 2);
  scene.add(fill);
  const rim = new THREE.DirectionalLight(rimColor, 3.2);
  rim.position.set(-2.5, 2.5, -3.5);
  scene.add(rim);
  const rim2 = new THREE.DirectionalLight(rimColor, 2);
  rim2.position.set(3, 1.2, -3);
  scene.add(rim2);
  return { rim, rim2 };
}

/** Crée la vue 3D d'un champion, posée à l'origine, en pose de repos. */
export function makeChampionModel(id: string): AnimatedChampionView | null {
  const view = createChampionView({ id: nextViewId--, subtype: id, team: 'neutral' });
  if (!view) return null;
  view.playAnim('idle');
  return view;
}

/** État factice minimal pour animer une vue hors de la partie. */
export function idleSnap(id: string): Parameters<AnimatedChampionView['update']>[0] {
  return { id: -1, kind: 'champion', team: 'neutral', x: 0, y: 0, facing: 0, hp: 1, maxHp: 1, subtype: id, anim: 'idle' };
}

const box = new THREE.Box3();
const size = new THREE.Vector3();
const center = new THREE.Vector3();

/** Cadre la caméra sur le modèle : `bust` = tête et épaules, `full` = en pied. */
export function frame(camera: THREE.PerspectiveCamera, object: THREE.Object3D, kind: ShotKind, aspect: number): void {
  box.setFromObject(object);
  box.getSize(size);
  box.getCenter(center);
  const h = Math.max(0.2, size.y);
  const fov = THREE.MathUtils.degToRad(camera.fov);
  if (kind === 'bust') {
    const focusY = box.max.y - h * 0.3;
    const visible = h * 0.62;
    const dist = visible / 2 / Math.tan(fov / 2);
    camera.position.set(center.x + dist * 0.42, focusY + h * 0.06, center.z + dist * 0.9);
    camera.lookAt(center.x, focusY, center.z);
  } else {
    const visible = Math.max(h * 1.12, (size.x * 1.15) / aspect);
    const dist = visible / 2 / Math.tan(fov / 2);
    camera.position.set(center.x + dist * 0.3, center.y + h * 0.12, center.z + dist * 0.95);
    camera.lookAt(center.x, center.y + h * 0.02, center.z);
  }
}

function shoot(id: string, kind: ShotKind): string | null {
  const w = kind === 'bust' ? 256 : 300;
  const h = kind === 'bust' ? 256 : 400;
  if (!renderer) {
    renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, preserveDrawingBuffer: true });
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.15;
    renderer.setClearColor(0x000000, 0);
  }
  renderer.setPixelRatio(1);
  renderer.setSize(w, h, false);

  const champ = CHAMPIONS[id]!;
  const scene = new THREE.Scene();
  studioLights(scene, champ.palette.primary);
  const view = makeChampionModel(id);
  if (!view) return null;
  scene.add(view.object3d);
  view.object3d.rotation.y = kind === 'bust' ? 0.35 : 0.25;
  // quelques pas d'animation pour sortir de la pose neutre
  const snap = idleSnap(id);
  for (let i = 0; i < 12; i++) view.update(snap, 1 / 30);
  view.object3d.updateMatrixWorld(true);

  const camera = new THREE.PerspectiveCamera(kind === 'bust' ? 26 : 30, w / h, 0.01, 100);
  frame(camera, view.object3d, kind, w / h);
  renderer.render(scene, camera);
  const url = renderer.domElement.toDataURL('image/png');
  scene.remove(view.object3d);
  view.dispose();
  return url;
}
