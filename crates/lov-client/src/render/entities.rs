//! Vues d'entités. PROTOTYPE : une forme simple par type d'entité, colorée par équipe, sans modèle,
//! sans animation ni barre de vie. La position est celle de `GameState::pose` : interpolée à
//! `INTERP_DELAY_MS` pour les autres entités, prédite pour le champion local.

use std::collections::HashMap;

use bevy::prelude::*;

use super::to_world;
use crate::net::{GameState, Pose};
use lov_shared::types::{EntityId, EntityKind, TeamOrNeutral};

#[derive(Component)]
struct EntityView;

#[derive(Resource, Default)]
struct ViewIndex(HashMap<EntityId, Entity>);

struct Shape {
    mesh: Handle<Mesh>,
    /// Hauteur du centre de la forme au-dessus du sol.
    lift: f32,
}

#[derive(Resource)]
struct ViewAssets {
    shapes: HashMap<EntityKind, Shape>,
    blue: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    neutral: Handle<StandardMaterial>,
    me: Handle<StandardMaterial>,
}

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewIndex>()
            .add_systems(Startup, load_assets)
            .add_systems(Update, sync_views);
    }
}

fn load_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut shape = |mesh: Mesh, lift: f32| Shape {
        mesh: meshes.add(mesh),
        lift,
    };
    let shapes = HashMap::from([
        (
            EntityKind::Champion,
            shape(Capsule3d::new(0.42, 0.9).into(), 0.87),
        ),
        (EntityKind::Minion, shape(Sphere::new(0.3).into(), 0.3)),
        (
            EntityKind::Tower,
            shape(Cylinder::new(0.7, 3.4).into(), 1.7),
        ),
        (
            EntityKind::Inhibitor,
            shape(Cuboid::new(1.6, 1.2, 1.6).into(), 0.6),
        ),
        (EntityKind::Nexus, shape(Sphere::new(1.6).into(), 1.6)),
        (EntityKind::Monster, shape(Sphere::new(0.6).into(), 0.6)),
        (
            EntityKind::Ward,
            shape(Cylinder::new(0.12, 0.7).into(), 0.35),
        ),
        (EntityKind::Projectile, shape(Sphere::new(0.14).into(), 0.9)),
    ]);
    commands.insert_resource(ViewAssets {
        shapes,
        blue: materials.add(Color::srgb(0.25, 0.5, 1.0)),
        red: materials.add(Color::srgb(1.0, 0.3, 0.28)),
        neutral: materials.add(Color::srgb(0.85, 0.75, 0.4)),
        me: materials.add(Color::srgb(1.0, 1.0, 1.0)),
    });
}

fn target(p: Pose, lift: f32) -> Transform {
    Transform::from_translation(to_world(Vec2::new(p.x as f32, p.y as f32), lift))
        .with_rotation(Quat::from_rotation_y(p.facing as f32))
}

fn sync_views(
    mut commands: Commands,
    state: Res<GameState>,
    assets: Res<ViewAssets>,
    mut index: ResMut<ViewIndex>,
    mut views: Query<&mut Transform, With<EntityView>>,
) {
    for (id, e) in state.entities() {
        let dead = e.dead.unwrap_or(false);
        // Les zones (blizzard, piège…) n'ont pas encore de vue.
        let Some(shape) = assets.shapes.get(&e.kind).filter(|_| !dead) else {
            if let Some(view) = index.0.remove(id) {
                commands.entity(view).despawn();
            }
            continue;
        };
        let Some(pose) = state.pose(*id) else {
            continue;
        };
        let goal = target(pose, shape.lift);
        if let Some(mut transform) = index.0.get(id).and_then(|view| views.get_mut(*view).ok()) {
            *transform = goal;
        } else if !index.0.contains_key(id) {
            let material = if state.my_entity == Some(*id) {
                &assets.me
            } else {
                match e.team {
                    TeamOrNeutral::Blue => &assets.blue,
                    TeamOrNeutral::Red => &assets.red,
                    TeamOrNeutral::Neutral => &assets.neutral,
                }
            };
            let view = commands
                .spawn((
                    EntityView,
                    Mesh3d(shape.mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    goal,
                ))
                .id();
            index.0.insert(*id, view);
        }
    }
    index.0.retain(|id, view| {
        let known = state.entities().contains_key(id);
        if !known {
            commands.entity(*view).despawn();
        }
        known
    });
}
