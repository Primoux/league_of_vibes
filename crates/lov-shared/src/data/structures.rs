//! Stats des structures (port de `data/structures.ts`).
//! Équilibrage (mesures headless, branche codex-objectives-balance) : une tour externe à 5000 PV + 40 armure
//! + backdoor +200 ne mourait pas en 90 s même pour un ADC niveau 6. PV -25 % environ, armure -10,
//!   résistance anti-backdoor 200 -> 60 (la fortification à -50 % avant 3:00 reste).

use crate::types::TowerTier;
use serde::{Deserialize, Serialize};

/// Stats d'une tour (type défini ici : le TS l'écrit en ligne).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TowerStats {
    pub hp: f64,
    pub ad: f64,
    pub armor: f64,
    pub local_gold: f64,
    pub team_gold: f64,
}

/// `STRUCTURES.towers` : `Record<TowerTier, …>`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TowerTable {
    pub outer: TowerStats,
    pub inner: TowerStats,
    pub inhib: TowerStats,
    pub nexus: TowerStats,
}

impl TowerTable {
    pub const fn get(&self, tier: TowerTier) -> &TowerStats {
        match tier {
            TowerTier::Outer => &self.outer,
            TowerTier::Inner => &self.inner,
            TowerTier::Inhib => &self.inhib,
            TowerTier::Nexus => &self.nexus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Structures {
    pub towers: TowerTable,
    pub inhibitor_hp: f64,
    pub nexus_hp: f64,
    pub range: f64,
    pub plate_gold: f64,
    pub plate_radius: f64,
    pub backdoor_radius: f64,
    pub backdoor_resistance: f64,
    pub fortification_until: f64,
}

pub const STRUCTURES: Structures = Structures {
    towers: TowerTable {
        outer: TowerStats {
            hp: 3400.0,
            ad: 152.0,
            armor: 30.0,
            local_gold: 250.0,
            team_gold: 50.0,
        },
        inner: TowerStats {
            hp: 2600.0,
            ad: 170.0,
            armor: 45.0,
            local_gold: 300.0,
            team_gold: 50.0,
        },
        inhib: TowerStats {
            hp: 2700.0,
            ad: 190.0,
            armor: 60.0,
            local_gold: 300.0,
            team_gold: 50.0,
        },
        nexus: TowerStats {
            hp: 2200.0,
            ad: 210.0,
            armor: 60.0,
            local_gold: 50.0,
            team_gold: 50.0,
        },
    },
    inhibitor_hp: 3200.0,
    nexus_hp: 4500.0,
    range: 750.0,
    plate_gold: 125.0,
    plate_radius: 1000.0,
    backdoor_radius: 1000.0,
    backdoor_resistance: 60.0,
    fortification_until: 180.0,
};
