//! Géométries procédurales, portage fidèle des générateurs de three.js (`BoxGeometry`, `SphereGeometry`,
//! `CylinderGeometry`, `ConeGeometry`, polyèdres, `TorusGeometry`) : mêmes sommets, même ordre, même
//! enroulement. Les modèles low-poly du client web reposent sur le nombre exact de segments (une sphère à
//! 6 × 5 facettes, un cône à 4 pans) et sur la position de chaque sommet (`jitter` des champions) : les
//! primitives de Bevy, découpées autrement, changeraient la silhouette.
//!
//! Une `Geo` garde ses sommets indexés, comme une `BufferGeometry` ; `to_flat_mesh` la convertit en
//! maillage à facettes (sommets dupliqués, normale par triangle), l'équivalent de `flatShading: true`.

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

/// Géométrie indexée (positions seules : le rendu à facettes recalcule les normales).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geo {
    pub positions: Vec<Vec3>,
    /// Triangles ; `None` = non indexée (trois sommets consécutifs par triangle).
    pub indices: Option<Vec<u32>>,
}

impl Geo {
    /// Nombre de triangles.
    #[cfg(test)]
    pub fn triangles(&self) -> usize {
        match &self.indices {
            Some(i) => i.len() / 3,
            None => self.positions.len() / 3,
        }
    }

    /// Sommets de chaque triangle, dans l'ordre.
    pub fn triangle_vertices(&self) -> Vec<Vec3> {
        match &self.indices {
            Some(idx) => idx.iter().map(|&i| self.positions[i as usize]).collect(),
            None => self.positions.clone(),
        }
    }

    pub fn translate(mut self, x: f32, y: f32, z: f32) -> Self {
        let d = Vec3::new(x, y, z);
        self.positions.iter_mut().for_each(|p| *p += d);
        self
    }

    pub fn scale(mut self, x: f32, y: f32, z: f32) -> Self {
        let s = Vec3::new(x, y, z);
        self.positions.iter_mut().for_each(|p| *p *= s);
        self
    }

    pub fn rotate_x(self, a: f32) -> Self {
        self.rotate(Quat::from_rotation_x(a))
    }

    pub fn rotate_z(self, a: f32) -> Self {
        self.rotate(Quat::from_rotation_z(a))
    }

    pub fn rotate(mut self, q: Quat) -> Self {
        self.positions.iter_mut().for_each(|p| *p = q * *p);
        self
    }

    /// Maillage à facettes : chaque triangle a ses propres sommets et sa normale géométrique.
    pub fn to_flat_mesh(&self) -> Mesh {
        let verts = self.triangle_vertices();
        let mut normals = Vec::with_capacity(verts.len());
        for t in verts.as_chunks::<3>().0 {
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            normals.extend([n; 3]);
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, verts)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    }
}

/// `BoxGeometry(w, h, d)`.
pub fn cuboid(w: f32, h: f32, d: f32) -> Geo {
    cuboid_segments(w, h, d, 1, 1, 1)
}

/// `BoxGeometry(w, h, d, ws, hs, ds)`.
pub fn cuboid_segments(w: f32, h: f32, d: f32, ws: u32, hs: u32, ds: u32) -> Geo {
    let mut g = Geo {
        positions: Vec::new(),
        indices: Some(Vec::new()),
    };
    // (u, v, w) : axes 0 = x, 1 = y, 2 = z ; mêmes appels que `BoxGeometry`, dans le même ordre.
    plane(&mut g, (2, 1, 0), -1.0, -1.0, d, h, w, ds, hs); // px
    plane(&mut g, (2, 1, 0), 1.0, -1.0, d, h, -w, ds, hs); // nx
    plane(&mut g, (0, 2, 1), 1.0, 1.0, w, d, h, ws, ds); // py
    plane(&mut g, (0, 2, 1), 1.0, -1.0, w, d, -h, ws, ds); // ny
    plane(&mut g, (0, 1, 2), 1.0, -1.0, w, h, d, ws, hs); // pz
    plane(&mut g, (0, 1, 2), -1.0, -1.0, w, h, -d, ws, hs); // nz
    g
}

