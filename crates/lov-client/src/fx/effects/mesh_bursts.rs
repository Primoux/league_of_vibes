//! Meshes animés : pics qui jaillissent du sol puis s'enfoncent, débris balistiques qui rebondissent, blocs
//! statiques (prison de glace). Pierre (Grondak), glace (Ilyra), éclats divers.
//!
//! Portage de `packages/client/src/fx/effects/meshBursts.ts`. Le web dessine chaque système en un
//! `InstancedMesh` ; ici chaque instance est une entité qui partage le maillage et le matériau de son
//! système : Bevy les regroupe lui-même en un seul appel de dessin (instanciation automatique). La couleur
//! par instance (`setColorAt`) devient un matériau par couleur, mis en cache. Les entités sont créées à la
//! demande, puis recyclées en anneau comme les instances du web ; aucune allocation par image.

use std::collections::HashMap;

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::primitives::{rand, rand_range, rand_sign};
use crate::fx::core::runtime::Fx;
use crate::fx::shaders::IceMaterial;

/// Système de meshes (`BurstSystems` du runtime web).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BurstKind {
    RockSpikes,
    RockDebris,
    IceSpikes,
    IceDebris,
}

impl BurstKind {
    const ALL: [BurstKind; 4] = [
        BurstKind::RockSpikes,
        BurstKind::RockDebris,
        BurstKind::IceSpikes,
        BurstKind::IceDebris,
    ];

    fn capacity(self) -> usize {
        match self {
            BurstKind::RockSpikes | BurstKind::IceSpikes => 96,
            BurstKind::RockDebris | BurstKind::IceDebris => 160,
        }
    }

    fn ice(self) -> bool {
        matches!(self, BurstKind::IceSpikes | BurstKind::IceDebris)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BurstMode {
    #[default]
    Spike,
    Debris,
    Static,
}

/// Une instance à faire apparaître (`BurstSpawn`).
#[derive(Clone, Copy, Debug)]
pub struct BurstSpawn {
    pub pos: Vec3,
    pub mode: BurstMode,
    /// Taille (multiplicateur de la géométrie).
    pub size: f32,
    /// Durée de vie (s).
    pub life: f32,
    /// Vitesse initiale (débris).
    pub vel: Vec3,
    /// Inclinaison d'un pic vers l'extérieur (rad) et direction (rad, autour de y).
    pub tilt: Option<f32>,
    pub heading: Option<f32>,
    /// Étirement vertical (pics).
    pub stretch: f32,
    pub color: Option<Vec3>,
    pub delay: f32,
}

impl Default for BurstSpawn {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            mode: BurstMode::Spike,
            size: 1.0,
            life: 1.0,
            vel: Vec3::ZERO,
            tilt: None,
            heading: None,
            stretch: 1.0,
            color: None,
            delay: 0.0,
        }
    }
}

/// État d'une instance ; `mode == None` = libre.
#[derive(Clone, Copy, Default)]
struct Slot {
    mode: Option<BurstMode>,
    p: Vec3,
    v: Vec3,
    r: Vec3,
    spin: f32,
    age: f32,
    life: f32,
    size: f32,
    stretch: f32,
}

struct BurstSystem {
    kind: BurstKind,
    entities: Vec<Entity>,
    slots: Vec<Slot>,
    head: usize,
    active: usize,
}

/// Couleur → clé de cache (au 1/64 près : assez fin pour les teintes des FX).
fn color_key(c: Vec3) -> [i32; 3] {
    [
        (c.x * 64.0) as i32,
        (c.y * 64.0) as i32,
        (c.z * 64.0) as i32,
    ]
}

/// Gravité des débris (monde/s²).
const GRAVITY: f32 = 14.0;

/// Les quatre systèmes et leurs ressources partagées.
#[derive(Resource, Default)]
pub struct MeshBursts {
    systems: Vec<BurstSystem>,
    meshes: HashMap<BurstKind, Handle<Mesh>>,
    stone: HashMap<[i32; 3], Handle<StandardMaterial>>,
    ice: HashMap<[i32; 3], Handle<IceMaterial>>,
}

impl MeshBursts {
    /// Instances vivantes d'un système.
    pub fn alive(&self, kind: BurstKind) -> usize {
        self.systems
            .iter()
            .find(|s| s.kind == kind)
            .map_or(0, |s| s.active)
    }
}

