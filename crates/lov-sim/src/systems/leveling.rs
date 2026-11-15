//! Expérience et niveaux : port de `packages/server/src/systems/leveling.ts`.
//!
//! Gain et partage d'XP à la mort d'une entité, montée de niveau (points de compétence, recalcul des
//! stats, soin/mana proportionnels), commande `levelAbility`, régénération passive de PV et de mana.
//!
//! Différences avec le TypeScript :
//! - les fonctions désignent l'entité par son `EntityId` (le monde est emprunté mutablement) ;
//! - `init` ne s'exécute qu'une fois par monde par construction (`World::init`) : le `WeakSet`
//!   `initialized` du TS et son appel défensif depuis `update` n'ont pas d'équivalent ;
//! - `entity.data.xpReward / xpRewarded / xpGlobal` deviennent les types [`XpReward`], [`XpRewarded`],
//!   [`XpGlobal`] de la table `entity.data` ;
//! - l'XP portée par les données de domaine (`data.minion / jungle / monster`, champ `xp`) n'a pas
//!   d'équivalent : ces tables n'existent pas encore en Rust. Un système qui les porte peut poser
//!   [`XpReward`] à la place, qui a la même priorité que `data.xpReward` en TS ;
//! - un `championId` inconnu du registre ne lève pas d'erreur : la régénération de mana est ignorée.

use lov_shared::constants::{MAX_LEVEL, ULT_LEVELS};
use lov_shared::data::champions::get_champion;
use lov_shared::math::dist2;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::Resource;
use lov_shared::types::{AbilitySlot, EntityId, EntityKind, GameEvent, SummonerId, TeamOrNeutral};

use crate::combat::buffs::recalculate_stats;
use crate::combat::damage::{HealOpts, apply_heal};
use crate::world::{Entity, EventKind, System, World};

/// `data.xpReward` : XP donnée à la mort de l'entité (prioritaire sur toute autre source).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XpReward(pub f64);

/// `data.xpRewarded` : l'XP de cette entité a déjà été distribuée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XpRewarded(pub bool);

/// `data.xpGlobal` : l'XP est partagée par toute l'équipe adverse, sans condition de distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XpGlobal(pub bool);

/// Nombre de montées de niveau (niveaux 1 à 18).
const LEVEL_STEPS: usize = 17;

/// XP dans le niveau courant : 280 pour atteindre le niveau 2, puis +100 par niveau.
pub const XP_TO_NEXT: [f64; LEVEL_STEPS] = {
    let mut a = [0.0; LEVEL_STEPS];
    let mut i = 0;
    while i < LEVEL_STEPS {
        a[i] = 280.0 + (i as f64) * 100.0;
        i += 1;
    }
    a
};

/// Portée de partage de l'XP.
const XP_RANGE: f64 = 1400.0;

pub fn xp_to_next_level(level: u32) -> f64 {
    if level >= MAX_LEVEL {
        0.0
    } else {
        XP_TO_NEXT[level.saturating_sub(1) as usize]
    }
}

/// XP cumulée nécessaire pour atteindre `level`.
pub fn xp_for_level(level: u32) -> f64 {
    let n = (level.saturating_sub(1) as usize).min(LEVEL_STEPS);
    XP_TO_NEXT[..n].iter().fold(0.0, |a, b| a + b)
}

pub fn cooldown_with_haste(seconds: f64, haste: f64) -> f64 {
    seconds * 100.0 / (100.0 + haste.max(0.0))
}

/// Rang maximal d'une compétence au niveau actuel du champion.
pub fn max_ability_rank(e: &Entity, slot: AbilitySlot) -> u32 {
    let Some(c) = &e.champ else {
        return 0;
    };
    if slot == AbilitySlot::R {
        return ULT_LEVELS.iter().filter(|&&n| c.level >= n).count() as u32;
    }
    5.min(c.level.div_ceil(2))
}

/// Dépense un point de compétence ; faux si impossible.
pub fn level_ability(e: &mut Entity, slot: AbilitySlot) -> bool {
    let max = max_ability_rank(e, slot);
    let Some(c) = &mut e.champ else {
        return false;
    };
    let rank = match slot {
        AbilitySlot::Q => &mut c.ability_levels.q,
        AbilitySlot::W => &mut c.ability_levels.w,
        AbilitySlot::E => &mut c.ability_levels.e,
        AbilitySlot::R => &mut c.ability_levels.r,
    };
    if c.skill_points == 0 || *rank >= max {
        return false;
    }
    *rank += 1;
    c.skill_points -= 1;
    true
}

/// Ajoute de l'XP à un champion et applique les montées de niveau (événement `levelUp` à chacune).
pub fn grant_xp(world: &mut World, id: EntityId, amount: f64) {
    match world.get(id).and_then(|e| e.champ.as_ref()) {
        Some(c) if c.level < MAX_LEVEL && amount.is_finite() && amount > 0.0 => {}
        _ => return,
    }
    if let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut()) {
        c.xp += amount;
    }
    loop {
        let Some(e) = world.get_mut(id) else {
            return;
        };
        let Some(c) = &mut e.champ else {
            return;
        };
        if c.level >= MAX_LEVEL || c.xp + 1e-8 < xp_to_next_level(c.level) {
            break;
        }
        c.xp -= xp_to_next_level(c.level);
        let old_hp = e.stats.max_hp;
        let old_mana = e.stats.max_mana;
        c.level += 1;
        c.skill_points += 1;
        let level = c.level;
        recalculate_stats(world, id);
        let Some(e) = world.get_mut(id) else {
            return;
        };
        if !e.dead {
            e.hp = e.stats.max_hp.min(e.hp + e.stats.max_hp - old_hp);
        }
        e.mana = e.stats.max_mana.min(e.mana + e.stats.max_mana - old_mana);
        world.emit(GameEvent::LevelUp {
            target: id,
            level,
            fx_key: Some("level_up".into()),
        });
    }
    if let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut())
        && c.level >= MAX_LEVEL
    {
        c.xp = 0.0;
    }
}

