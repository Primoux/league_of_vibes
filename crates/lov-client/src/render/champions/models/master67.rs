//! 67 Master : streetwear arcanique et balancier des deux paumes, même pendant la course.
//!
//! Portage de `packages/client/src/render/champions/models/master67.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette et clips.

use std::f32::consts::{FRAC_PI_2, PI};

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{Clip, ClipCtx, Pose, Sample, set_offset, set_rot};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;

/// Geste « six-seven » : les deux paumes vers le haut, en balancier.
pub fn sample_six_seven(t: f32, p: &mut Pose, energy: f32) {
    let wave = (t / 1.34 * PI * 2.0).sin() * energy;
    set_rot(p, ARM_L, 0.18, 0.0, 0.12 + wave * 0.18);
    set_rot(p, ARM_R, -0.18, 0.0, 0.12 - wave * 0.18);
    set_rot(p, FORE_L, 0.0, 0.0, FRAC_PI_2 + wave * 0.32);
    set_rot(p, FORE_R, 0.0, 0.0, FRAC_PI_2 - wave * 0.32);
    set_rot(p, HEAD, 0.0, wave * 0.06, -0.04);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_six_seven(t, p, 1.0);
    set_offset(p, 0.0, (t * PI * 2.0 / 1.34).sin() * 0.018, 0.0);
}

fn run(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_six_seven(t, p, 1.15);
    let s = (t * PI * 2.0 / 0.67).sin();
    set_rot(p, LEG_L, 0.0, 0.0, s * 0.7);
    set_rot(p, LEG_R, 0.0, 0.0, -s * 0.7);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.25 - (-s).max(0.0) * 0.7);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.25 - s.max(0.0) * 0.7);
    set_rot(p, SPINE, 0.0, s * 0.05, -0.1);
    set_offset(p, 0.0, s.abs() * 0.045, 0.0);
}

fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_six_seven(t, p, 1.6);
    set_rot(p, SPINE, 0.0, 0.0, -0.12 * (PI * t / 0.67).sin());
}

fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_six_seven(t, p, 1.7);
    set_offset(p, 0.0, ((t / 1.67).min(1.0) * PI).sin() * 0.12, 0.0);
}

fn recall(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_six_seven(t, p, 1.3);
    set_offset(p, 0.0, 0.12 + (t * 3.0).sin() * 0.07, 0.0);
}

const fn clip(looped: bool, duration: f32, sample: Sample) -> Clip {
    Clip {
        looped,
        duration,
        sample,
    }
}

const IDLE: Clip = clip(true, 1.34, idle);
const RUN: Clip = clip(true, 0.67, run);
const CAST: Clip = clip(false, 0.67, cast);

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.87,
        hip_w: 0.15,
        thigh: 0.43,
        shin: 0.42,
        spine: 0.56,
        shoulder_w: 0.29,
        upper_arm: 0.29,
        forearm: 0.29,
        neck: 0.16,
    },
    hand: Some(1.0),
    energy: Some(1.0),
    clips: &[
        ("idle", IDLE),
        ("run", RUN),
        ("dash", RUN),
        ("attackReady", IDLE),
        ("strike", CAST),
        ("cast", CAST),
        ("cast_Q", CAST),
        ("cast_W", CAST),
        ("cast_E", CAST),
        ("cast_R", clip(false, 1.67, cast_r)),
        ("recall", clip(true, 1.34, recall)),
    ],
    scale: 1.0,
    build: humanoids::master67,
    carry: None,
    extras: None,
    drop_angle: None,
};

/// Portage de `packages/client/test/master67-model.test.ts`.
#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::render::champions::anim::pose::{JOINT_COUNT, POSE_SIZE};
    use crate::render::champions::models::testing::clips_finis_et_animes;
    use crate::render::champions::rig::{PARENT, Skeleton};

    #[test]
    fn alterne_les_mains_en_gardant_les_paumes_vers_le_haut() {
        let skeleton = Skeleton {
            root: Entity::PLACEHOLDER,
            joints: [Entity::PLACEHOLDER; JOINT_COUNT],
            rest: MODEL.rig.rest_positions(),
        };
        let mut heights = Vec::new();
        for time in [0.335, 1.005] {
            let mut pose = [0.0; POSE_SIZE];
            sample_six_seven(time, &mut pose, 1.0);
            let local = skeleton.transforms(&pose);
            let mut world = [Mat4::IDENTITY; JOINT_COUNT];
            for j in 0..JOINT_COUNT {
                let parent = PARENT[j].map_or(Mat4::IDENTITY, |p| world[p]);
                world[j] = parent * local[j].to_matrix();
            }
            let h: Vec<f32> = [FORE_L, FORE_R]
                .iter()
                .map(|j| {
                    let normal = world[*j].transform_vector3(Vec3::X).normalize();
                    assert!(normal.y > 0.7, "paume vers le haut");
                    world[*j].transform_point3(Vec3::new(0.02, -0.36, 0.0)).y
                })
                .collect();
            heights.push(h);
        }
        assert!(heights[0][0] - heights[0][1] > 0.15);
        assert!(heights[1][0] - heights[1][1] < -0.15);
    }

    #[test]
    fn conserve_les_gestes_en_course_et_pendant_chaque_incantation() {
        let names = [
            "idle", "run", "strike", "cast_Q", "cast_W", "cast_E", "cast_R", "recall",
        ];
        for name in names {
            assert!(MODEL.clips.iter().any(|(n, _)| *n == name), "{name}");
        }
        clips_finis_et_animes(&MODEL);
    }

    #[test]
    fn modele_dedie_dans_son_budget_et_reprise_apres_la_mort() {
        use crate::render::champions::model::{
            ChampionMaterials, ModelColors, SpawnedModel, build_model,
        };
        use crate::render::champions::model_for;
        use crate::render::champions::view::ChampionView;
        use lov_shared::types::{EntityKind, EntitySnapshot, TeamOrNeutral};

        let Some(c) = ModelColors::of("master67", TeamOrNeutral::Blue) else {
            panic!("master67 connu");
        };
        let def = model_for("master67", c.class);
        assert_eq!(def.rig, MODEL.rig, "modèle dédié");
        let built = build_model(&def, &c);
        let triangles: usize = [
            &built.body,
            &built.metal,
            &built.glow,
            &built.outline_body,
            &built.outline_metal,
        ]
        .iter()
        .flat_map(|lot| lot.iter().flatten())
        .map(|m| m.count_vertices() / 3)
        .sum();
        assert!(triangles < 16_000, "{triangles} triangles");
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
            extras: vec![],
        };
        let mut v = ChampionView::new(67, &def, model);
        let mut snap = EntitySnapshot {
            id: 67,
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
            dead: Some(true),
            subtype: Some("master67".into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        };
        v.step(&snap, "death", 2.5, Vec3::ZERO, 0.0);
        snap.dead = None;
        v.step(&snap, "idle", 0.1, Vec3::ZERO, 0.0);
        assert!(v.step(&snap, "idle", 0.2, Vec3::ZERO, 0.0).model_visible);
    }
}
