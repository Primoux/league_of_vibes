//! Types cœur partagés serveur/client (port de `types.ts`).
//!
//! Le JSON est identique à celui du TypeScript : champs en camelCase, unions de littéraux en
//! énumérations aux mêmes chaînes, champs optionnels omis quand ils sont absents.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Team {
    Blue,
    Red,
}

pub const TEAMS: [Team; 2] = [Team::Blue, Team::Red];

pub const fn other_team(t: Team) -> Team {
    match t {
        Team::Blue => Team::Red,
        Team::Red => Team::Blue,
    }
}

/// `Team | 'neutral'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamOrNeutral {
    Blue,
    Red,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

pub type EntityId = u32;
pub type PlayerId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    Top,
    Mid,
    Bot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Top,
    Jungle,
    Mid,
    Bot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    Champion,
    Minion,
    Tower,
    Inhibitor,
    Nexus,
    Monster,
    Ward,
    Projectile,
    /// Zones persistantes (blizzard, piège...).
    Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MinionType {
    Melee,
    Caster,
    Cannon,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TowerTier {
    Outer,
    Inner,
    Inhib,
    Nexus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonsterType {
    BlueBuff,
    RedBuff,
    Wolves,
    WolfSmall,
    Raptors,
    RaptorSmall,
    Krugs,
    KrugSmall,
    Gromp,
    Scuttle,
    Dragon,
    ElderDragon,
    Grub,
    Herald,
    Leviathan,
}

impl MonsterType {
    /// La chaîne exacte du protocole (utile pour construire des ids, ex. `blue_gromp`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BlueBuff => "blue_buff",
            Self::RedBuff => "red_buff",
            Self::Wolves => "wolves",
            Self::WolfSmall => "wolf_small",
            Self::Raptors => "raptors",
            Self::RaptorSmall => "raptor_small",
            Self::Krugs => "krugs",
            Self::KrugSmall => "krug_small",
            Self::Gromp => "gromp",
            Self::Scuttle => "scuttle",
            Self::Dragon => "dragon",
            Self::ElderDragon => "elder_dragon",
            Self::Grub => "grub",
            Self::Herald => "herald",
            Self::Leviathan => "leviathan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DragonElement {
    Infernal,
    Ocean,
    Mountain,
    Cloud,
    Hextech,
    Chemtech,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbilitySlot {
    Q,
    W,
    E,
    R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SummonerSlot {
    D,
    F,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummonerId {
    Flash,
    Ignite,
    Heal,
    Barrier,
    Exhaust,
    Teleport,
    Smite,
    Ghost,
    Cleanse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DamageType {
    Physical,
    Magic,
    True,
}

/// `Record<AbilitySlot, T>` : une valeur par sort Q/W/E/R.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityMap<T> {
    #[serde(rename = "Q")]
    pub q: T,
    #[serde(rename = "W")]
    pub w: T,
    #[serde(rename = "E")]
    pub e: T,
    #[serde(rename = "R")]
    pub r: T,
}

impl<T> AbilityMap<T> {
    pub fn get(&self, slot: AbilitySlot) -> &T {
        match slot {
            AbilitySlot::Q => &self.q,
            AbilitySlot::W => &self.w,
            AbilitySlot::E => &self.e,
            AbilitySlot::R => &self.r,
        }
    }
}

/// `Record<SummonerSlot, T>` : une valeur par sort d'invocateur D/F.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummonerMap<T> {
    #[serde(rename = "D")]
    pub d: T,
    #[serde(rename = "F")]
    pub f: T,
}

impl<T> SummonerMap<T> {
    pub fn get(&self, slot: SummonerSlot) -> &T {
        match slot {
            SummonerSlot::D => &self.d,
            SummonerSlot::F => &self.f,
        }
    }
}

/// `Record<AbilitySlot | SummonerSlot, number>` : secondes restantes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cooldowns {
    #[serde(rename = "Q")]
    pub q: f64,
    #[serde(rename = "W")]
    pub w: f64,
    #[serde(rename = "E")]
    pub e: f64,
    #[serde(rename = "R")]
    pub r: f64,
    #[serde(rename = "D")]
    pub d: f64,
    #[serde(rename = "F")]
    pub f: f64,
}

/// Statistiques calculées (base + niveau + objets + buffs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub max_hp: f64,
    /// Par 5 s.
    pub hp_regen: f64,
    pub max_mana: f64,
    /// Par 5 s.
    pub mana_regen: f64,
    pub ad: f64,
    pub ap: f64,
    pub armor: f64,
    pub mr: f64,
    /// Attaques / s.
    pub attack_speed: f64,
    pub attack_range: f64,
    pub move_speed: f64,
    /// 0..1
    pub crit_chance: f64,
    pub ability_haste: f64,
    /// 0..1
    pub lifesteal: f64,
    /// Flat (lethality simplifiée).
    pub armor_pen: f64,
    /// Flat.
    pub magic_pen: f64,
}

/// `Partial<Stats>`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp_regen: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mana: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana_regen: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mr: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_range: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crit_chance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_haste: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifesteal: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor_pen: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magic_pen: Option<f64>,
}

/// Un buff visible sur une entité.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSnapshot {
    pub id: String,
    pub remaining: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacks: Option<u32>,
}

