//! Outils de géométrie et géométries du décor : portage de la première moitié de
//! `packages/client/src/render/map/vegetation.ts` (`hash3`, `facet`, `paint`, `mergeGeometries`, arbres,
//! rochers, touffes, fleurs).
//!
//! Les couleurs sont des couleurs de sommets linéaires (`THREE.Color` en gestion des couleurs) ; les
//! calculs de bruit lisent les positions `f32`, comme le client web lit ses `Float32Array`.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::geometry::smooth;
use crate::render::models::geometry::{Geo, cylinder, cylinder_full, icosahedron};

/// Couleur linéaire.
pub type Rgb = Vec3;

/// `new THREE.Color(hex)` : sRGB → linéaire.
pub fn hex(c: u32) -> Rgb {
    let l = Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8).to_linear();
    Vec3::new(l.red, l.green, l.blue)
}

/// `Math.round` de JavaScript (les demis vont vers +∞).
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Bruit 3D déterministe (fonction de la position seule : les sommets dupliqués bougent ensemble).
pub fn hash3(x: f64, y: f64, z: f64) -> f64 {
    let (a, b, c) = (
        js_round(x * 997.0) as i64,
        js_round(y * 997.0) as i64,
        js_round(z * 997.0) as i64,
    );
    let mut h = (a * 374_761_393 + b * 668_265_263 + c * 1_442_695_041) as i32;
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    f64::from((h ^ ((h as u32) >> 16) as i32) as u32) / 4_294_967_296.0
}

/// `color.offsetHSL(0, 0, dl)` en espace linéaire (l'espace de travail de three.js).
pub fn offset_lightness(c: Rgb, dl: f32) -> Rgb {
    let (max, min) = (c.max_element(), c.min_element());
    let l = (max + min) / 2.0;
    let (mut h, mut s) = (0.0, 0.0);
    if max != min {
        let d = max - min;
        s = if l <= 0.5 {
            d / (max + min)
        } else {
            d / (2.0 - max - min)
        };
        h = if max == c.x {
            (c.y - c.z) / d + if c.y < c.z { 6.0 } else { 0.0 }
        } else if max == c.y {
            (c.z - c.x) / d + 2.0
        } else {
            (c.x - c.y) / d + 4.0
        } / 6.0;
    }
    let l = (l + dl).clamp(0.0, 1.0);
    let s = s.clamp(0.0, 1.0);
    if s == 0.0 {
        return Vec3::splat(l);
    }
    let p = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let q = 2.0 * l - p;
    let hue = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            q + (p - q) * 6.0 * t
        } else if t < 0.5 {
            p
        } else if t < 2.0 / 3.0 {
            q + (p - q) * 6.0 * (2.0 / 3.0 - t)
        } else {
            q
        }
    };
    Vec3::new(hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0))
}

/// Géométrie non indexée : position, normale, couleur par sommet (`BufferGeometry` du client web).
#[derive(Clone, Debug, Default)]
pub struct VGeo {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub col: Vec<Rgb>,
}

impl VGeo {
    /// `toNonIndexed()`, sans normales ni couleurs (blanches).
    pub fn from_geo(g: &Geo) -> Self {
        let pos = g.triangle_vertices();
        let n = pos.len();
        Self {
            pos,
            nrm: vec![Vec3::Y; n],
            col: vec![Vec3::ONE; n],
        }
    }

    /// `computeVertexNormals()` d'une géométrie non indexée : normale de chaque facette.
    pub fn flat_normals(&mut self) {
        for (t, n) in self
            .pos
            .as_chunks::<3>()
            .0
            .iter()
            .zip(self.nrm.as_chunks_mut::<3>().0)
        {
            let normal = (t[2] - t[1]).cross(t[0] - t[1]).normalize_or_zero();
            *n = [normal; 3];
        }
    }

    /// `paint(g, fn)` : couleur de chaque sommet selon sa position et la composante y de sa normale.
    pub fn paint(mut self, f: impl Fn(f64, f64, f64, f64) -> Rgb) -> Self {
        for i in 0..self.pos.len() {
            let p = self.pos[i].as_dvec3();
            self.col[i] = f(p.x, p.y, p.z, f64::from(self.nrm[i].y));
        }
        self
    }

    /// `mergeGeometries` : concaténation.
    pub fn merge(parts: Vec<VGeo>) -> VGeo {
        let mut out = VGeo::default();
        for p in parts {
            out.pos.extend(p.pos);
            out.nrm.extend(p.nrm);
            out.col.extend(p.col);
        }
        out
    }

    pub fn to_mesh(&self) -> Mesh {
        let colors: Vec<[f32; 4]> = self.col.iter().map(|c| c.extend(1.0).to_array()).collect();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    }
}

