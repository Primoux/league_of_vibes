//! Squelette procédural des champions : hiérarchie d'articulations (segments rigides) pilotée par une
//! `Pose`. Le modèle regarde vers +x local, la droite est +z (voir `anim/pose.rs`).
//!
//! Portage de `packages/client/src/render/champions/rig.ts`. Le client web en fait un `Skeleton` de
//! `Bone` sur lequel un `SkinnedMesh` est lié avec un skinning rigide (100 % sur un os) ; ici chaque
//! articulation est une entité et porte directement les maillages de ses pièces (voir `builder.rs`), ce
//! qui revient au même sans skinning.

use bevy::prelude::*;

use super::anim::pose::j::*;
use super::anim::pose::{BODY_OFFSET, JOINT_COUNT, Pose};
use crate::render::models::kit::euler;

/// Proportions du squelette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigSpec {
    /// Hauteur de la hanche.
    pub hip_y: f32,
    /// Demi-écart des hanches (jambes).
    pub hip_w: f32,
    pub thigh: f32,
    pub shin: f32,
    /// Hanche → épaules.
    pub spine: f32,
    /// Demi-largeur d'épaules.
    pub shoulder_w: f32,
    pub upper_arm: f32,
    pub forearm: f32,
    /// Épaules → pivot de la tête.
    pub neck: f32,
}

/// Parent de chaque articulation (`None` : la racine du modèle).
pub const PARENT: [Option<usize>; JOINT_COUNT] = {
    let mut p = [None; JOINT_COUNT];
    p[HIPS] = Some(BODY);
    p[SPINE] = Some(HIPS);
    p[LEG_L] = Some(HIPS);
    p[LEG_R] = Some(HIPS);
    p[SHIN_L] = Some(LEG_L);
    p[SHIN_R] = Some(LEG_R);
    p[HEAD] = Some(SPINE);
    p[ARM_L] = Some(SPINE);
    p[ARM_R] = Some(SPINE);
    p[FORE_L] = Some(ARM_L);
    p[FORE_R] = Some(ARM_R);
    p[PROP] = Some(FORE_R);
    p
};

impl RigSpec {
    /// Position de repos de chaque articulation dans le repère de son parent.
    pub fn rest_positions(&self) -> [Vec3; JOINT_COUNT] {
        let s = self;
        let mut p = [Vec3::ZERO; JOINT_COUNT];
        // le corps pivote au sol : les chutes basculent autour des pieds
        p[HIPS] = Vec3::new(0.0, s.hip_y, 0.0);
        p[LEG_L] = Vec3::new(0.0, 0.0, -s.hip_w);
        p[LEG_R] = Vec3::new(0.0, 0.0, s.hip_w);
        p[SHIN_L] = Vec3::new(0.0, -s.thigh, 0.0);
        p[SHIN_R] = Vec3::new(0.0, -s.thigh, 0.0);
        p[HEAD] = Vec3::new(0.0, s.spine + s.neck, 0.0);
        p[ARM_L] = Vec3::new(0.0, s.spine, -s.shoulder_w);
        p[ARM_R] = Vec3::new(0.0, s.spine, s.shoulder_w);
        p[FORE_L] = Vec3::new(0.0, -s.upper_arm, 0.0);
        p[FORE_R] = Vec3::new(0.0, -s.upper_arm, 0.0);
        p[PROP] = Vec3::new(0.0, -s.forearm, 0.0);
        p
    }

    /// Position de repos de chaque articulation dans le repère du modèle (rotations nulles).
    pub fn rest_world(&self) -> [Vec3; JOINT_COUNT] {
        let local = self.rest_positions();
        let mut out = [Vec3::ZERO; JOINT_COUNT];
        // les parents précèdent toujours leurs enfants dans l'ordre de `J`
        for j in 0..JOINT_COUNT {
            out[j] = local[j] + PARENT[j].map_or(Vec3::ZERO, |p| out[p]);
        }
        out
    }

    /// Hauteur approximative du sommet de la tête (barres de vie, FX au-dessus).
    pub fn height(&self) -> f32 {
        self.hip_y + self.spine + self.neck * 2.0
    }
}

/// Squelette apparu : une entité par articulation.
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub root: Entity,
    pub joints: [Entity; JOINT_COUNT],
    pub rest: [Vec3; JOINT_COUNT],
}

impl Skeleton {
    /// Fait apparaître les articulations sous une nouvelle racine (rattachée à rien).
    pub fn spawn(commands: &mut Commands, spec: &RigSpec) -> Self {
        let rest = spec.rest_positions();
        let root = commands
            .spawn((Transform::default(), Visibility::default()))
            .id();
        let mut joints = [Entity::PLACEHOLDER; JOINT_COUNT];
        for j in 0..JOINT_COUNT {
            let parent = PARENT[j].map_or(root, |p| joints[p]);
            joints[j] = commands
                .spawn((
                    Transform::from_translation(rest[j]),
                    Visibility::default(),
                    ChildOf(parent),
                ))
                .id();
        }
        Self { root, joints, rest }
    }

    /// Transformation de chaque articulation pour la pose `p`.
    pub fn transforms(&self, p: &Pose) -> [Transform; JOINT_COUNT] {
        std::array::from_fn(|j| {
            let mut t = Transform::from_translation(self.rest[j]).with_rotation(euler(Vec3::new(
                p[j * 3],
                p[j * 3 + 1],
                p[j * 3 + 2],
            )));
            if j == BODY {
                t.translation += Vec3::new(p[BODY_OFFSET], p[BODY_OFFSET + 1], p[BODY_OFFSET + 2]);
            }
            t
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::pose::{POSE_SIZE, set_offset, set_rot};

    pub const SPEC: RigSpec = RigSpec {
        hip_y: 0.85,
        hip_w: 0.15,
        thigh: 0.42,
        shin: 0.42,
        spine: 0.58,
        shoulder_w: 0.28,
        upper_arm: 0.33,
        forearm: 0.31,
        neck: 0.15,
    };

    #[test]
    fn hierarchie_de_repos() {
        let w = SPEC.rest_world();
        assert_eq!(w[BODY], Vec3::ZERO);
        assert_eq!(w[HEAD], Vec3::new(0.0, 0.85 + 0.58 + 0.15, 0.0));
        // la main droite (arme) pend sous l'épaule droite (+z)
        let hand = w[PROP];
        assert!((hand - Vec3::new(0.0, 0.85 + 0.58 - 0.33 - 0.31, 0.28)).length() < 1e-6);
        assert!((w[SHIN_L] - Vec3::new(0.0, 0.85 - 0.42, -0.15)).length() < 1e-6);
        assert!((SPEC.height() - (0.85 + 0.58 + 0.3)).abs() < 1e-6);
    }

    #[test]
    fn la_pose_tourne_les_articulations_et_deplace_le_corps() {
        let skel = Skeleton {
            root: Entity::PLACEHOLDER,
            joints: [Entity::PLACEHOLDER; JOINT_COUNT],
            rest: SPEC.rest_positions(),
        };
        let mut p = [0.0; POSE_SIZE];
        set_rot(&mut p, ARM_R, 0.0, 0.0, 0.5);
        set_offset(&mut p, 0.1, -0.2, 0.0);
        let t = skel.transforms(&p);
        assert_eq!(t[BODY].translation, Vec3::new(0.1, -0.2, 0.0));
        assert!(t[ARM_R].rotation.angle_between(Quat::from_rotation_z(0.5)) < 1e-6);
        assert_eq!(t[ARM_R].translation, SPEC.rest_positions()[ARM_R]);
    }
}