/// Fait apparaître une instance dans le système `kind`.
pub fn burst(fx: &mut Fx, kind: BurstKind, o: BurstSpawn) {
    let world = fx.world();
    world.resource_scope(|world, mut mb: Mut<MeshBursts>| {
        if mb.systems.is_empty() {
            mb.systems = BurstKind::ALL
                .iter()
                .map(|&kind| BurstSystem {
                    kind,
                    entities: Vec::new(),
                    slots: vec![Slot::default(); kind.capacity()],
                    head: 0,
                    active: 0,
                })
                .collect();
        }
        let color = o.color.unwrap_or(Vec3::ONE);
        let key = color_key(color);
        let mesh = mb
            .meshes
            .entry(kind)
            .or_insert_with(|| {
                world.resource_mut::<Assets<Mesh>>().add(match kind {
                    BurstKind::RockSpikes => spike_geometry(5),
                    BurstKind::RockDebris => rock_geometry(),
                    BurstKind::IceSpikes | BurstKind::IceDebris => shard_geometry(),
                })
            })
            .clone();
        let material = if kind.ice() {
            let h = mb
                .ice
                .entry(key)
                .or_insert_with(|| {
                    // teinte d'instance du web : multiplie toute la couleur (`col * vTint`)
                    let m = IceMaterial::default();
                    let m = IceMaterial::new(m.u.color * color, m.u.deep * color);
                    world.resource_mut::<Assets<IceMaterial>>().add(m)
                })
                .clone();
            MaterialChoice::Ice(h)
        } else {
            let h = mb
                .stone
                .entry(key)
                .or_insert_with(|| {
                    world
                        .resource_mut::<Assets<StandardMaterial>>()
                        .add(StandardMaterial {
                            base_color: Color::LinearRgba(LinearRgba::rgb(
                                color.x, color.y, color.z,
                            )),
                            perceptual_roughness: 0.9,
                            ..default()
                        })
                })
                .clone();
            MaterialChoice::Stone(h)
        };
        let Some(sys) = mb.systems.iter_mut().find(|s| s.kind == kind) else {
            return;
        };
        let i = sys.head;
        sys.head = (sys.head + 1) % sys.slots.len();
        if sys.slots[i].mode.is_none() {
            sys.active += 1;
        }
        let (r, spin) = if o.mode == BurstMode::Debris {
            (
                Vec3::new(rand() * TAU, rand() * TAU, rand() * TAU),
                rand_range(4.0, 12.0) * rand_sign(),
            )
        } else {
            // pic : inclinaison vers l'extérieur selon `heading`, légère rotation propre
            (
                Vec3::new(
                    o.tilt.unwrap_or_else(|| rand_range(-0.15, 0.15)),
                    o.heading.unwrap_or_else(|| rand() * TAU),
                    rand() * TAU,
                ),
                0.0,
            )
        };
        sys.slots[i] = Slot {
            mode: Some(o.mode),
            p: o.pos,
            v: o.vel,
            r,
            spin,
            age: -o.delay,
            life: o.life,
            size: o.size,
            stretch: o.stretch,
        };
        if i >= sys.entities.len() {
            let e = world
                .spawn((
                    Name::new("fx-burst"),
                    Mesh3d(mesh),
                    Transform::from_scale(Vec3::ZERO),
                    Visibility::Hidden,
                    NoFrustumCulling,
                ))
                .id();
            sys.entities.push(e);
        }
        let e = sys.entities[i];
        match material {
            MaterialChoice::Stone(h) => {
                world.entity_mut(e).insert(MeshMaterial3d(h));
            }
            MaterialChoice::Ice(h) => {
                world.entity_mut(e).insert(MeshMaterial3d(h));
            }
        }
    });
}

enum MaterialChoice {
    Stone(Handle<StandardMaterial>),
    Ice(Handle<IceMaterial>),
}

