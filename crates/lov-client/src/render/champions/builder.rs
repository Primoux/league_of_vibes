//! Construction des modèles de champions : les pièces de chaque articulation sont fusionnées par matière
//! (corps, métal optionnel et lumières), avec couleurs de sommets. Un contour par coque inversée est
//! ajouté à chaque lot opaque.
//!
//! Portage de `packages/client/src/render/champions/builder.ts`. Le client web fusionne toutes les pièces
//! d'un lot en un seul `SkinnedMesh` à skinning rigide ; ici chaque lot donne un maillage par articulation,
//! porté par l'entité de l'articulation (voir `rig.rs`) : même rendu, sans skinning. La coque du contour
//! est construite comme dans le client web (sommets confondus fusionnés sur tout le modèle, normales
//! lissées), puis découpée par articulation.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::anim::pose::JOINT_COUNT;
use super::rig::RigSpec;
use crate::render::models::geometry::{self as g3, Geo};
use crate::render::models::kit::euler;

/// Options d'une pièce : placement dans le repère de l'articulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartOpts {
    pub pos: Vec3,
    /// Angles d'Euler (ordre XYZ de three.js).
    pub rot: Vec3,
    pub scale: Vec3,
    /// Conserve les normales lissées de la pièce (modèles sculptés) ; défaut : facettes.
    pub smooth: bool,
}

impl Default for PartOpts {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            rot: Vec3::ZERO,
            scale: Vec3::ONE,
            smooth: false,
        }
    }
}

impl PartOpts {
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            pos: Vec3::new(x, y, z),
            ..default()
        }
    }

    pub fn rot(mut self, x: f32, y: f32, z: f32) -> Self {
        self.rot = Vec3::new(x, y, z);
        self
    }

    pub fn scale(mut self, x: f32, y: f32, z: f32) -> Self {
        self.scale = Vec3::new(x, y, z);
        self
    }

    pub fn smooth(mut self) -> Self {
        self.smooth = true;
        self
    }

    fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, euler(self.rot), self.pos)
    }
}

/// Couleur CSS `#rrggbb` (sRGB) → linéaire, comme `new THREE.Color('#…')`. Couleur invalide : magenta.
pub fn css(s: &str) -> LinearRgba {
    Srgba::hex(s).map_or(LinearRgba::rgb(1.0, 0.0, 1.0), LinearRgba::from)
}

// ---------------------------------------------------------------- primitives

/// `hash3` du client web (calcul en f64, comme JavaScript).
fn hash3(x: f64, y: f64, z: f64) -> f64 {
    let h = (x * 127.1 + y * 311.7 + z * 74.7).sin() * 43758.5453;
    h - h.floor()
}

/// `Math.round` de JavaScript (moitiés arrondies vers +∞).
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// Déforme chaque sommet selon sa position (les sommets confondus bougent pareil : pas de fissures).
pub fn jitter(mut g: Geo, amount: f32, seed: f32) -> Geo {
    for p in &mut g.positions {
        let (x, y, z) = (f64::from(p.x), f64::from(p.y), f64::from(p.z));
        let h = hash3(
            js_round(x * 1000.0) + f64::from(seed),
            js_round(y * 1000.0),
            js_round(z * 1000.0),
        );
        let k = ((h - 0.5) * 2.0 * f64::from(amount)) as f32;
        let len = p.length();
        let len = if len == 0.0 { 1.0 } else { len };
        *p += *p / len * k;
    }
    g
}

/// Primitives des modèles (`geo` du client web).
pub mod geo {
    use super::*;

    pub fn cuboid(w: f32, h: f32, d: f32) -> Geo {
        g3::cuboid(w, h, d)
    }

    /// Pavé biseauté façon « pierre taillée ».
    pub fn slab(w: f32, h: f32, d: f32, seed: f32) -> Geo {
        jitter(
            g3::cuboid_segments(w, h, d, 2, 2, 2),
            w.min(h).min(d) * 0.12,
            seed,
        )
    }

