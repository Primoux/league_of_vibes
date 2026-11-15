//! Grille de navigation (cellules de 50 u) dérivée de `WALLS`, + A* octile + lissage par ligne de vue.
//!
//! Port fidèle de `packages/server/src/nav/grid.ts` : même résolution, même ordre de voisins, mêmes
//! départages dans le tas, mêmes résultats (les clients prédisent le déplacement avec ces chemins).
//!
//! Différence de structure : la grille est immuable (partageable entre threads) ; les tampons de
//! l'A* vivent dans un [`NavSearch`] que l'appelant crée une fois et réutilise, sans allocation par requête.

use std::sync::LazyLock;

use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{Polygon, WALLS};
use lov_shared::math::clamp;
use lov_shared::types::Vec2;

pub const NAV_CELL: f64 = 50.0;
/// Marge autour des murs (≈ rayon moyen d'une unité) pour que les unités ne frottent pas les murs.
pub const NAV_MARGIN: f64 = 35.0;

const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Grille statique : cellules bloquées par les murs.
pub struct NavGrid {
    pub cols: usize,
    pub rows: usize,
    pub blocked: Vec<u8>,
}

/// Tampons réutilisables d'une recherche A* (équivalent des champs privés de la classe TS).
pub struct NavSearch {
    g: Vec<f64>,
    parent: Vec<i32>,
    stamp: Vec<u32>,
    closed: Vec<u32>,
    epoch: u32,
    heap: Vec<i32>,
    heap_f: Vec<f64>,
    heap_size: usize,
    /// Cellules du chemin trouvé (réutilisé).
    cells: Vec<usize>,
    /// Points de passage avant lissage (réutilisé).
    pts: Vec<Vec2>,
}

impl NavSearch {
    pub fn new(grid: &NavGrid) -> Self {
        let n = grid.cols * grid.rows;
        Self {
            g: vec![0.0; n],
            parent: vec![0; n],
            stamp: vec![0; n],
            closed: vec![0; n],
            epoch: 0,
            heap: vec![0; n * 2],
            heap_f: vec![0.0; n * 2],
            heap_size: 0,
            cells: Vec::new(),
            pts: Vec::new(),
        }
    }

    fn push(&mut self, i: usize, f: f64) {
        if self.heap_size >= self.heap.len() {
            return; // ne devrait pas arriver (borne n*2)
        }
        let mut k = self.heap_size;
        self.heap_size += 1;
        while k > 0 {
            let p = (k - 1) >> 1;
            if self.heap_f[p] <= f {
                break;
            }
            self.heap[k] = self.heap[p];
            self.heap_f[k] = self.heap_f[p];
            k = p;
        }
        self.heap[k] = i as i32;
        self.heap_f[k] = f;
    }

    fn pop(&mut self) -> usize {
        let top = self.heap[0];
        self.heap_size -= 1;
        let last = self.heap_size;
        let li = self.heap[last];
        let lf = self.heap_f[last];
        let mut k = 0;
        loop {
            let mut c = 2 * k + 1;
            if c >= last {
                break;
            }
            if c + 1 < last && self.heap_f[c + 1] < self.heap_f[c] {
                c += 1;
            }
            if self.heap_f[c] >= lf {
                break;
            }
            self.heap[k] = self.heap[c];
            self.heap_f[k] = self.heap_f[c];
            k = c;
        }
        self.heap[k] = li;
        self.heap_f[k] = lf;
        top as usize
    }
}

impl NavGrid {
    pub fn new(walls: &[Polygon], size: f64, cell: f64, margin: f64) -> Self {
        let cols = (size / cell).ceil() as usize;
        let rows = cols;
        let mut grid = Self {
            cols,
            rows,
            blocked: vec![0; cols * rows],
        };
        for poly in walls {
            grid.rasterize(poly, cell, margin);
        }
        grid
    }

    /// Grille aux paramètres par défaut (`MAP_SIZE`, `NAV_CELL`, `NAV_MARGIN`).
    pub fn from_walls(walls: &[Polygon]) -> Self {
        Self::new(walls, MAP_SIZE, NAV_CELL, NAV_MARGIN)
    }

    /// Crée des tampons de recherche à la taille de cette grille.
    pub fn new_search(&self) -> NavSearch {
        NavSearch::new(self)
    }

