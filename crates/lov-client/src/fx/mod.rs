//! FX : particules, shaders, post-traitement, runtime des effets de sorts.
//!
//! Portage de `packages/client/src/fx/` (area `fx-core` pour le socle). Suivi : issue #111.

// Le socle arrive par tranches (#111) : la logique pure d'abord, puis les particules et les shaders qui
// l'utilisent. Jusque-là, rien dans le client ne l'appelle encore hors des tests.
#[allow(dead_code)]
pub mod core;
