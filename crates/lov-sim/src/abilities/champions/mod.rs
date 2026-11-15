//! Registre des kits de champions (équivalent de `abilities/registry.ts`).
//!
//! Ajouter le kit d'un champion = exactement deux lignes ici, plus son fichier `<nom>.rs` :
//!
//! 1. `pub mod <nom>;` (dans l'ordre alphabétique) ;
//! 2. `&<nom>::KIT,` dans `KITS` (dans l'ordre du TypeScript, `CHAMPION_ABILITIES` de `registry.ts`).
//!
//! `<nom>.rs` expose `pub static KIT: Kit = Kit::new("<nom>", [q, w, e, r])…` (voir le guide de
//! `abilities/mod.rs`). Rien d'autre à modifier : les systèmes, le moteur et les passifs parcourent
//! ce tableau.

pub mod grondak;
pub mod kaelen;

use crate::abilities::registry::Kit;

/// Les kits, une entrée par ligne.
// `rustfmt::skip` : une entrée par ligne quel que soit leur nombre, pour que chaque PR de champion
// n'ajoute qu'une ligne (rustfmt replierait la liste sur une ligne tant qu'elle est courte).
#[rustfmt::skip]
pub static KITS: &[&Kit] = &[
    &grondak::KIT,
    &kaelen::KIT,
];
