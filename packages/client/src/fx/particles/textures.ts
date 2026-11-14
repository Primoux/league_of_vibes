/**
 * Atlas de textures de particules dessinées procéduralement dans un canvas (aucun asset externe).
 * Grille 4×4 de cellules 128 px, masques blancs (la couleur vient du style et de la teinte).
 */
import * as THREE from 'three';

export const TEX = {
  glow: 0,
  spark: 1,
  smoke: 2,
  ring: 3,
  star: 4,
  snowflake: 5,
  feather: 6,
  rune: 7,
  shard: 8,
  dot: 9,
  streak: 10,
  cross: 11,
  coin: 12,
  flame: 13,
  swirl: 14,
  sparkles: 15,
} as const;
export type TexName = keyof typeof TEX;

const CELL = 128;
const GRID = 4;

type Painter = (g: CanvasRenderingContext2D, s: number) => void;

const radial = (g: CanvasRenderingContext2D, s: number, stops: [number, number][]): void => {
  const grad = g.createRadialGradient(s / 2, s / 2, 0, s / 2, s / 2, s / 2);
  for (const [o, a] of stops) grad.addColorStop(o, `rgba(255,255,255,${a})`);
  g.fillStyle = grad;
  g.fillRect(0, 0, s, s);
};

/** Bruit déterministe pour que l'atlas soit identique à chaque chargement. */
const hash = (n: number): number => {
  const x = Math.sin(n * 127.1) * 43758.5453;
  return x - Math.floor(x);
};

