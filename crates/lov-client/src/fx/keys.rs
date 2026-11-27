//! Traduction `GameEvent` → (fxKey, phase). Logique pure, testée.
//!
//! Portage de `packages/client/src/fx/keys.ts`. Une même fxKey (ex. `ilyra_w`) couvre plusieurs moments
//! d'un sort, chacun porté par un événement différent : `abilityCast` (anticipation) → `areaEffect`
//! (télégraphie, zone) → `projectileHit` (impact)…

use lov_shared::data::champions::get_champion;
use lov_shared::types::{AbilitySlot, GameEvent, SummonerId};

/// Moment d'un sort auquel un FX s'attache (`FxPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxPhase {
    Cast,
    Projectile,
    Hit,
    Area,
    Dash,
    Buff,
    BuffEnd,
    Summoner,
}

/// Clé et phase d'un événement (`ResolvedFx`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFx {
    pub key: String,
    pub phase: FxPhase,
}

/// Clé du sort `slot` d'un champion : `ilyra` + `R` → `ilyra_r`.
pub fn ability_fx_key(champion_id: &str, slot: AbilitySlot) -> String {
    let s = match slot {
        AbilitySlot::Q => "q",
        AbilitySlot::W => "w",
        AbilitySlot::E => "e",
        AbilitySlot::R => "r",
    };
    format!("{champion_id}_{s}")
}

/// Nom d'un sort d'invocateur dans le protocole (`flash`, `ignite`…).
pub fn summoner_name(s: SummonerId) -> &'static str {
    match s {
        SummonerId::Flash => "flash",
        SummonerId::Ignite => "ignite",
        SummonerId::Heal => "heal",
        SummonerId::Barrier => "barrier",
        SummonerId::Exhaust => "exhaust",
        SummonerId::Teleport => "teleport",
        SummonerId::Smite => "smite",
        SummonerId::Ghost => "ghost",
        SummonerId::Cleanse => "cleanse",
    }
}

/// Résolution de la fxKey d'un buff quand l'événement n'en porte pas (`setBuffFxResolver`) : l'id du buff
/// par défaut. Le serveur renseigne normalement `fxKey` : la clé du sort qui a posé le buff, ou `cc_stun`…
pub type BuffFxResolver = fn(&str) -> String;

pub fn default_buff_fx(buff_id: &str) -> String {
    buff_id.to_owned()
}

/// Clé et phase si l'événement déclenche un FX de sort, de buff ou d'invocateur (`resolveFx`).
pub fn resolve_fx(e: &GameEvent, buff_fx: BuffFxResolver) -> Option<ResolvedFx> {
    let (key, phase) = match e {
        GameEvent::AbilityCast {
            fx_key,
            champion_id,
            slot,
            ..
        } => (
            fx_key
                .clone()
                .unwrap_or_else(|| ability_fx_key(champion_id, *slot)),
            FxPhase::Cast,
        ),
        GameEvent::ProjectileSpawn { fx_key, .. } => (fx_key.clone(), FxPhase::Projectile),
        GameEvent::ProjectileHit { fx_key, .. } => (fx_key.clone(), FxPhase::Hit),
        GameEvent::AreaEffect { fx_key, .. } => (fx_key.clone(), FxPhase::Area),
        GameEvent::Dash { fx_key, .. } => (
            fx_key.clone().unwrap_or_else(|| "dash".into()),
            FxPhase::Dash,
        ),
        GameEvent::BuffApplied {
            fx_key, buff_id, ..
        } => (
            fx_key.clone().unwrap_or_else(|| buff_fx(buff_id)),
            FxPhase::Buff,
        ),
        GameEvent::BuffRemoved {
            fx_key, buff_id, ..
        } => (
            fx_key.clone().unwrap_or_else(|| buff_fx(buff_id)),
            FxPhase::BuffEnd,
        ),
        GameEvent::SummonerCast { fx_key, spell, .. } => (
            fx_key
                .clone()
                .unwrap_or_else(|| format!("sum_{}", summoner_name(*spell))),
            FxPhase::Summoner,
        ),
        _ => return None,
    };
    Some(ResolvedFx { key, phase })
}

