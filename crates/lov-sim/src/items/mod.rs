//! Objets (port de `packages/server/src/items`) : état économique des entités, passifs et actifs.
//!
//! La boutique (achat, vente, annulation) et le rappel vivent dans le système économie
//! (`systems/economy.rs`), qui appelle ce module.

pub mod handlers;
pub mod state;

pub use handlers::*;
pub use state::*;
