//! Ilyra, la Sentinelle du Givre — mage élancée en robe, couronne d'éclats de glace, bâton surmonté d'un
//! cristal, trois éclats en orbite autour d'elle.
//!
//! Portage de `packages/client/src/render/champions/models/ilyra.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, clips, port
//! d'arme et les éclats en orbite.
//!
//! Écart provisoire : les éclats prennent la couleur du matériau de glace (`createIceMaterial`, shader du
//! socle FX, #111) à son opacité, sans son dégradé de profondeur.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use crate::render::champions::anim::clips;
use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, Sample, add_rot, bell, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::{
    ChampionModelDef, ExtraPart, Extras, ExtrasOut, ExtrasState, ModelColors,
};
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;
use crate::render::models::geometry::octahedron;
use crate::render::models::kit::euler;

/// Bâton tenu verticalement dans la main droite, main gauche ouverte.
fn stance(p: &mut Pose) {
    set_rot(p, ARM_R, -0.15, 0.0, 0.35);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9);
    set_rot(p, PROP, 0.0, 0.0, -1.25); // garde le bâton vertical malgré le coude plié
    set_rot(p, ARM_L, 0.2, 0.0, 0.1);
    set_rot(p, FORE_L, 0.0, 0.0, 0.35);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let b = (t / 3.0 * TAU).sin();
    set_rot(p, SPINE, 0.0, 0.0, b * 0.02);
    add_rot(p, ARM_L, b * 0.05, 0.0, b * 0.04);
    set_rot(p, HEAD, 0.0, (t / 6.0 * TAU).sin() * 0.15, -0.05);
    set_offset(p, 0.0, b * 0.015, 0.0);
}

/// Auto-attaque : le bâton pointe la cible.
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let k = ease_out(t / 0.08) * (1.0 - ease_in((t - 0.15) / 0.25));
    add_rot(p, ARM_R, 0.0, 0.0, 0.9 * k);
    add_rot(p, FORE_R, 0.0, 0.0, -0.5 * k);
    add_rot(p, PROP, 0.0, 0.0, -0.5 * k);
    set_rot(p, SPINE, 0.0, 0.3 * k, -0.08 * k);
}

fn attack_ready(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let k = ease_out(t / 0.25);
    add_rot(p, ARM_R, 0.0, 0.0, -0.25 * k);
    add_rot(p, PROP, 0.0, 0.0, 0.3 * k);
    set_rot(p, SPINE, 0.0, -0.25 * k, 0.05 * k);
}

/// Q : main gauche projette l'éclat.
fn cast_q(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let wind = ease_out(t / 0.15);
    let throw = ease_out((t - 0.15) / 0.1);
    let rec = ease_in((t - 0.3) / 0.2);
    add_rot(
        p,
        ARM_L,
        0.2 * wind,
        0.0,
        (-0.6 * wind + 2.0 * throw) * (1.0 - rec),
    );
    add_rot(
        p,
        FORE_L,
        0.0,
        0.0,
        (1.2 * wind - 1.1 * throw) * (1.0 - rec),
    );
    set_rot(
        p,
        SPINE,
        0.0,
        (-0.4 * wind + 0.7 * throw) * (1.0 - rec),
        0.0,
    );
}

/// W / E : bâton levé, main tendue.
fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let k = bell(t, 0.0, 0.5);
    add_rot(p, ARM_R, 0.0, 0.0, 0.8 * k);
    add_rot(p, ARM_L, 0.3 * k, 0.0, 1.4 * k);
    add_rot(p, FORE_L, 0.0, 0.0, -0.3 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.1 * k);
}

/// R : bâton brandi au ciel, lévitation.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p);
    let k = ease_out(t / 0.3) * (1.0 - ease_in((t - 0.7) / 0.3));
    add_rot(p, ARM_R, -0.2 * k, 0.0, 2.4 * k);
    add_rot(p, FORE_R, 0.0, 0.0, -0.8 * k);
    add_rot(p, PROP, 0.0, 0.0, 1.0 * k);
    add_rot(p, ARM_L, 0.9 * k, 0.0, 0.4 * k);
    set_rot(p, SPINE, 0.0, 0.0, 0.2 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.3 * k);
    set_offset(p, 0.0, 0.25 * k, 0.0);
}

/// `uprightStaff` : garde le bâton vertical. Sa direction dépend de la somme des rotations z de la
/// chaîne corps → torse → bras → avant-bras → bâton ; on compense sur le bâton.
fn upright_staff(p: &mut Pose) {
    p[PROP * 3 + 2] = -(p[BODY * 3 + 2] + p[SPINE * 3 + 2] + p[ARM_R * 3 + 2] + p[FORE_R * 3 + 2]);
}

fn recall(t: f32, p: &mut Pose, ctx: &ClipCtx) {
    clips::recall(t, p, ctx);
    upright_staff(p);
}

