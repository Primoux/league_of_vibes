//! Le Touilleur : duelliste original, index levé en moulinet permanent ; les doigts restent habillés de
//! gants.
//!
//! Portage de `packages/client/src/render/champions/models/touilleur.ts`. Le Touilleur n'a pas de
//! portrait : sa géométrie est construite ici, avec l'anatomie des portraits.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{Clip, ClipCtx, Pose, Sample, set_offset, set_rot};
use crate::render::champions::builder::ModelBuilder;
use crate::render::champions::builder::PartOpts;
use crate::render::champions::model::{
    ChampionModelDef, ExtraPart, Extras, ExtrasOut, ExtrasState, ModelColors,
};
use crate::render::champions::portrait::anatomy::{Face, Outfit, face, hair, hand, human};
use crate::render::champions::portrait::geometry::{Facing, Point, Sculpt, cloth};
use crate::render::champions::portrait::three::{CatmullRom, tube};
use crate::render::champions::rig::RigSpec;
use crate::render::models::kit::euler;

/// Moulinet de l'index (os `prop`) ; `forward` : bras tendu vers l'avant (sorts).
pub fn sample_touilleur(t: f32, p: &mut Pose, forward: bool) {
    let arm = if forward { 0.95 } else { 0.35 };
    let fore = if forward { 0.62 } else { PI - 0.35 };
    set_rot(p, ARM_R, 0.0, 0.0, arm);
    set_rot(p, FORE_R, 0.0, 0.0, fore);
    let a = t * TAU;
    set_rot(p, PROP, 0.22 * a.cos(), 0.0, -arm - fore + 0.22 * a.sin());
    set_rot(p, ARM_L, 0.1, 0.0, -0.12);
    set_rot(p, FORE_L, 0.0, 0.0, 0.2);
    set_rot(p, HEAD, 0.0, -0.08, -0.04);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_touilleur(t, p, false);
    set_offset(p, 0.0, (t * 2.0).sin() * 0.015, 0.0);
}

fn run(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_touilleur(t, p, false);
    let a = (t * TAU / 0.7).sin();
    set_rot(p, LEG_L, 0.0, 0.0, a * 0.65);
    set_rot(p, LEG_R, 0.0, 0.0, -a * 0.65);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.18 - (-a).max(0.0) * 0.65);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.18 - a.max(0.0) * 0.65);
    set_rot(p, ARM_L, 0.12, 0.0, -a * 0.3);
    set_offset(p, 0.0, a.abs() * 0.035, 0.0);
}

fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_touilleur(t, p, true);
    set_rot(p, SPINE, 0.0, (t * 5.0).sin() * 0.1, 0.0);
}

fn ultimate(t: f32, p: &mut Pose, _: &ClipCtx) {
    sample_touilleur(t * 2.0, p, true);
    set_rot(p, ARM_L, 0.15, 0.0, 0.25);
    set_offset(p, 0.0, (t * 17.0).sin() * 0.015, 0.0);
}