const PAINTERS: Record<TexName, Painter> = {
  glow: (g, s) => radial(g, s, [[0, 1], [0.2, 0.75], [0.5, 0.22], [1, 0]]),
  spark: (g, s) => {
    radial(g, s, [[0, 1], [0.12, 0.5], [0.35, 0], [1, 0]]);
    g.translate(s / 2, s / 2);
    for (let i = 0; i < 2; i++) {
      const grad = g.createLinearGradient(-s / 2, 0, s / 2, 0);
      grad.addColorStop(0, 'rgba(255,255,255,0)');
      grad.addColorStop(0.5, 'rgba(255,255,255,1)');
      grad.addColorStop(1, 'rgba(255,255,255,0)');
      g.fillStyle = grad;
      g.beginPath();
      g.moveTo(-s / 2, 0);
      g.lineTo(0, -s * 0.035);
      g.lineTo(s / 2, 0);
      g.lineTo(0, s * 0.035);
      g.fill();
      g.rotate(Math.PI / 2);
    }
  },
  smoke: (g, s) => {
    for (let i = 0; i < 14; i++) {
      const r = s * (0.12 + hash(i) * 0.18);
      const x = s / 2 + (hash(i + 20) - 0.5) * s * 0.38;
      const y = s / 2 + (hash(i + 40) - 0.5) * s * 0.38;
      const grad = g.createRadialGradient(x, y, 0, x, y, r);
      grad.addColorStop(0, 'rgba(255,255,255,0.35)');
      grad.addColorStop(1, 'rgba(255,255,255,0)');
      g.fillStyle = grad;
      g.fillRect(0, 0, s, s);
    }
  },
  ring: (g, s) => {
    const grad = g.createRadialGradient(s / 2, s / 2, 0, s / 2, s / 2, s / 2);
    grad.addColorStop(0, 'rgba(255,255,255,0)');
    grad.addColorStop(0.62, 'rgba(255,255,255,0)');
    grad.addColorStop(0.8, 'rgba(255,255,255,1)');
    grad.addColorStop(0.9, 'rgba(255,255,255,0.4)');
    grad.addColorStop(1, 'rgba(255,255,255,0)');
    g.fillStyle = grad;
    g.fillRect(0, 0, s, s);
  },
  star: (g, s) => {
    g.translate(s / 2, s / 2);
    g.shadowColor = 'white';
    g.shadowBlur = s * 0.08;
    g.fillStyle = 'white';
    g.beginPath();
    for (let i = 0; i < 10; i++) {
      const r = (i % 2 ? 0.17 : 0.4) * s;
      const a = (i / 10) * Math.PI * 2 - Math.PI / 2;
      g.lineTo(Math.cos(a) * r, Math.sin(a) * r);
    }
    g.closePath();
    g.fill();
  },
  snowflake: (g, s) => {
    g.translate(s / 2, s / 2);
    g.strokeStyle = 'white';
    g.lineCap = 'round';
    g.shadowColor = 'white';
    g.shadowBlur = s * 0.05;
    g.lineWidth = s * 0.045;
    for (let i = 0; i < 6; i++) {
      g.beginPath();
      g.moveTo(0, 0);
      g.lineTo(0, -s * 0.4);
      g.moveTo(0, -s * 0.24);
      g.lineTo(-s * 0.09, -s * 0.33);
      g.moveTo(0, -s * 0.24);
      g.lineTo(s * 0.09, -s * 0.33);
      g.stroke();
      g.rotate(Math.PI / 3);
    }
  },
  feather: (g, s) => {
    g.translate(s / 2, s / 2);
    g.rotate(-Math.PI / 4);
    g.fillStyle = 'white';
    g.beginPath();
    g.ellipse(0, 0, s * 0.13, s * 0.42, 0, 0, Math.PI * 2);
    g.fill();
    g.globalCompositeOperation = 'destination-out';
    g.lineWidth = s * 0.012;
    for (let i = -6; i <= 6; i++) {
      g.beginPath();
      g.moveTo(0, i * s * 0.05);
      g.lineTo(s * 0.15 * Math.sign(i || 1), i * s * 0.05 - s * 0.06);
      g.stroke();
    }
    g.fillRect(-s * 0.008, -s * 0.42, s * 0.016, s * 0.84);
  },
  rune: (g, s) => {
    g.translate(s / 2, s / 2);
    g.strokeStyle = 'white';
    g.shadowColor = 'white';
    g.shadowBlur = s * 0.04;
    g.lineWidth = s * 0.03;
    g.beginPath();
    g.arc(0, 0, s * 0.42, 0, Math.PI * 2);
    g.stroke();
    g.lineWidth = s * 0.018;
    g.beginPath();
    g.arc(0, 0, s * 0.33, 0, Math.PI * 2);
    g.stroke();
    for (let i = 0; i < 3; i++) {
      const a = (i / 3) * Math.PI * 2 - Math.PI / 2;
      const b = a + (Math.PI * 2) / 3;
      g.beginPath();
      g.moveTo(Math.cos(a) * s * 0.33, Math.sin(a) * s * 0.33);
      g.lineTo(Math.cos(b) * s * 0.33, Math.sin(b) * s * 0.33);
      g.stroke();
    }
    for (let i = 0; i < 12; i++) {
      const a = (i / 12) * Math.PI * 2;
      g.fillStyle = 'white';
      g.fillRect(Math.cos(a) * s * 0.375 - 2, Math.sin(a) * s * 0.375 - 2, 4, 4);
    }
  },
  shard: (g, s) => {
    g.translate(s / 2, s / 2);
    const grad = g.createLinearGradient(-s * 0.15, 0, s * 0.15, 0);
    grad.addColorStop(0, 'rgba(255,255,255,0.55)');
    grad.addColorStop(0.5, 'rgba(255,255,255,1)');
    grad.addColorStop(1, 'rgba(255,255,255,0.4)');
    g.fillStyle = grad;
    g.beginPath();
    g.moveTo(0, -s * 0.45);
    g.lineTo(s * 0.16, 0);
    g.lineTo(0, s * 0.45);
    g.lineTo(-s * 0.16, 0);
    g.closePath();
    g.fill();
  },
  dot: (g, s) => radial(g, s, [[0, 1], [0.55, 1], [0.75, 0.35], [1, 0]]),
  streak: (g, s) => {
    const grad = g.createLinearGradient(0, 0, s, 0);
    grad.addColorStop(0, 'rgba(255,255,255,0)');
    grad.addColorStop(0.7, 'rgba(255,255,255,1)');
    grad.addColorStop(1, 'rgba(255,255,255,0)');
    g.fillStyle = grad;
    const v = g.createLinearGradient(0, s * 0.35, 0, s * 0.65);
    g.fillRect(0, s * 0.42, s, s * 0.16);
    g.globalCompositeOperation = 'destination-in';
    v.addColorStop(0, 'rgba(255,255,255,0)');
    v.addColorStop(0.5, 'rgba(255,255,255,1)');
    v.addColorStop(1, 'rgba(255,255,255,0)');
    g.fillStyle = v;
    g.fillRect(0, 0, s, s);
  },
  cross: (g, s) => {
    g.translate(s / 2, s / 2);
    g.shadowColor = 'white';
    g.shadowBlur = s * 0.08;
    g.fillStyle = 'white';
    const w = s * 0.12;
    const l = s * 0.34;
    g.fillRect(-w, -l, w * 2, l * 2);
    g.fillRect(-l, -w, l * 2, w * 2);
  },
  coin: (g, s) => {
    g.translate(s / 2, s / 2);
    g.fillStyle = 'white';
    g.beginPath();
    g.ellipse(0, 0, s * 0.36, s * 0.4, 0, 0, Math.PI * 2);
    g.fill();
    g.globalCompositeOperation = 'destination-out';
    g.lineWidth = s * 0.04;
    g.beginPath();
    g.ellipse(0, 0, s * 0.26, s * 0.3, 0, 0, Math.PI * 2);
    g.stroke();
  },
  flame: (g, s) => {
    g.translate(s / 2, s * 0.62);
    const grad = g.createRadialGradient(0, 0, 0, 0, 0, s * 0.36);
    grad.addColorStop(0, 'rgba(255,255,255,1)');
    grad.addColorStop(0.6, 'rgba(255,255,255,0.6)');
    grad.addColorStop(1, 'rgba(255,255,255,0)');
    g.fillStyle = grad;
    g.beginPath();
    g.moveTo(0, -s * 0.55);
    g.bezierCurveTo(s * 0.3, -s * 0.2, s * 0.32, s * 0.1, 0, s * 0.3);
    g.bezierCurveTo(-s * 0.32, s * 0.1, -s * 0.3, -s * 0.2, 0, -s * 0.55);
    g.fill();
  },
  swirl: (g, s) => {
    g.translate(s / 2, s / 2);
    g.strokeStyle = 'white';
    g.lineCap = 'round';
    for (let k = 0; k < 3; k++) {
      g.rotate((Math.PI * 2) / 3);
      g.beginPath();
      for (let i = 0; i <= 40; i++) {
        const t = i / 40;
        const a = t * Math.PI * 1.4;
        const r = s * (0.06 + 0.36 * t);
        g.lineWidth = s * 0.05 * (1 - t) + 1;
        g.lineTo(Math.cos(a) * r, Math.sin(a) * r);
      }
      g.stroke();
    }
  },
  sparkles: (g, s) => {
    for (let i = 0; i < 9; i++) {
      const x = s * (0.15 + hash(i + 3) * 0.7);
      const y = s * (0.15 + hash(i + 11) * 0.7);
      const r = s * (0.03 + hash(i + 17) * 0.05);
      const grad = g.createRadialGradient(x, y, 0, x, y, r);
      grad.addColorStop(0, 'rgba(255,255,255,1)');
      grad.addColorStop(1, 'rgba(255,255,255,0)');
      g.fillStyle = grad;
      g.fillRect(x - r, y - r, r * 2, r * 2);
    }
  },
};

