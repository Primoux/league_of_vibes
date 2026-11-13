/** FX procéduraux de satire parlementaire. Aucun asset externe. */
import * as THREE from 'three';
import { MelenchonView } from '../../render/champions/melenchon.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { dashTrail, flash, ProjectileFx, shockwave, telegraph, timed } from '../effects/primitives.ts';
import { registerFx } from '../registry.ts';

const COLORS = [new THREE.Color('#2865ff'), new THREE.Color('#fff8e6'), new THREE.Color('#f33450')];
let installed = false;

function papers(rt: FxRuntime, at: THREE.Vector3, count: number): void {
  const group = new THREE.Group();
  group.position.copy(at);
  const geometry = new THREE.PlaneGeometry(0.22, 0.3);
  const material = new THREE.MeshBasicMaterial({ color: '#fff6df', side: THREE.DoubleSide, transparent: true });
  for (let i = 0; i < count; i++) {
    const page = new THREE.Mesh(geometry, material);
    page.rotation.set(i * 0.8, i * 0.7, i);
    group.add(page);
  }
  rt.root.add(group);
  timed(rt, 0.9, (t) => {
    material.opacity = 1 - t;
    group.children.forEach((page, i) => {
      const a = i * 2.399;
      page.position.set(Math.cos(a) * t * 1.6, 0.6 + Math.sin(t * Math.PI) * 1.6, Math.sin(a) * t * 1.6);
      page.rotation.x += 0.025; page.rotation.z += 0.035;
    });
  }, () => { group.removeFromParent(); geometry.dispose(); material.dispose(); });
}

function proclamation(rt: FxRuntime, at: THREE.Vector3): void {
  if (typeof document === 'undefined') return;
  const canvas = document.createElement('canvas');
  canvas.width = 1024; canvas.height = 256;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.fillStyle = '#111c32'; ctx.fillRect(8, 18, 1008, 214);
  COLORS.forEach((c, i) => { ctx.fillStyle = `#${c.getHexString()}`; ctx.fillRect(8 + i * 336, 18, 336, 7); });
  ctx.textAlign = 'center'; ctx.fillStyle = '#fff6e5'; ctx.font = 'bold 60px Georgia';
  ctx.fillText('LA RÉPUBLIQUE,', 512, 110); ctx.fillText('C’EST MOI !', 512, 186);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  const material = new THREE.SpriteMaterial({ map: texture, transparent: true, depthWrite: false });
  const sprite = new THREE.Sprite(material);
  sprite.position.copy(at); sprite.position.y += 3.4;
  sprite.scale.set(5.4, 1.35, 1); rt.root.add(sprite);
  timed(rt, 2, (t) => { sprite.position.y = at.y + 3.4 + t * 0.45; material.opacity = Math.min(1, t * 10, (1 - t) * 5); },
    () => { sprite.removeFromParent(); material.dispose(); texture.dispose(); });
}

export function installMelenchonFx(): void {
  if (installed) return;
  installed = true;
  registerFx('melenchon_q', {
    projectile(rt, e, c) {
      const fx = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
        color: COLORS[2]!, core: COLORS[1]!, size: 0.38, height: 1.3,
        ribbonWidth: 0.14, trailStyle: rt.s.spark, trailDensity: 5,
      });
      rt.add(fx); return fx;
    },
    hit(rt, e) { papers(rt, rt.world(e.pos), 6); flash(rt, rt.world(e.pos, 1), 1.1, COLORS[2]!); },
  });
  registerFx('melenchon_w_telegraph', {
    area(rt, e) { telegraph(rt, rt.world(e.pos), rt.scale(e.radius), e.duration, COLORS[2]!, { fill: 0.1 }); },
  });
  registerFx('melenchon_w', {
    area(rt, e) { papers(rt, rt.world(e.pos), 14); shockwave(rt, rt.world(e.pos), rt.scale(e.radius), 0.55, COLORS[2]!); },
  });
  registerFx('melenchon_e', {
    dash(rt, e, c) { dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), 0.08, c.accent, false); },
    area(rt, e, c) { shockwave(rt, rt.world(e.pos), rt.scale(e.radius), 0.5, c.accent); },
  });
  registerFx('melenchon_e_telegraph', {
    area(rt, e, c) {
      telegraph(rt, rt.world(e.pos), rt.scale(e.radius), e.duration, c.accent, { fill: 0.08 });
      const clone = new MelenchonView(true);
      clone.object3d.position.copy(rt.world(e.pos));
      clone.object3d.rotation.y = rt.entities.object(e.source)?.rotation.y ?? 0;
      clone.playAnim('cast_Q'); rt.root.add(clone.object3d);
      timed(rt, e.duration, (_t, dt) => clone.update({ id: e.source, kind: 'champion', team: 'blue', x: e.pos.x, y: e.pos.y, hp: 1, maxHp: 1, facing: 0 }, dt), () => clone.dispose());
    },
  });
  registerFx('melenchon_r', {
    cast(rt, e) {
      const at = new THREE.Vector3();
      if (rt.entities.pos(e.source, at)) telegraph(rt, at, rt.scale(500), 0.6, COLORS[1]!, { fill: 0.04 });
    },
    area(rt, e) {
      const at = rt.world(e.pos);
      COLORS.forEach((color, i) => rt.after(i * 0.09, () => shockwave(rt, at, rt.scale(e.radius), 0.8, color, 0.055)));
      papers(rt, at, 18); proclamation(rt, at);
    },
    // L'aura du modèle suit le buff réel ; la pancarte est réservée à l'impact validé par le serveur.
    buff() {},
  });
  registerFx('melenchon_p', {
    buff(rt, e, c) { const pos = new THREE.Vector3(); if (rt.entities.pos(e.target, pos)) flash(rt, pos, 0.7, c.secondary, 0.3); },
  });
}