fn back_out(t: f32) -> f32 {
    let c1 = 2.2;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Transformation d'une instance vivante à son âge courant ; met à jour la balistique des débris.
fn step_slot(s: &mut Slot, dt: f32) -> Option<Transform> {
    let mode = s.mode?;
    let t = s.age / s.life;
    let size = s.size;
    let (rot, scale) = match mode {
        BurstMode::Debris => {
            // débris : balistique, rebond amorti, rotation
            s.v.y -= GRAVITY * dt;
            s.p += s.v * dt;
            if s.p.y < size * 0.3 && s.v.y < 0.0 {
                s.p.y = size * 0.3;
                s.v.y = -s.v.y * 0.3;
                s.v.x *= 0.6;
                s.v.z *= 0.6;
                s.spin *= 0.5;
            }
            let r = s.spin * s.age;
            let k = if t > 0.75 {
                1.0 - (t - 0.75) / 0.25
            } else {
                1.0
            };
            (
                Quat::from_euler(EulerRot::XYZ, s.r.x + r, s.r.y + r * 0.7, s.r.z),
                Vec3::splat(size * k),
            )
        }
        BurstMode::Spike => {
            // pic : jaillit (rebond), tient, s'enfonce
            let rise = if t < 0.12 { back_out(t / 0.12) } else { 1.0 };
            let sink = if t > 0.7 { (t - 0.7) / 0.3 } else { 0.0 };
            let h = (rise * (1.0 - sink * sink)).max(0.0);
            (
                Quat::from_euler(EulerRot::YXZ, s.r.y, s.r.x, s.r.z * 0.1),
                Vec3::new(
                    size * (0.6 + 0.4 * rise),
                    size * s.stretch * h,
                    size * (0.6 + 0.4 * rise),
                ),
            )
        }
        BurstMode::Static => {
            // statique (prison de glace…) : apparition rapide, disparition par réduction
            let k = (t / 0.08).min(1.0)
                * if t > 0.85 {
                    1.0 - (t - 0.85) / 0.15
                } else {
                    1.0
                };
            (
                Quat::from_euler(EulerRot::YXZ, s.r.y, s.r.x, 0.0),
                Vec3::new(size * k, size * s.stretch * k, size * k),
            )
        }
    };
    Some(Transform {
        translation: s.p,
        rotation: rot,
        scale,
    })
}

/// Avance toutes les instances (système Bevy, après les FX).
pub fn update_bursts(
    time: Res<Time>,
    mut mb: ResMut<MeshBursts>,
    mut q: Query<(&mut Transform, &mut Visibility)>,
) {
    let dt = time.delta_secs().min(0.1);
    for sys in &mut mb.systems {
        if sys.active == 0 {
            continue;
        }
        for (i, s) in sys.slots.iter_mut().enumerate() {
            if s.mode.is_none() {
                continue;
            }
            s.age += dt;
            let Some(&e) = sys.entities.get(i) else {
                continue;
            };
            let Ok((mut tf, mut vis)) = q.get_mut(e) else {
                continue;
            };
            if s.age < 0.0 {
                vis.set_if_neq(Visibility::Hidden);
                continue;
            }
            if s.age >= s.life {
                s.mode = None;
                sys.active -= 1;
                vis.set_if_neq(Visibility::Hidden);
                continue;
            }
            if let Some(t) = step_slot(s, dt) {
                *tf = t;
                vis.set_if_neq(Visibility::Inherited);
            }
        }
    }
}

// ---------------------------------------------------------------------------------- géométries

/// Maillage à facettes (sommets dupliqués, normales plates) à partir de positions et d'indices.
fn faceted(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices));
    m.duplicate_vertices();
    m.compute_flat_normals();
    m
}

/// Pic : cône à facettes, base à l'origine, pointe vers +y (hauteur 1).
pub fn spike_geometry(sides: u32) -> Mesh {
    let sides = sides.max(3);
    let mut p = vec![[0.0, 1.0, 0.0], [0.0, 0.0, 0.0]];
    for k in 0..sides {
        let a = k as f32 / sides as f32 * std::f32::consts::TAU;
        p.push([a.cos() * 0.35, 0.0, -a.sin() * 0.35]);
    }
    let mut idx = Vec::new();
    for k in 0..sides {
        let (a, b) = (2 + k, 2 + (k + 1) % sides);
        idx.extend([0, a, b, 1, b, a]);
    }
    faceted(p, idx)
}

/// Rocher (débris) : icosaèdre déformé par un bruit fixe.
pub fn rock_geometry() -> Mesh {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let raw = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    let p = raw
        .iter()
        .map(|&[x, y, z]| {
            let v = Vec3::new(x, y, z).normalize() * 0.5;
            let h = (v.x * 91.3 + v.y * 47.1 + v.z * 13.7).sin() * 0.5 + 0.5;
            [
                v.x * (0.8 + h * 0.4),
                v.y * (0.7 + h * 0.3),
                v.z * (0.8 + h * 0.4),
            ]
        })
        .collect();
    let idx = vec![
        0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7,
        1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9,
        8, 1,
    ];
    faceted(p, idx)
}

