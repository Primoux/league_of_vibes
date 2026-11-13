/**
 * Sol de la carte : géométrie avec relief + texture peinte dans un canvas. Propriétaire : claude-client.
 */
import * as THREE from 'three';
import { BRUSHES, FOUNTAINS, LANE_HALF_WIDTH, LANE_PATHS, MAP_SIZE, PITS, RIVER_HALF_WIDTH, WALLS, type Polygon } from '@lov/shared';
import { S } from '../coords.ts';
import { terrainHeight } from './geometry.ts';
import { rng } from './rng.ts';
import { applyFog } from '../FogOfWar.ts';

const TEX = 2048;
/** Le sol déborde de la carte pour remplir la vue aux bords. */
export const GROUND_MARGIN = 2000;

export function buildTerrain(quality: 'low' | 'high'): THREE.Mesh {
  const size = MAP_SIZE + GROUND_MARGIN * 2;
  const seg = quality === 'high' ? 200 : 120;
  const geo = new THREE.PlaneGeometry(size * S, size * S, seg, seg).rotateX(-Math.PI / 2);
  const pos = geo.attributes.position as THREE.BufferAttribute;
  for (let i = 0; i < pos.count; i++) {
    const gx = pos.getX(i) / S + MAP_SIZE / 2;
    const gy = -pos.getZ(i) / S + MAP_SIZE / 2;
    pos.setY(i, terrainHeight(gx, gy) * S);
  }
  geo.computeVertexNormals();
  // UV : la texture couvre exactement la carte (0..MAP_SIZE), le bord déborde en ClampToEdge
  const uv = geo.attributes.uv as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) {
    const gx = pos.getX(i) / S + MAP_SIZE / 2;
    const gy = -pos.getZ(i) / S + MAP_SIZE / 2;
    uv.setXY(i, gx / MAP_SIZE, gy / MAP_SIZE);
  }
  const tex = new THREE.CanvasTexture(paintGround());
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 8;
  tex.wrapS = tex.wrapT = THREE.ClampToEdgeWrapping;
  const detail = new THREE.CanvasTexture(paintDetail());
  detail.wrapS = detail.wrapT = THREE.RepeatWrapping;
  detail.repeat.set(60, 60);
  const mat = new THREE.MeshStandardMaterial({ map: tex, roughness: 0.95, metalness: 0 });
  // bruit de détail multiplicatif (grain de l'herbe/terre) à haute fréquence
  mat.onBeforeCompile = (sh) => {
    sh.uniforms.uDetail = { value: detail };
    sh.fragmentShader = sh.fragmentShader
      .replace('#include <common>', '#include <common>\nuniform sampler2D uDetail;')
      .replace('#include <map_fragment>', '#include <map_fragment>\ndiffuseColor.rgb *= 0.82 + 0.36 * texture2D(uDetail, vMapUv * 60.0).r;');
  };
  mat.customProgramCacheKey = () => 'terrain';
  applyFog(mat);
  const mesh = new THREE.Mesh(geo, mat);
  mesh.position.set((MAP_SIZE / 2) * S, 0, (-MAP_SIZE / 2) * S);
  mesh.receiveShadow = true;
  mesh.name = 'terrain';
  return mesh;
}

const K = TEX / MAP_SIZE;
const cx = (x: number) => x * K;
const cy = (y: number) => (MAP_SIZE - y) * K;

function pathPoly(ctx: CanvasRenderingContext2D, p: Polygon): void {
  ctx.beginPath();
  p.forEach((q, i) => (i ? ctx.lineTo(cx(q.x), cy(q.y)) : ctx.moveTo(cx(q.x), cy(q.y))));
  ctx.closePath();
}