    pub fn rock(r: f32, seed: f32, detail: u32) -> Geo {
        jitter(g3::dodecahedron(r, detail), r * 0.22, seed)
    }

    /// `ball(r, detail = 1)`.
    pub fn ball(r: f32, detail: u32) -> Geo {
        g3::icosahedron(r, detail)
    }

    /// `cyl(rTop, rBottom, h, seg = 6)`.
    pub fn cyl(r_top: f32, r_bottom: f32, h: f32, seg: u32) -> Geo {
        g3::cylinder(r_top, r_bottom, h, seg)
    }

    /// `cone(r, h, seg = 5)`.
    pub fn cone(r: f32, h: f32, seg: u32) -> Geo {
        g3::cone(r, h, seg)
    }

    pub fn crystal(r: f32, h: f32) -> Geo {
        g3::octahedron(r, 0).scale(1.0, h / r / 2.0, 1.0)
    }

    /// `torus(r, tube, seg = 10)` : 4 segments autour du tube.
    pub fn torus(r: f32, tube: f32, seg: u32) -> Geo {
        g3::torus(r, tube, 4, seg)
    }
}

// ---------------------------------------------------------------- normales

/// Normales lissées : somme des normales (pondérées par l'aire) des triangles autour de chaque position.
/// Les sommets confondus partagent leur normale, ce qui efface les coutures des sphères, tores et tubes
/// (le client web les obtient analytiquement ou en réparant la couture).
pub fn smooth_normals(tris: &[Vec3]) -> Vec<Vec3> {
    let key = |p: Vec3| (p * 1.0e5).round().as_ivec3();
    let mut acc: HashMap<IVec3, Vec3> = HashMap::new();
    for t in tris.as_chunks::<3>().0 {
        let n = (t[2] - t[1]).cross(t[0] - t[1]);
        for p in t {
            *acc.entry(key(*p)).or_default() += n;
        }
    }
    tris.iter()
        .map(|p| acc[&key(*p)].normalize_or_zero())
        .collect()
}

fn face_normals(tris: &[Vec3]) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(tris.len());
    for t in tris.as_chunks::<3>().0 {
        out.extend([(t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero(); 3]);
    }
    out
}

// ---------------------------------------------------------------- builder

/// Pièce préparée : triangles dans le repère de son articulation.
#[derive(Clone, Debug)]
struct Part {
    tris: Vec<Vec3>,
    /// Normales lissées (pièces `smooth`), sinon facettes.
    normals: Option<Vec<Vec3>>,
    joint: usize,
    color: LinearRgba,
}

/// Géométrie non indexée, normales lissées si demandées, transformée par les options de la pièce.
fn prepare(src: &Geo, o: &PartOpts, joint: usize, color: LinearRgba) -> Part {
    let m = o.matrix();
    let raw = src.triangle_vertices();
    let normals = o.smooth.then(|| {
        let nm = Mat3::from_mat4(m).inverse().transpose();
        smooth_normals(&raw)
            .into_iter()
            .map(|n| (nm * n).normalize_or_zero())
            .collect()
    });
    Part {
        tris: raw.iter().map(|p| m.transform_point3(*p)).collect(),
        normals,
        joint,
        color,
    }
}

/// Maillages d'un lot, un par articulation (`None` : aucune pièce sur cette articulation).
pub type JointMeshes = [Option<Mesh>; JOINT_COUNT];

/// Modèle construit : lots opaques (corps, métal), lumières, et coques de contour.
#[derive(Debug)]
pub struct BuiltModel {
    pub body: JointMeshes,
    pub metal: JointMeshes,
    pub glow: JointMeshes,
    pub outline_body: JointMeshes,
    pub outline_metal: JointMeshes,
    /// Modèle sculpté : surfaces lisses, rugosité 0,7 et pas de métal dans le lot principal.
    pub sculpted: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModelBuilder {
    solids: Vec<Part>,
    metals: Vec<Part>,
    glows: Vec<Part>,
    sculpted: bool,
}

impl ModelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Les nouveaux modèles combinent surfaces lisses et arêtes sculptées dans le même lot.
    pub fn sculpted_surfaces(&mut self) -> &mut Self {
        self.sculpted = true;
        self
    }

