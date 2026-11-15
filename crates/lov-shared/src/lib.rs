//! Contrats partagés serveur/client de League of Vibes.
//!
//! Port fidèle de `packages/shared/src` (hors `data/**`) : le JSON produit et lu ici est
//! identique à celui du serveur Node et du client web (protocole v2).

pub mod constants;
pub mod data;
pub mod map;
pub mod math;
pub mod protocol;
pub mod schema;
pub mod types;
