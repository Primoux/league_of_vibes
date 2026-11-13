/**
 * Grille de navigation (cellules de 50 u) dérivée de `WALLS`, + A* octile + lissage par ligne de vue.
 * Propriétaire : claude-server. Grille statique construite paresseusement une fois par processus.
 */
import { MAP_SIZE, WALLS, clamp, type Polygon, type Vec2 } from '@lov/shared';

export const NAV_CELL = 50;
/** Marge autour des murs (≈ rayon moyen d'une unité) pour que les unités ne frottent pas les murs. */
export const NAV_MARGIN = 35;

export class NavGrid {
  readonly cols: number;
  readonly rows: number;
  readonly blocked: Uint8Array;
  // Buffers A* réutilisés (pas d'allocation par requête).
  private g: Float64Array;
  private parent: Int32Array;
  private stamp: Uint32Array;
  private closed: Uint32Array;
  private gen = 0;
  private heap: Int32Array;
  private heapF: Float64Array;
  private heapSize = 0;

  constructor(walls: Polygon[], size = MAP_SIZE, cell = NAV_CELL, margin = NAV_MARGIN) {
    this.cols = Math.ceil(size / cell);
    this.rows = Math.ceil(size / cell);
    const n = this.cols * this.rows;
    this.blocked = new Uint8Array(n);
    this.g = new Float64Array(n);
    this.parent = new Int32Array(n);
    this.stamp = new Uint32Array(n);
    this.closed = new Uint32Array(n);
    this.heap = new Int32Array(n * 2);
    this.heapF = new Float64Array(n * 2);
    for (const poly of walls) this.rasterize(poly, cell, margin);
  }

