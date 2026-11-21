//! Géométrie originale des modèles inspirés des portraits : volumes, mèches, étoffes et ornements.
//!
//! Portage de `packages/client/src/render/champions/portrait/geometry.ts`.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use super::three::{CatmullRom, Extrude, extrude, tube};
use crate::render::champions::builder::{ModelBuilder, PartOpts, css};
use crate::render::models::geometry::{self as g3, Geo};

pub type Point = [f32; 3];

/// Anneau d'un `loft` : hauteur, profondeur (x), largeur (z), décalage en x.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub y: f32,
    pub depth: f32,
    pub width: f32,
    pub x: f32,
}

impl Ring {
    /// `[y, depth, width, x?]` du client web.
    pub const fn new(v: &[f32]) -> Self {
        Self {
            y: v[0],
            depth: v[1],
            width: v[2],
            x: if v.len() > 3 { v[3] } else { 0.0 },
        }
    }
}

/// Anneaux d'un `loft`, écrits comme dans le client web : `rings![[y, d, w], [y, d, w, x]]`.
#[macro_export]
macro_rules! rings {
    ($([$($v:expr),+ $(,)?]),* $(,)?) => {
        &[$($crate::render::champions::portrait::geometry::Ring::new(&[$($v),+])),*]
    };
}

/// Couleur d'une pièce : chaîne CSS (`#rrggbb`, sRGB) ou couleur linéaire (`THREE.Color`).
pub trait Paint {
    fn linear(self) -> LinearRgba;
}

impl Paint for &str {
    fn linear(self) -> LinearRgba {
        css(self)
    }
}

impl Paint for LinearRgba {
    fn linear(self) -> LinearRgba {
        self
    }
}

/// `new THREE.Color(c).multiplyScalar(k)`.
pub fn shade(c: impl Paint, k: f32) -> LinearRgba {
    let c = c.linear();
    LinearRgba::rgb(c.red * k, c.green * k, c.blue * k)
}

/// Surface de révolution aplatie, anneau par anneau (couvercles aux deux bouts si `caps`) : indexée.
pub fn loft(rings: &[Ring], segments: u32, caps: bool) -> Geo {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for r in rings {
        for i in 0..segments {
            let a = i as f32 / segments as f32 * PI * 2.0;
            positions.push(Vec3::new(r.x + a.cos() * r.depth, r.y, a.sin() * r.width));
        }
    }
    let n = rings.len() as u32;
    for j in 0..n - 1 {
        for i in 0..segments {
            let a = j * segments + i;
            let b = a + segments;
            let next = j * segments + (i + 1) % segments;
            indices.extend([a, b, next, b, next + segments, next]);
        }
    }
    if caps {
        for top in [false, true] {
            let row = if top { n - 1 } else { 0 };
            let ring = rings[row as usize];
            let center = positions.len() as u32;
            positions.push(Vec3::new(ring.x, ring.y, 0.0));
            for i in 0..segments {
                let a = row * segments + i;
                let next = row * segments + (i + 1) % segments;
                if top {
                    indices.extend([center, next, a]);
                } else {
                    indices.extend([center, a, next]);
                }
            }
        }
    }
    Geo {
        positions,
        indices: Some(indices),
    }
}

/// Mèche : tube le long d'une Catmull-Rom, rayon interpolé entre `radii` (au moins deux valeurs).
/// La couture du `TubeGeometry` est lissée par le builder (sommets confondus, normale commune).
pub fn strand(points: &[Point], radii: &[f32], sides: u32, steps: u32) -> Geo {
    let curve = CatmullRom::new(points);
    let mut g = tube(&curve, steps, 1.0, sides);
    let last = radii.len() - 2;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let center = curve.point_at(f64::from(t)).as_vec3();
        let f = t * (radii.len() - 1) as f32;
        let k = last.min(f.floor() as usize);
        let radius = (radii[k] + (radii[k + 1] - radii[k]) * (f - k as f32)).max(0.002);
        for j in 0..=sides {
            let p = &mut g.positions[(i * (sides + 1) + j) as usize];
            *p = center + (*p - center) * radius;
        }
    }
    g
}

/// Polygone sculpté dans le plan yz, face vers +x : `points` en (z, y), extrudé et biseauté.
pub fn plate(points: &[[f32; 2]], depth: f32, bevel: f32) -> Geo {
    let shape: Vec<[f32; 2]> = points.iter().map(|[z, y]| [-z, *y]).collect();
    extrude(
        &shape,
        Extrude {
            depth,
            bevel_enabled: bevel > 0.0,
            bevel_thickness: bevel,
            bevel_size: bevel,
            bevel_segments: 2,
            steps: 1,
        },
    )
    .rotate(Quat::from_rotation_y(FRAC_PI_2))
    .translate(-depth / 2.0, 0.0, 0.0)
}

