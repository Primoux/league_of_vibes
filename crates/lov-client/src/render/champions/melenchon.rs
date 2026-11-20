//! Caricature originale de Mélenchon, sans texture : costume, lunettes et gestuelle de tribun. Vue sur
//! mesure, hors du squelette commun des champions : huit groupes rigides (corps, buste, tête, bras,
//! jambes, auréole) que `Melenchon::step` anime.
//!
//! Portage de `packages/client/src/render/champions/melenchon.ts`. Comme dans le client web, les pièces
//! fixes d'un groupe sont fusionnées par matériau (un appel de dessin par couleur et par groupe) ; la
//! bouche et la cravate restent des pièces à part, animées. Les noms des pièces sont gardés dans
//! `pieces()` pour les tests.

use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use lov_shared::types::{AbilitySlot, EntityId, EntitySnapshot, GameEvent, TeamOrNeutral};

use super::builder::smooth_normals;
use super::portrait::geometry::{Ring, loft, strand};
use super::portrait::three::{Extrude, extrude};
use crate::net::GameState;
use crate::render::models::geometry::{self as g3, Geo};
use crate::render::models::kit::euler;
use crate::render::views::EntityView;

/// Identifiant du champion qui prend cette vue.
pub const ID: &str = "melenchon";

// ---------------------------------------------------------------- matériaux

/// Matériaux de la caricature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mat {
    Navy,
    Lapel,
    Trousers,
    Skin,
    Warm,
    White,
    Red,
    Hair,
    HairLight,
    Silver,
    Dark,
    Black,
    Blue,
    Team,
}

const MATS: [Mat; 14] = [
    Mat::Navy,
    Mat::Lapel,
    Mat::Trousers,
    Mat::Skin,
    Mat::Warm,
    Mat::White,
    Mat::Red,
    Mat::Hair,
    Mat::HairLight,
    Mat::Silver,
    Mat::Dark,
    Mat::Black,
    Mat::Blue,
    Mat::Team,
];

impl Mat {
    fn color(self, team: TeamOrNeutral) -> u32 {
        match self {
            Mat::Navy => 0x20304a,
            Mat::Lapel => 0x304566,
            Mat::Trousers => 0x192638,
            Mat::Skin => 0xdcae93,
            Mat::Warm => 0xc28b75,
            Mat::White => 0xf1eee6,
            Mat::Red => 0xce263b,
            Mat::Hair => 0xaaa9a5,
            Mat::HairLight => 0xc2c1bc,
            Mat::Silver => 0xd5d3cb,
            Mat::Dark => 0x302e30,
            Mat::Black => 0x18202a,
            Mat::Blue => 0x195dce,
            Mat::Team => match team {
                TeamOrNeutral::Red => 0xff4a4a,
                TeamOrNeutral::Blue => 0x3a7bff,
                TeamOrNeutral::Neutral => 0xc9a227,
            },
        }
    }

    fn metalness(self) -> f32 {
        if self == Mat::Silver { 0.65 } else { 0.0 }
    }
}