#[allow(clippy::too_many_arguments)] // signature de `buildPlane` de three.js, gardée telle quelle
fn plane(
    g: &mut Geo,
    (u, v, w): (usize, usize, usize),
    udir: f32,
    vdir: f32,
    width: f32,
    height: f32,
    depth: f32,
    grid_x: u32,
    grid_y: u32,
) {
    let seg_w = width / grid_x as f32;
    let seg_h = height / grid_y as f32;
    let start = g.positions.len() as u32;
    for iy in 0..=grid_y {
        let y = iy as f32 * seg_h - height / 2.0;
        for ix in 0..=grid_x {
            let x = ix as f32 * seg_w - width / 2.0;
            let mut p = [0.0; 3];
            p[u] = x * udir;
            p[v] = y * vdir;
            p[w] = depth / 2.0;
            g.positions.push(Vec3::from_array(p));
        }
    }
    let row = grid_x + 1;
    let idx = g.indices.get_or_insert_with(Vec::new);
    for iy in 0..grid_y {
        for ix in 0..grid_x {
            let a = start + ix + row * iy;
            let b = start + ix + row * (iy + 1);
            let c = start + ix + 1 + row * (iy + 1);
            let d = start + ix + 1 + row * iy;
            idx.extend([a, b, d, b, c, d]);
        }
    }
}

/// `SphereGeometry(r, ws, hs)`.
pub fn sphere(r: f32, ws: u32, hs: u32) -> Geo {
    sphere_part(r, ws, hs, 0.0, TAU, 0.0, PI)
}

/// `SphereGeometry(r, ws, hs, phiStart, phiLength, thetaStart, thetaLength)`.
pub fn sphere_part(
    r: f32,
    ws: u32,
    hs: u32,
    phi_start: f32,
    phi_len: f32,
    theta_start: f32,
    theta_len: f32,
) -> Geo {
    let ws = ws.max(3);
    let hs = hs.max(2);
    let theta_end = (theta_start + theta_len).min(PI);
    let mut positions = Vec::new();
    let mut grid = Vec::new();
    for iy in 0..=hs {
        let v = iy as f32 / hs as f32;
        let mut row = Vec::new();
        for ix in 0..=ws {
            let u = ix as f32 / ws as f32;
            let (phi, theta) = (phi_start + u * phi_len, theta_start + v * theta_len);
            positions.push(Vec3::new(
                -r * phi.cos() * theta.sin(),
                r * theta.cos(),
                r * phi.sin() * theta.sin(),
            ));
            row.push(positions.len() as u32 - 1);
        }
        grid.push(row);
    }
    let mut idx = Vec::new();
    for iy in 0..hs as usize {
        for ix in 0..ws as usize {
            let a = grid[iy][ix + 1];
            let b = grid[iy][ix];
            let c = grid[iy + 1][ix];
            let d = grid[iy + 1][ix + 1];
            if iy != 0 || theta_start > 0.0 {
                idx.extend([a, b, d]);
            }
            if iy != hs as usize - 1 || theta_end < PI {
                idx.extend([b, c, d]);
            }
        }
    }
    Geo {
        positions,
        indices: Some(idx),
    }
}

/// `CylinderGeometry(rTop, rBottom, h, radial)` : axe y, centrée, premier sommet vers +z.
pub fn cylinder(r_top: f32, r_bottom: f32, h: f32, radial: u32) -> Geo {
    cylinder_full(r_top, r_bottom, h, radial, 1, false)
}

