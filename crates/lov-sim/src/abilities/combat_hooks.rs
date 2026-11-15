//! Raccourci d'enregistrement d'un modificateur de dégâts : port de `abilities/combatHooks.ts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::world::DamageContext;
use crate::world::World;

/// Le `() => void` renvoyé par `registerDamageModifier`.
///
/// `HookList` n'a pas d'API de retrait (`world.rs` n'est pas modifié) : `remove` désactive donc le
/// modificateur, qui reste dans la liste comme un hook inerte.
#[derive(Debug, Clone)]
pub struct DamageModifierHandle(Arc<AtomicBool>);

impl DamageModifierHandle {
    /// Désactive le modificateur (sans effet s'il l'est déjà).
    pub fn remove(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Branche `f` sur le pipeline de dégâts d'avant mitigation (`world.hooks.before_damage`).
pub fn register_damage_modifier(
    world: &mut World,
    mut f: impl FnMut(&mut World, &mut DamageContext) + Send + 'static,
) -> DamageModifierHandle {
    let active = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&active);
    world.hooks.before_damage.push(Box::new(move |world, ctx| {
        if flag.load(Ordering::Relaxed) {
            f(world, ctx);
        }
    }));
    DamageModifierHandle(active)
}
