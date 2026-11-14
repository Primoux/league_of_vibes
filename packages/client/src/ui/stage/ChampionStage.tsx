/**
 * Scène 3D vivante d'un champion : modèle animé de claude-fx sur un piédestal lumineux à ses couleurs,
 * particules ascendantes, rotation lente (glisser pour tourner). Utilisée par l'accueil et la draft.
 * `pulse` qui change = le champion joue son ultime (verrouillage, changement de vedette).
 */
import * as THREE from 'three';
import { CHAMPIONS } from '@lov/shared';
import { useEffect, useRef } from 'preact/hooks';
import type { AnimatedChampionView } from '../../render/champions/index.ts';
import { idleSnap, makeChampionModel, studioLights } from './studio.ts';

const box = new THREE.Box3();

interface StageProps {
  championId: string | null;
  /** Incrémenter pour déclencher une animation d'éclat (ultime). */
  pulse?: number;
  class?: string;
  /** Rotation automatique (rad/s). */
  spin?: number;
}

const PARTICLES = 90;

function glowTexture(): THREE.CanvasTexture {
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const g = c.getContext('2d')!;
  const grad = g.createRadialGradient(64, 64, 0, 64, 64, 64);
  grad.addColorStop(0, 'rgba(255,255,255,1)');
  grad.addColorStop(0.25, 'rgba(255,255,255,0.55)');
  grad.addColorStop(1, 'rgba(255,255,255,0)');
  g.fillStyle = grad;
  g.fillRect(0, 0, 128, 128);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

class Stage {
  renderer: THREE.WebGLRenderer;
  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(28, 1, 0.01, 100);
  root = new THREE.Group();
  view: AnimatedChampionView | null = null;
  champ: string | null = null;
  rim: THREE.DirectionalLight;
  rim2: THREE.DirectionalLight;
  ring: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>;
  ring2: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>;
  floor: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>;
  pts: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial>;
  speeds = new Float32Array(PARTICLES);
  angle = 0.4;
  drag = 0;
  spin: number;
  appear = 1;
  raf = 0;
  last = performance.now();
  lastStill = 0;
  tex = glowTexture();

  constructor(
    private canvas: HTMLCanvasElement,
    spin: number,
  ) {
    this.spin = spin;
    this.renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.2;
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.setPixelRatio(Math.min(2, devicePixelRatio || 1));
    ({ rim: this.rim, rim2: this.rim2 } = studioLights(this.scene, 0xffffff));
    this.scene.add(this.root);

    // piédestal : disque sombre, deux anneaux additifs, halo au sol
    const disc = new THREE.Mesh(new THREE.CylinderGeometry(0.62, 0.7, 0.08, 64), new THREE.MeshStandardMaterial({ color: 0x15131f, metalness: 0.6, roughness: 0.35 }));
    disc.position.y = -0.04;
    this.scene.add(disc);
    const add = (m: THREE.Material) => Object.assign(m, { transparent: true, blending: THREE.AdditiveBlending, depthWrite: false });
    this.ring = new THREE.Mesh(new THREE.RingGeometry(0.66, 0.7, 96), add(new THREE.MeshBasicMaterial({ color: 0xffffff })) as THREE.MeshBasicMaterial);
    this.ring.rotation.x = -Math.PI / 2;
    this.ring.position.y = 0.002;
    this.scene.add(this.ring);
    this.ring2 = new THREE.Mesh(new THREE.RingGeometry(0.82, 0.84, 96, 1, 0, Math.PI * 1.6), add(new THREE.MeshBasicMaterial({ color: 0xffffff, opacity: 0.5 })) as THREE.MeshBasicMaterial);
    this.ring2.rotation.x = -Math.PI / 2;
    this.scene.add(this.ring2);
    this.floor = new THREE.Mesh(new THREE.PlaneGeometry(3.2, 3.2), add(new THREE.MeshBasicMaterial({ map: this.tex, opacity: 0.55 })) as THREE.MeshBasicMaterial);
    this.floor.rotation.x = -Math.PI / 2;
    this.floor.position.y = -0.085;
    this.scene.add(this.floor);

    // particules ascendantes
    const pos = new Float32Array(PARTICLES * 3);
    for (let i = 0; i < PARTICLES; i++) this.resetParticle(pos, i, Math.random() * 2.2);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
    this.pts = new THREE.Points(geo, add(new THREE.PointsMaterial({ size: 0.05, map: this.tex, color: 0xffffff, opacity: 0.9 })) as THREE.PointsMaterial);
    this.scene.add(this.pts);
  }

  private resetParticle(pos: Float32Array, i: number, y = 0): void {
    const a = Math.random() * Math.PI * 2;
    const r = 0.25 + Math.random() * 0.6;
    pos[i * 3] = Math.cos(a) * r;
    pos[i * 3 + 1] = y;
    pos[i * 3 + 2] = Math.sin(a) * r;
    this.speeds[i] = 0.15 + Math.random() * 0.45;
  }

  setChampion(id: string | null): void {
    if (id === this.champ) return;
    this.champ = id;
    if (this.view) {
      this.root.remove(this.view.object3d);
      this.view.dispose();
      this.view = null;
    }
    if (!id || !CHAMPIONS[id]) return;
    const pal = CHAMPIONS[id]!.palette;
    this.view = makeChampionModel(id);
    if (!this.view) return;
    this.root.add(this.view.object3d);
    const col = new THREE.Color(pal.primary);
    const acc = new THREE.Color(pal.accent);
    this.rim.color.copy(col);
    this.rim2.color.copy(acc);
    this.ring.material.color.copy(col);
    this.ring2.material.color.copy(acc);
    this.floor.material.color.copy(col);
    this.pts.material.color.copy(acc);
    this.appear = 0;
    this.resize();
  }

  pulse(): void {
    this.view?.cast('R');
  }

  resize(): void {
    const w = this.canvas.clientWidth || 300;
    const h = this.canvas.clientHeight || 400;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    if (this.view) {
      const rot = this.root.rotation.y;
      const sc = this.root.scale.x;
      this.root.rotation.y = 0;
      this.root.scale.setScalar(1);
      this.root.updateMatrixWorld(true);
      box.setFromObject(this.view.object3d);
      this.root.rotation.y = rot;
      this.root.scale.setScalar(sc);
      const h = Math.max(0.3, box.max.y - Math.min(0, box.min.y));
      const fov = THREE.MathUtils.degToRad(this.camera.fov);
      const visibleH = Math.max(h * 1.45, 2.0 / this.camera.aspect);
      const dist = visibleH / 2 / Math.tan(fov / 2);
      this.camera.position.set(0, h * 0.62, dist);
      this.camera.lookAt(0, h * 0.42, 0);
    }
  }

  start(): void {
    const loop = (now: number) => {
      this.raf = requestAnimationFrame(loop);
      const dt = Math.min(0.05, (now - this.last) / 1000);
      this.last = now;
      if (document.hidden) return;
      // mode figé (captures) : une image de temps en temps suffit
      if (document.documentElement.classList.contains('lov-still') && now - this.lastStill < 500) return;
      this.lastStill = now;
      this.angle += (this.spin + this.drag) * dt;
      this.drag *= 0.92;
      this.root.rotation.y = this.angle;
      if (this.view) this.view.update(idleSnap(this.champ ?? ''), dt);
      // apparition : montée + mise à l'échelle
      this.appear = Math.min(1, this.appear + dt * 2.2);
      const e = 1 - Math.pow(1 - this.appear, 3);
      this.root.scale.setScalar(0.85 + 0.15 * e);
      this.root.position.y = (1 - e) * -0.15;
      this.ring2.rotation.z += dt * 0.6;
      const t = now / 1000;
      this.ring.material.opacity = 0.65 + Math.sin(t * 2.4) * 0.25;
      const p = this.pts.geometry.getAttribute('position') as THREE.BufferAttribute;
      const arr = p.array as Float32Array;
      for (let i = 0; i < PARTICLES; i++) {
        arr[i * 3 + 1] = arr[i * 3 + 1]! + this.speeds[i]! * dt;
        if (arr[i * 3 + 1]! > 2.2) this.resetParticle(arr, i);
      }
      p.needsUpdate = true;
      this.renderer.render(this.scene, this.camera);
    };
    this.raf = requestAnimationFrame(loop);
  }

  dispose(): void {
    cancelAnimationFrame(this.raf);
    this.setChampion(null);
    this.scene.traverse((o) => {
      const m = o as THREE.Mesh;
      m.geometry?.dispose();
      const mat = m.material as THREE.Material | THREE.Material[] | undefined;
      if (Array.isArray(mat)) mat.forEach((x) => x.dispose());
      else mat?.dispose();
    });
    this.tex.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
  }
}

export function ChampionStage({ championId, pulse = 0, class: cls, spin = 0.35 }: StageProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<Stage | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let stage: Stage;
    try {
      stage = new Stage(canvas, spin);
    } catch (err) {
      console.warn('[ui/stage] WebGL indisponible', err);
      return;
    }
    stageRef.current = stage;
    stage.setChampion(championId);
    stage.start();
    const ro = new ResizeObserver(() => stage.resize());
    ro.observe(canvas);
    let dragging = false;
    let lastX = 0;
    const down = (e: PointerEvent) => {
      dragging = true;
      lastX = e.clientX;
      canvas.setPointerCapture(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (!dragging) return;
      stage.drag = ((e.clientX - lastX) / Math.max(1, canvas.clientWidth)) * 60;
      lastX = e.clientX;
    };
    const up = () => (dragging = false);
    canvas.addEventListener('pointerdown', down);
    canvas.addEventListener('pointermove', move);
    canvas.addEventListener('pointerup', up);
    return () => {
      ro.disconnect();
      stage.dispose();
      stageRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    stageRef.current?.setChampion(championId);
  }, [championId]);

  useEffect(() => {
    if (pulse) stageRef.current?.pulse();
  }, [pulse]);

  return <canvas ref={canvasRef} class={`lov-stage ${cls ?? ''}`} />;
}
