//! FX : particules, shaders, post-traitement, runtime des effets de sorts.
//!
//! Portage de `packages/client/src/fx/` (area `fx-core` pour le socle). Suivi : issue #111.

use bevy::prelude::*;

// Le pool, le minuteur et les easings servent au runtime des effets (`FxRuntime`), porté dans une PR
// suivante de #111 ; jusque-là, seuls les tests les appellent.
#[allow(dead_code)]
pub mod core;
pub mod demo;
pub mod particles;

/// Socle FX : à ajouter une fois au client.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(particles::ParticlesPlugin);
    }
}
