//! Positions et infos des entités pour les FX (suivre un champion, viser une cible…).
//!
//! Portage de `packages/client/src/fx/core/EntityTracker.ts`. Le web lit l'objet 3D du renderer s'il
//! existe (`getEntityObject`), sinon lisse lui-même les snapshots. Ici l'état réseau calcule déjà chaque
//! image la position affichée de chaque entité ([`GameState::pose`], interpolée) : c'est la même source
//! que les modèles, sans second lissage.

use bevy::prelude::*;
use lov_shared::types::{EntityId, EntitySnapshot, Team, TeamOrNeutral};

use crate::net::GameState;
use crate::render::RENDER_SCALE;

/// Vue en lecture sur les entités connues (`EntityTracker`).
#[derive(Clone, Copy)]
pub struct EntityTracker<'a> {
    state: Option<&'a GameState>,
}

impl<'a> EntityTracker<'a> {
    pub fn of(world: &'a World) -> Self {
        Self {
            state: world.get_resource::<GameState>(),
        }
    }

    /// Position monde au sol (+ hauteur `h`) de l'entité, si elle est connue.
    pub fn pos(&self, id: EntityId, h: f32) -> Option<Vec3> {
        let p = self.state?.pose(id)?;
        Some(Vec3::new(
            p.x as f32 * RENDER_SCALE,
            h,
            -(p.y as f32) * RENDER_SCALE,
        ))
    }

    /// Dernier état connu de l'entité.
    pub fn info(&self, id: EntityId) -> Option<&'a EntitySnapshot> {
        self.state?.entities().get(&id)
    }

    /// Équipe du joueur local, en partie.
    pub fn local_team(&self) -> Option<Team> {
        self.state?.game.as_ref().map(|g| g.team)
    }

    /// L'entité est-elle ennemie du joueur local ? (teinte rouge des FX ennemis)
    pub fn is_enemy(&self, id: EntityId) -> bool {
        let (Some(me), Some(e)) = (self.local_team(), self.info(id)) else {
            return false;
        };
        match e.team {
            TeamOrNeutral::Neutral => false,
            TeamOrNeutral::Blue => me != Team::Blue,
            TeamOrNeutral::Red => me != Team::Red,
        }
    }
}