/// Géométrie non indexée, sommets décalés de façon déterministe, normales de facettes.
pub fn facet(g: &Geo, jitter: f64) -> VGeo {
    let mut v = VGeo::from_geo(g);
    for p in &mut v.pos {
        let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        *p = Vec3::new(
            (x + (hash3(x, y, z) - 0.5) * jitter) as f32,
            (y + (hash3(z, x, y) - 0.5) * jitter * 0.6) as f32,
            (z + (hash3(y, z, x) - 0.5) * jitter) as f32,
        );
    }
    v.flat_normals();
    v
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    a.lerp(b, t as f32)
}

// ---------------------------------------------------------------- arbres

fn trunk(h: f32, r: f32) -> VGeo {
    let (trunk, bark) = (hex(0x5a3f2a), hex(0x2e2018));
    let hh = f64::from(h);
    facet(
        &cylinder(r * 0.65, r, h, 5).translate(0.0, h / 2.0, 0.0),
        0.02,
    )
    .paint(|_, y, _, _| lerp(bark, trunk, smooth(0.0, hh * 0.8, y)))
}

/// Sapin : étages de cônes (rayon, hauteur, base), chaque étage sombre à la base (ombre de l'étage
/// supérieur) et clair à la pointe.
fn pine(layers: &[(f32, f32, f32)], green: u32) -> VGeo {
    let base = hex(green);
    let dark = base * 0.42;
    let tip = base.lerp(hex(0xd8e88a), 0.25) * 1.25;
    let mut parts = vec![trunk(0.6, 0.11)];
    for &(r, h, y) in layers {
        let (y0, y1) = (f64::from(y), f64::from(y + h));
        let g = facet(
            &cylinder_full(0.0, r, h, 8, 2, false).translate(0.0, y + h / 2.0, 0.0),
            0.09,
        );
        parts.push(g.paint(|_, py, _, ny| {
            let t = smooth(y0, y1, py);
            let c = lerp(
                lerp(dark, base, smooth(0.0, 0.45, t)),
                tip,
                smooth(0.55, 1.0, t) * 0.8,
            );
            if ny < -0.3 { c * 0.6 } else { c }
        }));
    }
    VGeo::merge(parts)
}

/// Feuillu : touffes sphériques facettées, ombre douce en dessous, lumière sur le dessus.
fn broadleaf() -> VGeo {
    let base = hex(0x467f36);
    let dark = base * 0.4;
    let top = base.lerp(hex(0xd8e08a), 0.2) * 1.05;
    let mut parts = vec![trunk(0.95, 0.12)];
    let blobs: [(f32, f32, f32, f32); 5] = [
        (0.0, 1.45, 0.0, 0.62),
        (0.38, 1.2, 0.15, 0.45),
        (-0.32, 1.22, -0.2, 0.47),
        (0.05, 1.18, -0.38, 0.42),
        (-0.1, 1.85, 0.08, 0.42),
    ];
    for (x, y, z, r) in blobs {
        let g = facet(&icosahedron(r, 1).translate(x, y, z), 0.1);
        parts.push(g.paint(|_, py, _, ny| {
            let c = lerp(
                lerp(dark, base, smooth(0.8, 1.45, py)),
                top,
                smooth(1.4, 2.2, py) * 0.85,
            );
            c * (0.75 + 0.35 * ny.max(0.0)) as f32
        }));
    }
    VGeo::merge(parts)
}

/// Variantes d'arbres (sapin élancé, sapin trapu, feuillu).
pub fn tree_variants() -> [VGeo; 3] {
    [
        pine(
            &[
                (0.66, 1.05, 0.45),
                (0.52, 0.9, 0.95),
                (0.36, 0.75, 1.42),
                (0.2, 0.5, 1.85),
            ],
            0x3c7f3e,
        ),
        pine(
            &[(0.78, 0.9, 0.4), (0.58, 0.8, 0.85), (0.36, 0.7, 1.25)],
            0x4a8a3a,
        ),
        broadleaf(),
    ]
}

// ---------------------------------------------------------------- rochers

pub fn rock() -> VGeo {
    let (base, moss) = (hex(0x8f8676), hex(0x4d7a38));
    facet(&icosahedron(0.22, 0), 0.08).paint(|_, y, _, ny| {
        let c = base * (0.6 + 0.5 * smooth(-0.2, 0.2, y)) as f32;
        if ny > 0.55 { c.lerp(moss, 0.65) } else { c }
    })
}

// ---------------------------------------------------------------- brins d'herbe et fleurs

