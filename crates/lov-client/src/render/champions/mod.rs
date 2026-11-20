//! Socle des champions : animations, squelette, assemblage des modèles, vue animée.
//!
//! Portage de `packages/client/src/render/champions/` (area `render-champions`, issue #110).
//!
//! Écart provisoire : tous les champions prennent le modèle générique de leur classe (`modelFor` sans
//! modèle dédié). Les modèles dédiés et les portraits arrivent avec les tranches de champions.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lov_shared::schema::ChampionClass;

pub mod anim;
pub mod builder;
pub mod generic;
pub mod model;
pub mod outline;
pub mod rig;
pub mod view;

use model::ChampionModelDef;
use outline::OutlineMaterial;

/// Matériau de contour partagé par tous les champions.
#[derive(Resource)]
pub struct OutlineHandle(pub Handle<OutlineMaterial>);

impl FromWorld for OutlineHandle {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource_mut::<Assets<OutlineMaterial>>()
                .add(OutlineMaterial::default()),
        )
    }
}

/// Accès nécessaires à l'apparition d'un champion (en plus des maillages du `Kit`).
#[derive(SystemParam)]
pub struct ChampionAssets<'w> {
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
    pub outline: Res<'w, OutlineHandle>,
}

/// `modelFor` : modèle d'un champion selon sa classe.
pub fn model_for(class: ChampionClass) -> ChampionModelDef {
    generic::generic_for(class)
}

/// Matériau de contour, shader et animation des champions.
pub struct ChampionsPlugin;

impl Plugin for ChampionsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "outline.wesl");
        app.add_plugins(MaterialPlugin::<OutlineMaterial>::default())
            .init_resource::<OutlineHandle>();
    }
}
