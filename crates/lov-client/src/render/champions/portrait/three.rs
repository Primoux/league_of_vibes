//! Générateurs de three.js dont dépendent les portraits, portés fidèlement (r187) : `CatmullRomCurve3`
//! centripète et son paramétrage par longueur d'arc, `TubeGeometry` (repères de Frenet), `ExtrudeGeometry`
//! d'une forme polygonale (biseau, triangulation earcut des faces). Mêmes sommets, même ordre, même
//! enroulement ; les calculs se font en `f64`, comme en JavaScript.

use bevy::math::{DMat3, DQuat, DVec2, DVec3};
use bevy::prelude::*;

use crate::render::models::geometry::Geo;

/// `Curve.arcLengthDivisions`.
const ARC_LENGTH_DIVISIONS: usize = 200;

// ---------------------------------------------------------------- CatmullRomCurve3

/// `CatmullRomCurve3(points)` : ouverte, centripète (le défaut de three.js).
#[derive(Clone, Debug)]
pub struct CatmullRom {
    points: Vec<DVec3>,
    /// `getLengths()` : longueurs cumulées sur 200 divisions.
    lengths: Vec<f64>,
}

/// Polynôme cubique d'Hermite (`CubicPoly`).
fn hermite(x0: f64, x1: f64, t0: f64, t1: f64, t: f64) -> f64 {
    let c2 = -3.0 * x0 + 3.0 * x1 - 2.0 * t0 - t1;
    let c3 = 2.0 * x0 - 2.0 * x1 + t0 + t1;
    let t2 = t * t;
    x0 + t0 * t + c2 * t2 + c3 * t2 * t
}

