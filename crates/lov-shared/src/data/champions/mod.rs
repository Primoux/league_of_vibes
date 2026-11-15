//! Registre des champions (port de `data/champions/index.ts`).
//!
//! Ajouter un champion = exactement deux lignes ici : `pub mod <nom>;` et une entrée dans
//! `CHAMPIONS` (dans l'ordre du TypeScript, `CHAMPION_LIST` de `index.ts`). Rien d'autre à modifier,
//! le test `tests/champions.rs` couvre automatiquement tout champion enregistré.

mod _helpers;
pub mod corvin;
pub mod grondak;

use crate::schema::ChampionDef;
use std::sync::LazyLock;

/// `CHAMPION_LIST` : les champions, dans l'ordre du TypeScript.
// `rustfmt::skip` : une entrée par ligne quel que soit leur nombre, pour que chaque PR de champion
// n'ajoute qu'une ligne (rustfmt replierait la liste sur une ligne tant qu'elle est courte).
#[rustfmt::skip]
pub static CHAMPIONS: LazyLock<Vec<&'static ChampionDef>> = LazyLock::new(|| {
    vec![
        &*grondak::CHAMPION,
        &*corvin::CHAMPION,
    ]
});

/// Équivalent de `getChampion(id)`, sans panique : `None` pour un id inconnu.
///
/// Le TypeScript lève une erreur (« Champion inconnu »), mais un id de champion peut venir du
/// réseau (draft) : l'appelant décide quoi faire d'un id invalide.
pub fn get_champion(id: &str) -> Option<&'static ChampionDef> {
    CHAMPIONS.iter().copied().find(|c| c.id == id)
}
