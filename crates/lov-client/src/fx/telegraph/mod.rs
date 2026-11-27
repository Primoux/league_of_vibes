//! Télégraphes de visée : forme de l'indicateur d'un sort (logique pure) et décals au sol qui l'affichent.
//!
//! Portage de `packages/client/src/fx/telegraph/` (area `fx-effects`).

use bevy::prelude::*;

pub mod aim;
pub mod indicators;

pub use indicators::FxIndicators;

/// Indicateurs de visée et de portée.
pub struct TelegraphPlugin;

impl Plugin for TelegraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxIndicators>().add_systems(
            Update,
            (indicators::drive_indicators, indicators::apply_indicators)
                .chain()
                .after(crate::render::camera::drive_camera),
        );
    }
}
