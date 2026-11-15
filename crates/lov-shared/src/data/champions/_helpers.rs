//! Aides communes aux fiches de champions (équivalent Rust de `_helpers.ts`).
//!
//! Le TS complétait chaque champion avec `stats()` (valeurs de base par défaut) et `ability()`
//! (`maxRank`, `cost`, `cooldown` par défaut). Ici les fiches sont générées développées, avec
//! toutes les valeurs explicites : il ne reste que la construction de la table `params`.

use crate::schema::AbilityParam;
use std::collections::BTreeMap;

/// Table `params` d'un sort : paires (clé, valeur) rangées dans un `BTreeMap`.
pub fn params<const N: usize>(p: [(&str, AbilityParam); N]) -> BTreeMap<String, AbilityParam> {
    p.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}
