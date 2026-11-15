//! Système des sorts : port de `packages/server/src/systems/abilities.ts`.
//!
//! `init` installe les passifs (`init_passives`), `update` exécute `update_abilities`. À placer après
//! `buffs` (stats et CC avant que quiconque agisse) et avant `movement` (qui avance les dashes).

use crate::abilities::engine::{AbilityScratch, update_abilities_with};
use crate::abilities::passives::init_passives;
use crate::world::System;

pub fn abilities_system() -> System {
    // Tampons réutilisés d'un tick à l'autre.
    let mut scratch = AbilityScratch::default();
    System::new("abilities", move |world, dt| {
        update_abilities_with(world, dt, &mut scratch)
    })
    .with_init(init_passives)
}
