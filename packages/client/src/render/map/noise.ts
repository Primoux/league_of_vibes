/**
 * Texture de bruit tuilable partagée par les shaders du décor (sol, eau, falaises).
 * Propriétaire : claude-client (ajout claude-fx, refonte graphique).
 *
 * Un échantillon de texture coûte bien moins cher qu'un bruit calculé dans le shader : chaque canal porte
 * un fBm (bruit fractal) périodique de fréquence différente, combinés dans les shaders à plusieurs échelles.
 *  - r : basse fréquence (taches, variations de teinte)
 *  - g : fréquence moyenne (bords organiques)
 *  - b : haute fréquence (grain)
 *  - a : bruit cellulaire (cailloux, pavés, caustiques)
 */
import * as THREE from 'three';

const N = 256;

/** Temps (s) partagé par les shaders animés du décor (vent, eau, caustiques). Avancé par MapView. */
export const decorTime = { value: 0 };

function hash(i: number, j: number, seed: number): number {
  let h = (i * 374761393 + j * 668265263 + seed * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Bruit de valeur périodique (période `p` cellules sur la texture). */
function valueNoise(x: number, y: number, p: number, seed: number): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const fx = x - xi;
  const fy = y - yi;
  const u = fx * fx * (3 - 2 * fx);
  const v = fy * fy * (3 - 2 * fy);
  const i0 = ((xi % p) + p) % p;
  const j0 = ((yi % p) + p) % p;
  const i1 = (i0 + 1) % p;
  const j1 = (j0 + 1) % p;
  const a = hash(i0, j0, seed);
  const b = hash(i1, j0, seed);
  const c = hash(i0, j1, seed);
  const d = hash(i1, j1, seed);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

function fbm(x: number, y: number, base: number, octaves: number, seed: number): number {
  let sum = 0;
  let amp = 0.5;
  let norm = 0;
  let p = base;
  for (let o = 0; o < octaves; o++) {
    sum += valueNoise((x / N) * p, (y / N) * p, p, seed + o * 17) * amp;
    norm += amp;
    amp *= 0.5;
    p *= 2;
  }
  return sum / norm;
}

/** Distance au point de Worley le plus proche (périodique), normalisée 0..1. */
function cellular(x: number, y: number, p: number, seed: number): number {
  const cx = (x / N) * p;
  const cy = (y / N) * p;
  const xi = Math.floor(cx);
  const yi = Math.floor(cy);
  let best = 9;
  for (let dj = -1; dj <= 1; dj++)
    for (let di = -1; di <= 1; di++) {
      const i = xi + di;
      const j = yi + dj;
      const ii = ((i % p) + p) % p;
      const jj = ((j % p) + p) % p;
      const px = i + hash(ii, jj, seed);
      const py = j + hash(ii, jj, seed + 1);
      const d = Math.hypot(px - cx, py - cy);
      if (d < best) best = d;
    }
  return Math.min(1, best);
}

/** Contraste étiré : les fBm se concentrent autour de 0,5, on les ramène sur toute la plage. */
const stretch = (v: number, k: number): number => Math.min(1, Math.max(0, (v - 0.5) * k + 0.5));

let shared: THREE.DataTexture | null = null;

export function noiseTexture(): THREE.DataTexture {
  if (shared) return shared;
  const data = new Uint8Array(N * N * 4);
  for (let y = 0; y < N; y++)
    for (let x = 0; x < N; x++) {
      const k = (y * N + x) * 4;
      data[k] = stretch(fbm(x, y, 4, 4, 11), 1.9) * 255;
      data[k + 1] = stretch(fbm(x, y, 8, 4, 23), 1.9) * 255;
      data[k + 2] = stretch(fbm(x, y, 32, 3, 37), 1.7) * 255;
      data[k + 3] = cellular(x, y, 24, 53) * 255;
    }
  const tex = new THREE.DataTexture(data, N, N, THREE.RGBAFormat);
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 4;
  tex.needsUpdate = true;
  shared = tex;
  return tex;
}
