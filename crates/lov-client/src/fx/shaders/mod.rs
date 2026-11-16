//! Shaders FX : décals au sol (télégraphies, zones, ondes de choc) et rubans de traînée.
//!
//! Portage de `packages/client/src/fx/shaders/` (les matériaux fresnel, givre, eau et dissolution
//! arrivent dans une PR suivante de #111, la distorsion de chaleur avec le post-traitement).

use bevy::prelude::*;

pub mod decal;
pub mod ribbon;

pub use decal::{DecalMaterial, DecalMesh, DecalShape, GroundDecal};
pub use ribbon::{Ribbon, RibbonMaterial, RibbonMode};

pub struct ShadersPlugin;

impl Plugin for ShadersPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "decal.wesl");
        bevy::asset::embedded_asset!(app, "ribbon.wesl");
        app.add_plugins((
            MaterialPlugin::<DecalMaterial>::default(),
            MaterialPlugin::<RibbonMaterial>::default(),
        ))
        .init_resource::<DecalMesh>()
        .add_systems(PostUpdate, ribbon::update_ribbons);
    }
}