/// `initNonuniformCatmullRom` puis `calc(t)`.
#[allow(clippy::too_many_arguments)] // signature de three.js, gardée telle quelle
fn nonuniform(x0: f64, x1: f64, x2: f64, x3: f64, dt0: f64, dt1: f64, dt2: f64, t: f64) -> f64 {
    let t1 = ((x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1) * dt1;
    let t2 = ((x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2) * dt1;
    hermite(x1, x2, t1, t2, t)
}

impl CatmullRom {
    /// Au moins deux points.
    pub fn new(points: &[[f32; 3]]) -> Self {
        let mut c = Self {
            points: points
                .iter()
                .map(|p| Vec3::from_array(*p).as_dvec3())
                .collect(),
            lengths: Vec::new(),
        };
        let mut last = c.point(0.0);
        let mut sum = 0.0;
        c.lengths.push(0.0);
        for p in 1..=ARC_LENGTH_DIVISIONS {
            let current = c.point(p as f64 / ARC_LENGTH_DIVISIONS as f64);
            sum += current.distance(last);
            c.lengths.push(sum);
            last = current;
        }
        c
    }

    /// `getPoint(t)`.
    pub fn point(&self, t: f64) -> DVec3 {
        let pts = &self.points;
        let l = pts.len();
        let p = (l - 1) as f64 * t;
        let mut int = p.floor() as usize;
        let mut weight = p - int as f64;
        if weight == 0.0 && int == l - 1 {
            int = l - 2;
            weight = 1.0;
        }
        let p0 = if int > 0 {
            pts[int - 1]
        } else {
            pts[0] - pts[1] + pts[0]
        };
        let p1 = pts[int % l];
        let p2 = pts[(int + 1) % l];
        let p3 = if int + 2 < l {
            pts[int + 2]
        } else {
            pts[l - 1] - pts[l - 2] + pts[l - 1]
        };
        let mut dt0 = p0.distance_squared(p1).powf(0.25);
        let mut dt1 = p1.distance_squared(p2).powf(0.25);
        let mut dt2 = p2.distance_squared(p3).powf(0.25);
        if dt1 < 1e-4 {
            dt1 = 1.0;
        }
        if dt0 < 1e-4 {
            dt0 = dt1;
        }
        if dt2 < 1e-4 {
            dt2 = dt1;
        }
        DVec3::new(
            nonuniform(p0.x, p1.x, p2.x, p3.x, dt0, dt1, dt2, weight),
            nonuniform(p0.y, p1.y, p2.y, p3.y, dt0, dt1, dt2, weight),
            nonuniform(p0.z, p1.z, p2.z, p3.z, dt0, dt1, dt2, weight),
        )
    }

    /// `getUtoTmapping(u)` : paramètre `t` à la fraction `u` de la longueur.
    fn u_to_t(&self, u: f64) -> f64 {
        let arc = &self.lengths;
        let il = arc.len();
        let target = u * arc[il - 1];
        let (mut low, mut high) = (0_i64, il as i64 - 1);
        while low <= high {
            let i = low + (high - low) / 2;
            let comparison = arc[i as usize] - target;
            if comparison < 0.0 {
                low = i + 1;
            } else if comparison > 0.0 {
                high = i - 1;
            } else {
                high = i;
                break;
            }
        }
        let i = high.max(0) as usize;
        if arc[i] == target {
            return i as f64 / (il - 1) as f64;
        }
        let before = arc[i];
        let after = arc[i + 1];
        (i as f64 + (target - before) / (after - before)) / (il - 1) as f64
    }

    /// `getPointAt(u)`.
    pub fn point_at(&self, u: f64) -> DVec3 {
        self.point(self.u_to_t(u))
    }

    /// `getTangentAt(u)`.
    fn tangent_at(&self, u: f64) -> DVec3 {
        let t = self.u_to_t(u);
        let a = self.point((t - 0.0001).max(0.0));
        let b = self.point((t + 0.0001).min(1.0));
        (b - a).normalize_or_zero()
    }

    /// `computeFrenetFrames(segments, false)` : normales et binormales.
    fn frenet_frames(&self, segments: u32) -> (Vec<DVec3>, Vec<DVec3>) {
        let tangents: Vec<DVec3> = (0..=segments)
            .map(|i| self.tangent_at(f64::from(i) / f64::from(segments)))
            .collect();
        let t0 = tangents[0];
        let mut min = f64::MAX;
        let mut normal = DVec3::ZERO;
        let (tx, ty, tz) = (t0.x.abs(), t0.y.abs(), t0.z.abs());
        if tx <= min {
            min = tx;
            normal = DVec3::X;
        }
        if ty <= min {
            min = ty;
            normal = DVec3::Y;
        }
        if tz <= min {
            normal = DVec3::Z;
        }
        let vec = t0.cross(normal).normalize_or_zero();
        let mut normals = vec![t0.cross(vec)];
        let mut binormals = vec![t0.cross(normals[0])];
        for i in 1..=segments as usize {
            let mut n = normals[i - 1];
            let axis = tangents[i - 1].cross(tangents[i]);
            if axis.length() > f64::EPSILON {
                let theta = tangents[i - 1].dot(tangents[i]).clamp(-1.0, 1.0).acos();
                n = DMat3::from_quat(DQuat::from_axis_angle(axis.normalize(), theta)) * n;
            }
            binormals.push(tangents[i].cross(n));
            normals.push(n);
        }
        (normals, binormals)
    }
}

/// `TubeGeometry(curve, tubularSegments, radius, radialSegments, false)` : indexée.
pub fn tube(curve: &CatmullRom, tubular: u32, radius: f32, radial: u32) -> Geo {
    let (normals, binormals) = curve.frenet_frames(tubular);
    let r = f64::from(radius);
    let mut positions = Vec::with_capacity(((tubular + 1) * (radial + 1)) as usize);
    for i in 0..=tubular {
        let p = curve.point_at(f64::from(i) / f64::from(tubular));
        let (n, b) = (normals[i as usize], binormals[i as usize]);
        for j in 0..=radial {
            let v = f64::from(j) / f64::from(radial) * std::f64::consts::TAU;
            let (sin, cos) = (v.sin(), -v.cos());
            let normal = (n * cos + b * sin).normalize_or_zero();
            positions.push((p + normal * r).as_vec3());
        }
    }
    let mut indices = Vec::with_capacity((tubular * radial * 6) as usize);
    for j in 1..=tubular {
        for i in 1..=radial {
            let a = (radial + 1) * (j - 1) + (i - 1);
            let b = (radial + 1) * j + (i - 1);
            let c = (radial + 1) * j + i;
            let d = (radial + 1) * (j - 1) + i;
            indices.extend([a, b, d, b, c, d]);
        }
    }
    Geo {
        positions,
        indices: Some(indices),
    }
}

// ---------------------------------------------------------------- earcut

/// Sommet de la liste chaînée d'earcut.
#[derive(Clone, Copy, Debug)]
struct Node {
    i: usize,
    x: f64,
    y: f64,
    prev: usize,
    next: usize,
}

/// `earcut(data)` (mapbox/earcut 3.0.2, polygone sans trou, moins de 80 sommets : pas de hachage z).
/// Les passes 0 et 1 (sommets colinéaires filtrés) sont portées ; les passes de secours (auto-intersections,
/// découpe) ne servent qu'aux polygones croisés, que les portraits n'emploient pas : on finit alors en
/// éventail.
pub fn earcut(pts: &[DVec2]) -> Vec<usize> {
    let mut nodes: Vec<Node> = Vec::with_capacity(pts.len());
    let mut tris = Vec::new();
    let Some(start) = linked_list(&mut nodes, pts) else {
        return tris;
    };
    if nodes[start].next == nodes[start].prev {
        return tris;
    }
    earcut_linked(&mut nodes, start, &mut tris, 0);
    tris
}

fn signed_area(pts: &[DVec2]) -> f64 {
    let mut sum = 0.0;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        sum += (pts[j].x - pts[i].x) * (pts[i].y + pts[j].y);
        j = i;
    }
    sum
}

fn insert_node(nodes: &mut Vec<Node>, i: usize, p: DVec2, last: Option<usize>) -> usize {
    let id = nodes.len();
    let mut n = Node {
        i,
        x: p.x,
        y: p.y,
        prev: id,
        next: id,
    };
    if let Some(last) = last {
        n.next = nodes[last].next;
        n.prev = last;
        let after = nodes[last].next;
        nodes[after].prev = id;
        nodes[last].next = id;
    }
    nodes.push(n);
    id
}

fn remove_node(nodes: &mut [Node], p: usize) {
    let (prev, next) = (nodes[p].prev, nodes[p].next);
    nodes[next].prev = prev;
    nodes[prev].next = next;
}

fn equals(nodes: &[Node], a: usize, b: usize) -> bool {
    nodes[a].x == nodes[b].x && nodes[a].y == nodes[b].y
}

fn area(nodes: &[Node], p: usize, q: usize, r: usize) -> f64 {
    let (p, q, r) = (nodes[p], nodes[q], nodes[r]);
    (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y)
}

/// Liste chaînée circulaire dans le sens horaire (`linkedList(…, clockwise = true)`).
fn linked_list(nodes: &mut Vec<Node>, pts: &[DVec2]) -> Option<usize> {
    let mut last = None;
    if signed_area(pts) > 0.0 {
        for (i, p) in pts.iter().enumerate() {
            last = Some(insert_node(nodes, i, *p, last));
        }
    } else {
        for (i, p) in pts.iter().enumerate().rev() {
            last = Some(insert_node(nodes, i, *p, last));
        }
    }
    if let Some(l) = last
        && equals(nodes, l, nodes[l].next)
    {
        let next = nodes[l].next;
        remove_node(nodes, l);
        last = Some(next);
    }
    last
}

/// `filterPoints` : retire les doublons et les sommets colinéaires.
fn filter_points(nodes: &mut [Node], start: usize) -> usize {
    let mut end = start;
    let mut p = start;
    loop {
        let mut again = false;
        let (prev, next) = (nodes[p].prev, nodes[p].next);
        if equals(nodes, p, next) || area(nodes, prev, p, next) == 0.0 {
            remove_node(nodes, p);
            p = prev;
            end = prev;
            if p == nodes[p].next {
                break;
            }
            again = true;
        } else {
            p = next;
        }
        if !again && p == end {
            break;
        }
    }
    end
}

fn point_in_triangle(a: DVec2, b: DVec2, c: DVec2, p: DVec2) -> bool {
    (c.x - p.x) * (a.y - p.y) >= (a.x - p.x) * (c.y - p.y)
        && (a.x - p.x) * (b.y - p.y) >= (b.x - p.x) * (a.y - p.y)
        && (b.x - p.x) * (c.y - p.y) >= (c.x - p.x) * (b.y - p.y)
}

fn is_ear(nodes: &[Node], ear: usize) -> bool {
    let (a, b, c) = (nodes[ear].prev, ear, nodes[ear].next);
    if area(nodes, a, b, c) >= 0.0 {
        return false; // angle rentrant
    }
    let v = |n: usize| DVec2::new(nodes[n].x, nodes[n].y);
    let (pa, pb, pc) = (v(a), v(b), v(c));
    let min = pa.min(pb).min(pc);
    let max = pa.max(pb).max(pc);
    let mut p = nodes[c].next;
    while p != a {
        let pp = v(p);
        if pp.x >= min.x
            && pp.x <= max.x
            && pp.y >= min.y
            && pp.y <= max.y
            && pp != pa
            && point_in_triangle(pa, pb, pc, pp)
            && area(nodes, nodes[p].prev, p, nodes[p].next) >= 0.0
        {
            return false;
        }
        p = nodes[p].next;
    }
    true
}

fn earcut_linked(nodes: &mut [Node], mut ear: usize, tris: &mut Vec<usize>, pass: u8) {
    let mut stop = ear;
    while nodes[ear].prev != nodes[ear].next {
        let (prev, next) = (nodes[ear].prev, nodes[ear].next);
        if is_ear(nodes, ear) {
            tris.extend([nodes[prev].i, nodes[ear].i, nodes[next].i]);
            remove_node(nodes, ear);
            // sauter le sommet suivant donne moins de triangles effilés
            ear = nodes[next].next;
            stop = ear;
            continue;
        }
        ear = next;
        if ear == stop {
            if pass == 0 {
                let start = filter_points(nodes, ear);
                earcut_linked(nodes, start, tris, 1);
            } else {
                // polygone croisé : éventail sur ce qui reste
                let first = ear;
                let mut p = nodes[first].next;
                while nodes[p].next != first {
                    tris.extend([nodes[first].i, nodes[p].i, nodes[nodes[p].next].i]);
                    p = nodes[p].next;
                }
            }
            break;
        }
    }
}

// ---------------------------------------------------------------- ExtrudeGeometry

/// Options d'`ExtrudeGeometry` employées par les portraits (pas de trou, pas de chemin).
#[derive(Clone, Copy, Debug)]
pub struct Extrude {
    pub depth: f32,
    pub bevel_enabled: bool,
    pub bevel_thickness: f32,
    pub bevel_size: f32,
    pub bevel_segments: u32,
    pub steps: u32,
}

/// Contour d'une `Shape` faite de `moveTo` puis `lineTo` et `closePath` (`extractPoints`) : doublons
/// consécutifs ignorés, retour au départ.
fn shape_points(points: &[[f32; 2]]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::with_capacity(points.len() + 1);
    for p in points {
        let p = Vec2::from_array(*p).as_dvec2();
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    if let Some(first) = out.first().copied()
        && out.last() != Some(&first)
    {
        out.push(first);
    }
    out
}

/// `ShapeUtils.area`.
fn shape_area(c: &[DVec2]) -> f64 {
    let n = c.len();
    let mut a = 0.0;
    let mut p = n - 1;
    for q in 0..n {
        a += c[p].x * c[q].y - c[q].x * c[p].y;
        p = q;
    }
    a * 0.5
}

/// `mergeOverlappingPoints` : retire les voisins confondus (dont le doublon de fermeture).
fn merge_overlapping(points: &mut Vec<DVec2>) {
    const THRESHOLD_SQ: f64 = 1e-10 * 1e-10;
    let Some(mut prev) = points.first().copied() else {
        return;
    };
    let mut i = 1;
    while i <= points.len() {
        let index = i % points.len();
        let cur = points[index];
        let d = cur - prev;
        let scale = cur
            .x
            .abs()
            .max(cur.y.abs())
            .max(prev.x.abs())
            .max(prev.y.abs());
        if d.length_squared() <= THRESHOLD_SQ * scale * scale {
            points.remove(index);
            continue;
        }
        prev = cur;
        i += 1;
    }
}

/// `getBevelVec` : direction de déplacement d'un sommet pour décaler le contour d'une unité.
fn bevel_vec(pt: DVec2, prev: DVec2, next: DVec2) -> DVec2 {
    let vp = pt - prev;
    let vn = next - pt;
    let vp_lensq = vp.length_squared();
    let collinear = vp.x * vn.y - vp.y * vn.x;
    let (tx, ty, shrink);
    if collinear.abs() > f64::EPSILON {
        let vp_len = vp_lensq.sqrt();
        let vn_len = vn.length();
        let ps = DVec2::new(prev.x - vp.y / vp_len, prev.y + vp.x / vp_len);
        let ns = DVec2::new(next.x - vn.y / vn_len, next.y + vn.x / vn_len);
        let sf = ((ns.x - ps.x) * vn.y - (ns.y - ps.y) * vn.x) / (vp.x * vn.y - vp.y * vn.x);
        let t = DVec2::new(ps.x + vp.x * sf - pt.x, ps.y + vp.y * sf - pt.y);
        let lensq = t.length_squared();
        if lensq <= 2.0 {
            return t;
        }
        (tx, ty, shrink) = (t.x, t.y, (lensq / 2.0).sqrt());
    } else {
        let eps = f64::EPSILON;
        let same = if vp.x > eps {
            vn.x > eps
        } else if vp.x < -eps {
            vn.x < -eps
        } else {
            js_sign(vp.y) == js_sign(vn.y)
        };
        if same {
            (tx, ty, shrink) = (-vp.y, vp.x, vp_lensq.sqrt());
        } else {
            (tx, ty, shrink) = (vp.x, vp.y, (vp_lensq / 2.0).sqrt());
        }
    }
    DVec2::new(tx / shrink, ty / shrink)
}

/// `Math.sign` (0 garde son signe, ce qui ne change rien à la comparaison).
fn js_sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `ExtrudeGeometry(shape, options)` d'un polygone du plan xy, extrudé le long de +z : non indexée.
pub fn extrude(points: &[[f32; 2]], o: Extrude) -> Geo {
    let (segments, thickness, size) = if o.bevel_enabled {
        (
            o.bevel_segments,
            f64::from(o.bevel_thickness),
            f64::from(o.bevel_size),
        )
    } else {
        (0, 0.0, 0.0)
    };
    let depth = f64::from(o.depth);
    let steps = o.steps;
    let mut contour = shape_points(points);
    if shape_area(&contour) >= 0.0 {
        contour.reverse(); // sens horaire
    }
    merge_overlapping(&mut contour);
    let vlen = contour.len();
    let moves: Vec<DVec2> = (0..vlen)
        .map(|i| {
            let j = (i + vlen - 1) % vlen;
            let k = (i + 1) % vlen;
            bevel_vec(contour[i], contour[j], contour[k])
        })
        .collect();
    let scaled = |bs: f64| -> Vec<DVec2> {
        contour
            .iter()
            .zip(&moves)
            .map(|(p, m)| *p + *m * bs)
            .collect()
    };
    let mut layers: Vec<DVec3> = Vec::new();
    let mut push = |pts: &[DVec2], z: f64| layers.extend(pts.iter().map(|p| p.extend(z)));
    // biseau arrière : de la face (t = 0) vers le flanc
    let faces = if segments == 0 {
        earcut(&contour)
    } else {
        let mut contracted = Vec::new();
        for b in 0..segments {
            let t = f64::from(b) / f64::from(segments);
            let z = thickness * (t * std::f64::consts::FRAC_PI_2).cos();
            let bs = size * (t * std::f64::consts::FRAC_PI_2).sin();
            let pts = scaled(bs);
            push(&pts, -z);
            if b == 0 {
                contracted = pts;
            }
        }
        earcut(&contracted)
    };
    let side = if o.bevel_enabled {
        scaled(size)
    } else {
        contour.clone()
    };
    for s in 0..=steps {
        push(&side, depth / f64::from(steps) * f64::from(s));
    }
    // biseau avant
    for b in (0..segments).rev() {
        let t = f64::from(b) / f64::from(segments);
        let z = thickness * (t * std::f64::consts::FRAC_PI_2).cos();
        let bs = size * (t * std::f64::consts::FRAC_PI_2).sin();
        push(&scaled(bs), depth + z);
    }
    let mut out: Vec<Vec3> = Vec::new();
    let mut f3 = |a: usize, b: usize, c: usize| {
        out.extend([a, b, c].map(|i| layers[i].as_vec3()));
    };
    // faces arrière puis avant
    let top = vlen * (steps + segments * 2) as usize;
    for f in faces.as_chunks::<3>().0 {
        f3(f[2], f[1], f[0]);
    }
    for f in faces.as_chunks::<3>().0 {
        f3(f[0] + top, f[1] + top, f[2] + top);
    }
    // flancs
    for i in (0..vlen).rev() {
        let k = if i == 0 { vlen - 1 } else { i - 1 };
        for s in 0..(steps + segments * 2) as usize {
            let (l1, l2) = (vlen * s, vlen * (s + 1));
            let (a, b, c, d) = (i + l1, k + l1, k + l2, i + l2);
            f3(a, b, d);
            f3(b, c, d);
        }
    }
    Geo {
        positions: out,
        indices: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courbe_par_ses_points_et_a_vitesse_constante() {
        let c = CatmullRom::new(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 2.0, 0.0]]);
        assert!(c.point(0.0).distance(DVec3::ZERO) < 1e-12);
        assert!(c.point(0.5).distance(DVec3::X) < 1e-12);
        assert!(c.point(1.0).distance(DVec3::new(1.0, 2.0, 0.0)) < 1e-12);
        // `getPointAt` : pas d'arc égaux
        let total = c.lengths[ARC_LENGTH_DIVISIONS];
        let quarter = (0..=100)
            .map(|i| c.point_at(f64::from(i) / 400.0))
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| w[0].distance(w[1]))
            .sum::<f64>();
        assert!((quarter - total / 4.0).abs() < 1e-3, "{quarter} / {total}");
    }

    #[test]
    fn tube_comme_three() {
        let c = CatmullRom::new(&[[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]]);
        let g = tube(&c, 4, 0.5, 6);
        assert_eq!(g.positions.len(), 5 * 7);
        assert_eq!(g.triangles(), 4 * 6 * 2);
        // tube droit le long de +y : chaque anneau à 0,5 de l'axe, à sa hauteur
        for (i, p) in g.positions.iter().enumerate() {
            let y = (i / 7) as f32 * 0.5;
            assert!((p.y - y).abs() < 1e-5 && (p.xz().length() - 0.5).abs() < 1e-5);
        }
        // normales sortantes (enroulement de three.js)
        let v = g.triangle_vertices();
        for t in v.as_chunks::<3>().0 {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let mid = (t[0] + t[1] + t[2]) / 3.0;
            assert!(n.dot(Vec3::new(mid.x, 0.0, mid.z)) > 0.0);
        }
    }

    #[test]
    fn earcut_d_un_polygone_concave() {
        // flèche concave : 5 sommets, 3 triangles, aire conservée
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2.0, 1.0),
            DVec2::new(0.0, 2.0),
            DVec2::new(0.5, 1.0),
        ];
        let t = earcut(&pts);
        assert_eq!(t.len(), 6);
        let area: f64 = t
            .chunks(3)
            .map(|f| {
                let (a, b, c) = (pts[f[0]], pts[f[1]], pts[f[2]]);
                ((b - a).perp_dot(c - a) / 2.0).abs()
            })
            .sum();
        assert!((area - shape_area(&pts).abs()).abs() < 1e-12);
        // carré : la première oreille part du dernier sommet inséré, comme earcut
        let square = [
            DVec2::new(0.0, 0.0),
            DVec2::new(1.0, 0.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(0.0, 1.0),
        ];
        assert_eq!(earcut(&square), vec![2, 3, 0, 0, 1, 2]);
    }

    #[test]
    fn extrusion_biseautee_comme_three() {
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let flat = extrude(
            &square,
            Extrude {
                depth: 1.0,
                bevel_enabled: false,
                bevel_thickness: 0.0,
                bevel_size: 0.0,
                bevel_segments: 0,
                steps: 1,
            },
        );
        // 2 × 2 triangles de faces, 4 flancs × 2
        assert_eq!(flat.triangles(), 4 + 8);
        let bevel = extrude(
            &square,
            Extrude {
                depth: 0.5,
                bevel_enabled: true,
                bevel_thickness: 0.1,
                bevel_size: 0.1,
                bevel_segments: 2,
                steps: 1,
            },
        );
        // flancs : 4 côtés × (1 pas + 2 × 2 segments de biseau) × 2
        assert_eq!(bevel.triangles(), 4 + 4 * 5 * 2);
        let z = bevel.positions.iter().map(|p| p.z);
        let (lo, hi) = z.fold((f32::MAX, f32::MIN), |(a, b), z| (a.min(z), b.max(z)));
        assert!((lo + 0.1).abs() < 1e-6 && (hi - 0.6).abs() < 1e-6);
        // le flanc déborde du contour de la taille du biseau, les faces restent dedans
        let x = bevel.positions.iter().map(|p| p.x);
        let (lo, hi) = x.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
        assert!((lo + 0.1).abs() < 1e-6 && (hi - 1.1).abs() < 1e-6);
        // toutes les normales sortent du solide (centre en 0,5 ; 0,5 ; 0,25)
        let center = Vec3::new(0.5, 0.5, 0.25);
        let v = bevel.triangle_vertices();
        for t in v.as_chunks::<3>().0 {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            assert!(n.dot((t[0] + t[1] + t[2]) / 3.0 - center) > 0.0);
        }
    }
}
