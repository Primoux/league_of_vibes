//! Utilitaires partagés par les kits : port de `packages/server/src/abilities/kitUtils.ts`.
//!
//! Le TypeScript y ré-exporte aussi les primitives les plus utilisées ; c'est conservé pour que
//! `use crate::abilities::kit_utils::{…}` d'un kit se traduise ligne à ligne.

use lov_shared::math::dist2;

use crate::world::Entity;

pub use crate::abilities::primitives::{
    allies_in_radius, blink, cone, delayed_area, direction, endpoint, enemies_in_radius,
    remove_zone, schedule, spawn_projectile, spawn_zone, start_dash, tether,
};

/// `distWithin(a, b, range)` : `b` est vivante et à portée de `a`, rayons compris.
pub fn dist_within(a: &Entity, b: &Entity, range: f64) -> bool {
    let reach = range + a.radius + b.radius;
    !b.dead && dist2(a.pos, b.pos) <= reach * reach
}