/// Touffe de brins courbés (2 triangles par brin) ; couleurs de sommets racine sombre → pointe claire.
/// Normales vers le haut : l'herbe est éclairée comme le sol, sans facettes noires selon l'orientation.
pub fn blade_tuft(n: u32, h: f64, spread: f64, w: f64, root: Rgb, tip: Rgb) -> VGeo {
    let mut g = VGeo::default();
    let mut push = |x: f64, y: f64, z: f64, t: f64| {
        g.pos.push(Vec3::new(x as f32, y as f32, z as f32));
        g.col.push(lerp(root, tip, t));
        g.nrm.push(Vec3::Y);
    };
    for i in 0..n {
        let fi = f64::from(i);
        let a = fi / f64::from(n) * std::f64::consts::TAU + fi * 1.7;
        let bh = h * (0.75 + f64::from((i * 7) % 5) * 0.08);
        let (ox, oz) = (a.cos() * spread, a.sin() * spread);
        // plan du brin perpendiculaire à sa direction d'inclinaison
        let (px, pz) = (-a.sin() * w, a.cos() * w);
        let lean = 0.35 + f64::from(i % 3) * 0.12;
        let mx = ox + a.cos() * bh * lean * 0.45;
        let mz = oz + a.sin() * bh * lean * 0.45;
        let tx = ox + a.cos() * bh * lean;
        let tz = oz + a.sin() * bh * lean;
        // base → milieu (quad), milieu → pointe (triangle)
        push(ox - px, 0.0, oz - pz, 0.0);
        push(ox + px, 0.0, oz + pz, 0.0);
        push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
        push(ox - px, 0.0, oz - pz, 0.0);
        push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
        push(mx - px * 0.6, bh * 0.55, mz - pz * 0.6, 0.55);
        push(mx - px * 0.6, bh * 0.55, mz - pz * 0.6, 0.55);
        push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
        push(tx, bh, tz, 1.0);
    }
    g
}

/// Fleur : disque à 5 pétales (blanc, teinté par l'instance), cœur jaune, tige.
pub fn flower() -> VGeo {
    let mut g = VGeo::default();
    let h = 0.1f32;
    let p = |a: f32, r: f32, y: f32| Vec3::new(a.cos() * r, y, a.sin() * r);
    for i in 0..5 {
        let a0 = i as f32 / 5.0 * std::f32::consts::TAU;
        let a1 = a0 + std::f32::consts::TAU / 5.0;
        let am = (a0 + a1) / 2.0;
        g.pos.extend([
            Vec3::new(0.0, h, 0.0),
            p(a1, 0.03, h),
            p(am, 0.075, h + 0.01),
        ]);
        g.pos.extend([
            Vec3::new(0.0, h, 0.0),
            p(am, 0.075, h + 0.01),
            p(a0, 0.03, h),
        ]);
        for _ in 0..2 {
            g.col.extend([
                Vec3::new(1.0, 0.75, 0.15),
                Vec3::splat(0.95),
                Vec3::splat(0.9),
            ]);
        }
    }
    // tige
    g.pos.extend([
        Vec3::new(-0.006, 0.0, 0.0),
        Vec3::new(0.006, 0.0, 0.0),
        Vec3::new(0.0, h, 0.0),
    ]);
    g.col.extend([
        Vec3::new(0.1, 0.25, 0.08),
        Vec3::new(0.1, 0.25, 0.08),
        Vec3::new(0.15, 0.35, 0.1),
    ]);
    g.nrm = vec![Vec3::Y; g.pos.len()];
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variantes_d_arbres_du_meme_nombre_de_sommets_que_le_web() {
        // `treeVerts` du client web : 444, 348, 1260
        let counts: Vec<usize> = tree_variants().iter().map(|g| g.pos.len()).collect();
        assert_eq!(counts, [444, 348, 1260]);
    }

    #[test]
    fn eclaircir_une_couleur_comme_offset_hsl() {
        assert_eq!(offset_lightness(Vec3::ONE, -0.1), Vec3::splat(0.9));
        assert_eq!(offset_lightness(Vec3::ONE, 0.2), Vec3::ONE);
        // l = 0,375 → 0,475 à saturation constante (1/3)
        let lighter = offset_lightness(Vec3::new(0.5, 0.25, 0.25), 0.1);
        let expected = Vec3::new(
            0.475 * 4.0 / 3.0,
            0.95 - 0.475 * 4.0 / 3.0,
            0.95 - 0.475 * 4.0 / 3.0,
        );
        assert!((lighter - expected).length() < 1e-5, "{lighter}");
    }

    #[test]
    fn bruit_3d_deterministe() {
        assert_eq!(hash3(1.0, 2.0, 3.0), hash3(1.0, 2.0, 3.0));
        assert!((0.0..1.0).contains(&hash3(-12.5, 0.3, 99.0)));
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(2.5), 3.0);
    }
}