/// `CylinderGeometry(rTop, rBottom, h, radial, heightSegments, openEnded)`.
pub fn cylinder_full(
    r_top: f32,
    r_bottom: f32,
    h: f32,
    radial: u32,
    height_segs: u32,
    open: bool,
) -> Geo {
    let half = h / 2.0;
    let mut positions = Vec::new();
    let mut idx = Vec::new();
    let mut grid = Vec::new();
    for y in 0..=height_segs {
        let v = y as f32 / height_segs as f32;
        let radius = v * (r_bottom - r_top) + r_top;
        let mut row = Vec::new();
        for x in 0..=radial {
            let theta = x as f32 / radial as f32 * TAU;
            positions.push(Vec3::new(
                radius * theta.sin(),
                -v * h + half,
                radius * theta.cos(),
            ));
            row.push(positions.len() as u32 - 1);
        }
        grid.push(row);
    }
    for x in 0..radial as usize {
        for y in 0..height_segs as usize {
            let a = grid[y][x];
            let b = grid[y + 1][x];
            let c = grid[y + 1][x + 1];
            let d = grid[y][x + 1];
            if r_top > 0.0 || y != 0 {
                idx.extend([a, b, d]);
            }
            if r_bottom > 0.0 || y != height_segs as usize - 1 {
                idx.extend([b, c, d]);
            }
        }
    }
    if !open {
        for top in [true, false] {
            let radius = if top { r_top } else { r_bottom };
            if radius <= 0.0 {
                continue;
            }
            let sign = if top { 1.0 } else { -1.0 };
            let center = positions.len() as u32;
            for _ in 1..=radial {
                positions.push(Vec3::new(0.0, half * sign, 0.0));
            }
            let ring = positions.len() as u32;
            for x in 0..=radial {
                let theta = x as f32 / radial as f32 * TAU;
                positions.push(Vec3::new(
                    radius * theta.sin(),
                    half * sign,
                    radius * theta.cos(),
                ));
            }
            for x in 0..radial {
                let (c, i) = (center + x, ring + x);
                if top {
                    idx.extend([i, i + 1, c]);
                } else {
                    idx.extend([i + 1, i, c]);
                }
            }
        }
    }
    Geo {
        positions,
        indices: Some(idx),
    }
}

/// `ConeGeometry(r, h, radial)` : pointe en +y.
pub fn cone(r: f32, h: f32, radial: u32) -> Geo {
    cylinder(0.0, r, h, radial)
}

/// `TorusGeometry(R, tube, radialSegments, tubularSegments)` : anneau dans le plan xy.
pub fn torus(radius: f32, tube: f32, radial: u32, tubular: u32) -> Geo {
    let mut positions = Vec::new();
    for j in 0..=radial {
        for i in 0..=tubular {
            let u = i as f32 / tubular as f32 * TAU;
            let v = j as f32 / radial as f32 * TAU;
            positions.push(Vec3::new(
                (radius + tube * v.cos()) * u.cos(),
                (radius + tube * v.cos()) * u.sin(),
                tube * v.sin(),
            ));
        }
    }
    let mut idx = Vec::new();
    let row = tubular + 1;
    for j in 1..=radial {
        for i in 1..=tubular {
            let a = row * j + i - 1;
            let b = row * (j - 1) + i - 1;
            let c = row * (j - 1) + i;
            let d = row * j + i;
            idx.extend([a, b, d, b, c, d]);
        }
    }
    Geo {
        positions,
        indices: Some(idx),
    }
}

const PHI: f32 = std::f32::consts::GOLDEN_RATIO;

/// `IcosahedronGeometry(r, detail)`.
pub fn icosahedron(r: f32, detail: u32) -> Geo {
    let t = PHI;
    #[rustfmt::skip]
    let v = [
        -1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t, 0.0,
        0.0, -1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t,
        t, 0.0, -1.0, t, 0.0, 1.0, -t, 0.0, -1.0, -t, 0.0, 1.0,
    ];
    #[rustfmt::skip]
    let i = [
        0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7, 1, 8,
        3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9, 8, 1,
    ];
    polyhedron(&v, &i, r, detail)
}

