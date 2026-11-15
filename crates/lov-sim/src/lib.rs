//! Simulation autoritaire de League of Vibes.
//!
//! Portage de `packages/server/src` (monde, systèmes, combat, sorts, navigation, objets, IA du monde).
//!
//! Règles de cette crate :
//! - aucune E/S, aucune horloge murale, aucun aléatoire non semé : le temps et le générateur aléatoire
//!   sont fournis par l'appelant, pour que les tests soient déterministes ;
//! - chaque système porté arrive avec les tests de `packages/server/test/` qui le couvrent.
//!
//! Squelette : les modules arrivent un par un, une issue chacun.

pub mod abilities;
pub mod combat;
pub mod nav;
pub mod systems;
pub mod world;

pub use world::{
    AttackContext, ChampionState, CreateChampion, DamageContext, DataTable, Entity, EventBus,
    EventKind, HealContext, HookList, Listener, QueuedCommand, Rng, Subscription, System, World,
    WorldHooks, create_champion,
};