/// XP donnée par la mort de `e`.
fn xp_reward(e: &Entity) -> f64 {
    if let Some(r) = e.data.get::<XpReward>() {
        return r.0;
    }
    if let Some(c) = &e.champ {
        return 50.0 + 50.0 * c.level as f64;
    }
    match e.kind {
        EntityKind::Minion => match e.subtype.as_deref().unwrap_or("melee") {
            "melee" => 60.45,
            "caster" => 29.76,
            "cannon" => 93.0,
            "super" => 97.0,
            _ => 60.0,
        },
        EntityKind::Monster => 100.0,
        _ => 0.0,
    }
}

/// Branche le hook `selfState` (`xpToNext`) et l'écouteur de mort qui distribue l'XP.
pub fn init_leveling(world: &mut World) {
    world.hooks.self_state.push(Box::new(|_, e, s| {
        if let Some(c) = &e.champ {
            s.xp_to_next = xp_to_next_level(c.level);
        }
    }));
    // Tampon des bénéficiaires, réutilisé d'une mort à l'autre.
    let mut recipients: Vec<EntityId> = Vec::new();
    world.events.on(EventKind::Death, move |world, ev| {
        let GameEvent::Death { target, killer, .. } = ev else {
            return;
        };
        let Some(victim) = world.get(*target) else {
            return;
        };
        if victim.data.get::<XpRewarded>().is_some_and(|x| x.0)
            || matches!(victim.kind, EntityKind::Projectile | EntityKind::Zone)
        {
            return;
        }
        let (victim_team, victim_pos, victim_kind) = (victim.team, victim.pos, victim.kind);
        let global = victim.data.get::<XpGlobal>().is_some_and(|x| x.0);
        let reward = xp_reward(victim);
        if let Some(v) = world.get_mut(*target) {
            v.data.insert(XpRewarded(true));
        }
        let team = match victim_team {
            TeamOrNeutral::Neutral => killer.and_then(|k| world.get(k)).map(|s| s.team),
            TeamOrNeutral::Blue => Some(TeamOrNeutral::Red),
            TeamOrNeutral::Red => Some(TeamOrNeutral::Blue),
        };
        let Some(team @ (TeamOrNeutral::Blue | TeamOrNeutral::Red)) = team else {
            return;
        };
        recipients.clear();
        for ally in world.query(Some(EntityKind::Champion)) {
            if !ally.dead
                && ally.team == team
                && ally.champ.is_some()
                && (global || dist2(ally.pos, victim_pos) <= XP_RANGE * XP_RANGE)
            {
                recipients.push(ally.id);
            }
        }
        let reward = reward / recipients.len().max(1) as f64;
        for &ally in &recipients {
            let has_smite = world
                .get(ally)
                .and_then(|a| a.champ.as_ref())
                .is_some_and(|c| {
                    c.summoners.d == SummonerId::Smite || c.summoners.f == SummonerId::Smite
                });
            let factor = if victim_kind == EntityKind::Monster && !has_smite {
                0.5
            } else {
                1.0
            };
            grant_xp(world, ally, reward * factor);
        }
    });
}

/// Le système d'expérience : commandes `levelAbility`, puis régénération passive des champions.
pub fn leveling_system() -> System {
    // Tampon d'ids réutilisé d'un tick à l'autre.
    let mut ids: Vec<EntityId> = Vec::new();
    System::new("leveling", move |world, dt| {
        for i in 0..world.commands.len() {
            let ClientMsg::LevelAbility { slot } = &world.commands[i].msg else {
                continue;
            };
            let slot = *slot;
            if let Some(id) = world.champion_id_of(&world.commands[i].player_id)
                && let Some(e) = world.get_mut(id)
            {
                level_ability(e, slot);
            }
        }
        world.ids_into(Some(EntityKind::Champion), &mut ids);
        for &id in &ids {
            let Some(e) = world.get_mut(id) else {
                continue;
            };
            let Some(c) = &e.champ else {
                continue;
            };
            if e.dead {
                continue;
            }
            let uses_mana =
                get_champion(&c.champion_id).is_some_and(|d| d.resource != Resource::None);
            if let Some(x) = e.data.get_mut::<XpRewarded>() {
                x.0 = false;
            }
            if e.hp < e.stats.max_hp {
                let amount = e.stats.hp_regen / 5.0 * dt;
                apply_heal(
                    world,
                    HealOpts {
                        silent: true,
                        ..HealOpts::new(Some(id), id, amount)
                    },
                );
            }
            if uses_mana && let Some(e) = world.get_mut(id) {
                e.mana = e.stats.max_mana.min(e.mana + e.stats.mana_regen / 5.0 * dt);
            }
        }
    })
    .with_init(init_leveling)
}