/// `OctahedronGeometry(r, detail)`.
pub fn octahedron(r: f32, detail: u32) -> Geo {
    #[rustfmt::skip]
    let v = [1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0];
    let i = [
        0, 2, 4, 0, 4, 3, 0, 3, 5, 0, 5, 2, 1, 2, 5, 1, 5, 3, 1, 3, 4, 1, 4, 2,
    ];
    polyhedron(&v, &i, r, detail)
}

/// `DodecahedronGeometry(r, detail)`.
pub fn dodecahedron(r: f32, detail: u32) -> Geo {
    let t = PHI;
    let q = 1.0 / t;
    #[rustfmt::skip]
    let v = [
        -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
        1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
        0.0, -q, -t, 0.0, -q, t, 0.0, q, -t, 0.0, q, t,
        -q, -t, 0.0, -q, t, 0.0, q, -t, 0.0, q, t, 0.0,
        -t, 0.0, -q, t, 0.0, -q, -t, 0.0, q, t, 0.0, q,
    ];
    #[rustfmt::skip]
    let i = [
        3, 11, 7, 3, 7, 15, 3, 15, 13, 7, 19, 17, 7, 17, 6, 7, 6, 15, 17, 4, 8, 17, 8, 10, 17, 10, 6,
        8, 0, 16, 8, 16, 2, 8, 2, 10, 0, 12, 1, 0, 1, 18, 0, 18, 16, 6, 10, 2, 6, 2, 13, 6, 13, 15,
        2, 16, 18, 2, 18, 3, 2, 3, 13, 18, 1, 9, 18, 9, 11, 18, 11, 3, 4, 14, 12, 4, 12, 0, 4, 0, 8,
        11, 9, 5, 11, 5, 19, 11, 19, 7, 19, 5, 14, 19, 14, 4, 19, 4, 17, 1, 12, 14, 1, 14, 5, 1, 5, 9,
    ];
    polyhedron(&v, &i, r, detail)
}

/// `PolyhedronGeometry` : subdivision de chaque face puis projection sur la sphère ; non indexée.
fn polyhedron(vertices: &[f32], faces: &[u32], r: f32, detail: u32) -> Geo {
    let at = |i: u32| {
        let i = i as usize * 3;
        Vec3::new(vertices[i], vertices[i + 1], vertices[i + 2])
    };
    let mut positions = Vec::new();
    let cols = detail + 1;
    for f in faces.as_chunks::<3>().0 {
        let (a, b, c) = (at(f[0]), at(f[1]), at(f[2]));
        let mut v: Vec<Vec<Vec3>> = Vec::new();
        for i in 0..=cols {
            let aj = a.lerp(c, i as f32 / cols as f32);
            let bj = b.lerp(c, i as f32 / cols as f32);
            let rows = cols - i;
            let row = (0..=rows)
                .map(|j| {
                    if j == 0 && i == cols {
                        aj
                    } else {
                        aj.lerp(bj, j as f32 / rows as f32)
                    }
                })
                .collect();
            v.push(row);
        }
        for i in 0..cols as usize {
            for j in 0..(2 * (cols as usize - i) - 1) {
                let k = j / 2;
                if j % 2 == 0 {
                    positions.extend([v[i][k + 1], v[i + 1][k], v[i][k]]);
                } else {
                    positions.extend([v[i][k + 1], v[i + 1][k + 1], v[i + 1][k]]);
                }
            }
        }
    }
    for p in &mut positions {
        *p = p.normalize() * r;
    }
    Geo {
        positions,
        indices: None,
    }
}

