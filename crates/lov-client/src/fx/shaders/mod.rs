//! Shaders FX : décals au sol (télégraphies, zones, ondes de choc), rubans de traînée, matériaux fresnel,
//! givre, eau et dissolution.
//!
//! Portage de `packages/client/src/fx/shaders/` (la distorsion de chaleur arrive avec le post-traitement,
//! qui la lit).

use bevy::prelude::*;

pub mod decal;
pub mod dissolve;
pub mod materials;
pub mod ribbon;

pub use decal::{DecalMaterial, DecalMesh, DecalShape, GroundDecal};
pub use dissolve::{DissolveMaterial, dissolvable, set_dissolve};
pub use materials::{FresnelMaterial, IceMaterial, WaterMaterial};
pub use ribbon::{Ribbon, RibbonMaterial, RibbonMode};

pub struct ShadersPlugin;

impl Plugin for ShadersPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "decal.wesl");
        bevy::asset::embedded_asset!(app, "ribbon.wesl");
        bevy::asset::embedded_asset!(app, "fresnel.wesl");
        bevy::asset::embedded_asset!(app, "ice.wesl");
        bevy::asset::embedded_asset!(app, "water.wesl");
        bevy::asset::embedded_asset!(app, "dissolve.wesl");
        app.add_plugins((
            MaterialPlugin::<DecalMaterial>::default(),
            MaterialPlugin::<RibbonMaterial>::default(),
            MaterialPlugin::<FresnelMaterial>::default(),
            MaterialPlugin::<IceMaterial>::default(),
            MaterialPlugin::<WaterMaterial>::default(),
            MaterialPlugin::<DissolveMaterial>::default(),
        ))
        .init_resource::<DecalMesh>()
        .add_systems(PostUpdate, ribbon::update_ribbons);
    }
}
