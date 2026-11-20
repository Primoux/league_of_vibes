//! Modèles low-poly procéduraux des unités non-champions : sbires, monstres, structures, balises.
//!
//! Portage de `packages/client/src/render/models/`. Chaque fabrique renvoie un `Built` : l'arbre du
//! modèle et la description de son rig, que la vue fait apparaître et anime (`rig::RigView`).
//!
//! Écart : le `MeshBatcher` du client web (fusion des maillages statiques pour réduire les draw calls)
//! n'est pas porté : Bevy regroupe déjà automatiquement les objets qui partagent maillage et matériau, et
//! les caches de `kit` font que toutes les unités d'un même type les partagent.

use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub mod geometry;
pub mod kit;
pub mod minions;
pub mod rig;
pub mod structures;

pub use kit::{Kit, MatOpts, Model, ModelMaterial};
pub use rig::{Rig, RigView};

/// Modèle construit, prêt à apparaître.
#[derive(Clone, Debug)]
pub struct Built {
    pub model: Model,
    pub rig: Rig<kit::N>,
}

impl Built {
    /// Fait apparaître le modèle sous `parent` (la vue) et renvoie son animateur.
    pub fn spawn(&self, commands: &mut Commands, parent: Entity, phase: f32) -> RigView {
        let entities = self.model.spawn(commands);
        commands
            .entity(entities[Model::ROOT])
            .insert(ChildOf(parent));
        RigView::new(&self.model, &self.rig, &entities, phase)
    }
}

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "rim.wesl");
        app.add_plugins(MaterialPlugin::<ModelMaterial>::default())
            .init_resource::<kit::ModelCache>();
    }
}