fn hex(c: u32) -> Color {
    Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// `MeshStandardMaterial` d'un matériau ; l'hologramme est cyan, translucide et sans écriture de
/// profondeur.
pub fn material(m: Mat, hologram: bool, team: TeamOrNeutral) -> StandardMaterial {
    let metal = m.metalness();
    StandardMaterial {
        base_color: if hologram {
            hex(0x55ddff).with_alpha(0.32)
        } else {
            hex(m.color(team))
        },
        perceptual_roughness: if metal > 0.0 { 0.28 } else { 0.62 },
        metallic: metal,
        emissive: if hologram {
            LinearRgba::from(hex(0x167b9c)) * 0.7
        } else {
            LinearRgba::BLACK
        },
        alpha_mode: if hologram {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

// ---------------------------------------------------------------- géométries

/// Géométries partagées par les pièces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sphere,
    Box,
    Cylinder,
    Torus,
    Face,
    Jacket,
    Lapel,
    Tie,
}

/// Panneau plat extrudé (revers, cravate) : profondeur 1 le long de +z.
fn front_panel(points: &[[f32; 2]]) -> Geo {
    extrude(
        points,
        Extrude {
            depth: 1.0,
            bevel_enabled: false,
            bevel_thickness: 0.0,
            bevel_size: 0.0,
            bevel_segments: 0,
            steps: 1,
        },
    )
}

fn rings(v: &[[f32; 3]]) -> Vec<Ring> {
    v.iter().map(|r| Ring::new(r)).collect()
}

/// Géométrie d'une forme et ses normales par sommet de triangle (celles de three.js : analytiques pour
/// la sphère et le cylindre, par face pour la boîte et les panneaux, lissées pour le reste).
fn shape(kind: Kind) -> (Vec<Vec3>, Vec<Vec3>) {
    let flat = |g: Geo| {
        let tris = g.triangle_vertices();
        let normals = tris
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|t| [(t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero(); 3])
            .collect();
        (tris, normals)
    };
    let smooth = |g: Geo| {
        let tris = g.triangle_vertices();
        let normals = smooth_normals(&tris);
        (tris, normals)
    };
    match kind {
        Kind::Sphere => {
            let tris = g3::sphere(1.0, 14, 10).triangle_vertices();
            let normals = tris.iter().map(|p| p.normalize_or_zero()).collect();
            (tris, normals)
        }
        Kind::Box => flat(g3::cuboid(1.0, 1.0, 1.0)),
        Kind::Cylinder => {
            let tris = g3::cylinder(1.0, 1.0, 1.0, 12).triangle_vertices();
            let mut normals = Vec::with_capacity(tris.len());
            for t in tris.as_chunks::<3>().0 {
                if t[0].y == t[1].y && t[1].y == t[2].y {
                    normals.extend([Vec3::Y * t[0].y.signum(); 3]); // couvercle
                } else {
                    normals.extend(t.map(|p| Vec3::new(p.x, 0.0, p.z).normalize_or_zero()));
                }
            }
            (tris, normals)
        }
        Kind::Torus => smooth(g3::torus(1.0, 0.026, 4, 20)),
        Kind::Face => smooth(loft(
            &rings(&[
                [-1.0, 0.38, 0.4],
                [-0.78, 0.7, 0.78],
                [-0.4, 0.92, 0.94],
                [0.12, 0.98, 0.95],
                [0.53, 0.94, 0.9],
                [0.85, 0.77, 0.74],
                [1.0, 0.3, 0.3],
            ]),
            24,
            true,
        )),
        Kind::Jacket => smooth(loft(
            &rings(&[
                [-1.0, 0.78, 0.7],
                [-0.75, 0.95, 0.95],
                [-0.05, 0.95, 1.0],
                [0.6, 0.95, 0.95],
                [0.95, 0.8, 0.7],
                [1.0, 0.52, 0.45],
            ]),
            20,
            true,
        )),
        Kind::Lapel => flat(front_panel(&[
            [-0.5, 0.5],
            [0.5, 0.3],
            [0.4, -0.5],
            [-0.35, -0.14],
        ])),
        Kind::Tie => flat(front_panel(&[
            [-0.3, 0.5],
            [0.3, 0.5],
            [0.5, -0.35],
            [0.0, -0.5],
            [-0.5, -0.35],
        ])),
    }
}

// ---------------------------------------------------------------- pièces

/// Groupes rigides de la caricature ; `Rig` porte tout le reste.
/// Bras et jambes gardent l'indice du client web : 0 du côté x < 0 de la caricature (sa droite, +z en
/// jeu), 1 du côté de l'index accusateur.
pub mod g {
    pub const RIG: usize = 0;
    pub const TORSO: usize = 1;
    pub const HEAD: usize = 2;
    pub const ARM_0: usize = 3;
    pub const ARM_1: usize = 4;
    pub const LEG_0: usize = 5;
    pub const LEG_1: usize = 6;
    pub const REPUBLIC: usize = 7;
}
pub const GROUPS: usize = 8;

/// Parent de chaque groupe (`None` : la vue elle-même).
pub const PARENT: [Option<usize>; GROUPS] = [
    None,
    Some(g::RIG),
    Some(g::TORSO),
    Some(g::TORSO),
    Some(g::TORSO),
    Some(g::RIG),
    Some(g::RIG),
    Some(g::RIG),
];

/// Pièce : forme posée dans son groupe.
#[derive(Clone, Debug)]
pub struct Piece {
    pub name: &'static str,
    pub group: usize,
    pub mat: Mat,
    pub geo: PieceGeo,
    pub transform: Transform,
}

/// Forme partagée, ou mèche propre à la pièce.
#[derive(Clone, Debug)]
pub enum PieceGeo {
    Shared(Kind),
    Detail(Geo),
}

/// Pièces animées à part (hors des lots fusionnés).
pub const MOUTH: &str = "bouche";
pub const TIE: &str = "cravate-rouge";

fn tr(pos: [f32; 3], scale: [f32; 3]) -> Transform {
    Transform::from_translation(Vec3::from_array(pos)).with_scale(Vec3::from_array(scale))
}

/// Accumulateur des pièces.
struct Out(Vec<Piece>);

impl Out {
    fn mesh(&mut self, group: usize, name: &'static str, kind: Kind, mat: Mat, t: Transform) {
        self.0.push(Piece {
            name,
            group,
            mat,
            geo: PieceGeo::Shared(kind),
            transform: t,
        });
    }

    fn detail(&mut self, group: usize, name: &'static str, geo: Geo, mat: Mat) {
        self.0.push(Piece {
            name,
            group,
            mat,
            geo: PieceGeo::Detail(geo),
            transform: Transform::IDENTITY,
        });
    }
}

fn rot(t: Transform, x: f32, z: f32) -> Transform {
    t.with_rotation(euler(Vec3::new(x, 0.0, z)))
}

/// Toutes les pièces, dans l'ordre du client web.
pub fn pieces() -> Vec<Piece> {
    use Kind::*;
    use Mat::*;
    let mut o = Out(Vec::new());
    let ring = tr([0.0, 0.035, 0.0], [0.57; 3]);
    o.mesh(
        g::RIG,
        "anneau-equipe",
        Torus,
        Team,
        rot(ring, FRAC_PI_2, 0.0),
    );
    let t = tr([0.0, 0.36, 0.0], [0.42, 0.58, 0.26]);
    o.mesh(g::TORSO, "veste-croisee", Jacket, Navy, t);
    let t = tr([0.0, 0.65, 0.242], [0.25, 0.46, 0.04]);
    o.mesh(g::TORSO, "chemise", Box, White, t);
    for side in [-1.0, 1.0] {
        let t = tr([side * 0.15, 0.61, 0.254], [side * 0.15, 0.46, 0.025]);
        o.mesh(
            g::TORSO,
            "revers",
            Kind::Lapel,
            Mat::Lapel,
            rot(t, 0.0, side * -0.3),
        );
        let t = tr([side * 0.066, 0.84, 0.25], [0.105, 0.13, 0.035]);
        o.mesh(g::TORSO, "col", Box, White, rot(t, 0.0, side * 0.45));
    }
    let t = tr([0.0, 0.78, 0.29], [0.055, 0.054, 0.027]);
    o.mesh(g::TORSO, "noeud-cravate", Sphere, Red, t);
    let t = tr([0.0, 0.53, 0.288], [0.085, 0.4, 0.025]);
    o.mesh(g::TORSO, TIE, Tie, Red, rot(t, 0.0, -0.05));
    let t = tr([0.04, 0.23, 0.26], [0.018, 0.018, 0.012]);
    o.mesh(g::TORSO, "bouton", Sphere, Silver, t);
    let t = tr([-0.28, 0.69, 0.245], [0.08, 0.025, 0.018]);
    o.mesh(g::TORSO, "pochette", Box, White, t);
    let t = tr([0.0, 0.98, 0.0], [0.13, 0.2, 0.12]);
    o.mesh(g::TORSO, "cou", Cylinder, Skin, t);
    // Visage large, joues basses, front haut et nez proéminent : lisible même en vue de jeu.
    let h = g::HEAD;
    o.mesh(h, "visage", Face, Skin, tr([0.0; 3], [0.32, 0.4, 0.265]));
    let t = tr([0.0, -0.23, 0.025], [0.26, 0.17, 0.215]);
    o.mesh(h, "machoire", Sphere, Skin, t);
    for side in [-1.0, 1.0] {
        let t = tr([side * 0.322, -0.025, 0.0], [0.06, 0.11, 0.055]);
        o.mesh(h, "oreille", Sphere, Warm, t);
        let t = tr([side * 0.17, -0.1, 0.175], [0.11, 0.105, 0.04]);
        o.mesh(h, "joue", Sphere, Skin, t);
        let t = tr([side * 0.139, 0.038, 0.25], [0.071, 0.029, 0.015]);
        o.mesh(h, "oeil", Sphere, White, t);
        let t = tr([side * 0.133, 0.039, 0.267], [0.018, 0.02, 0.008]);
        o.mesh(h, "regard", Sphere, Dark, t);
        let t = tr([side * 0.139, 0.068, 0.255], [0.078, 0.016, 0.02]);
        o.mesh(h, "paupiere", Sphere, Skin, t);
        let t = tr([side * 0.133 - 0.005, 0.046, 0.276], [0.005, 0.005, 0.002]);
        o.mesh(h, "reflet-regard", Sphere, White, t);
        let t = tr([side * 0.139, 0.117, 0.252], [0.16, 0.027, 0.024]);
        o.mesh(h, "sourcil-indigne", Box, Dark, rot(t, 0.0, side * 0.13));
        // Quatre branches par verre : silhouette rectangulaire plutôt que lunettes rondes génériques.
        for y in [-0.005, 0.084] {
            let t = tr([side * 0.145, y, 0.285], [0.2, 0.009, 0.012]);
            o.mesh(h, "lunettes-monture", Box, Silver, t);
        }
        for x in [0.045, 0.245] {
            let t = tr([side * x, 0.04, 0.285], [0.009, 0.089, 0.012]);
            o.mesh(h, "lunettes-monture", Box, Silver, t);
        }
        let t = tr([side * 0.28, 0.075, 0.13], [0.012, 0.015, 0.3]);
        o.mesh(h, "lunettes-branche", Box, Silver, t);
        let t = tr([side * 0.108, -0.165, 0.268], [0.011, 0.1, 0.008]);
        o.mesh(h, "pli-nasolabial", Box, Warm, rot(t, 0.0, side * 0.3));
        let t = tr([side * 0.283, 0.15, -0.035], [0.065, 0.19, 0.2]);
        o.mesh(h, "tempe-grise", Sphere, Hair, t);
    }
    let t = tr([0.0, 0.055, 0.299], [0.06, 0.013, 0.014]);
    o.mesh(h, "lunettes-pont", Box, Silver, t);
    let t = tr([0.0, -0.035, 0.294], [0.066, 0.13, 0.105]);
    o.mesh(h, "nez", Sphere, Skin, rot(t, -0.18, 0.0));
    let t = tr([0.0, -0.112, 0.333], [0.073, 0.043, 0.058]);
    o.mesh(h, "pointe-nez", Sphere, Warm, t);
    let t = tr([0.0, -0.238, 0.273], [0.11, 0.016, 0.012]);
    o.mesh(h, MOUTH, Sphere, Dark, t);
    let t = tr([0.0, -0.263, 0.268], [0.1, 0.019, 0.016]);
    o.mesh(h, "levre", Sphere, Warm, t);
    // Masse de cheveux rejetée en arrière et mèches séparées (front dégagé).
    let t = tr([0.0, 0.22, -0.1], [0.32, 0.205, 0.24]);
    o.mesh(h, "cheveux-arriere", Sphere, Hair, t);
    for i in 0..9 {
        let x = (i as f32 - 4.0) * 0.06;
        let lift = (4 - (i - 4_i32).abs()) as f32 * 0.007;
        let geo = strand(
            &[
                [x - 0.02, 0.24, 0.15],
                [x + 0.025, 0.35 + lift, 0.07],
                [x + 0.02, 0.34, -0.13],
                [x, 0.14, -0.245],
            ],
            &[0.025, 0.036, 0.03, 0.006],
            6,
            10,
        );
        let mat = if i % 2 != 0 { HairLight } else { Hair };
        o.detail(h, "meche-coiffee-en-arriere", geo, mat);
    }
    for (arm, leg, right) in [(g::ARM_0, g::LEG_0, false), (g::ARM_1, g::LEG_1, true)] {
        let t = tr([0.0, -0.38, 0.0], [0.135, 0.75, 0.14]);
        o.mesh(leg, "pantalon", Cylinder, Trousers, t);
        let t = tr([0.0, -0.8, 0.065], [0.145, 0.1, 0.24]);
        o.mesh(leg, "chaussure", Sphere, Black, t);
        o.mesh(
            arm,
            "epaule",
            Sphere,
            Navy,
            tr([0.0; 3], [0.12, 0.13, 0.12]),
        );
        let t = tr([0.0, -0.27, 0.0], [0.12, 0.54, 0.12]);
        o.mesh(arm, "manche", Cylinder, Navy, t);
        let t = tr([0.0, -0.56, 0.0], [0.1, 0.065, 0.1]);
        o.mesh(arm, "poignet", Cylinder, White, t);
        let t = tr([0.0, -0.66, 0.01], [0.105, 0.125, 0.085]);
        o.mesh(arm, "main", Sphere, Skin, t);
        let geo = strand(
            &[
                [-0.07, -0.62, 0.025],
                [-0.11, -0.67, 0.055],
                [-0.075, -0.72, 0.06],
            ],
            &[0.035, 0.03, 0.022],
            6,
            10,
        );
        o.detail(arm, "pouce", geo, Skin);
        for f in 0..3 {
            let x = (f as f32 - 1.0) * 0.05;
            let geo = strand(
                &[[x, -0.7, 0.035], [x, -0.765, 0.065], [x, -0.75, 0.09]],
                &[0.025, 0.024, 0.018],
                5,
                6,
            );
            o.detail(arm, "doigt-replie", geo, Skin);
        }
        if right {
            // L'index fait partie de la silhouette ; rotations du bras = doigt accusateur.
            let t = tr([0.046, -0.83, 0.018], [0.028, 0.24, 0.028]);
            o.mesh(arm, "index-accusateur", Cylinder, Skin, t);
        } else {
            let t = tr([0.0, -0.67, 0.1], [0.23, 0.3, 0.055]);
            o.mesh(arm, "dossier-parlementaire", Box, Red, t);
            let t = tr([0.0, -0.67, 0.133], [0.2, 0.27, 0.012]);
            o.mesh(arm, "feuilles", Box, White, t);
        }
    }
    for (i, mat) in [Blue, White, Red].into_iter().enumerate() {
        let s = 0.7 + i as f32 * 0.09;
        let t = tr([0.0, 0.06 + i as f32 * 0.07, 0.0], [s; 3]);
        o.mesh(
            g::REPUBLIC,
            "aureole-republicaine",
            Torus,
            mat,
            rot(t, FRAC_PI_2, 0.0),
        );
    }
    o.0
}

// ---------------------------------------------------------------- lots fusionnés

/// Triangles et normales d'une pièce, dans le repère de son groupe (`applyMatrix4`, enroulement rétabli
/// si l'échelle retourne la pièce).
fn bake(piece: &Piece) -> (Vec<Vec3>, Vec<Vec3>) {
    let (mut tris, mut normals) = match &piece.geo {
        PieceGeo::Shared(kind) => shape(*kind),
        PieceGeo::Detail(geo) => {
            let tris = geo.triangle_vertices();
            let normals = smooth_normals(&tris);
            (tris, normals)
        }
    };
    let m = piece.transform.to_matrix();
    let nm = Mat3::from_mat4(m).inverse().transpose();
    for p in &mut tris {
        *p = m.transform_point3(*p);
    }
    for n in &mut normals {
        *n = (nm * *n).normalize_or_zero();
    }
    if m.determinant() < 0.0 {
        for t in tris.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
        for t in normals.as_chunks_mut::<3>().0 {
            t.swap(1, 2);
        }
    }
    (tris, normals)
}

fn mesh_of(tris: Vec<Vec3>, normals: Vec<Vec3>) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, tris)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

/// Lot fusionné : pièces fixes d'un groupe qui partagent un matériau.
pub struct Batch {
    pub group: usize,
    pub mat: Mat,
    pub tris: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

/// Lots fusionnés, dans l'ordre d'apparition des matériaux de chaque groupe (bouche et cravate exclues).
pub fn batches(pieces: &[Piece]) -> Vec<Batch> {
    let mut out: Vec<Batch> = Vec::new();
    for group in 0..GROUPS {
        let start = out.len();
        for p in pieces.iter().filter(|p| p.group == group) {
            if p.name == MOUTH || p.name == TIE {
                continue;
            }
            let (tris, normals) = bake(p);
            match out[start..].iter_mut().find(|b| b.mat == p.mat) {
                Some(b) => {
                    b.tris.extend(tris);
                    b.normals.extend(normals);
                }
                None => out.push(Batch {
                    group,
                    mat: p.mat,
                    tris,
                    normals,
                }),
            }
        }
    }
    out
}

// ---------------------------------------------------------------- animation

const ARM_X: f32 = 0.42;
const LEG_X: f32 = 0.18;
const TORSO_Y: f32 = 0.92;
const HEAD_POS: Vec3 = Vec3::new(0.0, 1.28, 0.015);

/// Ce que la frame doit écrire dans le monde.
#[derive(Clone, Debug, PartialEq)]
pub struct MelenchonFrame {
    /// Transformation locale de chaque groupe.
    pub groups: [Transform; GROUPS],
    pub mouth: Transform,
    pub tie: Transform,
    pub rig_visible: bool,
    pub republic_visible: bool,
}

/// État d'animation de la caricature.
#[derive(Clone, Debug, Default)]
pub struct Melenchon {
    anim: String,
    age: f32,
    time: f32,
    death_time: f32,
    recalling: bool,
    /// Dernière animation reçue du serveur (seuls les changements la rejouent).
    played: String,
}

impl Melenchon {
    pub fn new() -> Self {
        Self {
            anim: "idle".into(),
            ..default()
        }
    }

    pub fn anim(&self) -> &str {
        &self.anim
    }

    pub fn play_anim(&mut self, name: &str) {
        // Un snapshot idle ne coupe pas un geste après seulement un tick serveur.
        if name == "idle" && self.anim.starts_with("cast_") && self.age < 0.75 {
            return;
        }
        if name != self.anim || name.starts_with("cast_") || name == "attack" {
            self.anim = name.to_owned();
            self.age = 0.0;
        }
    }

    pub fn strike(&mut self) {
        self.play_anim("attack");
    }

    pub fn cast(&mut self, slot: &str) {
        self.play_anim(&format!("cast_{slot}"));
    }

    pub fn set_recalling(&mut self, on: bool) {
        self.recalling = on;
    }

    /// Une frame ; `anim` est l'animation du snapshot (`death` si l'entité est morte).
    pub fn step(&mut self, snap: &EntitySnapshot, anim: &str, dt: f32) -> MelenchonFrame {
        if anim != self.played {
            self.played = anim.to_owned();
            self.play_anim(anim);
        }
        let dead = snap.dead == Some(true);
        self.time += dt;
        self.age += dt;
        self.death_time = if dead { self.death_time + dt } else { 0.0 };
        if dead || self.anim == "run" {
            self.recalling = false;
        }
        let ult = snap.buffs.iter().flatten().any(|b| b.id == "melenchon_r");
        let declaim = !dead && (ult || (self.anim == "cast_R" && self.age < 1.3));
        let point = !dead && (self.anim == "cast_Q" || self.anim == "attack") && self.age < 0.85;
        let run = !dead && self.anim == "run";
        let t = self.time;
        let gait = if run { (t * 11.0).sin() * 0.55 } else { 0.0 };
        let at = |x: f32, y: f32, z: f32| Transform::from_xyz(x, y, z);
        let rotated =
            |tr: Transform, x: f32, y: f32, z: f32| tr.with_rotation(euler(Vec3::new(x, y, z)));
        let mut groups = [Transform::IDENTITY; GROUPS];
        groups[g::RIG] = Transform::from_xyz(0.0, if dead { -0.6 } else { 0.0 }, 0.0)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
            .with_scale(Vec3::splat(if ult { 1.12 } else { 1.0 }));
        let torso_y = TORSO_Y
            + if run {
                gait.abs() * 0.05
            } else {
                (t * 2.0).sin() * 0.012
            };
        let torso_x = if dead {
            -1.2
        } else if self.anim == "cast_W" && self.age < 0.7 {
            0.15
        } else {
            0.0
        };
        groups[g::TORSO] = rotated(at(0.0, torso_y, 0.0), torso_x, 0.0, 0.0);
        let head_x = if declaim {
            -0.16
        } else if self.recalling {
            0.15
        } else {
            0.02 * (t * 1.5).sin()
        };
        groups[g::HEAD] = Transform::from_translation(HEAD_POS).with_rotation(euler(Vec3::new(
            head_x,
            if point { -0.1 } else { 0.0 },
            0.0,
        )));
        let (a0x, a0z) = if declaim {
            (-2.45, -0.45)
        } else {
            (-gait * 0.6, 0.09)
        };
        let a1x = if declaim {
            -2.8
        } else if point {
            -1.85
        } else {
            gait * 0.6
        };
        let a1z = if declaim { 0.35 } else { -0.1 };
        groups[g::ARM_0] = rotated(at(-ARM_X, 0.77, 0.0), a0x, 0.0, a0z);
        groups[g::ARM_1] = rotated(at(ARM_X, 0.77, 0.0), a1x, 0.0, a1z);
        groups[g::LEG_0] = rotated(at(-LEG_X, 0.88, 0.0), gait, 0.0, 0.0);
        groups[g::LEG_1] = rotated(at(LEG_X, 0.88, 0.0), -gait, 0.0, 0.0);
        groups[g::REPUBLIC] = Transform::from_rotation(Quat::from_rotation_y(t * 0.65));
        let tie_x = (t * if run { 11.0 } else { 2.0 }).sin() * if run { 0.22 } else { 0.025 };
        let tie = rotated(at(0.0, 0.53, 0.288), tie_x, 0.0, -0.05)
            .with_scale(Vec3::new(0.085, 0.4, 0.025));
        let mouth_y = if declaim {
            0.058 + (t * 16.0).sin() * 0.012
        } else {
            0.016
        };
        let mouth = at(0.0, -0.238, 0.273).with_scale(Vec3::new(0.11, mouth_y, 0.012));
        if self.anim.starts_with("cast_") && self.age > 1.4 {
            self.anim = "idle".into();
        }
        MelenchonFrame {
            groups,
            mouth,
            tie,
            rig_visible: self.death_time < 1.4,
            republic_visible: ult,
        }
    }
}

/// Hauteur de la caricature (`height`).
pub const HEIGHT: f32 = 2.65;

// ---------------------------------------------------------------- monde

/// Vue de Mélenchon : état d'animation et entités animées.
#[derive(Component, Clone, Debug)]
pub struct MelenchonView {
    pub id: EntityId,
    pub state: Melenchon,
    groups: [Entity; GROUPS],
    mouth: Entity,
    tie: Entity,
}

/// Fait apparaître la caricature sous `view` (un hologramme est translucide et sans ombre).
pub fn spawn_melenchon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    view: Entity,
    id: EntityId,
    team: TeamOrNeutral,
    hologram: bool,
) -> MelenchonView {
    let mats: Vec<(Mat, Handle<StandardMaterial>)> = MATS
        .iter()
        .map(|m| (*m, materials.add(material(*m, hologram, team))))
        .collect();
    let handle = |m: Mat| {
        mats.iter()
            .find(|(k, _)| *k == m)
            .map(|(_, h)| h.clone())
            .unwrap_or_default()
    };
    let mut state = Melenchon::new();
    let frame = state.step(&rest_snapshot(id), "idle", 0.0);
    let mut groups = [Entity::PLACEHOLDER; GROUPS];
    for gi in 0..GROUPS {
        let parent = PARENT[gi].map_or(view, |p| groups[p]);
        let visible = if gi == g::REPUBLIC {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        groups[gi] = commands
            .spawn((frame.groups[gi], visible, ChildOf(parent)))
            .id();
    }
    let mut spawn =
        |commands: &mut Commands, mesh: Mesh, mat: Mat, parent: Entity, t: Transform| {
            let mut e = commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(handle(mat)),
                t,
                ChildOf(parent),
            ));
            if hologram {
                e.insert(NotShadowCaster);
            }
            e.id()
        };
    let all = pieces();
    for b in batches(&all) {
        spawn(
            commands,
            mesh_of(b.tris, b.normals),
            b.mat,
            groups[b.group],
            Transform::IDENTITY,
        );
    }
    let unit = |kind: Kind| {
        let (tris, normals) = shape(kind);
        mesh_of(tris, normals)
    };
    let mouth = spawn(
        commands,
        unit(Kind::Sphere),
        Mat::Dark,
        groups[g::HEAD],
        frame.mouth,
    );
    let tie = spawn(
        commands,
        unit(Kind::Tie),
        Mat::Red,
        groups[g::TORSO],
        frame.tie,
    );
    MelenchonView {
        id,
        state,
        groups,
        mouth,
        tie,
    }
}

/// Snapshot minimal, vivant, sans buff (pose de départ).
fn rest_snapshot(id: EntityId) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind: lov_shared::types::EntityKind::Champion,
        team: TeamOrNeutral::Neutral,
        x: 0.0,
        y: 0.0,
        facing: 0.0,
        hp: 1.0,
        max_hp: 1.0,
        mana: None,
        max_mana: None,
        shield: None,
        dead: None,
        subtype: Some(ID.into()),
        owner_id: None,
        level: None,
        anim: None,
        buffs: None,
        champ: None,
    }
}

