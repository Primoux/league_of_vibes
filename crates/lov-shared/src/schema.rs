//! Schémas de données : champions, sorts, buffs, objets, sorts d'invocateur (port de `schema.ts`).
//! Seuls les TYPES sont portés ici ; les tables (`data/**`) viendront avec leurs areas.

use crate::types::{AbilityMap, AbilitySlot, DamageType, PartialStats, Role, Stats, SummonerId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChampionClass {
    Tank,
    Fighter,
    Mage,
    Marksman,
    Assassin,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Targeting {
    /// Projectile en ligne (width, speed, range).
    Skillshot,
    /// Cible unitaire.
    Targeted,
    /// Point au sol (zone).
    Ground,
    Cone,
    #[serde(rename = "self")]
    SelfCast,
    /// Déplacement vers un point.
    Dash,
    /// Déplacement vers une cible.
    TargetedDash,
    Global,
}

/// Ratio de scaling : valeur = base[rank] + ad*adRatio + ap*apRatio + ...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scaling {
    /// Une valeur par rang du sort.
    pub base: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_ad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus_hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<f64>,
    /// % PV max de la cible (0.05 = 5 %).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_max_hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_missing_hp: Option<f64>,
}

/// `Scaling & { type: DamageType }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityDamage {
    #[serde(flatten)]
    pub scaling: Scaling,
    #[serde(rename = "type")]
    pub damage_type: DamageType,
}

/// `AbilitySlot | 'P'` (P = passif).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityDefSlot {
    Q,
    W,
    E,
    R,
    P,
}

impl From<AbilitySlot> for AbilityDefSlot {
    fn from(s: AbilitySlot) -> Self {
        match s {
            AbilitySlot::Q => Self::Q,
            AbilitySlot::W => Self::W,
            AbilitySlot::E => Self::E,
            AbilitySlot::R => Self::R,
        }
    }
}

/// `number | number[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AbilityParam {
    One(f64),
    Many(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityDef {
    /// Ex. `grondak_q`.
    pub id: String,
    pub slot: AbilityDefSlot,
    pub name: String,
    /// Texte joueur (FR).
    pub description: String,
    pub targeting: Targeting,
    /// 5 pour QWE, 3 pour R, 0 pour passif.
    pub max_rank: u32,
    /// Secondes par rang.
    pub cooldown: Vec<f64>,
    /// Mana par rang.
    pub cost: Vec<f64>,
    pub range: f64,
    /// Zone / cône.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Skillshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Projectile / dash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// Secondes d'immobilisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<AbilityDamage>,
    /// Clé FX : convention `<championId>_<slot>[_<variante>]`.
    pub fx_key: String,
    /// Paramètres libres spécifiques au kit (durées de CC, %, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, AbilityParam>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resource {
    Mana,
    Energy,
    None,
}

/// Couleurs principales pour modèle/FX (hex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub primary: String,
    pub secondary: String,
    pub accent: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionDef {
    /// Minuscules, ex. `grondak`.
    pub id: String,
    pub name: String,
    pub title: String,
    pub class: ChampionClass,
    pub roles: Vec<Role>,
    /// 1, 2 ou 3.
    pub difficulty: u8,
    pub ranged: bool,
    pub resource: Resource,
    pub base_stats: Stats,
    /// Croissance appliquée via `stat_at_level`.
    pub per_level: PartialStats,
    /// Fraction de l'attaque avant l'impact (0.25 typique).
    pub attack_windup: f64,
    /// Auto-attaques à distance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projectile_speed: Option<f64>,
    pub passive: AbilityDef,
    pub abilities: AbilityMap<AbilityDef>,
    pub palette: Palette,
    pub lore: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuffKind {
    Buff,
    Debuff,
    Cc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CcType {
    Stun,
    Root,
    Slow,
    Silence,
    Knockup,
    Taunt,
    Suppress,
    Fear,
    Blind,
    Grounded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffDef {
    pub id: String,
    pub name: String,
    pub kind: BuffKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc: Option<CcType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stacks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_mods: Option<PartialStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_percent_mods: Option<PartialStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerDef {
    pub id: SummonerId,
    pub name: String,
    pub description: String,
    pub cooldown: f64,
    /// Paramètres de gameplay résolus par le serveur (source de vérité).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, f64>>,
    pub targeting: Targeting,
    pub range: f64,
    pub fx_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemCategory {
    Starter,
    Consumable,
    Boots,
    Basic,
    Epic,
    Legendary,
    Trinket,
}

/// Passif nommé, résolu côté serveur par le système economy/items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPassive {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemActive {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cooldown: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ItemCategory,
    /// Coût total.
    pub cost: f64,
    pub builds_from: Vec<String>,
    pub stats: PartialStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive: Option<ItemPassive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<ItemActive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique: Option<bool>,
    /// Consommables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stacks: Option<u32>,
    /// `false` = non achetable (ex. Œil du colosse). Défaut true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shop: Option<bool>,
    /// Clé d'icône pour l'UI.
    pub icon: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degats_aplatis_et_params_ouverts() {
        let j = r#"{"base":[10.5,20.5],"ad":0.5,"type":"magic"}"#;
        let d: AbilityDamage = serde_json::from_str(j).unwrap();
        assert_eq!(d.damage_type, DamageType::Magic);
        assert_eq!(d.scaling.ad, Some(0.5));
        assert_eq!(serde_json::to_string(&d).unwrap(), j);
        let p: BTreeMap<String, AbilityParam> =
            serde_json::from_str(r#"{"a":1,"b":[1,2]}"#).unwrap();
        assert_eq!(p["b"], AbilityParam::Many(vec![1.0, 2.0]));
    }

    #[test]
    fn ciblage_self() {
        assert_eq!(
            serde_json::to_string(&Targeting::SelfCast).unwrap(),
            "\"self\""
        );
        assert_eq!(
            serde_json::to_string(&Targeting::TargetedDash).unwrap(),
            "\"targetedDash\""
        );
    }
}
