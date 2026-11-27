//! Briques de base des FX : courbes, pools, tampon circulaire, aléatoire, minuteur, palettes, suivi des
//! entités et runtime des effets.
//!
//! Portage de `packages/client/src/fx/core/`.

pub mod curves;
pub mod entities;
pub mod palette;
pub mod pool;
pub mod ring;
pub mod rng;
pub mod runtime;
pub mod scheduler;
