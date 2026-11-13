/**
 * Barres de vie 3D au-dessus des unités. Propriétaire : claude-client.
 *
 * - Toutes les barres en UN draw call : quads instanciés, taille fixe en pixels (projection de l'ancre 3D
 *   puis décalage écran dans le vertex shader). PV, bouclier, mana (champions), graduations tous les 100 PV.
 * - Couleurs : joueur local vert-jaune, alliés bleu, ennemis rouge, neutres orange.
 * - Champions : étiquette niveau + nom (sprite canvas, redessinée seulement quand elle change).
 * Aucune allocation par frame.
 */
import * as THREE from 'three';
import type { EntityId, EntitySnapshot, Team } from '@lov/shared';

const MAX = 320;

const vert = /* glsl */ `
attribute vec3 iAnchor;
attribute vec2 iSize;      // px
attribute vec4 iBars;      // hp, shield, mana (-1 = pas de mana), maxHp
attribute vec3 iColor;
uniform vec2 uViewport;
varying vec2 vUv;
varying vec4 vBars;
varying vec3 vColor;
varying vec2 vSize;
void main() {
  vUv = uv;
  vBars = iBars;
  vColor = iColor;
  vSize = iSize;
  vec4 clip = projectionMatrix * viewMatrix * vec4(iAnchor, 1.0);
  clip.xy += position.xy * iSize * 2.0 / uViewport * clip.w;
  gl_Position = clip;
}`;

const frag = /* glsl */ `
varying vec2 vUv;
varying vec4 vBars;
varying vec3 vColor;
varying vec2 vSize;
void main() {
  vec2 px = vUv * vSize;
  float border = 1.0;
  bool hasMana = vBars.z >= 0.0;
  // cadre
  if (px.x < border || px.y < border || px.x > vSize.x - border || px.y > vSize.y - border) {
    gl_FragColor = vec4(0.04, 0.04, 0.06, 0.95);
    return;
  }
  float manaH = hasMana ? max(3.0, vSize.y * 0.28) : 0.0;
  vec2 inner = vec2((px.x - border) / (vSize.x - 2.0 * border), px.y);
  vec3 col = vec3(0.07, 0.07, 0.09);
  if (hasMana && px.y < border + manaH) {
    // barre de mana (en bas)
    if (inner.x < vBars.z) col = mix(vec3(0.20, 0.45, 0.95), vec3(0.35, 0.65, 1.0), (px.y - border) / manaH);
    if (px.y > border + manaH - 1.0) col = vec3(0.04);
  } else {
    float hp = vBars.x;
    float sh = vBars.y;
    float yN = (px.y - border - manaH) / (vSize.y - 2.0 * border - manaH);
    if (inner.x < hp) col = vColor * (0.82 + 0.3 * yN);
    else if (inner.x < hp + sh) col = vec3(0.92, 0.92, 0.95);
    // graduations tous les 100 PV (épaisses tous les 1000)
    float maxHp = vBars.w;
    if (maxHp > 0.0 && inner.x < hp + sh) {
      float w = vSize.x - 2.0 * border;
      float step = w * 100.0 / maxHp;
      if (step > 3.0) {
        float m = mod(inner.x * w, step);
        float big = mod(inner.x * w, step * 10.0);
        if (m < 1.0 && yN > 0.45) col *= 0.35;
        if (big < 1.5) col *= 0.35;
      }
    }
  }
  gl_FragColor = vec4(col, 1.0);
  #include <colorspace_fragment>
}`;

const COLOR_SELF = new THREE.Color(0x9be03a);
const COLOR_ALLY = new THREE.Color(0x3a8fff);
const COLOR_ENEMY = new THREE.Color(0xe8433a);
const COLOR_NEUTRAL = new THREE.Color(0xe89a2a);

