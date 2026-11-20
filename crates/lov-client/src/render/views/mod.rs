//! Vues par défaut des entités : modèles low-poly des sbires, monstres et structures, et formes simples
//! en attendant les modèles des champions.
//!
//! Portage de `packages/client/src/render/views/DefaultViews.ts` et de l'interface `EntityView`. Une vue
//! est une entité Bevy (`EntityView`) que `entities.rs` place chaque frame au sol (rotation y =
//! orientation) ; son contenu (modèle, animation) est animé par `RigView` ou `SimpleView`. Un objet
//! orienté vers +x local regarde dans la direction `facing` de l'entité.

use std::f32::consts::PI;

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::types::{
    DragonElement, EntityId, EntityKind, EntitySnapshot, TeamOrNeutral, TowerTier,
};

use super::models::geometry::{cone, dodecahedron, torus};
use super::models::{Built, Kit, MatOpts, Model, RigView, minions, monsters, structures};

/// Couleur d'équipe des formes simples (`TEAM_COLOR`).
pub fn team_color(team: TeamOrNeutral) -> u32 {
    match team {
        TeamOrNeutral::Blue => 0x3a7bff,
        TeamOrNeutral::Red => 0xff4a4a,
        TeamOrNeutral::Neutral => 0xc9a227,
    }
}

/// Vue d'une entité du jeu.
#[derive(Component, Debug)]
pub struct EntityView {
    /// Animation demandée (`snap.anim`, ou `death`).
    pub anim: String,
    pub dead: bool,
    /// Élément du drake au moment de la construction (une vue périmée est reconstruite).
    pub drake: Option<DragonElement>,
}

/// `mat` de `DefaultViews.ts` : matériau à facettes sans liseré.
fn plain(kit: &mut Kit, color: u32, emissive: Option<u32>) -> Handle<super::models::ModelMaterial> {
    kit.material(
        color,
        MatOpts {
            emissive,
            rough: 0.75,
            metal: 0.05,
            rim: false,
            ..default()
        },
    )
}

/// Vue simple : un corps, animations procédurales génériques (`SimpleView`).
#[derive(Component, Debug)]
pub struct SimpleView {
    body: Entity,
    body_height: f32,
    anim: String,
    t: f32,
    death_t: f32,
    pos: Vec3,
    rot: Vec3,
}

impl SimpleView {
    pub fn new(body: Entity, body_height: f32) -> Self {
        Self {
            body,
            body_height,
            anim: "idle".into(),
            t: 0.0,
            death_t: 0.0,
            pos: Vec3::ZERO,
            rot: Vec3::ZERO,
        }
    }

    pub fn play_anim(&mut self, name: &str) {
        if name != self.anim {
            self.t = 0.0;
            self.anim = name.to_owned();
        }
    }

    pub fn step(&mut self, dt: f32, dead: bool) {
        self.t += dt;
        if dead {
            self.death_t = (self.death_t + dt * 1.5).min(1.0);
            self.pos.y = -self.body_height * self.death_t;
            self.rot.z = self.death_t * 0.6;
            return;
        }
        self.death_t = 0.0;
        self.rot.z = 0.0;
        match self.anim.as_str() {
            "run" => {
                self.pos.y = (self.t * 10.0).sin().abs() * 0.08;
                self.rot.x = (self.t * 10.0).sin() * 0.05;
                self.pos.x = 0.0;
            }
            "attack" | "cast_Q" | "cast_W" | "cast_E" | "cast_R" => {
                self.pos.y = 0.0;
                self.pos.x = (self.t * 12.0).sin().max(0.0) * 0.12;
            }
            _ => {
                self.pos.y = (self.t * 2.0).sin() * 0.02;
                self.pos.x = 0.0;
                self.rot.x = 0.0;
            }
        }
    }

    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.pos).with_rotation(super::models::kit::euler(self.rot))
    }

    pub fn body(&self) -> Entity {
        self.body
    }
}

/// Fait apparaître une vue simple : `body` (un modèle sans rig) sous une nouvelle vue.
fn spawn_simple(commands: &mut Commands, view: Entity, body: &Model, height: f32) {
    let nodes = body.spawn(commands);
    commands.entity(nodes[Model::ROOT]).insert(ChildOf(view));
    commands
        .entity(view)
        .insert(SimpleView::new(nodes[Model::ROOT], height));
}

/// Couleur `#rrggbb` d'une palette de champion.
fn palette_hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.strip_prefix('#')?, 16).ok()
}

/// Capsule colorée + « nez » indiquant l'orientation (+x) : `championPlaceholder`.
fn champion_placeholder(kit: &mut Kit, snap: &EntitySnapshot) -> Model {
    let def = snap.subtype.as_deref().and_then(get_champion);
    let color = def
        .and_then(|d| palette_hex(&d.palette.primary))
        .unwrap_or(0xcccccc);
    let accent = def
        .and_then(|d| palette_hex(&d.palette.accent))
        .unwrap_or(0xffffff);
    let team = team_color(snap.team);
    let mut m = Model::new();
    let capsule = kit.mesh("champ", || {
        // `CapsuleGeometry(0.42, 0.9, 4, 10)` : 4 segments par demi-sphère, 10 autour de l'axe.
        let mut mesh = Capsule3d::new(0.42, 0.9)
            .mesh()
            .latitudes(8)
            .longitudes(10)
            .build();
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        mesh
    });
    let mat = plain(kit, color, None);
    m.part(Model::ROOT, capsule, mat, 0.0, 0.9, 0.0);
    let nose = kit.geo("champ_nose", || cone(0.16, 0.4, 6).rotate_z(-PI / 2.0));
    let mat = plain(kit, accent, None);
    m.part(Model::ROOT, nose, mat, 0.45, 1.25, 0.0);
    let ring = kit.geo("team_ring", || torus(0.55, 0.05, 6, 24).rotate_x(PI / 2.0));
    let mat = plain(kit, team, Some(team));
    m.part(Model::ROOT, ring, mat, 0.0, 0.05, 0.0);
    m
}

