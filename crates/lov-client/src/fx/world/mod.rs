//! FX du monde : chiffres flottants, statuts, auto-attaques, événements (mort, rappel, structures,
//! objectifs…) et ambiance de la carte.
//!
//! Portage de `packages/client/src/fx/world/` (area `fx-effects`).

use bevy::prelude::*;

use crate::fx::particles::ParticleSystem;
use crate::fx::registry::RegisterFx;

pub mod ambient;
pub mod auto_attacks;
pub mod damage_numbers;
pub mod events;
pub mod status;

/// Handlers des événements du monde et systèmes associés.
pub struct WorldFxPlugin;

impl Plugin for WorldFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<auto_attacks::AutoAttacks>()
            .init_resource::<status::ActiveStatuses>()
            .init_resource::<damage_numbers::FloatingNumbers>()
            .on_world_event(auto_attacks::auto_attack_events)
            .on_world_event(status::status_events)
            .on_world_event(events::world_events)
            .add_systems(Update, ambient::update_ambient)
            .add_systems(
                PostUpdate,
                damage_numbers::update_numbers.after(bevy::transform::TransformSystems::Propagate),
            );
        let ambient = app
            .world_mut()
            .get_resource_mut::<ParticleSystem>()
            .map(|mut ps| ambient::Ambient::new(&mut ps));
        if let Some(a) = ambient {
            app.insert_resource(a);
        }
    }
}