  private rasterize(poly: Polygon, cell: number, margin: number): void {
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const p of poly) {
      minX = Math.min(minX, p.x); minY = Math.min(minY, p.y);
      maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y);
    }
    const c0 = Math.max(0, Math.floor((minX - margin) / cell));
    const c1 = Math.min(this.cols - 1, Math.floor((maxX + margin) / cell));
    const r0 = Math.max(0, Math.floor((minY - margin) / cell));
    const r1 = Math.min(this.rows - 1, Math.floor((maxY + margin) / cell));
    const m2 = margin * margin;
    const p = { x: 0, y: 0 };
    for (let r = r0; r <= r1; r++) {
      for (let c = c0; c <= c1; c++) {
        p.x = (c + 0.5) * cell;
        p.y = (r + 0.5) * cell;
        if (insidePoly(p, poly) || distToPoly2(p, poly) < m2) this.blocked[r * this.cols + c] = 1;
      }
    }
  }

  cellIndex(x: number, y: number): number {
    const c = clamp(Math.floor(x / NAV_CELL), 0, this.cols - 1);
    const r = clamp(Math.floor(y / NAV_CELL), 0, this.rows - 1);
    return r * this.cols + c;
  }

  cellCenter(i: number): Vec2 {
    return { x: ((i % this.cols) + 0.5) * NAV_CELL, y: (Math.floor(i / this.cols) + 0.5) * NAV_CELL };
  }

  walkable(x: number, y: number): boolean {
    if (x < 0 || y < 0 || x > MAP_SIZE || y > MAP_SIZE) return false;
    return this.blocked[this.cellIndex(x, y)] === 0;
  }

  /** Point accessible le plus proche de p (p lui-même s'il est accessible). Recherche en anneaux croissants. */
  nearestWalkable(p: Vec2): Vec2 {
    const x = clamp(p.x, 1, MAP_SIZE - 1);
    const y = clamp(p.y, 1, MAP_SIZE - 1);
    if (this.walkable(x, y)) return { x, y };
    const c0 = Math.floor(x / NAV_CELL);
    const r0 = Math.floor(y / NAV_CELL);
    const maxR = Math.max(this.cols, this.rows);
    for (let rad = 1; rad < maxR; rad++) {
      let best = -1;
      let bestD = Infinity;
      for (let dr = -rad; dr <= rad; dr++) {
        for (let dc = -rad; dc <= rad; dc++) {
          if (Math.abs(dr) !== rad && Math.abs(dc) !== rad) continue;
          const r = r0 + dr;
          const c = c0 + dc;
          if (r < 0 || c < 0 || r >= this.rows || c >= this.cols) continue;
          const i = r * this.cols + c;
          if (this.blocked[i]) continue;
          const cx = (c + 0.5) * NAV_CELL;
          const cy = (r + 0.5) * NAV_CELL;
          const d = (cx - x) ** 2 + (cy - y) ** 2;
          if (d < bestD) {
            bestD = d;
            best = i;
          }
        }
      }
      if (best >= 0) return this.cellCenter(best);
    }
    return { x, y };
  }

  /** Ligne de vue « marchable » entre a et b (échantillonnage tous les NAV_CELL/2). */
  lineWalkable(a: Vec2, b: Vec2): boolean {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const len = Math.hypot(dx, dy);
    const steps = Math.ceil(len / (NAV_CELL / 2));
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      if (!this.walkable(a.x + dx * t, a.y + dy * t)) return false;
    }
    return true;
  }

  /**
   * Dernier point accessible le long du segment a→b (pour les dashes qui s'arrêtent aux murs).
   * Retourne b si tout le segment est accessible.
   */
  lastWalkableOnSegment(a: Vec2, b: Vec2): Vec2 {
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const len = Math.hypot(dx, dy);
    const steps = Math.max(1, Math.ceil(len / 10));
    let lx = a.x;
    let ly = a.y;
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const x = a.x + dx * t;
      const y = a.y + dy * t;
      if (!this.walkable(x, y)) return { x: lx, y: ly };
      lx = x;
      ly = y;
    }
    return { x: b.x, y: b.y };
  }

  /**
   * Chemin de `from` à `to` : liste de points de passage (sans le point de départ, finit par la destination
   * ramenée au point accessible le plus proche). [] si déjà arrivé ou inaccessible.
   */
  findPath(from: Vec2, to: Vec2): Vec2[] {
    const goal = this.nearestWalkable(to);
    const start = this.walkable(from.x, from.y) ? from : this.nearestWalkable(from);
    if (this.lineWalkable(start, goal)) return start === from ? [goal] : [start, goal];
    const s = this.cellIndex(start.x, start.y);
    const g = this.cellIndex(goal.x, goal.y);
    const cells = this.astar(s, g);
    if (!cells) return [];
    const pts: Vec2[] = cells.map((i) => this.cellCenter(i));
    pts[pts.length - 1] = goal;
    const smoothed = this.smooth(start, pts);
    return start === from ? smoothed : [start, ...smoothed];
  }

  /** Lissage glouton : depuis chaque point, saute au point le plus lointain visible. */
  private smooth(start: Vec2, pts: Vec2[]): Vec2[] {
    const out: Vec2[] = [];
    let cur = start;
    let i = 0;
    while (i < pts.length) {
      let j = pts.length - 1;
      while (j > i && !this.lineWalkable(cur, pts[j]!)) j--;
      out.push(pts[j]!);
      cur = pts[j]!;
      i = j + 1;
    }
    return out;
  }

  private astar(start: number, goal: number): number[] | null {
    if (start === goal) return [goal];
    if (++this.gen === 0xffffffff) {
      this.stamp.fill(0);
      this.closed.fill(0);
      this.gen = 1;
    }
    const gen = this.gen;
    const cols = this.cols;
    const gc = goal % cols;
    const gr = (goal / cols) | 0;
    const h = (i: number): number => {
      const dx = Math.abs((i % cols) - gc);
      const dy = Math.abs(((i / cols) | 0) - gr);
      return Math.max(dx, dy) + (Math.SQRT2 - 1) * Math.min(dx, dy);
    };
    this.heapSize = 0;
    this.stamp[start] = gen;
    this.g[start] = 0;
    this.parent[start] = -1;
    this.push(start, h(start));
    while (this.heapSize > 0) {
      const cur = this.pop();
      if (this.closed[cur] === gen) continue;
      this.closed[cur] = gen;
      if (cur === goal) {
        const path: number[] = [];
        for (let i = goal; i !== start; i = this.parent[i]!) path.push(i);
        return path.reverse();
      }
      const cc = cur % cols;
      const cr = (cur / cols) | 0;
      const gCur = this.g[cur]!;
      for (let dr = -1; dr <= 1; dr++) {
        for (let dc = -1; dc <= 1; dc++) {
          if (dr === 0 && dc === 0) continue;
          const nc = cc + dc;
          const nr = cr + dr;
          if (nc < 0 || nr < 0 || nc >= cols || nr >= this.rows) continue;
          const ni = nr * cols + nc;
          if (this.blocked[ni] || this.closed[ni] === gen) continue;
          // Pas de coupe de coin en diagonale.
          if (dr !== 0 && dc !== 0 && (this.blocked[cr * cols + nc] || this.blocked[nr * cols + cc])) continue;
          const ng = gCur + (dr !== 0 && dc !== 0 ? Math.SQRT2 : 1);
          if (this.stamp[ni] !== gen || ng < this.g[ni]!) {
            this.stamp[ni] = gen;
            this.g[ni] = ng;
            this.parent[ni] = cur;
            this.push(ni, ng + h(ni));
          }
        }
      }
    }
    return null;
  }

  private push(i: number, f: number): void {
    if (this.heapSize >= this.heap.length) return; // ne devrait pas arriver (borne n*2)
    let k = this.heapSize++;
    while (k > 0) {
      const p = (k - 1) >> 1;
      if (this.heapF[p]! <= f) break;
      this.heap[k] = this.heap[p]!;
      this.heapF[k] = this.heapF[p]!;
      k = p;
    }
    this.heap[k] = i;
    this.heapF[k] = f;
  }

  private pop(): number {
    const top = this.heap[0]!;
    const last = --this.heapSize;
    const li = this.heap[last]!;
    const lf = this.heapF[last]!;
    let k = 0;
    for (;;) {
      let c = 2 * k + 1;
      if (c >= last) break;
      if (c + 1 < last && this.heapF[c + 1]! < this.heapF[c]!) c++;
      if (this.heapF[c]! >= lf) break;
      this.heap[k] = this.heap[c]!;
      this.heapF[k] = this.heapF[c]!;
      k = c;
    }
    this.heap[k] = li;
    this.heapF[k] = lf;
    return top;
  }
}

function insidePoly(p: Vec2, poly: Polygon): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i]!;
    const b = poly[j]!;
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

function distToPoly2(p: Vec2, poly: Polygon): number {
  let best = Infinity;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[j]!;
    const b = poly[i]!;
    const abx = b.x - a.x;
    const aby = b.y - a.y;
    const l2 = abx * abx + aby * aby;
    const t = l2 === 0 ? 0 : clamp(((p.x - a.x) * abx + (p.y - a.y) * aby) / l2, 0, 1);
    const d = (a.x + abx * t - p.x) ** 2 + (a.y + aby * t - p.y) ** 2;
    if (d < best) best = d;
  }
  return best;
}

let shared: NavGrid | null = null;
/** Grille partagée construite depuis WALLS (une seule fois par processus). */
export function navGrid(): NavGrid {
  return (shared ??= new NavGrid(WALLS));
}