/// Données champion visibles par tous.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSnapshot {
    pub player_id: PlayerId,
    pub name: String,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub items: Vec<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respawn_in: Option<f64>,
}

/// Ce que le client reçoit pour chaque entité visible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub kind: EntityKind,
    pub team: TeamOrNeutral,
    pub x: f64,
    pub y: f64,
    /// Radians, 0 = +x.
    pub facing: f64,
    pub hp: f64,
    pub max_hp: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mana: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead: Option<bool>,
    /// Type précis : championId, MinionType, TowerTier, MonsterType, etc. (ouvert).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Animation courante pour le client (idle/run/attack/cast_Q/death...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffs: Option<Vec<BuffSnapshot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub champ: Option<ChampSnapshot>,
}

/// Charges du trinket de vision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WardCharges {
    pub charges: u32,
    pub cooldown_remaining: f64,
}

/// Données privées envoyées uniquement au joueur propriétaire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfState {
    pub entity_id: EntityId,
    pub gold: f64,
    pub xp: f64,
    pub xp_to_next: f64,
    pub level: u32,
    pub skill_points: u32,
    pub ability_levels: AbilityMap<u32>,
    /// Secondes restantes.
    pub cooldowns: Cooldowns,
    pub summoners: SummonerMap<SummonerId>,
    pub stats: Stats,
    /// Secondes restantes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recalling: Option<f64>,
    pub can_shop: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ward_charges: Option<WardCharges>,
    /// Charges par emplacement d'inventaire, `null` si sans objet ou sans charges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_charges: Option<Vec<Option<f64>>>,
    /// Recharge restante (s) de l'actif de chaque emplacement, `null` si aucune.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_cooldowns: Option<Vec<Option<f64>>>,
}

/// `'tower' | 'inhibitor' | 'nexus'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StructureKind {
    Tower,
    Inhibitor,
    Nexus,
}

/// Valeur d'un paramètre d'annonce : `string | number`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Num(f64),
    Str(String),
}