/// Anime les caricatures d'après leur snapshot.
pub fn animate_melenchon(
    time: Res<Time>,
    state: Res<GameState>,
    mut views: Query<(&EntityView, &mut MelenchonView)>,
    mut transforms: Query<&mut Transform, Without<MelenchonView>>,
    mut visibility: Query<&mut Visibility>,
) {
    let dt = time.delta_secs().min(0.1);
    for (view, mut m) in &mut views {
        let Some(snap) = state.entities().get(&m.id) else {
            continue;
        };
        let f = m.state.step(snap, &view.anim, dt);
        let targets = m
            .groups
            .iter()
            .zip(f.groups)
            .chain([(&m.mouth, f.mouth), (&m.tie, f.tie)]);
        for (e, t) in targets {
            if let Ok(mut tr) = transforms.get_mut(*e) {
                *tr = t;
            }
        }
        let shown = |on: bool| {
            if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        };
        for (gi, on) in [(g::RIG, f.rig_visible), (g::REPUBLIC, f.republic_visible)] {
            if let Ok(mut v) = visibility.get_mut(m.groups[gi]) {
                v.set_if_neq(shown(on));
            }
        }
    }
}

/// Gestes reçus du serveur (frappe, sort, rappel) ; la caricature ne recule pas sous les coups.
pub fn route(m: &mut MelenchonView, ev: &GameEvent) {
    match ev {
        GameEvent::AutoAttack { .. } => m.state.strike(),
        GameEvent::AbilityCast { slot, .. } => m.state.cast(match slot {
            AbilitySlot::Q => "Q",
            AbilitySlot::W => "W",
            AbilitySlot::E => "E",
            AbilitySlot::R => "R",
        }),
        GameEvent::RecallStart { .. } => m.state.set_recalling(true),
        GameEvent::RecallEnd { .. } => m.state.set_recalling(false),
        _ => {}
    }
}