function paintGround(): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = c.height = TEX;
  const ctx = c.getContext('2d')!;
  const r = rng(1337);

  // herbe de base + taches de variation
  ctx.fillStyle = '#41703a';
  ctx.fillRect(0, 0, TEX, TEX);
  const greens = ['#4a7d3f', '#386433', '#4f8444', '#3b6a36', '#567f3e', '#466f3a'];
  for (let i = 0; i < 2600; i++) {
    ctx.globalAlpha = 0.08 + r() * 0.12;
    ctx.fillStyle = greens[Math.floor(r() * greens.length)]!;
    ctx.beginPath();
    ctx.arc(r() * TEX, r() * TEX, 6 + r() * 38, 0, Math.PI * 2);
    ctx.fill();
  }
  // petites fleurs / touffes claires
  for (let i = 0; i < 2500; i++) {
    ctx.globalAlpha = 0.5;
    ctx.fillStyle = r() < 0.15 ? '#e8d77a' : '#6f9a52';
    ctx.fillRect(r() * TEX, r() * TEX, 1.5, 1.5);
  }
  ctx.globalAlpha = 1;

  // lit de la rivière (sable humide sous l'eau, berges)
  ctx.save();
  ctx.lineCap = 'butt';
  const river = (w: number, color: string) => {
    ctx.strokeStyle = color;
    ctx.lineWidth = w * K * 2;
    ctx.beginPath();
    ctx.moveTo(cx(1350), cy(10650));
    ctx.lineTo(cx(10650), cy(1350));
    ctx.stroke();
  };
  river(RIVER_HALF_WIDTH + 160, '#5f6e45');
  river(RIVER_HALF_WIDTH + 40, '#7c7652');
  river(RIVER_HALF_WIDTH - 120, '#3f5a50');
  ctx.restore();

  // voies : bord sombre, terre, centre plus clair, cailloux
  const stroke = (w: number, color: string, alpha = 1) => {
    ctx.globalAlpha = alpha;
    ctx.strokeStyle = color;
    ctx.lineWidth = w * K * 2;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    for (const p of Object.values(LANE_PATHS)) {
      ctx.beginPath();
      p.forEach((q, i) => (i ? ctx.lineTo(cx(q.x), cy(q.y)) : ctx.moveTo(cx(q.x), cy(q.y))));
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
  };
  stroke(LANE_HALF_WIDTH + 70, '#5e5a3c', 0.7);
  stroke(LANE_HALF_WIDTH, '#9a8460');
  stroke(LANE_HALF_WIDTH * 0.6, '#a99270', 0.8);
  stroke(LANE_HALF_WIDTH * 0.25, '#b39d7a', 0.5);
  for (const p of Object.values(LANE_PATHS)) {
    for (let i = 0; i < p.length - 1; i++) {
      const a = p[i]!;
      const b = p[i + 1]!;
      const n = Math.hypot(b.x - a.x, b.y - a.y) / 25;
      for (let k = 0; k < n; k++) {
        const t = k / n;
        const ox = (r() - 0.5) * LANE_HALF_WIDTH * 1.8;
        const oy = (r() - 0.5) * LANE_HALF_WIDTH * 1.8;
        ctx.fillStyle = r() < 0.5 ? '#7d6a4c' : '#c2ad88';
        ctx.globalAlpha = 0.55;
        ctx.fillRect(cx(a.x + (b.x - a.x) * t + ox), cy(a.y + (b.y - a.y) * t + oy), 2, 2);
      }
    }
  }
  ctx.globalAlpha = 1;

  // bases : dallage circulaire autour de la fontaine, liseré d'équipe
  for (const team of ['blue', 'red'] as const) {
    const f = FOUNTAINS[team];
    const tint = team === 'blue' ? '#4c78c8' : '#c84c4c';
    ctx.fillStyle = '#7d7768';
    ctx.beginPath();
    ctx.arc(cx(f.x), cy(f.y), 2300 * K, 0, Math.PI * 2);
    ctx.fill();
    for (let ring = 2300; ring > 200; ring -= 260) {
      ctx.strokeStyle = 'rgba(40,36,30,0.35)';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(cx(f.x), cy(f.y), ring * K, 0, Math.PI * 2);
      ctx.stroke();
      const n = Math.floor(ring / 120);
      for (let i = 0; i < n; i++) {
        const a = (i / n) * Math.PI * 2 + ring;
        ctx.beginPath();
        ctx.moveTo(cx(f.x + Math.cos(a) * ring), cy(f.y + Math.sin(a) * ring));
        ctx.lineTo(cx(f.x + Math.cos(a) * (ring - 260)), cy(f.y + Math.sin(a) * (ring - 260)));
        ctx.stroke();
      }
    }
    ctx.strokeStyle = tint;
    ctx.globalAlpha = 0.8;
    ctx.lineWidth = 5;
    ctx.beginPath();
    ctx.arc(cx(f.x), cy(f.y), 2300 * K, 0, Math.PI * 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(cx(f.x), cy(f.y), 650 * K, 0, Math.PI * 2);
    ctx.stroke();
    ctx.globalAlpha = 1;
  }

  // fosses : terre battue + anneau de pierres
  for (const pit of Object.values(PITS)) {
    const g = ctx.createRadialGradient(cx(pit.pos.x), cy(pit.pos.y), 0, cx(pit.pos.x), cy(pit.pos.y), pit.radius * K);
    g.addColorStop(0, '#6a6247');
    g.addColorStop(0.85, '#5a5a40');
    g.addColorStop(1, 'rgba(90,90,64,0)');
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(cx(pit.pos.x), cy(pit.pos.y), pit.radius * K, 0, Math.PI * 2);
    ctx.fill();
  }

  // ombre au pied des murs, sol sombre sous les brousses
  ctx.fillStyle = 'rgba(20,30,15,0.55)';
  ctx.filter = 'blur(6px)';
  for (const w of WALLS) {
    pathPoly(ctx, w);
    ctx.fill();
  }
  ctx.filter = 'none';
  ctx.fillStyle = '#2f5a2c';
  for (const b of BRUSHES) {
    pathPoly(ctx, b);
    ctx.fill();
  }
  return c;
}

/** Texture de grain répétée (bruit). */
function paintDetail(): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const ctx = c.getContext('2d')!;
  const img = ctx.createImageData(128, 128);
  const r = rng(7);
  for (let i = 0; i < 128 * 128; i++) {
    const v = 110 + r() * 145;
    img.data[i * 4] = img.data[i * 4 + 1] = img.data[i * 4 + 2] = v;
    img.data[i * 4 + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
  return c;
}
