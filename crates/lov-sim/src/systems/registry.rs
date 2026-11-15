//! Registre ordonné des systèmes de simulation : port de `packages/server/src/systems/index.ts`.
//!
//! C'est un contrat partagé : l'ordre d'exécution ne change que par une PR « contract: ». Chaque
//! système s'y inscrit sur une ligne ; les dépendances d'ordre sont celles du TypeScript (les buffs
//! fixent stats et contrôles avant que quiconque agisse, l'économie après les morts du tick, la vision
//! en dernier pour le snapshot).

use crate::System;
use crate::systems::abilities::abilities_system;
use crate::systems::buffs::buffs_system;
use crate::systems::combat::combat_system;
use crate::systems::economy::economy_system;
use crate::systems::input::input_system;
use crate::systems::jungle::jungle_system;
use crate::systems::leveling::leveling_system;
use crate::systems::minions::minions_system;
use crate::systems::movement::movement_system;
use crate::systems::objectives::objectives_system;
use crate::systems::structures::structures_system;
use crate::systems::summoners::summoners_system;
use crate::systems::vision::vision_system;

/// Systèmes d'une partie complète, dans l'ordre d'exécution. Le combat central en fait partie : les
/// systèmes PvE lui délèguent armement, projectiles et hooks (`core_combat = true`).
pub fn create_systems() -> Vec<System> {
    vec![
        input_system(),
        buffs_system(),
        summoners_system(),
        abilities_system(),
        movement_system(),
        combat_system(),
        minions_system(true),
        structures_system(true),
        jungle_system(true),
        objectives_system(),
        leveling_system(),
        economy_system(),
        vision_system(),
    ]
}