/// Cape épaisse à ourlet ondulé ; chaque couche possède ses normales propres.
pub fn cloth(height: f32, width: f32, top_width: f32, back: f32, wave: f32) -> Geo {
    const NX: u32 = 14;
    const NY: u32 = 8;
    let layer = (NX + 1) * (NY + 1);
    let mut positions = Vec::with_capacity(2 * layer as usize);
    let mut indices = Vec::new();
    for side in 0..2 {
        for j in 0..=NY {
            for i in 0..=NX {
                let t = j as f32 / NY as f32;
                let u = i as f32 / NX as f32 * 2.0 - 1.0;
                let half = top_width + (width - top_width) * t;
                positions.push(Vec3::new(
                    back - 0.12 * t - 0.08 * (1.0 - u * u)
                        + (u * PI * 4.0).sin() * wave * t
                        + side as f32 * 0.014,
                    0.48 - height * t + (u * PI * 5.0).cos() * 0.025 * t * t,
                    u * half,
                ));
            }
        }
    }
    for side in 0..2 {
        for j in 0..NY {
            for i in 0..NX {
                let a = side * layer + j * (NX + 1) + i;
                let b = a + NX + 1;
                if side == 0 {
                    indices.extend([a, b, a + 1, b, b + 1, a + 1]);
                } else {
                    indices.extend([a, a + 1, b, b, a + 1, b + 1]);
                }
            }
        }
    }
    Geo {
        positions,
        indices: Some(indices),
    }
}

/// `cloth(height, width, topWidth)` avec ses valeurs par défaut.
pub fn cape(height: f32, width: f32, top_width: f32) -> Geo {
    cloth(height, width, top_width, -0.18, 0.025)
}

/// Normale d'un anneau décoratif : le tore regarde +x (face) ou +y (bague horizontale).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    X,
    Y,
}

/// Outils de sculpture des portraits ; le builder passe en surfaces sculptées.
pub struct Sculpt<'a> {
    pub b: &'a mut ModelBuilder,
}

impl<'a> Sculpt<'a> {
    pub fn new(b: &'a mut ModelBuilder) -> Self {
        b.sculpted_surfaces();
        Self { b }
    }

    /// Ellipsoïde lisse (`detail = 10`, non métallique).
    pub fn ell(&mut self, j: usize, color: impl Paint, pos: Point, size: Point) {
        self.ell_with(j, color, pos, size, 10, false);
    }

    pub fn ell_with(
        &mut self,
        j: usize,
        color: impl Paint,
        pos: Point,
        size: Point,
        detail: u32,
        metal: bool,
    ) {
        let g = g3::sphere(1.0, detail, 6.max(detail * 13 / 20));
        let o = PartOpts::at(pos[0], pos[1], pos[2])
            .scale(size[0], size[1], size[2])
            .smooth();
        self.part(j, &g, color, o, metal);
    }

    pub fn part(&mut self, j: usize, g: &Geo, color: impl Paint, o: PartOpts, metal: bool) {
        if metal {
            self.b.metal(j, g, color.linear(), o);
        } else {
            self.b.add(j, g, color.linear(), o);
        }
    }

    /// Mèche lisse à 6 pans (non métallique).
    pub fn tube(&mut self, j: usize, color: impl Paint, points: &[Point], radii: &[f32]) {
        self.tube_with(j, color, points, radii, false, 6);
    }

    pub fn tube_with(
        &mut self,
        j: usize,
        color: impl Paint,
        points: &[Point],
        radii: &[f32],
        metal: bool,
        sides: u32,
    ) {
        let steps = if points.len() >= 4 { 10 } else { 8 };
        let g = strand(points, radii, sides, steps);
        self.part(j, &g, color, PartOpts::default().smooth(), metal);
    }

    /// Plaque de 0,025 d'épaisseur (non métallique).
    pub fn panel(&mut self, j: usize, color: impl Paint, points: &[[f32; 2]], pos: Point) {
        self.panel_with(j, color, points, pos, false, 0.025);
    }

    pub fn panel_with(
        &mut self,
        j: usize,
        color: impl Paint,
        points: &[[f32; 2]],
        pos: Point,
        metal: bool,
        depth: f32,
    ) {
        let g = plate(points, depth, 0.008);
        self.part(j, &g, color, PartOpts::at(pos[0], pos[1], pos[2]), metal);
    }

    /// Volume lissé (non métallique).
    pub fn form(&mut self, j: usize, color: impl Paint, rings: &[Ring]) {
        self.form_with(j, color, rings, false, PartOpts::default());
    }

