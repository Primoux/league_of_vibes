//! Passifs : port de `packages/server/src/abilities/passives.ts`.
//!
//! En TypeScript, `passives.ts` contenait en dur les branches de chaque champion (marques de Corvin,
//! saignements de Vrenna, élan de Tharox…). Ici elles vivent dans le kit du champion (`Kit::install`,
//! `update_champion`, `update_entity`, `reset_dead`, voir `registry.rs`) ; ce fichier garde la
//! mécanique commune :
//!
//! - `init_passives` : installe les kits (dans l'ordre du registre), le relevé des PV avant dégâts
//!   (`hp_before_damage`, la `beforeHp` du TS) et la rupture d'invisibilité sur `autoAttack` ;
//! - `update_passives` : remise à zéro de `PassiveState` des champions morts, puis les crochets de
//!   mise à jour des kits, entité par entité dans l'ordre des ids (comme `world.entities.values()`) ;
//! - `PassiveState` / `passive_state` : le `state(e)` du TS (coups, élan, dernière position), partagé
//!   par les kits de Kaelen et de Tharox.
//!
//! `init_passives` doit être appelé une seule fois par monde (c'est l'`init` du système `abilities`) :
//! le `WeakSet<World>` du TS n'a pas d'équivalent sans champ dans `World`.
//!
//! Différences avec le TypeScript : le crochet `before_damage` des kits n'est plus unique (chaque kit
//! enregistre le sien dans `install`), donc l'ordre entre champions suit le registre ; une distribution
//! imbriquée saute seulement le crochet en cours (`HookList`). `hp_before_damage` est une valeur par
//! entité cible, non par contexte : des dégâts imbriqués sur la même cible, infligés entre l'avant et
//! l'après d'un coup, l'écrasent.

use lov_shared::types::{EntityId, GameEvent, Vec2};

use crate::abilities::registry::{Kit, kits_into};
use crate::combat::buffs::break_invisibility;
use crate::world::{Entity, EventKind, World};

/// `state(e)` du TS : cumuls de passif communs à plusieurs kits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassiveState {
    /// Auto-attaques réussies (Kaelen).
    pub attacks: u32,
    /// Élan de Tharox (0..100).
    pub momentum: f64,
    pub last_pos: Vec2,
}

/// `state(e)` : créé à la demande, avec la position courante comme dernière position.
pub fn passive_state(e: &mut Entity) -> &mut PassiveState {
    let pos = e.pos;
    e.data.get_or_insert_with(|| PassiveState {
        attacks: 0,
        momentum: 0.0,
        last_pos: pos,
    })
}

/// PV de la cible juste avant les crochets de dégâts du dernier coup reçu (`beforeHp` du TS).
#[derive(Debug, Clone, Copy, PartialEq)]
struct HpBeforeDamage(f64);

/// PV de `target` avant le dernier coup reçu ; `None` si aucun coup n'a été relevé.
pub fn hp_before_damage(target: &Entity) -> Option<f64> {
    target.data.get::<HpBeforeDamage>().map(|h| h.0)
}

/// PV réellement perdus par `target` lors du coup qui vient d'être appliqué (`dealt` : PV + bouclier
/// retirés) : jamais le bouclier ni le dépassement. À lire dans un crochet `after_damage`.
pub fn hp_lost(target: &Entity, dealt: f64) -> f64 {
    let before = hp_before_damage(target);
    before
        .unwrap_or(dealt)
        .min((before.unwrap_or(target.hp) - target.hp).max(0.0))
}

/// Branche les passifs sur le monde, une fois. `damage` sans `ability` désigne un coup d'auto-attaque
/// confirmé ; l'événement `autoAttack` est le lancer de l'attaque : il ne fait que rompre l'invisibilité,
/// jamais un effet à l'impact.
pub fn init_passives(world: &mut World) {
    world.hooks.before_damage.push(Box::new(|world, ctx| {
        if let Some(t) = world.get_mut(ctx.target) {
            let hp = t.hp;
            t.data.insert(HpBeforeDamage(hp));
        }
    }));
    let mut kits: Vec<&'static Kit> = Vec::new();
    kits_into(&mut kits);
    for kit in kits {
        if let Some(install) = kit.install {
            install(world);
        }
    }
    world.events.on(EventKind::AutoAttack, |world, event| {
        if let GameEvent::AutoAttack { source, .. } = event {
            break_invisibility(world, *source);
        }
    });
}

/// Tampons réutilisés d'un tick à l'autre.
#[derive(Default)]
pub struct PassivesScratch {
    ids: Vec<EntityId>,
    kits: Vec<&'static Kit>,
}

/// `updatePassives(world, dt)`.
pub fn update_passives(world: &mut World, dt: f64, sc: &mut PassivesScratch) {
    world.ids_into(None, &mut sc.ids);
    kits_into(&mut sc.kits);
    for &id in &sc.ids {
        let Some(e) = world.get_mut(id) else { continue };
        if e.dead {
            if e.champ.is_some() {
                let pos = e.pos;
                let s = passive_state(e);
                s.momentum = 0.0;
                s.attacks = 0;
                s.last_pos = pos;
            }
            for kit in &sc.kits {
                if let Some(f) = kit.reset_dead {
                    f(world, id);
                }
            }
            continue;
        }
        for kit in &sc.kits {
            let is_own = world
                .get(id)
                .and_then(|e| e.champ.as_ref())
                .is_some_and(|c| c.champion_id == kit.champion_id);
            if is_own && let Some(f) = kit.update_champion {
                f(world, id, dt);
            }
        }
        for kit in &sc.kits {
            if let Some(f) = kit.update_entity {
                f(world, id, dt);
            }
        }
    }
}
