//! Brûlures à trois tics (buff rouge, Dragon ancestral) : port de
//! `packages/server/src/world-ai/burns.ts`.
//!
//! Les deux brûlures de `entity.data` (`redBurn`, `elderBurn`) vivent dans une seule structure
//! [`Burns`], indexée par [`BurnKey`].

use lov_shared::types::EntityId;

use crate::world::{Entity, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnKey {
    /// `redBurn` : buff rouge de la jungle.
    Red,
    /// `elderBurn` : buff du Dragon ancestral.
    Elder,
}

/// Une brûlure en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnState {
    pub source: EntityId,
    /// Temps de jeu du prochain tic.
    pub next: f64,
    pub remaining: u32,
}

/// `entity.data.redBurn` et `entity.data.elderBurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Burns {
    pub red: Option<BurnState>,
    pub elder: Option<BurnState>,
}

impl Burns {
    pub fn get(&self, key: BurnKey) -> Option<&BurnState> {
        match key {
            BurnKey::Red => self.red.as_ref(),
            BurnKey::Elder => self.elder.as_ref(),
        }
    }

    fn slot(&mut self, key: BurnKey) -> &mut Option<BurnState> {
        match key {
            BurnKey::Red => &mut self.red,
            BurnKey::Elder => &mut self.elder,
        }
    }
}

/// La brûlure `key` de l'entité, si elle en porte une.
pub fn burn(e: &Entity, key: BurnKey) -> Option<&BurnState> {
    e.data.get::<Burns>().and_then(|b| b.get(key))
}

/// Rafraîchit trois tics sans repousser celui qui était déjà programmé.
pub fn refresh_burn(world: &mut World, target: EntityId, key: BurnKey, source: EntityId) {
    let time = world.time;
    let Some(t) = world.get_mut(target) else {
        return;
    };
    let slot = t.data.get_or_insert_with(Burns::default).slot(key);
    match slot {
        Some(b) if b.remaining > 0 => {
            b.source = source;
            b.remaining = 3;
        }
        _ => {
            *slot = Some(BurnState {
                source,
                next: time + 1.0,
                remaining: 3,
            });
        }
    }
}

/// Consomme un tic échu et renvoie sa source ; le compteur reste borné même après un long pas de
/// simulation (un seul tic par appel : l'appelant boucle).
pub fn take_burn_tick(world: &mut World, target: EntityId, key: BurnKey) -> Option<EntityId> {
    let time = world.time;
    let source_alive = {
        let t = world.get(target)?;
        let b = t.data.get::<Burns>()?.get(key)?;
        world.contains(b.source)
    };
    let t = world.get_mut(target)?;
    let (dead, hp) = (t.dead, t.hp);
    let slot = t.data.get_mut::<Burns>()?.slot(key);
    let b = slot.as_mut()?;
    if dead || hp <= 0.0 || b.remaining == 0 {
        *slot = None;
        return None;
    }
    if time + 1e-8 < b.next {
        return None;
    }
    if !source_alive {
        *slot = None;
        return None;
    }
    let source = b.source;
    b.next += 1.0;
    b.remaining -= 1;
    if b.remaining == 0 {
        *slot = None;
    }
    Some(source)
}
