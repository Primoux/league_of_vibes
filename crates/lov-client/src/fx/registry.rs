//! Registre des FX par fxKey : portage de `packages/client/src/fx/registry.ts`.
//!
//! Chaque fxKey déclare les phases qu'elle personnalise ; les phases non déclarées retombent sur l'effet
//! générique (`defaults`) : tout sort a donc toujours un FX propre.
//!
//! ```ignore
//! app.register_fx("ilyra_w", FxHandlers {
//!     cast: Some(cast),   // anticipation (abilityCast)
//!     area: Some(area),   // télégraphie + nova (areaEffect)
//!     ..default()
//! });
//! ```
//!
//! Un handler reçoit l'événement entier et le déstructure (`let GameEvent::AreaEffect { pos, .. } = e`) :
//! le dispatcher garantit que la variante correspond à la phase.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::Fx;
use crate::fx::keys::{BuffFxResolver, FxPhase, default_buff_fx};

/// Handler d'une phase.
pub type FxFn = fn(&mut Fx, &GameEvent, &FxColors);
/// Handler `projectile` : peut renvoyer une poignée, pour que l'impact serveur arrête le projectile.
pub type ProjectileFn = fn(&mut Fx, &GameEvent, &FxColors) -> Option<Box<dyn ProjectileHandle>>;
/// Handler d'événements sans fxKey (dégâts, mort, montée de niveau, structures…).
pub type WorldFxFn = fn(&mut Fx, &GameEvent);

/// Poignée d'un projectile en vol, arrêté à l'impact (`ProjectileHandle`).
pub trait ProjectileHandle: Send + Sync {
    fn finish(&mut self, fx: &mut Fx, at: Vec3);
}

/// Handlers d'une fxKey (`FxHandlers`) ; `None` = effet générique.
#[derive(Clone, Copy, Default)]
pub struct FxHandlers {
    pub cast: Option<FxFn>,
    pub projectile: Option<ProjectileFn>,
    pub hit: Option<FxFn>,
    pub area: Option<FxFn>,
    pub dash: Option<FxFn>,
    pub buff: Option<FxFn>,
    pub buff_end: Option<FxFn>,
    pub summoner: Option<FxFn>,
}

impl FxHandlers {
    /// Complète `self` avec les handlers déclarés dans `other` (ceux de `other` l'emportent).
    pub fn merge(self, other: FxHandlers) -> FxHandlers {
        FxHandlers {
            cast: other.cast.or(self.cast),
            projectile: other.projectile.or(self.projectile),
            hit: other.hit.or(self.hit),
            area: other.area.or(self.area),
            dash: other.dash.or(self.dash),
            buff: other.buff.or(self.buff),
            buff_end: other.buff_end.or(self.buff_end),
            summoner: other.summoner.or(self.summoner),
        }
    }

    /// Un handler est-il déclaré pour cette phase ?
    pub fn has(&self, phase: FxPhase) -> bool {
        match phase {
            FxPhase::Cast => self.cast.is_some(),
            FxPhase::Projectile => self.projectile.is_some(),
            FxPhase::Hit => self.hit.is_some(),
            FxPhase::Area => self.area.is_some(),
            FxPhase::Dash => self.dash.is_some(),
            FxPhase::Buff => self.buff.is_some(),
            FxPhase::BuffEnd => self.buff_end.is_some(),
            FxPhase::Summoner => self.summoner.is_some(),
        }
    }
}

/// Handlers génériques de toutes les phases (`AllHandlers`), pour les fxKeys sans handler dédié.
#[derive(Clone, Copy)]
pub struct AllHandlers {
    pub cast: FxFn,
    pub projectile: ProjectileFn,
    pub hit: FxFn,
    pub area: FxFn,
    pub dash: FxFn,
    pub buff: FxFn,
    pub buff_end: FxFn,
    pub summoner: FxFn,
}

fn nothing(_: &mut Fx, _: &GameEvent, _: &FxColors) {}

fn no_projectile(_: &mut Fx, _: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    None
}

impl Default for AllHandlers {
    /// Effets génériques vides : les vrais (`defaults.ts`) reposent sur les primitives de l'area fx-effects
    /// (`effects/primitives`), qui les branchera.
    fn default() -> Self {
        Self {
            cast: nothing,
            projectile: no_projectile,
            hit: nothing,
            area: nothing,
            dash: nothing,
            buff: nothing,
            buff_end: nothing,
            summoner: nothing,
        }
    }
}

/// Registre des FX (`registry`), ressource du client.
#[derive(Resource)]
pub struct FxRegistry {
    keys: HashMap<String, FxHandlers>,
    world: Vec<WorldFxFn>,
    ability_hits: HashMap<String, WorldFxFn>,
    /// Effets génériques des phases sans handler dédié.
    pub defaults: AllHandlers,
    /// fxKey d'un buff dont l'événement n'en porte pas (`setBuffFxResolver`).
    pub buff_fx: BuffFxResolver,
}

impl Default for FxRegistry {
    fn default() -> Self {
        Self {
            keys: HashMap::new(),
            world: Vec::new(),
            ability_hits: HashMap::new(),
            defaults: AllHandlers::default(),
            buff_fx: default_buff_fx,
        }
    }
}

impl FxRegistry {
    /// Enregistre (ou complète) les handlers d'une fxKey.
    pub fn register(&mut self, key: &str, handlers: FxHandlers) {
        let merged = self
            .keys
            .get(key)
            .copied()
            .unwrap_or_default()
            .merge(handlers);
        self.keys.insert(key.to_owned(), merged);
    }

    pub fn get(&self, key: &str) -> Option<&FxHandlers> {
        self.keys.get(key)
    }

    /// fxKeys personnalisées (fxlab, couverture).
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.keys.keys().map(String::as_str)
    }

    /// Ajoute un handler d'événements du monde. Il reçoit tous les événements et filtre lui-même ceux qui
    /// l'intéressent (`onWorldEvent(type, fn)` du web).
    pub fn on_world_event(&mut self, f: WorldFxFn) {
        self.world.push(f);
    }

    pub fn world_handlers(&self) -> &[WorldFxFn] {
        &self.world
    }

    /// FX d'impact d'un sort à l'instant exact de ses dégâts serveur (événement `damage` dont `ability` est
    /// l'id du sort). Utile pour les sorts ciblés ou de zone instantanée qui n'émettent ni projectile ni zone.
    pub fn on_ability_hit(&mut self, ability_id: &str, f: WorldFxFn) {
        self.ability_hits.insert(ability_id.to_owned(), f);
    }

    pub fn ability_hit(&self, ability_id: &str) -> Option<WorldFxFn> {
        self.ability_hits.get(ability_id).copied()
    }
}

/// Enregistrement des FX depuis le plugin d'un champion ou d'un effet.
pub trait RegisterFx {
    fn register_fx(&mut self, key: &str, handlers: FxHandlers) -> &mut Self;
    fn on_world_event(&mut self, f: WorldFxFn) -> &mut Self;
    fn on_ability_hit(&mut self, ability_id: &str, f: WorldFxFn) -> &mut Self;
}

impl RegisterFx for App {
    fn register_fx(&mut self, key: &str, handlers: FxHandlers) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<FxRegistry>()
            .register(key, handlers);
        self
    }

    fn on_world_event(&mut self, f: WorldFxFn) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<FxRegistry>()
            .on_world_event(f);
        self
    }

    fn on_ability_hit(&mut self, ability_id: &str, f: WorldFxFn) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<FxRegistry>()
            .on_ability_hit(ability_id, f);
        self
    }
}