    pub fn form_with(
        &mut self,
        j: usize,
        color: impl Paint,
        rings: &[Ring],
        metal: bool,
        o: PartOpts,
    ) {
        self.part(j, &loft(rings, 12, true), color, o.smooth(), metal);
    }

    /// Anneau face à +x (non métallique).
    pub fn ring(&mut self, j: usize, color: impl Paint, radius: f32, tube: f32, pos: Point) {
        self.ring_with(j, color, radius, tube, pos, Facing::X, false);
    }

    #[allow(clippy::too_many_arguments)] // signature de `Sculpt.ring` du client web, gardée telle quelle
    pub fn ring_with(
        &mut self,
        j: usize,
        color: impl Paint,
        radius: f32,
        tube: f32,
        pos: Point,
        normal: Facing,
        metal: bool,
    ) {
        let o = PartOpts::at(pos[0], pos[1], pos[2]);
        let o = match normal {
            Facing::X => o.rot(0.0, FRAC_PI_2, 0.0),
            Facing::Y => o.rot(FRAC_PI_2, 0.0, 0.0),
        };
        self.part(j, &g3::torus(radius, tube, 5, 18), color, o.smooth(), metal);
    }

    /// Gemme lumineuse (octaèdre non éclairé).
    pub fn gem(&mut self, j: usize, color: LinearRgba, pos: Point, scale: Point) {
        let o = PartOpts::at(pos[0], pos[1], pos[2]).scale(scale[0], scale[1], scale[2]);
        self.b.glow(j, &g3::octahedron(1.0, 0), color, o);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loft_ferme_et_oriente_vers_l_exterieur() {
        let g = loft(rings![[0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.5]], 8, true);
        assert_eq!(g.positions.len(), 2 * 8 + 2);
        assert_eq!(g.triangles(), 8 * 2 + 8 * 2);
        // décalage en x du second anneau et de son couvercle
        assert!((g.positions[8].x - 1.5).abs() < 1e-6);
        assert_eq!(g.positions[17], Vec3::new(0.5, 1.0, 0.0));
        let v = g.triangle_vertices();
        let center = Vec3::new(0.25, 0.5, 0.0);
        for t in v.as_chunks::<3>().0 {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            assert!(n.dot((t[0] + t[1] + t[2]) / 3.0 - center) > 0.0);
        }
        // sans couvercles : tube ouvert
        assert_eq!(
            loft(rings![[0.0, 1.0, 1.0], [1.0, 1.0, 1.0]], 8, false).triangles(),
            16
        );
    }

    #[test]
    fn meche_effilee_selon_ses_rayons() {
        let pts = [[0.0, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 1.0, 0.0]];
        let g = strand(&pts, &[0.1, 0.05, 0.0], 6, 8);
        assert_eq!(g.positions.len(), 9 * 7);
        let radius = |ring: usize| g.positions[ring * 7].xz().length();
        assert!((radius(0) - 0.1).abs() < 1e-5);
        assert!((radius(4) - 0.05).abs() < 1e-5);
        assert!((radius(8) - 0.002).abs() < 1e-5, "rayon minimal 0,002");
    }

    #[test]
    fn plaque_face_a_x_centree_en_profondeur() {
        let g = plate(
            &[[-0.1, 0.1], [0.1, 0.1], [0.1, -0.1], [-0.1, -0.1]],
            0.02,
            0.0,
        );
        let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
        for p in &g.positions {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        assert!((lo.x + 0.01).abs() < 1e-6 && (hi.x - 0.01).abs() < 1e-6);
        assert!((lo.z + 0.1).abs() < 1e-6 && (hi.y - 0.1).abs() < 1e-6);
        // l'ordre (z, y) du client web : un point en z positif reste en z positif
        let g = plate(&[[0.0, 0.0], [0.2, 0.0], [0.2, 0.1]], 0.02, 0.0);
        assert!(g.positions.iter().all(|p| p.z >= -1e-6));
    }

    #[test]
    fn cape_a_deux_couches_opposees() {
        let g = cape(0.9, 0.4, 0.25);
        assert_eq!(g.positions.len(), 2 * 15 * 9);
        assert_eq!(g.triangles(), 2 * 14 * 8 * 2);
        let v = g.triangle_vertices();
        let n = |k: usize| {
            let t = &v[k * 3..k * 3 + 3];
            (t[1] - t[0]).cross(t[2] - t[0]).x
        };
        // la couche intérieure regarde vers l'avant, l'extérieure vers l'arrière
        assert!(n(0) * n(14 * 8 * 2) < 0.0);
    }
}
