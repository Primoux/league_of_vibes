//! FX dédiés des champions : un fichier par champion, qui enregistre ses handlers (`register_fx`). Les
//! sorts sans FX dédié utilisent les FX génériques (`fx/defaults.rs`).
//!
//! Portage de `packages/client/src/fx/champions/`. Registre partagé : chaque champion y ajoute sa ligne
//! `pub mod` et son appel à `register`.

use bevy::prelude::*;

pub mod corvin;
pub mod grondak;

/// Enregistre les FX de tous les champions.
pub struct ChampionFxPlugin;

impl Plugin for ChampionFxPlugin {
    fn build(&self, app: &mut App) {
        corvin::register(app);
        grondak::register(app);
    }
}
