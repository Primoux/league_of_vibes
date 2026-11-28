//! Vues d'entités : création, placement, animation et retrait des vues de `GameState`.
//!
//! Portage de la partie « vues » de `packages/client/src/render/GameRenderer.ts` (`applySnapshot`,
//! `createView`, `removeSlot`, structures statiques, placement par frame). La position est celle de
//! `GameState::pose` : interpolée à `INTERP_DELAY_MS` pour les autres entités, prédite pour le champion
//! local. La hauteur du terrain n'est pas encore portée (area `render-map`) : les vues sont posées à 0.
//!
//! Écart provisoire : les projectiles gardent une petite sphère, tant que les FX de monde qui les dessinent
//! dans le client web (`fx/world`) ne sont pas portés.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::map::{INHIBITORS, NEXUS, TOWERS};
use lov_shared::types::{
    DragonElement, EntityId, EntityKind, EntitySnapshot, GameEvent, MonsterType, Team,
    TeamOrNeutral,
};

use super::champions::melenchon::animate_melenchon;
use super::champions::view::{animate_champions, route_events};
use super::champions::{ChampionAssets, ChampionsPlugin};
use super::healthbars::{BarWorld, HealthBarsPlugin, update_bars};
use super::models::{Kit, ModelsPlugin, RigView, structures};
use super::views::{EntityView, animate_views, phase_of, spawn_default_view};
use super::{from_shared, to_ground};
use crate::net::{GameState, Pose, ServerEvent};

/// Distance (unités de jeu) sous laquelle une entité serveur masque la structure statique de la carte.
const STATIC_HIDE_RADIUS: f64 = 200.0;

/// Vue d'une entité du serveur.
struct Slot {
    view: Entity,
    anim: String,
}

#[derive(Resource, Default)]
struct ViewIndex(HashMap<EntityId, Slot>);

/// Élément du prochain drake (mis à jour depuis l'événement `objectiveSpawn`).
#[derive(Resource, Default)]
pub struct DragonElementState(pub Option<DragonElement>);

/// Structure issue de `map.rs`, visible tant que le serveur n'envoie pas l'entité correspondante.
#[derive(Component)]
struct StaticStructure {
    x: f64,
    y: f64,
}

/// Projectile sans FX (voir l'écart en tête de fichier).
#[derive(Component)]
struct ProjectilePlaceholder;

#[derive(Resource)]
struct ProjectileAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((ModelsPlugin, HealthBarsPlugin, ChampionsPlugin))
            .init_resource::<ViewIndex>()
            .init_resource::<DragonElementState>()
            .add_systems(Startup, (load_assets, spawn_static_structures))
            .add_systems(
                Update,
                (
                    track_dragon_element,
                    sync_views,
                    place_views,
                    animate_views,
                    route_events,
                    animate_champions,
                    animate_melenchon,
                    update_health_bars,
                )
                    .chain(),
            );
    }
}

fn load_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ProjectileAssets {
        mesh: meshes.add(Sphere::new(0.14)),
        material: materials.add(Color::srgb(1.0, 0.9, 0.6)),
    });
}

fn spawn_static_structures(mut commands: Commands, mut kit: Kit) {
    let team = |t: Team| match t {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    };
    let spawn = |commands: &mut Commands, x: f64, y: f64, built: super::models::Built| {
        let view = commands
            .spawn((
                StaticStructure { x, y },
                EntityView {
                    anim: "idle".into(),
                    dead: false,
                    drake: None,
                },
                Transform::from_translation(to_ground(Vec2::new(x as f32, y as f32), 0.0)),
                Visibility::default(),
            ))
            .id();
        let rig = built.spawn(commands, view, phase_of(view.index_u32()));
        commands.entity(view).insert(rig);
    };
    for t in TOWERS.iter() {
        let built = structures::tower(&mut kit, team(t.team), Some(t.tier));
        spawn(&mut commands, t.pos.x, t.pos.y, built);
    }
    for i in INHIBITORS.iter() {
        let built = structures::inhibitor(&mut kit, team(i.team));
        spawn(&mut commands, i.pos.x, i.pos.y, built);
    }
    for t in [Team::Blue, Team::Red] {
        let pos = NEXUS.get(t);
        let built = structures::nexus(&mut kit, team(t));
        spawn(&mut commands, pos.x, pos.y, built);
    }
}

/// Métadonnées d'apparition, lues AVANT la création des vues (un drake prend son élément).
fn track_dragon_element(
    mut events: MessageReader<ServerEvent>,
    mut element: ResMut<DragonElementState>,
) {
    for ServerEvent(ev) in events.read() {
        if let GameEvent::ObjectiveSpawn {
            monster: MonsterType::Dragon,
            element: e,
            ..
        } = ev
        {
            element.0 = *e;
        }
    }
}

