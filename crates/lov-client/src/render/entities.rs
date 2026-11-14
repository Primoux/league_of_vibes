//! Vues d'entités. PROTOTYPE : une forme simple par type d'entité, colorée par équipe, sans modèle,
//! sans animation ni barre de vie. La position suit le dernier snapshot avec un lissage exponentiel,
//! pas encore l'interpolation à 100 ms du client web.

use std::collections::HashMap;

use bevy::prelude::*;

use super::to_world;
use crate::net::{EntityData, GameState, num, text};

const SMOOTHING: f32 = 18.0;

#[derive(Component)]
struct EntityView;

#[derive(Resource, Default)]
struct ViewIndex(HashMap<u64, Entity>);

struct Shape {
    mesh: Handle<Mesh>,
    /// Hauteur du centre de la forme au-dessus du sol.
    lift: f32,
}

#[derive(Resource)]
struct ViewAssets {
    shapes: HashMap<&'static str, Shape>,
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
        ("champion", shape(Capsule3d::new(0.42, 0.9).into(), 0.87)),
        ("minion", shape(Sphere::new(0.3).into(), 0.3)),
        ("tower", shape(Cylinder::new(0.7, 3.4).into(), 1.7)),
        ("inhibitor", shape(Cuboid::new(1.6, 1.2, 1.6).into(), 0.6)),
        ("nexus", shape(Sphere::new(1.6).into(), 1.6)),
        ("monster", shape(Sphere::new(0.6).into(), 0.6)),
        ("ward", shape(Cylinder::new(0.12, 0.7).into(), 0.35)),
        ("projectile", shape(Sphere::new(0.14).into(), 0.9)),
    ]);
    commands.insert_resource(ViewAssets {
        shapes,
        blue: materials.add(Color::srgb(0.25, 0.5, 1.0)),
        red: materials.add(Color::srgb(1.0, 0.3, 0.28)),
        neutral: materials.add(Color::srgb(0.85, 0.75, 0.4)),
        me: materials.add(Color::srgb(1.0, 1.0, 1.0)),
    });
}

fn target(e: &EntityData, lift: f32) -> Transform {
    Transform::from_translation(to_world(Vec2::new(num(e, "x"), num(e, "y")), lift))
        .with_rotation(Quat::from_rotation_y(num(e, "facing")))
}

fn sync_views(
    mut commands: Commands,
    state: Res<GameState>,
    assets: Res<ViewAssets>,
    mut index: ResMut<ViewIndex>,
    mut views: Query<&mut Transform, With<EntityView>>,
    time: Res<Time>,
) {
    let blend = 1.0 - (-SMOOTHING * time.delta_secs()).exp();
    for (id, e) in &state.entities {
        let dead = e.get("dead").and_then(|d| d.as_bool()).unwrap_or(false);
        // Les zones (blizzard, piège…) n'ont pas encore de vue.
        let Some(shape) = assets.shapes.get(text(e, "kind")).filter(|_| !dead) else {
            if let Some(view) = index.0.remove(id) {
                commands.entity(view).despawn();
            }
            continue;
        };
        let goal = target(e, shape.lift);
        if let Some(mut transform) = index.0.get(id).and_then(|view| views.get_mut(*view).ok()) {
            transform.translation = transform.translation.lerp(goal.translation, blend);
            transform.rotation = transform.rotation.slerp(goal.rotation, blend);
        } else if !index.0.contains_key(id) {
            let material = if state.my_entity == Some(*id) {
                &assets.me
            } else {
                match text(e, "team") {
                    "blue" => &assets.blue,
                    "red" => &assets.red,
                    _ => &assets.neutral,
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
        let known = state.entities.contains_key(id);
        if !known {
            commands.entity(*view).despawn();
        }
        known
    });
}