/** Hauteur de l'ancre au-dessus du sol (monde) et largeur de barre (px) selon le type. */
export function barLayout(e: EntitySnapshot): { h: number; w: number; hgt: number } | null {
  switch (e.kind) {
    case 'champion':
      return { h: 2.5, w: 104, hgt: 12 };
    case 'minion':
      return e.subtype === 'super' || e.subtype === 'cannon' ? { h: 1.5, w: 62, hgt: 5 } : { h: 1.2, w: 48, hgt: 5 };
    case 'tower':
      return { h: 6.2, w: 130, hgt: 9 };
    case 'inhibitor':
      return { h: 2.8, w: 120, hgt: 8 };
    case 'nexus':
      return { h: 4.6, w: 150, hgt: 10 };
    case 'monster': {
      const big = e.subtype === 'dragon' || e.subtype === 'elder_dragon' || e.subtype === 'leviathan' || e.subtype === 'herald';
      const buff = e.subtype === 'blue_buff' || e.subtype === 'red_buff';
      return big ? { h: 4.2, w: 150, hgt: 10 } : buff ? { h: 2.4, w: 90, hgt: 7 } : { h: 1.5, w: 60, hgt: 6 };
    }
    case 'ward':
      return { h: 0.9, w: 34, hgt: 4 };
    default:
      return null;
  }
}

interface Label {
  sprite: THREE.Sprite;
  canvas: HTMLCanvasElement;
  tex: THREE.CanvasTexture;
  text: string;
}

export class HealthBars {
  readonly mesh: THREE.Mesh;
  private geo: THREE.InstancedBufferGeometry;
  private anchor: THREE.InstancedBufferAttribute;
  private size: THREE.InstancedBufferAttribute;
  private bars: THREE.InstancedBufferAttribute;
  private color: THREE.InstancedBufferAttribute;
  private mat: THREE.ShaderMaterial;
  private labels = new Map<EntityId, Label>();
  private count = 0;
  private usedLabels = new Set<EntityId>();

  constructor(private readonly scene: THREE.Scene) {
    const quad = new THREE.PlaneGeometry(1, 1);
    this.geo = new THREE.InstancedBufferGeometry();
    this.geo.index = quad.index;
    this.geo.setAttribute('position', quad.getAttribute('position'));
    this.geo.setAttribute('uv', quad.getAttribute('uv'));
    this.anchor = new THREE.InstancedBufferAttribute(new Float32Array(MAX * 3), 3).setUsage(THREE.DynamicDrawUsage);
    this.size = new THREE.InstancedBufferAttribute(new Float32Array(MAX * 2), 2).setUsage(THREE.DynamicDrawUsage);
    this.bars = new THREE.InstancedBufferAttribute(new Float32Array(MAX * 4), 4).setUsage(THREE.DynamicDrawUsage);
    this.color = new THREE.InstancedBufferAttribute(new Float32Array(MAX * 3), 3).setUsage(THREE.DynamicDrawUsage);
    this.geo.setAttribute('iAnchor', this.anchor);
    this.geo.setAttribute('iSize', this.size);
    this.geo.setAttribute('iBars', this.bars);
    this.geo.setAttribute('iColor', this.color);
    this.geo.instanceCount = 0;
    this.mat = new THREE.ShaderMaterial({
      vertexShader: vert,
      fragmentShader: frag,
      uniforms: { uViewport: { value: new THREE.Vector2(1, 1) } },
      depthTest: false,
      depthWrite: false,
      transparent: true,
    });
    this.mesh = new THREE.Mesh(this.geo, this.mat);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 1000;
    scene.add(this.mesh);
  }

  setViewport(w: number, h: number): void {
    (this.mat.uniforms.uViewport!.value as THREE.Vector2).set(w, h);
  }

  begin(): void {
    this.count = 0;
    this.usedLabels.clear();
  }