/// Événements émis par la simulation, transmis aux clients dans les snapshots.
/// Discriminant : champ `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GameEvent {
    Damage {
        source: EntityId,
        target: EntityId,
        amount: f64,
        damage_type: DamageType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crit: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ability: Option<String>,
    },
    Heal {
        source: EntityId,
        target: EntityId,
        amount: f64,
    },
    Shield {
        source: EntityId,
        target: EntityId,
        amount: f64,
    },
    Death {
        target: EntityId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        killer: Option<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assists: Option<Vec<EntityId>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bounty: Option<f64>,
    },
    Respawn {
        target: EntityId,
        pos: Vec2,
    },
    AutoAttack {
        source: EntityId,
        target: EntityId,
        ranged: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crit: Option<bool>,
    },
    AbilityCast {
        source: EntityId,
        champion_id: String,
        slot: AbilitySlot,
        ability_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_pos: Option<Vec2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
    },
    SummonerCast {
        source: EntityId,
        spell: SummonerId,
        from: Vec2,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Vec2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
    },
    ProjectileSpawn {
        id: EntityId,
        source: EntityId,
        fx_key: String,
        from: Vec2,
        to: Vec2,
        speed: f64,
        width: f64,
    },
    ProjectileHit {
        id: EntityId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<EntityId>,
        pos: Vec2,
        fx_key: String,
    },
    Dash {
        source: EntityId,
        from: Vec2,
        to: Vec2,
        duration: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
    },
    AreaEffect {
        source: EntityId,
        fx_key: String,
        pos: Vec2,
        radius: f64,
        duration: f64,
    },
    BuffApplied {
        target: EntityId,
        buff_id: String,
        duration: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stacks: Option<u32>,
    },
    BuffRemoved {
        target: EntityId,
        buff_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
    },
    LevelUp {
        target: EntityId,
        level: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fx_key: Option<String>,
    },
    GoldGain {
        target: EntityId,
        amount: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pos: Option<Vec2>,
    },
    RecallStart {
        target: EntityId,
    },
    RecallEnd {
        target: EntityId,
        completed: bool,
    },
    ItemBought {
        target: EntityId,
        item_id: String,
    },
    ItemSold {
        target: EntityId,
        item_id: String,
    },
    ItemUsed {
        target: EntityId,
        item_id: String,
    },
    StructureDestroyed {
        target: EntityId,
        kind: StructureKind,
        team: Team,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plate: Option<bool>,
    },
    ObjectiveSpawn {
        monster: MonsterType,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        element: Option<DragonElement>,
        pos: Vec2,
    },
    ObjectiveTaken {
        monster: MonsterType,
        team: Team,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        killer: Option<EntityId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        element: Option<DragonElement>,
    },
    Announcement {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team: Option<Team>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<BTreeMap<String, ParamValue>>,
    },
    Ping {
        player_id: PlayerId,
        pos: Vec2,
        kind: String,
    },
    Chat {
        player_id: PlayerId,
        text: String,
        team_only: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_et_noms_de_champs() {
        let e = GameEvent::AbilityCast {
            source: 1,
            champion_id: "grondak".into(),
            slot: AbilitySlot::Q,
            ability_id: "grondak_q".into(),
            target_pos: None,
            target_id: Some(7),
            fx_key: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(
            s,
            r#"{"type":"abilityCast","source":1,"championId":"grondak","slot":"Q","abilityId":"grondak_q","targetId":7}"#
        );
        let back: GameEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn unions_de_litteraux() {
        assert_eq!(
            serde_json::to_string(&TeamOrNeutral::Neutral).unwrap(),
            "\"neutral\""
        );
        assert_eq!(
            serde_json::to_string(&MonsterType::ElderDragon).unwrap(),
            "\"elder_dragon\""
        );
        assert_eq!(MonsterType::WolfSmall.as_str(), "wolf_small");
        assert_eq!(other_team(Team::Blue), Team::Red);
    }

    #[test]
    fn annonce_params_ouverts() {
        let j = r#"{"type":"announcement","key":"k","params":{"a":1,"b":"x"}}"#;
        let e: GameEvent = serde_json::from_str(j).unwrap();
        let GameEvent::Announcement {
            params: Some(p), ..
        } = &e
        else {
            panic!("annonce attendue");
        };
        assert_eq!(p["a"], ParamValue::Num(1.0));
        assert_eq!(p["b"], ParamValue::Str("x".into()));
    }
}