/// Phase d'horloge d'un modèle animé : le client web la tire au hasard dans [0, 10) s ; ici elle découle
/// de l'id, pour que deux unités voisines ne bougent pas en cadence sans dépendre d'un aléa.
pub fn phase_of(id: EntityId) -> f32 {
    let h = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (h >> 40) as f32 / (1u64 << 24) as f32 * 10.0
}

fn tower_tier(subtype: Option<&str>) -> Option<TowerTier> {
    match subtype? {
        "outer" => Some(TowerTier::Outer),
        "inner" => Some(TowerTier::Inner),
        "inhib" => Some(TowerTier::Inhib),
        "nexus" => Some(TowerTier::Nexus),
        _ => None,
    }
}

/// Modèle animé d'une entité qui en a un (`defaultViewFor` sans les formes simples).
pub fn built_for(
    kit: &mut Kit,
    snap: &EntitySnapshot,
    drake: Option<DragonElement>,
) -> Option<Built> {
    let sub = snap.subtype.as_deref();
    match snap.kind {
        EntityKind::Minion => Some(minions::minion(kit, sub, snap.team)),
        EntityKind::Tower => Some(structures::tower(kit, snap.team, tower_tier(sub))),
        EntityKind::Inhibitor => Some(structures::inhibitor(kit, snap.team)),
        EntityKind::Nexus => Some(structures::nexus(kit, snap.team)),
        EntityKind::Monster => monsters::monster(kit, sub, drake),
        EntityKind::Ward => Some(structures::ward(kit, snap.team)),
        _ => None,
    }
}

/// `defaultViewFor` : fait apparaître la vue d'une entité, ou rien (projectiles et zones sont gérés par
/// les FX). La vue n'est pas encore placée : `entities.rs` la pose à la première frame.
pub fn spawn_default_view(
    commands: &mut Commands,
    kit: &mut Kit,
    snap: &EntitySnapshot,
    drake: Option<DragonElement>,
) -> Option<Entity> {
    let is_view = matches!(
        snap.kind,
        EntityKind::Champion
            | EntityKind::Minion
            | EntityKind::Tower
            | EntityKind::Inhibitor
            | EntityKind::Nexus
            | EntityKind::Monster
            | EntityKind::Ward
    );
    if !is_view {
        return None;
    }
    let view = commands
        .spawn((
            EntityView {
                anim: String::new(),
                dead: false,
                drake: (snap.subtype.as_deref() == Some("dragon"))
                    .then_some(drake)
                    .flatten(),
            },
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    if snap.kind == EntityKind::Champion {
        let body = champion_placeholder(kit, snap);
        spawn_simple(commands, view, &body, 1.8);
    } else if let Some(built) = built_for(kit, snap, drake) {
        let rig = built.spawn(commands, view, phase_of(snap.id));
        commands.entity(view).insert(rig);
    } else {
        // monstre inconnu : un caillou
        let mut body = Model::new();
        let g = kit.geo("unknown_monster", || dodecahedron(0.4, 0));
        let mat = plain(kit, 0x9a8a5a, None);
        body.part(Model::ROOT, g, mat, 0.0, 0.0, 0.0);
        spawn_simple(commands, view, &body, 0.8);
    }
    Some(view)
}

/// Anime les vues : rig des modèles low-poly, corps des vues simples.
pub fn animate_views(
    time: Res<Time>,
    mut rigs: Query<(&EntityView, &mut RigView), Without<SimpleView>>,
    mut simples: Query<(&EntityView, &mut SimpleView), Without<RigView>>,
    mut transforms: Query<&mut Transform, (Without<RigView>, Without<SimpleView>)>,
) {
    let dt = time.delta_secs().min(0.1);
    for (view, mut rig) in &mut rigs {
        rig.play_anim(&view.anim);
        rig.step(dt, view.dead);
        rig.flush(&mut transforms);
    }
    for (view, mut simple) in &mut simples {
        simple.play_anim(&view.anim);
        simple.step(dt, view.dead);
        if let Ok(mut t) = transforms.get_mut(simple.body()) {
            *t = simple.transform();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vue_simple_couchee_a_la_mort() {
        let mut v = SimpleView::new(Entity::PLACEHOLDER, 1.8);
        for _ in 0..60 {
            v.step(1.0 / 30.0, true);
        }
        assert!((v.pos.y + 1.8).abs() < 1e-5);
        assert!((v.rot.z - 0.6).abs() < 1e-5);
        v.play_anim("run");
        v.step(0.05, false);
        assert_eq!(v.rot.z, 0.0);
        assert!(v.pos.y > 0.0);
    }

    #[test]
    fn phases_variees_et_bornees() {
        let p: Vec<f32> = (1..50).map(phase_of).collect();
        assert!(p.iter().all(|x| (0.0..10.0).contains(x)));
        assert!(p.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn couleurs_de_palette() {
        assert_eq!(palette_hex("#3a7bff"), Some(0x3a7bff));
        assert_eq!(palette_hex("rouge"), None);
    }
}