/// Géométrie libre (triangles bruts), comme une `BufferGeometry` remplie à la main.
// Sert aux ailes des drakes, qui arrivent avec les monstres dans la PR suivante de #109.
#[allow(dead_code)]
pub fn triangles(points: &[Vec3]) -> Geo {
    Geo {
        positions: points.to_vec(),
        indices: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    /// Normale sortante : chaque triangle d'un solide convexe centré regarde à l'opposé du centre.
    fn outward(g: &Geo) -> bool {
        g.triangle_vertices().as_chunks::<3>().0.iter().all(|t| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            n.length() < 1e-9 || n.dot(t[0] + t[1] + t[2]) > 0.0
        })
    }

    #[test]
    fn cube_comme_three() {
        let g = cuboid(2.0, 4.0, 6.0);
        assert_eq!(g.positions.len(), 24);
        assert_eq!(g.triangles(), 12);
        // premier sommet de la face +x : (w/2, h/2, d/2) avec u = z inversé, v = y inversé
        assert!(close(g.positions[0], Vec3::new(1.0, 2.0, 3.0)));
        assert!(outward(&g));
        let g = cuboid_segments(1.0, 1.0, 1.0, 2, 2, 2);
        assert_eq!(g.positions.len(), 6 * 9);
        assert_eq!(g.triangles(), 6 * 8);
    }

    #[test]
    fn sphere_comme_three() {
        let g = sphere(1.0, 6, 5);
        assert_eq!(g.positions.len(), 7 * 6);
        // pôles : une seule rangée de triangles en haut et en bas
        assert_eq!(g.triangles(), 6 * 5 * 2 - 6 * 2);
        assert!(close(g.positions[0], Vec3::Y));
        assert!(outward(&g));
        // demi-sphère (carapaces) : pas de calotte inférieure
        let dome = sphere_part(1.0, 8, 5, 0.0, TAU, 0.0, PI / 2.0);
        assert_eq!(dome.triangles(), 8 * 5 * 2 - 8);
        assert!(dome.positions.iter().all(|p| p.y >= -1e-6));
    }

    #[test]
    fn cylindre_et_cone_comme_three() {
        let g = cylinder(1.0, 2.0, 3.0, 8);
        // flanc 2 × 9, deux couvercles 8 + 9
        assert_eq!(g.positions.len(), 18 + 2 * 17);
        assert_eq!(g.triangles(), 16 + 8 + 8);
        // premier sommet vers +z, en haut
        assert!(close(g.positions[0], Vec3::new(0.0, 1.5, 1.0)));
        assert!(outward(&g));
        let c = cone(1.0, 2.0, 4);
        assert_eq!(c.triangles(), 4 + 4);
        assert!(close(c.positions[0], Vec3::new(0.0, 1.0, 0.0)));
        assert!(outward(&c));
    }

    #[test]
    fn polyedres_sur_la_sphere() {
        for (g, faces) in [
            (icosahedron(2.0, 0), 20),
            (octahedron(2.0, 0), 8),
            (dodecahedron(2.0, 0), 36),
            (icosahedron(2.0, 1), 80),
        ] {
            assert_eq!(g.triangles(), faces);
            assert!(g.positions.iter().all(|p| (p.length() - 2.0).abs() < 1e-5));
            assert!(outward(&g));
        }
    }

    #[test]
    fn tore_comme_three() {
        let g = torus(1.0, 0.1, 6, 24);
        assert_eq!(g.positions.len(), 7 * 25);
        assert_eq!(g.triangles(), 6 * 24 * 2);
        assert!(close(g.positions[0], Vec3::new(1.1, 0.0, 0.0)));
        assert!(g.positions.iter().all(|p| p.z.abs() <= 0.1 + 1e-6));
    }

    #[test]
    fn transformations_dans_l_ordre() {
        // `rotateZ(-π/2)` puis `translate` : le cône couché pointe vers +x, décalé
        let g = cone(0.1, 1.0, 4)
            .rotate_z(-PI / 2.0)
            .translate(0.5, 0.0, 0.0);
        assert!(close(g.positions[0], Vec3::new(1.0, 0.0, 0.0)));
        let mesh = g.to_flat_mesh();
        assert_eq!(mesh.count_vertices(), g.triangles() * 3);
    }
}