fn is_structure(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus
    )
}

/// Crée les vues des nouvelles entités, transmet l'animation, retire les vues des entités disparues.
fn sync_views(
    mut commands: Commands,
    (mut kit, mut champs): (Kit, ChampionAssets),
    (state, element, projectile): (
        Res<GameState>,
        Res<DragonElementState>,
        Res<ProjectileAssets>,
    ),
    mut index: ResMut<ViewIndex>,
    mut views: Query<&mut EntityView, Without<StaticStructure>>,
    mut statics: Query<(&StaticStructure, &mut Visibility)>,
) {
    for (id, e) in state.entities() {
        // drake construit avec un élément périmé : on reconstruit sa vue
        let stale = index.0.get(id).is_some_and(|slot| {
            e.subtype.as_deref() == Some("dragon")
                && views.get(slot.view).is_ok_and(|v| v.drake != element.0)
        });
        if stale && let Some(slot) = index.0.remove(id) {
            commands.entity(slot.view).despawn();
        }
        if !index.0.contains_key(id) {
            let view = if e.kind == EntityKind::Projectile {
                commands
                    .spawn((
                        ProjectilePlaceholder,
                        Mesh3d(projectile.mesh.clone()),
                        MeshMaterial3d(projectile.material.clone()),
                        Transform::default(),
                    ))
                    .id()
            } else if let Some(view) =
                spawn_default_view(&mut commands, &mut kit, &mut champs, e, element.0)
            {
                view
            } else {
                continue;
            };
            index.0.insert(
                *id,
                Slot {
                    view,
                    anim: String::new(),
                },
            );
            if is_structure(e.kind) {
                hide_static_near(e, &mut statics);
            }
        }
        let anim = if e.dead == Some(true) {
            "death"
        } else {
            e.anim.as_deref().unwrap_or("idle")
        };
        let Some(slot) = index.0.get_mut(id) else {
            continue;
        };
        if anim != slot.anim {
            slot.anim = anim.to_owned();
        }
        if let Ok(mut view) = views.get_mut(slot.view) {
            if view.anim != slot.anim {
                view.anim.clone_from(&slot.anim);
            }
            view.dead = e.dead == Some(true);
        }
    }
    // entités disparues (snapshot delta ou complet : le buffer fait foi)
    index.0.retain(|id, slot| {
        let known = state.entities().contains_key(id);
        if !known {
            commands.entity(slot.view).despawn();
        }
        known
    });
}

fn hide_static_near(e: &EntitySnapshot, statics: &mut Query<(&StaticStructure, &mut Visibility)>) {
    for (s, mut vis) in statics.iter_mut() {
        if (s.x - e.x).abs() < STATIC_HIDE_RADIUS && (s.y - e.y).abs() < STATIC_HIDE_RADIUS {
            *vis = Visibility::Hidden;
        }
    }
}

fn target(p: Pose, lift: f32) -> Transform {
    Transform::from_translation(to_ground(
        from_shared(lov_shared::types::Vec2 { x: p.x, y: p.y }),
        lift,
    ))
    .with_rotation(Quat::from_rotation_y(p.facing as f32))
}

/// Place chaque vue à la position interpolée de son entité, posée sur le relief du terrain, tournée
/// vers `facing`.
fn place_views(
    state: Res<GameState>,
    index: Res<ViewIndex>,
    mut transforms: Query<(&mut Transform, Has<ProjectilePlaceholder>), Without<RigView>>,
    mut rig_views: Query<&mut Transform, With<RigView>>,
) {
    for (id, slot) in &index.0 {
        let Some(pose) = state.pose(*id) else {
            continue;
        };
        if let Ok(mut t) = rig_views.get_mut(slot.view) {
            *t = target(pose, 0.0);
        } else if let Ok((mut t, projectile)) = transforms.get_mut(slot.view) {
            *t = target(pose, if projectile { 0.9 } else { 0.0 });
        }
    }
}

/// Barres de vie des vues du serveur (les structures statiques de la carte n'en ont pas).
fn update_health_bars(mut w: BarWorld, state: Res<GameState>, index: Res<ViewIndex>) {
    let my_team = state.game.as_ref().map(|g| g.team);
    update_bars(
        &mut w,
        state.entities(),
        index.0.iter().map(|(id, slot)| (id, slot.view)),
        state.my_entity,
        my_team,
    );
}