fn build(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let (ivory, brass, copper) = ("#efe4ca", "#bc914b", "#7b3d23");
    let outfit = Outfit {
        cloth: "#17434a",
        pants: "#263135",
        skin: "#c79371",
        trim: brass,
        open_hands: true,
        width: Some(0.23),
        ..default()
    };
    human(s, c, rig, &outfit);
    let traits = Face {
        skin: "#cfa07c",
        hair: copper,
        eye: Some("#6b5334"),
        smile: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#864321", "#b96631", false, false);
    // Moustache à deux vraies boucles, front dégagé et mèches cuivrées.
    for side in [-1.0, 1.0] {
        s.tube(
            HEAD,
            copper,
            &[
                [0.183, 0.052, side * 0.012],
                [0.186, 0.038, side * 0.067],
                [0.168, 0.049, side * 0.11],
                [0.17, 0.085, side * 0.112],
                [0.18, 0.09, side * 0.088],
            ],
            &[0.013, 0.016, 0.01, 0.007, 0.002],
        );
        s.panel_with(
            SPINE,
            "#1d5358",
            &[
                [side * 0.08, 0.15],
                [side * 0.24, 0.24],
                [side * 0.21, -0.09],
                [side * 0.065, -0.2],
            ],
            [0.15, 0.48, 0.0],
            false,
            0.035,
        );
        s.tube_with(
            SPINE,
            brass,
            &[
                [0.17, 0.65, side * 0.235],
                [0.19, 0.47, side * 0.18],
                [0.16, 0.29, side * 0.08],
            ],
            &[0.009, 0.011, 0.007],
            true,
            6,
        );
        s.panel_with(
            HIPS,
            "#17434a",
            &[
                [side * 0.035, 0.04],
                [side * 0.19, 0.07],
                [side * 0.28, -0.55],
                [side * 0.08, -0.64],
            ],
            [-0.16, -0.02, 0.0],
            false,
            0.025,
        );
        for i in 0..3 {
            s.ell_with(
                SPINE,
                brass,
                [0.172, 0.15 + i as f32 * 0.1, side * 0.065],
                [0.013, 0.02, 0.02],
                8,
                true,
            );
        }
    }
    let coat = cloth(0.78, 0.31, 0.2, -0.16, 0.018);
    s.part(SPINE, &coat, "#12393f", PartOpts::default().smooth(), false);
    for i in 0..3 {
        let w = i as f32 * 0.012;
        s.panel_with(
            SPINE,
            ivory,
            &[
                [-0.07 - w, 0.045],
                [0.07 + w, 0.045],
                [0.05 + w, -0.04],
                [0.0, -0.07],
                [-0.05 - w, -0.04],
            ],
            [0.18 + i as f32 * 0.015, 0.49 - i as f32 * 0.058, 0.0],
            false,
            0.012,
        );
    }
    s.ring_with(
        SPINE,
        brass,
        0.047,
        0.009,
        [0.202, 0.5, 0.0],
        Facing::X,
        true,
    );
    s.gem(SPINE, c.glow, [0.215, 0.5, 0.0], [0.014, 0.038, 0.027]);
    hand(s, FORE_L, ivory, rig.forearm);
    let end = rig.forearm;
    s.ell_with(
        FORE_R,
        ivory,
        [0.0, -end - 0.045, 0.0],
        [0.054, 0.07, 0.07],
        12,
        false,
    );
    for i in 0..3 {
        let z = (i as f32 - 1.0) * 0.037;
        s.tube(
            FORE_R,
            ivory,
            &[
                [0.035, -end - 0.025, z],
                [0.06, -end - 0.07, z],
                [0.026, -end - 0.087, z],
            ],
            &[0.018, 0.018, 0.012],
        );
    }
    s.ell_with(
        FORE_R,
        ivory,
        [0.06, -end + 0.005, -0.05],
        [0.024, 0.045, 0.025],
        8,
        false,
    );
    // L'index appartient à l'os prop : son mouvement est indépendant du poignet et des autres doigts.
    s.tube_with(
        PROP,
        ivory,
        &[[0.0, 0.075, 0.046], [0.0, 0.19, 0.046], [0.012, 0.3, 0.046]],
        &[0.023, 0.021, 0.014],
        false,
        8,
    );
    let spiral: Vec<Point> = (0..=24)
        .map(|i| {
            let a = i as f32 / 24.0 * PI * 4.0;
            [
                a.cos() * 0.05,
                0.17 + i as f32 / 24.0 * 0.22,
                0.046 + a.sin() * 0.05,
            ]
        })
        .collect();
    let swirl = tube(&CatmullRom::new(&spiral), 24, 0.006, 4);
    s.b.glow(PROP, &swirl, c.glow, PartOpts::default());
}

fn no_parts(_: &ModelColors) -> Vec<ExtraPart> {
    Vec::new()
}

/// Horloge continue entre les clips ; compensation du bras pour garder le doigt vers le haut.
fn upright_finger(s: &ExtrasState, out: &mut ExtrasOut) {
    if s.dead || s.anim == "stunned" {
        return;
    }
    let a = s.time * PI * if s.anim == "cast_R" { 4.0 } else { 2.0 };
    let parent = [BODY, HIPS, SPINE, ARM_R, FORE_R]
        .iter()
        .fold(Quat::IDENTITY, |q, j| q * out.joints[*j].rotation);
    let desired = euler(Vec3::new(0.22 * a.cos(), 0.0, 0.22 * a.sin()));
    out.joints[PROP].rotation = parent.inverse() * desired;
}

const fn clip(looped: bool, duration: f32, sample: Sample) -> Clip {
    Clip {
        looped,
        duration,
        sample,
    }
}

const IDLE: Clip = clip(true, 1.0, idle);
const RUN: Clip = clip(true, 0.7, run);
const CAST: Clip = clip(false, 0.6, cast);

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.9,
        hip_w: 0.14,
        thigh: 0.43,
        shin: 0.43,
        spine: 0.57,
        shoulder_w: 0.26,
        upper_arm: 0.3,
        forearm: 0.29,
        neck: 0.16,
    },
    hand: None,
    energy: None,
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
        ("cast_R", clip(false, 2.25, ultimate)),
        ("recall", IDLE),
    ],
    scale: 1.0,
    build,
    carry: None,
    extras: Some(Extras {
        parts: no_parts,
        update: upright_finger,
    }),
    drop_angle: None,
};

