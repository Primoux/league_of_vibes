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
pub mod shaders;

/// Socle FX : à ajouter une fois au client.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((particles::ParticlesPlugin, shaders::ShadersPlugin));
    }
}

/// Ordre de dessin d'un FX transparent (`renderOrder` du client web : décals 5, particules 10 et 11,
/// rubans 12), traduit en biais de tri pour `Material::depth_bias`. Bevy trie les objets transparents par
/// profondeur de vue + biais : un écart bien plus grand que la scène fait primer l'ordre sur la distance,
/// comme dans three.js, et laisse les objets transparents ordinaires (biais 0) dessinés avant les FX.
pub(crate) fn render_order(order: f32) -> f32 {
    order * 1.0e5
}
