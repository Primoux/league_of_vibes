//! Moteur de sorts (port de `packages/server/src/abilities/*.ts`, sans les kits de champions).
//!
//! | Rust                | TypeScript            | Rôle                                                       |
//! |---------------------|-----------------------|------------------------------------------------------------|
//! | `engine`            | `engine.ts`           | validation et dépense d'un lancer, incantations, mise à jour |
//! | `context`           | `context.ts`          | `CastContext`, `param`, `scaling`, `hit`, `cc`, `line`       |
//! | `primitives`        | `primitives.ts`       | projectiles, zones, dashes, blinks, tâches, liens, cônes     |
//! | `effects`           | `effects.ts`          | `apply_shield` (bouclier + buff), ré-export dégâts/soins     |
//! | `kit_utils`         | `kitUtils.ts`         | `dist_within` et ré-exports de primitives                    |
//! | `passives`          | `passives.ts`         | installation des passifs, `PassiveState`, mise à jour        |
//! | `combat_hooks`      | `combatHooks.ts`      | `register_damage_modifier`                                   |
//! | `registry`          | `registry.ts`         | `Kit`, recherche d'un kit                                    |
//! | `champions`         | `champions/*.ts`      | un fichier par kit + le tableau `KITS`                       |
//! | `systems::abilities`| `systems/abilities.ts`| le système (`abilities_system`)                              |
//!
//! # Écrire le kit d'un champion
//!
//! Un kit TypeScript est un objet de quatre fonctions qui reçoivent `ctx` ; en Rust ce sont quatre
//! fonctions `fn(&mut World, &CastContext)` regroupées dans un `Kit` statique. Le TS `ctx.world`
//! devient le paramètre `world`, et `ctx.source`/`ctx.target` des ids (`EntityId`) : on lit l'entité
//! avec `world.get(id)` au moment voulu (`ctx.source(world)`, `ctx.target(world)`). `CastContext` est
//! `Copy` : `let ctx = *ctx;` puis `move |world, …| …` pour l'utiliser dans un rappel. Les noms des
//! aides sont ceux du TS en `snake_case` (`line`, `hit`, `cc`, `param`, `delayed_area`,
//! `enemies_in_radius`, `spawn_zone`, `start_dash`, `blink`, `schedule`, `tether`, `apply_shield`…) ;
//! `use lov_sim::abilities::prelude::*;` les importe presque toutes.
//!
//! Ce qui change quand on traduit :
//! - un rappel reçoit `world` en premier argument (`|world, target| …`) et ne capture que des valeurs
//!   `Send + 'static` : ids, `CastContext`, nombres, `Arc<Mutex<…>>` pour un état partagé ;
//! - `schedule(world, host, delay, f)` prend l'entité qui porte la tâche : passer `ctx.source` ;
//! - `hit(world, &ctx, target)` / `hit_with(world, &ctx, target, multiplier, Some(type))`,
//!   `cc(world, &ctx, target, CcType::Slow, durée, Some(montant))`, `param(&ctx, "clé")` ou
//!   `param_or(&ctx, "clé", défaut)` ;
//! - l'état propre au champion (compteurs, marques) se range dans `entity.data` sous un type Rust à soi,
//!   jamais dans une variable globale : l'état d'une partie vit dans son `World` ;
//! - le passif se branche dans `Kit::install` (`world.hooks.*`, `world.events.on`) et les crochets
//!   `update_*` ; les règles de ciblage (`TargetRule`) et le sort renforcé (`on_cast`) sont des
//!   données du kit.
//!
//! Exemple : un kit à deux sorts, une ligne et un sort ciblé (le reste du kit de TS est analogue).
//!
//! ```no_run
//! use lov_shared::schema::CcType;
//! use lov_sim::abilities::prelude::*;
//!
//! // Q : projectile en ligne, blesse et ralentit le premier ennemi touché.
//! fn q(world: &mut World, ctx: &CastContext) {
//!     let ctx = *ctx;
//!     line(world, &ctx, LineOptions::default(), move |world, target| {
//!         hit(world, &ctx, target);
//!         cc(world, &ctx, target, CcType::Slow, param(&ctx, "slowDuration"), Some(param(&ctx, "slow")));
//!     });
//! }
//!
//! // E : sort ciblé, bouclier sur un allié (ou sur soi, sans cible).
//! fn e(world: &mut World, ctx: &CastContext) {
//!     let Some(target) = ctx.target else { return };
//!     let ap = ctx.source(world).map_or(0.0, |s| s.stats.ap);
//!     let mut shield = ShieldOptions::new(ctx.source, target, param(ctx, "shield") + ap * param(ctx, "shieldAp"));
//!     shield.duration = Some(param(ctx, "duration"));
//!     shield.buff_id = Some("ilyra_e".to_owned());
//!     apply_shield(world, shield);
//! }
//!
//! fn nothing(_: &mut World, _: &CastContext) {}
//!
//! // `abilities/champions/ilyra.rs` : le kit, puis `pub mod ilyra;` et `&ilyra::KIT,` dans `champions/mod.rs`.
//! pub static KIT: Kit = Kit::new("ilyra", [q, nothing, e, nothing])
//!     .with_targeting([TargetRule::Enemy, TargetRule::Enemy, TargetRule::AllyChampion, TargetRule::Enemy]);
//! ```

pub mod champions;
pub mod combat_hooks;
pub mod context;
pub mod effects;
pub mod engine;
pub mod kit_utils;
pub mod passives;
pub mod primitives;
pub mod registry;

/// Ce qu'un kit importe presque toujours.
pub mod prelude {
    pub use super::combat_hooks::{DamageModifierHandle, register_damage_modifier};
    pub use super::context::{
        CastContext, LineOptions, at_rank, cc, def_param, hit, hit_with, line, param, param_or,
        scaling,
    };
    pub use super::effects::{
        ShieldOptions, apply_damage, apply_heal, apply_shield, shield_remaining,
    };
    pub use super::engine::{AbilitySlot, ability_state, cast_ability};
    pub use super::kit_utils::dist_within;
    pub use super::passives::{PassiveState, hp_before_damage, hp_lost, passive_state};
    pub use super::primitives::{
        DashOptions, OwnerId, ProjectileOptions, TaskHandle, ZoneOptions, allies_in_radius, blink,
        cone, delayed_area, direction, endpoint, enemies_in_radius, enemies_in_radius_into,
        is_enemy, is_unit, is_visible_target, is_walkable, remove_zone, schedule, schedule_with,
        spawn_projectile, spawn_zone, start_dash, tether, valid_landing, valid_point,
    };
    pub use super::registry::{AbilityHandler, Kit, TargetRule};
    pub use crate::world::{Entity, World};
}