/// Portage de `packages/client/test/touilleur-model.test.ts` (index du Touilleur ; la fin du canal est
/// testée avec sa vue, les moulinets FX attendent #111).
#[cfg(test)]
mod tests {
    use lov_shared::types::{EntityKind, EntitySnapshot, TeamOrNeutral};

    use super::*;
    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::model::{ChampionMaterials, SpawnedModel, build_model};
    use crate::render::champions::model_for;
    use crate::render::champions::models::testing::clips_finis_et_animes;
    use crate::render::champions::rig::Skeleton;
    use crate::render::champions::touilleur::Channel;
    use crate::render::champions::view::ChampionView;

    fn view() -> (ChampionView, ChampionModelDef) {
        let Some(c) = ModelColors::of("touilleur", TeamOrNeutral::Blue) else {
            panic!("touilleur connu");
        };
        let def = model_for("touilleur", c.class);
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
        let mut v = ChampionView::new(301, &def, model);
        v.channel = Some(Channel::default());
        (v, def)
    }

    fn snap() -> EntitySnapshot {
        EntitySnapshot {
            id: 301,
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
            subtype: Some("touilleur".into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    #[test]
    fn clips_du_touilleur() {
        clips_finis_et_animes(&MODEL);
    }

    #[test]
    fn reste_vers_le_haut_et_decrit_un_moulinet_dans_tous_les_etats_vivants() {
        let s = snap();
        let names = [
            "idle", "run", "strike", "cast_Q", "cast_W", "cast_E", "cast_R", "recall",
        ];
        for name in names {
            let (mut v, _) = view();
            let mut pos = Vec3::ZERO;
            v.step(&s, "idle", 0.05, pos, 0.0);
            v.play_anim(name);
            let (mut xs, mut zs) = (Vec::new(), Vec::new());
            for _ in 0..24 {
                if name == "run" {
                    pos.x += 0.17;
                }
                let f = v.step(&s, "idle", 0.05, pos, 0.0);
                // direction de l'index : produit des rotations de la chaîne jusqu'à l'os prop
                let rot = [BODY, HIPS, SPINE, ARM_R, FORE_R, PROP]
                    .iter()
                    .fold(Quat::from_rotation_y(f.model_yaw), |q, j| {
                        q * f.joints[*j].rotation
                    });
                let dir = rot * Vec3::Y;
                assert!(dir.y > 0.9, "{name} : index à {dir}");
                xs.push(dir.x * 0.3);
                zs.push(dir.z * 0.3);
            }
            let range = |v: &[f32]| {
                v.iter().copied().fold(f32::MIN, f32::max)
                    - v.iter().copied().fold(f32::MAX, f32::min)
            };
            assert!(range(&xs) > 0.06, "{name}");
            assert!(range(&zs) > 0.06, "{name}");
        }
    }

    #[test]
    fn reste_dans_son_budget_et_reprend_apres_la_mort() {
        let (mut v, def) = view();
        let Some(c) = ModelColors::of("touilleur", TeamOrNeutral::Red) else {
            panic!("touilleur connu");
        };
        let built = build_model(&def, &c);
        assert!(built.sculpted);
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
        let mut s = snap();
        s.dead = Some(true);
        v.step(&s, "death", 2.5, Vec3::ZERO, 0.0);
        s.dead = None;
        assert!(v.step(&s, "idle", 0.2, Vec3::ZERO, 0.0).model_visible);
    }
}
