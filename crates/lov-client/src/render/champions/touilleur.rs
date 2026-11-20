//! Vue du Touilleur : le canal de son R suit le buff serveur, y compris une fin anticipée par
//! purification ou interruption.
//!
//! Portage de `packages/client/src/render/champions/TouilleurView.ts`. Le modèle du Touilleur (index en
//! moulinet porté par l'os `prop`) appartient à sa tranche de champion (`models/touilleur.rs`) ; en
//! attendant, il prend le modèle générique de sa classe.

use lov_shared::types::EntitySnapshot;

use super::anim::pose::Animator;

/// Identifiant du champion qui prend cette vue.
pub const ID: &str = "touilleur";
/// Buff serveur du canal de son R.
pub const CHANNEL_BUFF: &str = "touilleur_r_channel";

/// Suivi du canal (`stirring`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Channel {
    stirring: bool,
}

impl Channel {
    /// Avant la frame de la vue : lance le clip du canal à l'arrivée du buff, revient au repos à son
    /// retrait si le clip du canal joue encore.
    pub fn update(&mut self, animator: &mut Animator, snap: &EntitySnapshot) {
        let stirring =
            snap.dead != Some(true) && snap.buffs.iter().flatten().any(|b| b.id == CHANNEL_BUFF);
        if stirring && !self.stirring {
            animator.play("cast_R", Some(0.06), None);
        } else if !stirring && self.stirring && animator.current() == "cast_R" {
            animator.play("idle", Some(0.1), None);
        }
        self.stirring = stirring;
    }
}

/// Portage de `packages/client/test/touilleur-model.test.ts` (fin du canal ; le moulinet de l'index
/// et le budget du modèle arrivent avec la tranche du Touilleur).
#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use lov_shared::types::{BuffSnapshot, EntityKind, TeamOrNeutral};

    use super::*;
    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::model::{ChampionMaterials, ModelColors, SpawnedModel};
    use crate::render::champions::model_for;
    use crate::render::champions::rig::Skeleton;
    use crate::render::champions::view::ChampionView;

    fn view() -> ChampionView {
        let Some(c) = ModelColors::of(ID, TeamOrNeutral::Blue) else {
            panic!("touilleur connu");
        };
        let def = model_for(ID, c.class);
        let model = SpawnedModel {
            skeleton: Skeleton {
                root: Entity::PLACEHOLDER,
                joints: [Entity::PLACEHOLDER; JOINT_COUNT],
                rest: def.rig.rest_positions(),
            },
            materials: ChampionMaterials {
                body: Handle::default(),
                metal: Handle::default(),
                glow: Handle::default(),
            },
            meshes: vec![],
            outlines: vec![],
            glows: vec![],
        };
        let mut v = ChampionView::new(303, &def, model);
        v.channel = Some(Channel::default());
        v
    }

    fn snap(buff: Option<&str>) -> EntitySnapshot {
        EntitySnapshot {
            id: 303,
            kind: EntityKind::Champion,
            team: TeamOrNeutral::Blue,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 100.0,
            max_hp: 100.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: Some(ID.into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: buff.map(|id| {
                vec![BuffSnapshot {
                    id: id.into(),
                    remaining: 2.0,
                    stacks: None,
                }]
            }),
            champ: None,
        }
    }

    #[test]
    fn termine_l_animation_de_canal_des_que_le_buff_serveur_disparait() {
        let mut v = view();
        v.cast("R");
        v.step(&snap(Some(CHANNEL_BUFF)), "idle", 0.1, Vec3::ZERO, 0.0);
        assert_eq!(v.animator.current(), "cast_R");
        v.step(&snap(None), "idle", 0.1, Vec3::ZERO, 0.0);
        assert_eq!(v.animator.current(), "idle");
        v.step(&snap(Some(CHANNEL_BUFF)), "idle", 0.1, Vec3::ZERO, 0.0);
        v.step(&snap(Some("stun")), "idle", 0.1, Vec3::ZERO, 0.0);
        assert_eq!(v.animator.current(), "stunned");
    }

    #[test]
    fn le_canal_ne_coupe_pas_un_autre_geste() {
        let mut v = view();
        v.step(&snap(Some(CHANNEL_BUFF)), "idle", 0.1, Vec3::ZERO, 0.0);
        v.strike();
        v.step(&snap(None), "idle", 0.05, Vec3::ZERO, 0.0);
        assert_eq!(
            v.animator.current(),
            "strike",
            "fin du canal hors de son clip"
        );
    }
}
