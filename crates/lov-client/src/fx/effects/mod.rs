//! Effets composables des sorts et du monde : primitives (télégraphie, onde de choc, éclat, zone, projectile,
//! traînées), meshes animés (pics, débris) et auras (bouclier, étourdissement, brume).
//!
//! Portage de `packages/client/src/fx/effects/` (area `fx-effects`).

use bevy::prelude::*;

pub mod auras;
pub mod mesh_bursts;
pub mod primitives;

pub use mesh_bursts::MeshBursts;

/// Ressources et systèmes des effets.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshBursts>()
            .add_systems(Update, mesh_bursts::update_bursts);
    }
}
