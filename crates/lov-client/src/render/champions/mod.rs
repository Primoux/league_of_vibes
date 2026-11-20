//! Socle des champions : animations, squelette, assemblage des modèles, vue animée.
//!
//! Portage de `packages/client/src/render/champions/` (area `render-champions`, issue #110).
//!
//! `model_for` suit l'ordre du client web : portrait, sur la base du modèle dédié du champion, sinon du
//! modèle générique de sa classe. Les modèles dédiés (squelette, clips, port d'arme) arrivent avec les
//! tranches de champions (`models/<nom>.rs`, area `champion`), chacune en ajoutant sa ligne à `MODELS`.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lov_shared::schema::ChampionClass;

pub mod anim;
pub mod builder;
pub mod generic;
pub mod model;
pub mod outline;
pub mod portrait;
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

/// Modèles dédiés, par identifiant de champion (registre partagé : une ligne par champion).
const MODELS: &[(&str, ChampionModelDef)] = &[];

/// `modelFor` : portrait > modèle dédié > modèle générique de la classe.
pub fn model_for(champion: &str, class: ChampionClass) -> ChampionModelDef {
    let base = MODELS
        .iter()
        .find(|(id, _)| *id == champion)
        .map_or_else(|| generic::generic_for(class), |(_, model)| *model);
    portrait::portrait_model(champion, base)
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
