//! Socle des champions : animations, squelette, assemblage des modèles, vue animée.
//!
//! Portage de `packages/client/src/render/champions/` (area `render-champions`, issue #110). La vue
//! animée, qui branche le tout au rendu, arrive dans la PR suivante de #110.

use bevy::prelude::*;

pub mod anim;
pub mod builder;
pub mod generic;
pub mod model;
pub mod outline;
pub mod rig;

/// Matériau de contour et son shader.
pub struct ChampionsPlugin;

impl Plugin for ChampionsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "outline.wesl");
        app.add_plugins(MaterialPlugin::<outline::OutlineMaterial>::default());
    }
}