    /// Pièce opaque de couleur `color` attachée à `joint`.
    pub fn add(&mut self, joint: usize, g: &Geo, color: LinearRgba, o: PartOpts) -> &mut Self {
        self.solids.push(prepare(g, &o, joint, color));
        self
    }

    /// Lot métallique distinct : la peau et le tissu gardent leur propre rugosité.
    pub fn metal(&mut self, joint: usize, g: &Geo, color: LinearRgba, o: PartOpts) -> &mut Self {
        self.metals.push(prepare(g, &o, joint, color));
        self
    }

    /// Pièce lumineuse (non éclairée, peut être HDR pour le bloom).
    pub fn glow(&mut self, joint: usize, g: &Geo, color: LinearRgba, o: PartOpts) -> &mut Self {
        self.glows.push(prepare(g, &o, joint, color));
        self
    }

    /// Fusionne les pièces : un maillage par articulation et par lot, contours compris (`outline`).
    pub fn build(&self, spec: &RigSpec, outline: bool) -> BuiltModel {
        let rest = spec.rest_world();
        let hull = |parts: &[Part]| {
            if outline {
                hull_meshes(parts, &rest)
            } else {
                std::array::from_fn(|_| None)
            }
        };
        BuiltModel {
            body: self.merge(&self.solids),
            metal: self.merge(&self.metals),
            glow: self.merge(&self.glows),
            outline_body: hull(&self.solids),
            outline_metal: hull(&self.metals),
            sculpted: self.sculpted,
        }
    }

    fn merge(&self, parts: &[Part]) -> JointMeshes {
        std::array::from_fn(|j| {
            let mut pos = Vec::new();
            let mut nor = Vec::new();
            let mut col = Vec::new();
            for p in parts.iter().filter(|p| p.joint == j) {
                pos.extend_from_slice(&p.tris);
                // modèle non sculpté : facettes partout (`flatShading`)
                match p.normals.as_ref().filter(|_| self.sculpted) {
                    Some(n) => nor.extend_from_slice(n),
                    None => nor.extend(face_normals(&p.tris)),
                }
                col.extend(std::iter::repeat_n(p.color.to_f32_array(), p.tris.len()));
            }
            (!pos.is_empty()).then(|| {
                Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
                .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
            })
        })
    }
}