let atlas: THREE.CanvasTexture | null = null;

/** Atlas partagé (créé au premier appel). */
export function getParticleAtlas(): THREE.CanvasTexture {
  if (atlas) return atlas;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = CELL * GRID;
  const g = canvas.getContext('2d')!;
  for (const [name, idx] of Object.entries(TEX) as [TexName, number][]) {
    g.save();
    g.translate((idx % GRID) * CELL, Math.floor(idx / GRID) * CELL);
    g.beginPath();
    g.rect(0, 0, CELL, CELL);
    g.clip();
    PAINTERS[name](g, CELL);
    g.restore();
  }
  atlas = new THREE.CanvasTexture(canvas);
  atlas.colorSpace = THREE.NoColorSpace; // masque, pas une couleur
  // Pas de mipmaps : aux niveaux réduits, chaque cellule (128 px) ne fait que quelques texels et déborde sur
  // ses voisines → le quad entier d'une particule sombre apparaissait comme un carré noir à distance.
  atlas.generateMipmaps = false;
  atlas.minFilter = THREE.LinearFilter;
  atlas.magFilter = THREE.LinearFilter;
  return atlas;
}

/** Texture d'une seule cellule (sprites, décals) : réutilise l'atlas via offset/repeat. */
export function atlasCell(name: TexName): THREE.Texture {
  const t = getParticleAtlas().clone();
  const idx = TEX[name];
  // cellule légèrement rentrée (comme les particules) : le filtrage ne lit pas la cellule voisine
  t.repeat.set(0.97 / GRID, 0.97 / GRID);
  t.offset.set(((idx % GRID) + 0.015) / GRID, 1 - (Math.floor(idx / GRID) + 1 - 0.015) / GRID);
  t.needsUpdate = true;
  return t;
}
