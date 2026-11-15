//! Système des buffs : port de `packages/server/src/systems/buffs.ts`.
//!
//! `init` branche les hooks de contrôle et de mitigation (`combat::buffs::init_buffs`), `update`
//! expire les buffs et recalcule les stats.

use crate::combat::buffs::{init_buffs, update_buffs_with};
use crate::world::System;

pub fn buffs_system() -> System {
    // Tampon d'ids réutilisé d'un tick à l'autre.
    let mut ids = Vec::new();
    System::new("buffs", move |world, dt| {
        update_buffs_with(world, dt, &mut ids)
    })
    .with_init(init_buffs)
}