/// Portage de `packages/client/test/melenchon-model.test.ts`.
#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::BuffSnapshot;

    fn snap() -> EntitySnapshot {
        EntitySnapshot {
            team: TeamOrNeutral::Blue,
            hp: 1000.0,
            max_hp: 1000.0,
            ..rest_snapshot(1)
        }
    }

    /// Matrice monde de chaque groupe pour une frame (la vue à l'origine).
    fn world(f: &MelenchonFrame) -> [Mat4; GROUPS] {
        let mut w = [Mat4::IDENTITY; GROUPS];
        for gi in 0..GROUPS {
            let parent = PARENT[gi].map_or(Mat4::IDENTITY, |p| w[p]);
            w[gi] = parent * f.groups[gi].to_matrix();
        }
        w
    }

    fn piece<'a>(all: &'a [Piece], name: &str) -> &'a Piece {
        let Some(p) = all.iter().find(|p| p.name == name) else {
            panic!("pièce {name} absente");
        };
        p
    }

    #[test]
    fn recoit_les_gestes_du_bus() {
        let mut v = MelenchonView {
            id: 1,
            state: Melenchon::new(),
            groups: [Entity::PLACEHOLDER; GROUPS],
            mouth: Entity::PLACEHOLDER,
            tie: Entity::PLACEHOLDER,
        };
        let cast = GameEvent::AbilityCast {
            source: 1,
            champion_id: ID.into(),
            slot: AbilitySlot::R,
            ability_id: "melenchon_r".into(),
            target_pos: None,
            target_id: None,
            fx_key: None,
        };
        route(&mut v, &cast);
        let f = v.state.step(&snap(), "idle", 0.1);
        assert!(f.mouth.scale.y > 0.04);
    }

    #[test]
    fn silhouette_lisible_dans_le_budget_geometrique() {
        let all = pieces();
        for name in [
            "index-accusateur",
            "cravate-rouge",
            "lunettes-monture",
            "meche-coiffee-en-arriere",
            "nez",
        ] {
            piece(&all, name);
        }
        let triangles: usize = all.iter().map(|p| bake(p).0.len() / 3).sum();
        assert!(triangles < 14_000, "{triangles} triangles");
        // les lots fusionnés gardent tous les triangles (bouche et cravate à part)
        let batched: usize = batches(&all).iter().map(|b| b.tris.len() / 3).sum();
        let apart: usize = [MOUTH, TIE]
            .map(|n| bake(piece(&all, n)).0.len() / 3)
            .iter()
            .sum();
        assert_eq!(batched + apart, triangles);
        let f = Melenchon::new().step(&snap(), "idle", 0.0);
        let w = world(&f);
        let top = all
            .iter()
            .filter(|p| p.group != g::REPUBLIC)
            .flat_map(|p| {
                let m = w[p.group];
                bake(p).0.into_iter().map(move |v| m.transform_point3(v).y)
            })
            .fold(f32::MIN, f32::max);
        assert!(top > 2.4 && top < 3.0, "sommet à {top}");
    }

    #[test]
    fn regarde_x_anime_son_geste_et_revient_vivant_apres_une_mort() {
        let all = pieces();
        let mut m = Melenchon::new();
        let f = m.step(&snap(), "idle", 0.0);
        let nose = piece(&all, "nez").transform.translation;
        assert!(world(&f)[g::HEAD].transform_point3(nose).x > 0.25);
        m.play_anim("cast_R");
        let f = m.step(&snap(), "idle", 0.1);
        assert!(f.mouth.scale.y > 0.04);
        let mut dead = snap();
        dead.dead = Some(true);
        m.step(&dead, "death", 0.1);
        let f = m.step(&snap(), "idle", 2.0);
        assert_eq!(f.groups[g::RIG].translation.y, 0.0);
        assert!(f.rig_visible && !f.republic_visible);
        let mut ult = snap();
        ult.buffs = Some(vec![BuffSnapshot {
            id: "melenchon_r".into(),
            remaining: 2.0,
            stacks: None,
        }]);
        assert!(m.step(&ult, "idle", 0.1).republic_visible);
    }

    #[test]
    fn disparait_apres_la_chute() {
        let mut m = Melenchon::new();
        let mut dead = snap();
        dead.dead = Some(true);
        let f = m.step(&dead, "death", 1.0);
        assert!(f.rig_visible);
        assert_eq!(f.groups[g::RIG].translation.y, -0.6);
        assert!(!m.step(&dead, "death", 0.5).rig_visible);
    }

    #[test]
    fn hologramme_translucide() {
        let holo = material(Mat::Skin, true, TeamOrNeutral::Blue);
        assert_eq!(holo.alpha_mode, AlphaMode::Blend);
        assert!((holo.base_color.alpha() - 0.32).abs() < 1e-6);
        let plain = material(Mat::Skin, false, TeamOrNeutral::Blue);
        assert_eq!(plain.alpha_mode, AlphaMode::Opaque);
        assert_eq!(plain.emissive, LinearRgba::BLACK);
        // l'anneau d'équipe prend la couleur de l'équipe
        let red = material(Mat::Team, false, TeamOrNeutral::Red);
        assert_eq!(red.base_color, hex(0xff4a4a));
    }

    #[test]
    fn revers_retourne_sans_inverser_ses_faces() {
        // le revers gauche a une échelle négative : ses normales sortent toujours du panneau
        let all = pieces();
        let lapels: Vec<_> = all.iter().filter(|p| p.name == "revers").collect();
        assert_eq!(lapels.len(), 2);
        for p in lapels {
            let (tris, normals) = bake(p);
            for (t, n) in tris
                .as_chunks::<3>()
                .0
                .iter()
                .zip(normals.as_chunks::<3>().0)
            {
                let geometric = (t[1] - t[0]).cross(t[2] - t[0]);
                assert!(geometric.dot(n[0]) >= -1e-9);
            }
        }
    }
}