/// Éclat de glace : octaèdre étiré, posé au sol.
pub fn shard_geometry() -> Mesh {
    let s = |x: f32, y: f32, z: f32| [x * 0.5 * 0.55, y * 0.5 * 1.4 + 0.6, z * 0.5 * 0.55];
    let p = vec![
        s(1.0, 0.0, 0.0),
        s(-1.0, 0.0, 0.0),
        s(0.0, 1.0, 0.0),
        s(0.0, -1.0, 0.0),
        s(0.0, 0.0, 1.0),
        s(0.0, 0.0, -1.0),
    ];
    let idx = vec![
        0, 2, 4, 0, 4, 3, 0, 3, 5, 0, 5, 2, 1, 2, 5, 1, 5, 3, 1, 3, 4, 1, 4, 2,
    ];
    faceted(p, idx)
}

/// Normales tournées vers l'extérieur (vérification des géométries).
#[cfg(test)]
fn outward(m: &Mesh, center: Vec3) -> bool {
    use bevy::mesh::VertexAttributeValues as V;
    let (Some(V::Float32x3(p)), Some(V::Float32x3(n))) = (
        m.attribute(Mesh::ATTRIBUTE_POSITION),
        m.attribute(Mesh::ATTRIBUTE_NORMAL),
    ) else {
        return false;
    };
    p.chunks(3).zip(n.chunks(3)).all(|(tri, nn)| {
        let c = (Vec3::from(tri[0]) + Vec3::from(tri[1]) + Vec3::from(tri[2])) / 3.0;
        (c - center).dot(Vec3::from(nn[0])) > 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn geometries_fermees_et_normales_exterieures() {
        assert!(outward(&spike_geometry(5), Vec3::new(0.0, 0.3, 0.0)));
        assert!(outward(&rock_geometry(), Vec3::ZERO));
        assert!(outward(&shard_geometry(), Vec3::new(0.0, 0.6, 0.0)));
    }

    #[test]
    fn pic_jaillit_tient_puis_s_enfonce() {
        let mut s = Slot {
            mode: Some(BurstMode::Spike),
            life: 1.0,
            size: 1.0,
            stretch: 2.0,
            ..default()
        };
        s.age = 0.5;
        let mid = step_slot(&mut s, 0.0).map(|t| t.scale.y);
        assert_eq!(mid, Some(2.0));
        s.age = 0.99;
        let late = step_slot(&mut s, 0.0).map_or(1.0, |t| t.scale.y);
        assert!(late < 0.2, "presque enfoncé à la fin : {late}");
    }

    #[test]
    fn debris_rebondissent_au_sol() {
        let mut s = Slot {
            mode: Some(BurstMode::Debris),
            p: Vec3::new(0.0, 0.31, 0.0),
            v: Vec3::new(1.0, -5.0, 0.0),
            life: 2.0,
            size: 1.0,
            age: 0.1,
            ..default()
        };
        step_slot(&mut s, 0.05);
        assert!(s.v.y > 0.0, "rebond");
        assert!((s.p.y - 0.3).abs() < 1e-5);
        assert!(s.v.x < 1.0, "frottement");
    }

    #[test]
    fn instances_recyclees_en_anneau() {
        let mut w = fx_world();
        w.init_resource::<MeshBursts>();
        w.init_resource::<Time>();
        let mut fx = Fx::new(&mut w);
        for _ in 0..200 {
            burst(
                &mut fx,
                BurstKind::RockSpikes,
                BurstSpawn {
                    life: 0.5,
                    color: Some(Vec3::new(0.5, 0.4, 0.3)),
                    ..default()
                },
            );
        }
        let mb = w.resource::<MeshBursts>();
        assert_eq!(
            mb.alive(BurstKind::RockSpikes),
            96,
            "plafonné à la capacité"
        );
        assert_eq!(mb.stone.len(), 1, "un seul matériau par couleur");
        let n = w.query::<&Mesh3d>().iter(&w).count();
        assert_eq!(n, 96, "aucune entité au-delà de la capacité");
    }
}