/// Coque du contour (`hullGeometry`) : en pose de repos, les sommets confondus d'une même articulation
/// sont fusionnés (précision 1e-3, comme `mergeVertices`) et leurs normales lissées, pour que l'extrusion
/// ne se fende pas aux arêtes ; puis chaque articulation reprend ses triangles dans son repère.
fn hull_meshes(parts: &[Part], rest: &[Vec3; JOINT_COUNT]) -> JointMeshes {
    let key = |p: Vec3, j: usize| {
        let k = (p * 1000.0).round().as_ivec3();
        (k.x, k.y, k.z, j)
    };
    // normales accumulées par sommet fusionné, sur tout le modèle
    let mut acc: HashMap<(i32, i32, i32, usize), Vec3> = HashMap::new();
    for part in parts {
        let off = rest[part.joint];
        for t in part.tris.as_chunks::<3>().0 {
            let w = t.map(|p| p + off);
            let n = (w[2] - w[1]).cross(w[0] - w[1]);
            for p in w {
                *acc.entry(key(p, part.joint)).or_default() += n;
            }
        }
    }
    std::array::from_fn(|j| {
        let mut pos = Vec::new();
        let mut nor = Vec::new();
        for part in parts.iter().filter(|p| p.joint == j) {
            for p in &part.tris {
                pos.push(*p);
                nor.push(acc[&key(*p + rest[j], j)].normalize_or_zero());
            }
        }
        (!pos.is_empty()).then(|| {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::pose::j;

    const SPEC: RigSpec = RigSpec {
        hip_y: 0.85,
        hip_w: 0.15,
        thigh: 0.42,
        shin: 0.42,
        spine: 0.58,
        shoulder_w: 0.28,
        upper_arm: 0.33,
        forearm: 0.31,
        neck: 0.15,
    };

    #[test]
    fn jitter_deterministe_et_sans_fissure() {
        let a = geo::slab(0.4, 0.3, 0.2, 2.0);
        let b = geo::slab(0.4, 0.3, 0.2, 2.0);
        assert_eq!(a, b);
        // les coins dupliqués d'une face à l'autre bougent pareil
        let corner = Vec3::new(0.2, 0.15, 0.1);
        let moved: Vec<Vec3> = g3::cuboid_segments(0.4, 0.3, 0.2, 2, 2, 2)
            .positions
            .iter()
            .zip(&a.positions)
            .filter(|(src, _)| (**src - corner).length() < 1e-6)
            .map(|(_, dst)| *dst)
            .collect();
        assert_eq!(moved.len(), 3);
        assert!(moved.iter().all(|p| (*p - moved[0]).length() < 1e-7));
        assert_ne!(moved[0], corner);
        assert_ne!(
            geo::slab(0.4, 0.3, 0.2, 3.0),
            a,
            "la graine change la taille"
        );
    }

    #[test]
    fn un_maillage_par_articulation_et_par_lot() {
        let mut b = ModelBuilder::new();
        b.add(
            j::HEAD,
            &geo::ball(0.16, 1),
            css("#e6c9a8"),
            PartOpts::at(0.02, 0.12, 0.0),
        );
        b.add(
            j::HEAD,
            &geo::cuboid(0.1, 0.1, 0.1),
            css("#ff0000"),
            PartOpts::default(),
        );
        b.glow(
            j::PROP,
            &geo::ball(0.09, 1),
            LinearRgba::rgb(3.0, 2.0, 1.0),
            PartOpts::default(),
        );
        let m = b.build(&SPEC, true);
        let head = m.body[j::HEAD].as_ref().map(Mesh::count_vertices);
        assert_eq!(head, Some(80 * 3 + 12 * 3));
        assert!(m.body[j::SPINE].is_none());
        assert!(m.glow[j::PROP].is_some() && m.glow[j::HEAD].is_none());
        assert!(m.outline_body[j::HEAD].is_some());
        assert!(
            m.outline_body[j::PROP].is_none(),
            "pas de contour sur les lumières"
        );
        assert!(m.metal.iter().all(Option::is_none));
    }

    #[test]
    fn coque_lissee_aux_aretes() {
        let mut b = ModelBuilder::new();
        b.add(
            j::SPINE,
            &geo::cuboid(1.0, 1.0, 1.0),
            css("#ffffff"),
            PartOpts::default(),
        );
        let m = b.build(&SPEC, true);
        let hull = m.outline_body[j::SPINE].as_ref().and_then(|h| {
            h.attribute(Mesh::ATTRIBUTE_NORMAL)?
                .as_float3()
                .map(<[_]>::to_vec)
        });
        // au coin (+,+,+), les trois faces se rejoignent : la normale penche vers les trois (somme des
        // triangles incidents, comme `computeVertexNormals` : deux triangles de la face +z touchent ce coin)
        let n = hull.and_then(|n| n.first().copied()).map(Vec3::from_array);
        let want = Vec3::new(1.0, 1.0, 2.0).normalize();
        assert!(n.is_some_and(|n| (n - want).length() < 1e-5), "{n:?}");
    }

    #[test]
    fn normales_lissees_d_une_sphere() {
        let s = g3::sphere(1.0, 12, 8);
        let tris = s.triangle_vertices();
        let n = smooth_normals(&tris);
        for (p, n) in tris.iter().zip(&n) {
            assert!(n.dot(p.normalize()) > 0.97, "couture visible en {p}");
        }
    }

    #[test]
    fn couleurs_css_lineaires() {
        let c = css("#ffffff");
        assert_eq!(c, LinearRgba::WHITE);
        assert!(css("#808080").red < 0.25, "sRGB → linéaire");
    }
}
