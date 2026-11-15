//! Stats des monstres de jungle et d'objectifs (port de `data/monsters.ts`).
//! Équilibrage (mesures headless, branche codex-objectives-balance) : les camps étaient ~3x trop longs à clear
//! pour un jungler niveau 3 (gromp ~60 s) et leurs dégâts de base tuaient un jungler 1v1.
//! PV réduits d'environ 35-40 %, dégâts réduits d'environ 25-30 %, armure 20 -> 15.

use crate::types::{DragonElement, MonsterType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterDef {
    pub hp: f64,
    pub ad: f64,
    pub armor: f64,
    pub range: f64,
    pub gold: f64,
    pub xp: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_gold: Option<f64>,
}

/// `MONSTERS` : paires (type, définition) dans l'ordre du TypeScript.
pub const MONSTERS: [(MonsterType, MonsterDef); 15] = [
    (
        MonsterType::BlueBuff,
        MonsterDef {
            hp: 1500.0,
            ad: 50.0,
            armor: 15.0,
            range: 175.0,
            gold: 90.0,
            xp: 280.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::RedBuff,
        MonsterDef {
            hp: 1500.0,
            ad: 50.0,
            armor: 15.0,
            range: 175.0,
            gold: 90.0,
            xp: 280.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Gromp,
        MonsterDef {
            hp: 1350.0,
            ad: 55.0,
            armor: 15.0,
            range: 250.0,
            gold: 90.0,
            xp: 280.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Wolves,
        MonsterDef {
            hp: 1100.0,
            ad: 32.0,
            armor: 15.0,
            range: 125.0,
            gold: 55.0,
            xp: 160.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::WolfSmall,
        MonsterDef {
            hp: 300.0,
            ad: 9.0,
            armor: 0.0,
            range: 125.0,
            gold: 7.0,
            xp: 25.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Raptors,
        MonsterDef {
            hp: 750.0,
            ad: 15.0,
            armor: 15.0,
            range: 125.0,
            gold: 30.0,
            xp: 110.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::RaptorSmall,
        MonsterDef {
            hp: 240.0,
            ad: 9.0,
            armor: 0.0,
            range: 125.0,
            gold: 7.0,
            xp: 25.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Krugs,
        MonsterDef {
            hp: 1100.0,
            ad: 45.0,
            armor: 15.0,
            range: 125.0,
            gold: 65.0,
            xp: 180.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::KrugSmall,
        MonsterDef {
            hp: 300.0,
            ad: 9.0,
            armor: 0.0,
            range: 125.0,
            gold: 7.0,
            xp: 25.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Scuttle,
        MonsterDef {
            hp: 1100.0,
            ad: 0.0,
            armor: 15.0,
            range: 0.0,
            gold: 55.0,
            xp: 180.0,
            team_gold: None,
        },
    ),
    (
        MonsterType::Dragon,
        MonsterDef {
            hp: 5000.0,
            ad: 70.0,
            armor: 25.0,
            range: 500.0,
            gold: 25.0,
            xp: 240.0,
            team_gold: Some(25.0),
        },
    ),
    (
        MonsterType::ElderDragon,
        MonsterDef {
            hp: 9000.0,
            ad: 115.0,
            armor: 50.0,
            range: 500.0,
            gold: 250.0,
            xp: 750.0,
            team_gold: Some(250.0),
        },
    ),
    (
        MonsterType::Grub,
        MonsterDef {
            hp: 1300.0,
            ad: 28.0,
            armor: 15.0,
            range: 200.0,
            gold: 20.0,
            xp: 120.0,
            team_gold: Some(20.0),
        },
    ),
    (
        MonsterType::Herald,
        MonsterDef {
            hp: 6500.0,
            ad: 75.0,
            armor: 40.0,
            range: 250.0,
            gold: 100.0,
            xp: 320.0,
            team_gold: Some(100.0),
        },
    ),
    (
        MonsterType::Leviathan,
        MonsterDef {
            hp: 10000.0,
            ad: 120.0,
            armor: 65.0,
            range: 600.0,
            gold: 300.0,
            xp: 900.0,
            team_gold: Some(300.0),
        },
    ),
];

/// `MONSTERS[type]`.
pub const fn monster(kind: MonsterType) -> &'static MonsterDef {
    match kind {
        MonsterType::BlueBuff => &MONSTERS[0].1,
        MonsterType::RedBuff => &MONSTERS[1].1,
        MonsterType::Gromp => &MONSTERS[2].1,
        MonsterType::Wolves => &MONSTERS[3].1,
        MonsterType::WolfSmall => &MONSTERS[4].1,
        MonsterType::Raptors => &MONSTERS[5].1,
        MonsterType::RaptorSmall => &MONSTERS[6].1,
        MonsterType::Krugs => &MONSTERS[7].1,
        MonsterType::KrugSmall => &MONSTERS[8].1,
        MonsterType::Scuttle => &MONSTERS[9].1,
        MonsterType::Dragon => &MONSTERS[10].1,
        MonsterType::ElderDragon => &MONSTERS[11].1,
        MonsterType::Grub => &MONSTERS[12].1,
        MonsterType::Herald => &MONSTERS[13].1,
        MonsterType::Leviathan => &MONSTERS[14].1,
    }
}

pub const DRAGON_ELEMENTS: [DragonElement; 6] = [
    DragonElement::Infernal,
    DragonElement::Ocean,
    DragonElement::Mountain,
    DragonElement::Cloud,
    DragonElement::Hextech,
    DragonElement::Chemtech,
];
