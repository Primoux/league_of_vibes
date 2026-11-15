//! Récompenses et stats de sbires (port de `data/minions.ts`), consommées par economy/leveling via
//! `entity.data.rewards`.
//! Équilibrage (mesures headless, branche codex-objectives-balance) : dégâts -30 % (avant : une
//! vague tuait un tank lvl6 en ~31 s et un squishy en ~5 s). PV ajustés à la baisse pour garder
//! la même durée de combat de vague (sinon le test d'endurance minions.test.ts ralentit en CI).

use crate::types::MinionType;
use serde::{Deserialize, Serialize};

/// Valeur de `MINIONS[type]` (type défini ici : le TS l'écrit en ligne).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinionDef {
    pub hp: f64,
    pub ad: f64,
    pub armor: f64,
    pub range: f64,
    pub speed: f64,
    pub attack_speed: f64,
    pub gold: f64,
    pub xp: f64,
}

/// `MINIONS` : paires (type, définition) dans l'ordre du TypeScript.
pub const MINIONS: [(MinionType, MinionDef); 4] = [
    (
        MinionType::Melee,
        MinionDef {
            hp: 370.0,
            ad: 10.0,
            armor: 0.0,
            range: 125.0,
            speed: 325.0,
            attack_speed: 1.25,
            gold: 21.0,
            xp: 60.0,
        },
    ),
    (
        MinionType::Caster,
        MinionDef {
            hp: 230.0,
            ad: 17.0,
            armor: 0.0,
            range: 550.0,
            speed: 325.0,
            attack_speed: 0.67,
            gold: 14.0,
            xp: 29.0,
        },
    ),
    (
        MinionType::Cannon,
        MinionDef {
            hp: 700.0,
            ad: 32.0,
            armor: 0.0,
            range: 300.0,
            speed: 325.0,
            attack_speed: 1.0,
            gold: 60.0,
            xp: 93.0,
        },
    ),
    (
        MinionType::Super,
        MinionDef {
            hp: 1240.0,
            ad: 190.0,
            armor: 100.0,
            range: 170.0,
            speed: 325.0,
            attack_speed: 0.85,
            gold: 90.0,
            xp: 97.0,
        },
    ),
];

/// `MINIONS[type]`.
pub const fn minion(kind: MinionType) -> &'static MinionDef {
    match kind {
        MinionType::Melee => &MINIONS[0].1,
        MinionType::Caster => &MINIONS[1].1,
        MinionType::Cannon => &MINIONS[2].1,
        MinionType::Super => &MINIONS[3].1,
    }
}
