//! Boucliers de sorts : port de `packages/server/src/abilities/effects.ts`.
//!
//! `apply_shield` est un vrai wrapper : il pose un buff portant `shield_id` (affichage, retrait par
//! `remove_buff`) puis le bouclier de `combat::damage`, et garde le plus grand des deux montants pour
//! un même couple (buff, source). `apply_damage` et `apply_heal` sont ré-exportés tels quels.
//!
//! Différence avec le TypeScript : la « vue vivante » `buff.shieldRemaining` (propriété posée sur
//! l'objet du buff) n'existe pas, `BuffInstance` étant une copie. `shield_remaining` lit la même
//! valeur, directement dans le registre des boucliers.

pub use crate::combat::damage::{apply_damage, apply_heal};

use lov_shared::types::EntityId;

use crate::combat::buffs::{BuffDefOverride, BuffInstance, BuffOptions, add_buff};
use crate::combat::damage::{CombatData, ShieldOpts, apply_shield as server_shield};
use crate::world::{Entity, World};

/// Durée par défaut d'un bouclier de sort (s).
const DEFAULT_SHIELD_DURATION: f64 = 2.5;

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldOptions {
    pub source: EntityId,
    pub target: EntityId,
    pub amount: f64,
    pub duration: Option<f64>,
    pub buff_id: Option<String>,
    pub ability: Option<String>,
    pub fx_key: Option<String>,
}

impl ShieldOptions {
    /// Bouclier de `amount` sur `target`, sans durée ni id explicites (défauts de `apply_shield`).
    pub fn new(source: EntityId, target: EntityId, amount: f64) -> Self {
        Self {
            source,
            target,
            amount,
            duration: None,
            buff_id: None,
            ability: None,
            fx_key: None,
        }
    }
}

/// Montant restant du bouclier lié à `buff` (`buff.shieldRemaining`), 0 s'il n'en a pas ou s'il est épuisé.
pub fn shield_remaining(e: &Entity, buff: &BuffInstance) -> f64 {
    let Some(id) = buff.params.shield_id.as_deref() else {
        return 0.0;
    };
    e.data.get::<CombatData>().map_or(0.0, |c| {
        c.shields
            .iter()
            .find(|s| s.id.as_deref() == Some(id))
            .map_or(0.0, |s| s.amount)
    })
}

/// Pose un bouclier de sort. Renvoie l'augmentation effective (0 si rien n'a été posé).
pub fn apply_shield(world: &mut World, opts: ShieldOptions) -> f64 {
    let (source, target) = (opts.source, opts.target);
    if !world.contains(source)
        || world.get(target).is_none_or(|t| t.dead)
        || !opts.amount.is_finite()
        || opts.amount <= 0.0
    {
        return 0.0;
    }
    let buff_id = opts
        .buff_id
        .or(opts.ability)
        .unwrap_or_else(|| "shield".to_owned());
    let id = format!("{buff_id}:{source}");
    let previous = world
        .get(target)
        .and_then(|t| t.data.get::<CombatData>())
        .and_then(|c| c.shields.iter().find(|s| s.id.as_deref() == Some(&id)))
        .map_or(0.0, |s| s.amount);
    let amount = previous.max(opts.amount);
    let duration = opts.duration.unwrap_or(DEFAULT_SHIELD_DURATION);
    let mut buff = BuffOptions::new(duration);
    buff.source = Some(source);
    buff.params.shield_id = Some(id.clone());
    buff.definition = Some(BuffDefOverride {
        fx_key: Some(opts.fx_key.unwrap_or_else(|| buff_id.clone())),
        ..BuffDefOverride::default()
    });
    if add_buff(world, target, &buff_id, buff).is_none() {
        return 0.0;
    }
    let mut shield = ShieldOpts::new(Some(source), target, amount);
    shield.duration = Some(duration);
    shield.id = Some(id);
    server_shield(world, shield);
    amount - previous
}
