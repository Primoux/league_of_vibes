//! Murs de la carte en falaises rocheuses : portage de `subdivide` / `buildWallBases`
//! (`packages/client/src/render/map/vegetation.ts`).
//!
//! Chaque polygone de `WALLS` est subdivisé, extrudé avec biseau (`ExtrudeGeometry`, portée dans
//! `render/champions/portrait/three.rs`), puis les parois sont bosselées et peintes : mousse sur le
//! dessus, strates sur les parois, pied assombri. Le dessus est recoloré par pixel dans `cliff.wesl`.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use lov_shared::map::WALLS;
use lov_shared::types::Vec2 as GameVec2;

use super::geometry::smooth;
use super::vgeo::{Rgb, VGeo, hash3, hex, js_round};
use crate::render::RENDER_SCALE;
use crate::render::champions::portrait::three::{Extrude, extrude};

pub const SHADER: &str = "embedded://lov_client/render/map/cliff.wesl";

/// Hauteur du dessus des murs (profondeur + biseau − enfoncement).
pub const WALL_TOP: f64 = 0.8 + 0.3 - 0.08;

/// Insère des points le long des arêtes (pas ≈ `step` unités monde) pour pouvoir bosseler les parois.
/// Points en unités monde, dans le plan de jeu (y vers le haut de la carte).
pub fn subdivide(poly: &[GameVec2], step: f64) -> Vec<[f32; 2]> {
    let s = f64::from(RENDER_SCALE);
    let mut out = Vec::new();
    for (i, a) in poly.iter().enumerate() {
        let b = poly[(i + 1) % poly.len()];
        let n = js_round((b.x - a.x).hypot(b.y - a.y) * s / step).max(1.0) as u32;
        for k in 0..n {
            let t = f64::from(k) / f64::from(n);
            out.push([
                ((a.x + (b.x - a.x) * t) * s) as f32,
                ((a.y + (b.y - a.y) * t) * s) as f32,
            ]);
        }
    }
    out
}

/// Géométrie fusionnée de tous les murs, bosselée et peinte.
pub fn wall_geometry() -> VGeo {
    let (rock, rock_dark, rock_light) = (hex(0x8d806a), hex(0x4f463b), hex(0xb3a589));
    let (moss, moss_light) = (hex(0x2c5428), hex(0x4a7a36));
    let options = Extrude {
        depth: 0.8,
        bevel_enabled: true,
        bevel_thickness: 0.3,
        bevel_size: 0.4,
        bevel_segments: 2,
        steps: 2,
    };
    let parts = WALLS
        .iter()
        .map(|poly| {
            let g = extrude(&subdivide(poly, 0.55), options)
                .rotate_x(-std::f32::consts::FRAC_PI_2)
                .translate(0.0, -0.08, 0.0);
            VGeo::from_geo(&g)
        })
        .collect();
    let mut merged = VGeo::merge(parts);
    // parois bosselées : décalage horizontal fort, vertical faible (le sommet reste praticable visuellement)
    for p in &mut merged.pos {
        let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        let k = smooth(-0.1, 0.4, y);
        let n1 = hash3(
            js_round(x * 2.2) / 2.2,
            js_round(y * 3.0) / 3.0,
            js_round(z * 2.2) / 2.2,
        );
        // le dessus (capot triangulé en longues lamelles) reste plan : seul son contour bouge
        let ky = if y > WALL_TOP - 0.01 { 0.0 } else { k };
        *p = Vec3::new(
            (x + (hash3(x, y, z) - 0.5) * 0.32 * k) as f32,
            (y + (n1 - 0.5) * 0.14 * ky) as f32,
            (z + (hash3(z, y, x) - 0.5) * 0.32 * k) as f32,
        );
    }
    merged.flat_normals();
    // couleurs : mousse sur le dessus, strates sur les parois, pied assombri (occlusion)
    merged.paint(|x, y, z, ny| -> Rgb {
        if ny > 0.72 {
            let t = hash3((x * 1.5).floor(), 0.0, (z * 1.5).floor()) * 0.8;
            return moss.lerp(moss_light, t as f32);
        }
        let strata = (y * 13.0 + (x * 0.7 + z * 0.5).sin() * 1.6).sin() * 0.5 + 0.5;
        let grain = hash3(js_round(x * 4.0), js_round(y * 6.0), js_round(z * 4.0)) * 0.3;
        let mut c = rock_dark
            .lerp(rock, (0.55 + strata * 0.3) as f32)
            .lerp(rock_light, grain as f32);
        c *= (0.55 + 0.45 * smooth(-0.1, 0.55, y)) as f32;
        if ny > 0.35 {
            c = c.lerp(moss, ((ny - 0.35) * 1.2) as f32);
        }
        c
    })
}

/// Couleurs de la mousse du dessus (`cliff.wesl`).
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct CliffUniform {
    moss: Vec4,
    moss_light: Vec4,
}

impl CliffUniform {
    pub fn new() -> Self {
        Self {
            moss: hex(0x2c5428).extend(1.0),
            moss_light: hex(0x4a7a36).extend(1.0),
        }
    }
}

/// Extension du `StandardMaterial` des falaises : herbe/mousse calculée par pixel sur le dessus (bruit
/// monde), comme le sol, au lieu d'un aplat par triangle.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CliffExt {
    #[uniform(100)]
    pub u: CliffUniform,
    #[texture(101)]
    #[sampler(102)]
    pub noise: Handle<Image>,
    /// Visibilité du brouillard de guerre ([`super::fog::FogOfWar`]).
    #[texture(110)]
    #[sampler(111)]
    pub fog: Handle<Image>,
}

impl MaterialExtension for CliffExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub type CliffMaterial = ExtendedMaterial<StandardMaterial, CliffExt>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn murs_identiques_au_client_web() {
        let g = wall_geometry();
        // `wallVerts` et premiers sommets (position, couleur) du client web
        assert_eq!(g.pos.len(), 69384);
        let expected: [(usize, [f32; 6]); 4] = [
            (0, [17.025, -0.38, -33.5, 0.15706, 0.12885, 0.08616]),
            (1, [16.5, -0.38, -33.5, 0.14810, 0.12107, 0.08101]),
            (2, [16.5, -0.38, -34.04839, 0.13736, 0.11172, 0.07483]),
            (100, [20.7, -0.38, -34.59678, 0.14035, 0.11422, 0.07646]),
        ];
        for (i, e) in expected {
            let (p, c) = (g.pos[i], g.col[i]);
            let got = [p.x, p.y, p.z, c.x, c.y, c.z];
            for (a, b) in got.iter().zip(e) {
                assert!(
                    (a - b).abs() < 2e-4,
                    "sommet {i} : {got:?} au lieu de {e:?}"
                );
            }
        }
    }

    #[test]
    fn subdivision_des_aretes() {
        let square = [
            GameVec2 { x: 0.0, y: 0.0 },
            GameVec2 { x: 110.0, y: 0.0 },
            GameVec2 { x: 110.0, y: 55.0 },
        ];
        let pts = subdivide(&square, 0.55);
        // 1,1 monde → 2 pas ; 0,55 → 1 ; diagonale 1,23 → 2
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[1], [0.55, 0.0]);
    }
}