    fn rasterize(&mut self, poly: &[Vec2], cell: f64, margin: f64) {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for p in poly {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let c0 = ((min_x - margin) / cell).floor().max(0.0) as i64;
        let c1 = ((max_x + margin) / cell)
            .floor()
            .min((self.cols - 1) as f64) as i64;
        let r0 = ((min_y - margin) / cell).floor().max(0.0) as i64;
        let r1 = ((max_y + margin) / cell)
            .floor()
            .min((self.rows - 1) as f64) as i64;
        let m2 = margin * margin;
        for r in r0..=r1 {
            for c in c0..=c1 {
                let p = Vec2 {
                    x: (c as f64 + 0.5) * cell,
                    y: (r as f64 + 0.5) * cell,
                };
                if inside_poly(p, poly) || dist_to_poly2(p, poly) < m2 {
                    self.blocked[r as usize * self.cols + c as usize] = 1;
                }
            }
        }
    }

    pub fn cell_index(&self, x: f64, y: f64) -> usize {
        let c = clamp((x / NAV_CELL).floor(), 0.0, (self.cols - 1) as f64) as usize;
        let r = clamp((y / NAV_CELL).floor(), 0.0, (self.rows - 1) as f64) as usize;
        r * self.cols + c
    }

    pub fn cell_center(&self, i: usize) -> Vec2 {
        Vec2 {
            x: ((i % self.cols) as f64 + 0.5) * NAV_CELL,
            y: ((i / self.cols) as f64 + 0.5) * NAV_CELL,
        }
    }

    pub fn walkable(&self, x: f64, y: f64) -> bool {
        if x < 0.0 || y < 0.0 || x > MAP_SIZE || y > MAP_SIZE {
            return false;
        }
        self.blocked[self.cell_index(x, y)] == 0
    }

    /// Point accessible le plus proche de p (p lui-même s'il est accessible). Recherche en anneaux croissants.
    pub fn nearest_walkable(&self, p: Vec2) -> Vec2 {
        let x = clamp(p.x, 1.0, MAP_SIZE - 1.0);
        let y = clamp(p.y, 1.0, MAP_SIZE - 1.0);
        if self.walkable(x, y) {
            return Vec2 { x, y };
        }
        let c0 = (x / NAV_CELL).floor() as i64;
        let r0 = (y / NAV_CELL).floor() as i64;
        let max_r = self.cols.max(self.rows) as i64;
        for rad in 1..max_r {
            let mut best: Option<usize> = None;
            let mut best_d = f64::INFINITY;
            for dr in -rad..=rad {
                for dc in -rad..=rad {
                    if dr.abs() != rad && dc.abs() != rad {
                        continue;
                    }
                    let r = r0 + dr;
                    let c = c0 + dc;
                    if r < 0 || c < 0 || r >= self.rows as i64 || c >= self.cols as i64 {
                        continue;
                    }
                    let i = r as usize * self.cols + c as usize;
                    if self.blocked[i] != 0 {
                        continue;
                    }
                    let cx = (c as f64 + 0.5) * NAV_CELL;
                    let cy = (r as f64 + 0.5) * NAV_CELL;
                    let d = (cx - x) * (cx - x) + (cy - y) * (cy - y);
                    if d < best_d {
                        best_d = d;
                        best = Some(i);
                    }
                }
            }
            if let Some(b) = best {
                return self.cell_center(b);
            }
        }
        Vec2 { x, y }
    }

    /// Ligne de vue « marchable » entre a et b (échantillonnage tous les NAV_CELL/2).
    pub fn line_walkable(&self, a: Vec2, b: Vec2) -> bool {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy);
        let steps = (len / (NAV_CELL / 2.0)).ceil() as i64;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            if !self.walkable(a.x + dx * t, a.y + dy * t) {
                return false;
            }
        }
        true
    }

    /// Dernier point accessible le long du segment a→b (pour les dashes qui s'arrêtent aux murs).
    /// Retourne b si tout le segment est accessible.
    pub fn last_walkable_on_segment(&self, a: Vec2, b: Vec2) -> Vec2 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = dx.hypot(dy);
        let steps = ((len / 10.0).ceil() as i64).max(1);
        let (mut lx, mut ly) = (a.x, a.y);
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let x = a.x + dx * t;
            let y = a.y + dy * t;
            if !self.walkable(x, y) {
                return Vec2 { x: lx, y: ly };
            }
            lx = x;
            ly = y;
        }
        Vec2 { x: b.x, y: b.y }
    }

    /// Chemin de `from` à `to` (voir [`Self::find_path_into`]) dans un nouveau `Vec`.
    pub fn find_path(&self, search: &mut NavSearch, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let mut out = Vec::new();
        self.find_path_into(search, from, to, &mut out);
        out
    }

    /// Chemin de `from` à `to` : liste de points de passage (sans le point de départ, finit par la
    /// destination ramenée au point accessible le plus proche). `out` est vidé puis rempli ; il reste
    /// vide si déjà arrivé ou inaccessible. Aucune allocation une fois les tampons dimensionnés.
    pub fn find_path_into(
        &self,
        search: &mut NavSearch,
        from: Vec2,
        to: Vec2,
        out: &mut Vec<Vec2>,
    ) {
        out.clear();
        let goal = self.nearest_walkable(to);
        let start_is_from = self.walkable(from.x, from.y);
        let start = if start_is_from {
            from
        } else {
            self.nearest_walkable(from)
        };
        if self.line_walkable(start, goal) {
            if !start_is_from {
                out.push(start);
            }
            out.push(goal);
            return;
        }
        let s = self.cell_index(start.x, start.y);
        let g = self.cell_index(goal.x, goal.y);
        if !self.astar(search, s, g) {
            return;
        }
        search.pts.clear();
        for k in 0..search.cells.len() {
            let c = self.cell_center(search.cells[k]);
            search.pts.push(c);
        }
        if let Some(l) = search.pts.last_mut() {
            *l = goal;
        }
        if !start_is_from {
            out.push(start);
        }
        self.smooth(start, &search.pts, out);
    }

    /// Lissage glouton : depuis chaque point, saute au point le plus lointain visible.
    fn smooth(&self, start: Vec2, pts: &[Vec2], out: &mut Vec<Vec2>) {
        let mut cur = start;
        let mut i = 0;
        while i < pts.len() {
            let mut j = pts.len() - 1;
            while j > i && !self.line_walkable(cur, pts[j]) {
                j -= 1;
            }
            out.push(pts[j]);
            cur = pts[j];
            i = j + 1;
        }
    }

    /// A* octile. Remplit `search.cells` (sans `start`, finit par `goal`) ; `false` si inaccessible.
    fn astar(&self, s: &mut NavSearch, start: usize, goal: usize) -> bool {
        s.cells.clear();
        if start == goal {
            s.cells.push(goal);
            return true;
        }
        s.epoch += 1;
        if s.epoch == 0xffff_ffff {
            s.stamp.fill(0);
            s.closed.fill(0);
            s.epoch = 1;
        }
        let epoch = s.epoch;
        let cols = self.cols;
        let gc = goal % cols;
        let gr = goal / cols;
        let h = |i: usize| -> f64 {
            let dx = ((i % cols) as i64 - gc as i64).abs() as f64;
            let dy = ((i / cols) as i64 - gr as i64).abs() as f64;
            dx.max(dy) + (SQRT2 - 1.0) * dx.min(dy)
        };
        s.heap_size = 0;
        s.stamp[start] = epoch;
        s.g[start] = 0.0;
        s.parent[start] = -1;
        s.push(start, h(start));
        while s.heap_size > 0 {
            let cur = s.pop();
            if s.closed[cur] == epoch {
                continue;
            }
            s.closed[cur] = epoch;
            if cur == goal {
                let mut i = goal;
                while i != start {
                    s.cells.push(i);
                    i = s.parent[i] as usize;
                }
                s.cells.reverse();
                return true;
            }
            let cc = (cur % cols) as i64;
            let cr = (cur / cols) as i64;
            let g_cur = s.g[cur];
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let nc = cc + dc;
                    let nr = cr + dr;
                    if nc < 0 || nr < 0 || nc >= cols as i64 || nr >= self.rows as i64 {
                        continue;
                    }
                    let ni = nr as usize * cols + nc as usize;
                    if self.blocked[ni] != 0 || s.closed[ni] == epoch {
                        continue;
                    }
                    let diag = dr != 0 && dc != 0;
                    // Pas de coupe de coin en diagonale.
                    if diag
                        && (self.blocked[cr as usize * cols + nc as usize] != 0
                            || self.blocked[nr as usize * cols + cc as usize] != 0)
                    {
                        continue;
                    }
                    let ng = g_cur + if diag { SQRT2 } else { 1.0 };
                    if s.stamp[ni] != epoch || ng < s.g[ni] {
                        s.stamp[ni] = epoch;
                        s.g[ni] = ng;
                        s.parent[ni] = cur as i32;
                        s.push(ni, ng + h(ni));
                    }
                }
            }
        }
        false
    }
}

fn inside_poly(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_poly2(p: Vec2, poly: &[Vec2]) -> f64 {
    let mut best = f64::INFINITY;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let a = poly[j];
        let b = poly[i];
        let abx = b.x - a.x;
        let aby = b.y - a.y;
        let l2 = abx * abx + aby * aby;
        let t = if l2 == 0.0 {
            0.0
        } else {
            clamp(((p.x - a.x) * abx + (p.y - a.y) * aby) / l2, 0.0, 1.0)
        };
        let ex = a.x + abx * t - p.x;
        let ey = a.y + aby * t - p.y;
        let d = ex * ex + ey * ey;
        if d < best {
            best = d;
        }
        j = i;
    }
    best
}

static SHARED: LazyLock<NavGrid> = LazyLock::new(|| NavGrid::from_walls(&WALLS));

/// Grille partagée construite depuis `WALLS` (une seule fois par processus).
pub fn nav_grid() -> &'static NavGrid {
    &SHARED
}