  /** Ajoute la barre d'une entité à la position monde (wx, wy sol, wz). */
  push(e: EntitySnapshot, wx: number, wy: number, wz: number, myTeam: Team | undefined, isSelf: boolean): void {
    if (e.dead || this.count >= MAX) return;
    const L = barLayout(e);
    if (!L) return;
    const i = this.count++;
    const maxHp = Math.max(1, e.maxHp);
    const shield = e.shield ?? 0;
    const total = Math.max(maxHp, e.hp + shield);
    this.anchor.setXYZ(i, wx, wy + L.h, wz);
    this.size.setXY(i, L.w, e.kind === 'champion' ? L.hgt : L.hgt);
    const hasMana = e.kind === 'champion' && (e.maxMana ?? 0) > 0;
    this.bars.setXYZW(i, e.hp / total, shield / total, hasMana ? (e.mana ?? 0) / (e.maxMana ?? 1) : -1, e.kind === 'champion' ? total : 0);
    const c = isSelf ? COLOR_SELF : e.team === 'neutral' ? COLOR_NEUTRAL : e.team === myTeam ? COLOR_ALLY : COLOR_ENEMY;
    this.color.setXYZ(i, c.r, c.g, c.b);

    if (e.kind === 'champion') {
      const text = `${e.level ?? 1}|${e.champ?.name ?? ''}`;
      const label = this.label(e.id, text);
      label.sprite.position.set(wx, wy + L.h, wz);
      label.sprite.visible = true;
      this.usedLabels.add(e.id);
    }
  }

  end(): void {
    this.geo.instanceCount = this.count;
    this.anchor.needsUpdate = true;
    this.size.needsUpdate = true;
    this.bars.needsUpdate = true;
    this.color.needsUpdate = true;
    for (const [id, l] of this.labels) if (!this.usedLabels.has(id)) l.sprite.visible = false;
  }

  /** Supprime l'étiquette d'une entité disparue. */
  remove(id: EntityId): void {
    const l = this.labels.get(id);
    if (!l) return;
    l.sprite.removeFromParent();
    l.tex.dispose();
    (l.sprite.material as THREE.SpriteMaterial).dispose();
    this.labels.delete(id);
  }

  private label(id: EntityId, text: string): Label {
    let l = this.labels.get(id);
    if (!l) {
      const canvas = document.createElement('canvas');
      canvas.width = 256;
      canvas.height = 64;
      const tex = new THREE.CanvasTexture(canvas);
      tex.colorSpace = THREE.SRGBColorSpace;
      const mat = new THREE.SpriteMaterial({ map: tex, depthTest: false, depthWrite: false, sizeAttenuation: false, transparent: true });
      const sprite = new THREE.Sprite(mat);
      sprite.renderOrder = 1001;
      // taille écran fixe : 256x64 px de canvas -> ~128x32 px à l'écran (dépend de la hauteur de vue)
      sprite.center.set(0.5, -0.15);
      sprite.scale.set(192 * this.labelK, 48 * this.labelK, 1);
      this.scene.add(sprite);
      l = { sprite, canvas, tex, text: '' };
      this.labels.set(id, l);
    }
    if (l.text !== text) {
      l.text = text;
      const [level, name] = text.split('|');
      const ctx = l.canvas.getContext('2d')!;
      ctx.clearRect(0, 0, 256, 64);
      ctx.font = '600 26px system-ui, sans-serif';
      ctx.textBaseline = 'middle';
      // pastille de niveau
      ctx.fillStyle = 'rgba(10,10,16,0.85)';
      ctx.beginPath();
      ctx.arc(22, 40, 18, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = '#c8aa6e';
      ctx.lineWidth = 2;
      ctx.stroke();
      ctx.fillStyle = '#f0e6d2';
      ctx.textAlign = 'center';
      ctx.fillText(level ?? '1', 22, 41);
      ctx.textAlign = 'left';
      ctx.lineWidth = 4;
      ctx.strokeStyle = 'rgba(0,0,0,0.8)';
      ctx.strokeText(name ?? '', 46, 40);
      ctx.fillText(name ?? '', 46, 40);
      l.tex.needsUpdate = true;
    }
    return l;
  }

  /** Échelle des étiquettes selon la hauteur du viewport (taille constante en pixels). */
  updateLabelScale(viewH: number, fovDeg: number): void {
    // sizeAttenuation=false : 1 unité = hauteur de vue complète (en NDC/2) ; on vise 128x32 px
    const k = (2 * Math.tan((fovDeg * Math.PI) / 360)) / viewH;
    for (const l of this.labels.values()) l.sprite.scale.set(192 * k, 48 * k, 1);
    this.labelK = k;
  }
  labelK = 0;
}