/// Zone de télégraphie d'un sort à effet retardé : le serveur émet `<fxKey>_telegraph` puis `<fxKey>`.
pub const TELEGRAPH_SUFFIX: &str = "_telegraph";

pub fn is_telegraph_key(key: &str) -> bool {
    key.ends_with(TELEGRAPH_SUFFIX)
}

/// Champion propriétaire d'une fxKey (`ilyra_r_shard` → `ilyra`), s'il existe.
pub fn champion_of_key(key: &str) -> Option<&'static str> {
    let id = key.split_once('_').map_or(key, |(id, _)| id);
    get_champion(id).map(|c| c.id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::Vec2;

    const O: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    fn resolved(key: &str, phase: FxPhase) -> Option<ResolvedFx> {
        Some(ResolvedFx {
            key: key.into(),
            phase,
        })
    }

    #[test]
    fn ability_cast_donne_champion_et_emplacement() {
        let e = GameEvent::AbilityCast {
            source: 1,
            champion_id: "ilyra".into(),
            slot: AbilitySlot::R,
            ability_id: "ilyra_r".into(),
            target_pos: None,
            target_id: None,
            fx_key: None,
        };
        assert_eq!(
            resolve_fx(&e, default_buff_fx),
            resolved("ilyra_r", FxPhase::Cast)
        );
    }

    #[test]
    fn evenements_porteurs_de_fx_key() {
        let area = GameEvent::AreaEffect {
            source: 1,
            fx_key: "grondak_r".into(),
            pos: O,
            radius: 400.0,
            duration: 0.75,
        };
        assert_eq!(
            resolve_fx(&area, default_buff_fx),
            resolved("grondak_r", FxPhase::Area)
        );
        let hit = GameEvent::ProjectileHit {
            id: 9,
            target: None,
            pos: O,
            fx_key: "ilyra_q".into(),
        };
        assert_eq!(
            resolve_fx(&hit, default_buff_fx).map(|r| r.phase),
            Some(FxPhase::Hit)
        );
        let dash = GameEvent::Dash {
            source: 1,
            from: O,
            to: Vec2 { x: 1.0, y: 1.0 },
            duration: 0.2,
            fx_key: None,
        };
        assert_eq!(
            resolve_fx(&dash, default_buff_fx),
            resolved("dash", FxPhase::Dash)
        );
    }

    #[test]
    fn sorts_d_invocateur_et_buffs() {
        let flash = GameEvent::SummonerCast {
            source: 1,
            spell: SummonerId::Flash,
            from: O,
            to: None,
            target_id: None,
            fx_key: None,
        };
        assert_eq!(
            resolve_fx(&flash, default_buff_fx),
            resolved("sum_flash", FxPhase::Summoner)
        );
        let removed = GameEvent::BuffRemoved {
            target: 1,
            buff_id: "stun".into(),
            fx_key: None,
        };
        assert_eq!(
            resolve_fx(&removed, default_buff_fx),
            resolved("stun", FxPhase::BuffEnd)
        );
    }

    #[test]
    fn ignore_les_evenements_sans_fx_de_sort() {
        let chat = GameEvent::Chat {
            player_id: "p".into(),
            text: "gg".into(),
            team_only: false,
        };
        assert_eq!(resolve_fx(&chat, default_buff_fx), None);
    }

    #[test]
    fn trouve_le_champion_depuis_la_fx_key() {
        assert_eq!(champion_of_key("ilyra_r"), Some("ilyra"));
        assert_eq!(champion_of_key("zephyr_q_return"), Some("zephyr"));
        assert_eq!(champion_of_key("sum_flash"), None);
        assert!(is_telegraph_key("grondak_r_telegraph"));
    }
}