fn stunned(t: f32, p: &mut Pose, ctx: &ClipCtx) {
    clips::stunned(t, p, ctx);
    upright_staff(p);
}

/// Couleur du matériau de glace (`createIceMaterial`) et son opacité.
const ICE: (f32, f32, f32, f32) = (0.35, 0.65, 0.95, 0.92);

/// Trois éclats de glace en orbite autour de la taille.
fn shards(_: &ModelColors) -> Vec<ExtraPart> {
    let geo = octahedron(0.07, 0).scale(1.0, 2.2, 1.0);
    (0..3)
        .map(|_| ExtraPart {
            joint: BODY,
            geo: geo.clone(),
            color: LinearRgba::rgb(ICE.0, ICE.1, ICE.2),
            opacity: ICE.3,
        })
        .collect()
}

fn orbit(s: &ExtrasState, out: &mut ExtrasOut) {
    for (i, part) in out.parts.iter_mut().enumerate() {
        let a = s.time * 1.4 + i as f32 / 3.0 * TAU;
        let y = 1.05 + (s.time * 2.0 + i as f32).sin() * 0.08;
        *part = (
            Transform::from_xyz(a.cos() * 0.55, y, a.sin() * 0.55)
                .with_rotation(euler(Vec3::new(0.0, -a, 0.3))),
            !s.dead,
        );
    }
}

const fn clip(looped: bool, duration: f32, sample: Sample) -> Clip {
    Clip {
        looped,
        duration,
        sample,
    }
}

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.9,
        hip_w: 0.11,
        thigh: 0.45,
        shin: 0.43,
        spine: 0.55,
        shoulder_w: 0.23,
        upper_arm: 0.3,
        forearm: 0.29,
        neck: 0.16,
    },
    hand: Some(1.0),
    energy: Some(0.85),
    clips: &[
        ("idle", clip(true, 3.0, idle)),
        ("strike", clip(false, 0.4, strike)),
        ("attackReady", clip(true, 1.0, attack_ready)),
        ("cast", clip(false, 0.5, cast)),
        ("cast_Q", clip(false, 0.5, cast_q)),
        ("cast_R", clip(false, 1.0, cast_r)),
        // mêmes boucle et durée que les clips génériques qu'ils corrigent
        ("recall", clip(true, 3.0, recall)),
        ("stunned", clip(true, 1.2, stunned)),
    ],
    scale: 1.0,
    build: humanoids::ilyra,
    carry: Some(0.0),
    extras: Some(Extras {
        parts: shards,
        update: orbit,
    }),
    drop_angle: Some(-FRAC_PI_2),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::clips::generic_clips;
    use crate::render::champions::anim::pose::{JOINT_COUNT, POSE_SIZE};
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_d_ilyra() {
        clips_finis_et_animes(&MODEL);
        // les clips corrigés gardent boucle et durée des génériques
        let generic = generic_clips();
        for name in ["recall", "stunned"] {
            let Some((_, mine)) = MODEL.clips.iter().find(|(n, _)| *n == name) else {
                panic!("{name}");
            };
            assert_eq!(mine.looped, generic[name].looped, "{name}");
            assert_eq!(mine.duration, generic[name].duration, "{name}");
        }
    }

    #[test]
    fn baton_vertical_pendant_le_rappel_et_l_etourdissement() {
        let ctx = ClipCtx {
            hand: 1.0,
            energy: 0.85,
        };
        for f in [recall as fn(f32, &mut Pose, &ClipCtx), stunned] {
            for t in [0.1, 0.5, 1.3] {
                let mut p = [0.0; POSE_SIZE];
                f(t, &mut p, &ctx);
                let chain: f32 = [BODY, SPINE, ARM_R, FORE_R, PROP]
                    .iter()
                    .map(|j| p[j * 3 + 2])
                    .sum();
                assert!(chain.abs() < 1e-6);
            }
        }
    }

    #[test]
    fn trois_eclats_en_orbite() {
        let Some(c) = ModelColors::of("ilyra", lov_shared::types::TeamOrNeutral::Blue) else {
            panic!("ilyra connue");
        };
        assert_eq!(shards(&c).len(), 3);
        let mut parts = [(Transform::IDENTITY, false); 3];
        let mut joints = [Transform::IDENTITY; JOINT_COUNT];
        let mut glow = None;
        let state = ExtrasState {
            time: 0.0,
            dead: false,
            moving: false,
            anim: "idle",
        };
        orbit(
            &state,
            &mut ExtrasOut {
                parts: &mut parts,
                glow: &mut glow,
                joints: &mut joints,
            },
        );
        for (t, on) in parts {
            assert!(on && (t.translation.xz().length() - 0.55).abs() < 1e-5);
        }
        let dead = ExtrasState {
            dead: true,
            ..state
        };
        orbit(
            &dead,
            &mut ExtrasOut {
                parts: &mut parts,
                glow: &mut glow,
                joints: &mut joints,
            },
        );
        assert!(parts.iter().all(|(_, on)| !on));
    }
}
