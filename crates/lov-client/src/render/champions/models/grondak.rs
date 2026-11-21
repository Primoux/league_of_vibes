//! Grondak, le Rempart de Granit — golem de pierre, fissures de magma ambré, longs bras aux poings
//! massifs.
//!
//! Portage de `packages/client/src/render/champions/models/grondak.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, échelle,
//! clips et la pulsation des fissures.

use std::f32::consts::TAU;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, add_rot, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::{
    ChampionModelDef, ExtraPart, Extras, ExtrasOut, ExtrasState, ModelColors,
};
use crate::render::champions::portrait::creatures;
use crate::render::champions::rig::RigSpec;

/// Posture de gorille : bras pendants vers l'avant, dos voûté.
fn stance(p: &mut Pose, breathe: f32) {
    set_rot(p, SPINE, 0.0, 0.0, -0.22 + breathe * 0.03);
    set_rot(p, HEAD, 0.0, 0.0, 0.18);
    set_rot(p, ARM_L, 0.18, 0.0, 0.25);
    set_rot(p, ARM_R, -0.18, 0.0, 0.25);
    set_rot(p, FORE_L, 0.0, 0.0, 0.2);
    set_rot(p, FORE_R, 0.0, 0.0, 0.2);
    set_rot(p, LEG_L, 0.08, 0.0, 0.1);
    set_rot(p, LEG_R, -0.08, 0.0, 0.1);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.2);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.2);
    set_offset(p, 0.0, -0.04 + breathe * 0.015, 0.0);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    let b = (t / 3.4 * TAU).sin();
    stance(p, b);
    add_rot(p, ARM_L, b * 0.04, 0.0, 0.0);
    add_rot(p, ARM_R, -b * 0.04, 0.0, 0.0);
    set_rot(p, HEAD, 0.0, (t / 6.8 * TAU).sin() * 0.15, 0.18);
}

/// Double frappe des poings (auto-attaque).
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let hit = ease_out(t / 0.1);
    let back = ease_in((t - 0.18) / 0.27);
    let k = hit * (1.0 - back);
    add_rot(p, ARM_L, 0.0, 0.0, 1.0 * k);
    add_rot(p, ARM_R, 0.0, 0.0, 1.0 * k);
    add_rot(p, SPINE, 0.0, 0.0, -0.25 * k);
    set_offset(p, 0.1 * k, -0.08 * k, 0.0);
}

fn attack_ready(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let k = ease_out(t / 0.3);
    add_rot(p, ARM_L, 0.0, 0.0, 1.8 * k);
    add_rot(p, ARM_R, 0.0, 0.0, 1.8 * k);
    add_rot(p, FORE_L, 0.0, 0.0, 0.6 * k);
    add_rot(p, FORE_R, 0.0, 0.0, 0.6 * k);
    add_rot(p, SPINE, 0.0, 0.0, 0.18 * k);
}

/// Q : poing droit abattu sur le sol.
fn cast_q(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let up = ease_out(t / 0.15);
    let slam = ease_in((t - 0.12) / 0.1);
    let rec = ease_in((t - 0.35) / 0.25);
    add_rot(p, ARM_R, 0.0, 0.0, (2.6 * up - 2.2 * slam) * (1.0 - rec));
    add_rot(p, SPINE, 0.0, 0.0, (0.2 * up - 0.55 * slam) * (1.0 - rec));
    set_offset(
        p,
        0.15 * slam * (1.0 - rec),
        -0.18 * slam * (1.0 - rec),
        0.0,
    );
}

/// W : rugissement, poitrine en avant, bras écartés.
fn cast_w(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let k = ease_out(t / 0.15) * (1.0 - ease_in((t - 0.45) / 0.25));
    add_rot(p, ARM_L, 1.1 * k, 0.0, 0.3 * k);
    add_rot(p, ARM_R, -1.1 * k, 0.0, 0.3 * k);
    add_rot(p, SPINE, 0.0, 0.0, 0.45 * k);
    add_rot(p, HEAD, 0.0, 0.0, -0.5 * k);
    set_offset(p, 0.0, 0.05 * k, 0.0);
}

/// Charge : épaule en avant (dash du E).
fn dash(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let s = (t / 0.4 * TAU).sin();
    set_rot(p, BODY, 0.0, -0.35, -0.3);
    add_rot(p, ARM_R, -0.3, 0.0, -0.4);
    add_rot(p, ARM_L, 0.2, 0.0, 0.9);
    set_rot(p, LEG_L, 0.0, 0.0, s * 0.9);
    set_rot(p, LEG_R, 0.0, 0.0, -s * 0.9);
    set_offset(p, 0.0, s.abs() * 0.05, 0.0);
}

/// R : deux poings levés puis abattus.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    stance(p, 0.0);
    let up = ease_out(t / 0.3);
    let slam = ease_in((t - 0.3) / 0.12);
    let rec = ease_in((t - 0.6) / 0.3);
    let raise = (3.0 * up - 2.5 * slam) * (1.0 - rec);
    add_rot(p, ARM_L, 0.0, 0.0, raise);
    add_rot(p, ARM_R, 0.0, 0.0, raise);
    add_rot(p, SPINE, 0.0, 0.0, (0.4 * up - 0.75 * slam) * (1.0 - rec));
    set_offset(
        p,
        0.1 * slam * (1.0 - rec),
        (0.12 * up - 0.3 * slam) * (1.0 - rec),
        0.0,
    );
}

fn no_parts(_: &ModelColors) -> Vec<ExtraPart> {
    Vec::new()
}

/// Les fissures pulsent doucement (respiration du magma) : la teinte multiplie les couleurs de sommets.
fn pulse(s: &ExtrasState, out: &mut ExtrasOut) {
    *out.glow = Some(0.75 + 0.25 * (s.time * 2.2).sin());
}

const fn clip(
    looped: bool,
    duration: f32,
    sample: crate::render::champions::anim::pose::Sample,
) -> Clip {
    Clip {
        looped,
        duration,
        sample,
    }
}

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.82,
        hip_w: 0.26,
        thigh: 0.38,
        shin: 0.42,
        spine: 0.78,
        shoulder_w: 0.55,
        upper_arm: 0.46,
        forearm: 0.44,
        neck: 0.1,
    },
    hand: Some(1.0),
    energy: Some(0.7),
    clips: &[
        ("idle", clip(true, 3.4, idle)),
        ("strike", clip(false, 0.45, strike)),
        ("attackReady", clip(true, 1.0, attack_ready)),
        ("cast_Q", clip(false, 0.6, cast_q)),
        ("cast_W", clip(false, 0.7, cast_w)),
        ("cast_R", clip(false, 0.9, cast_r)),
        ("dash", clip(true, 0.4, dash)),
    ],
    scale: 1.05,
    build: creatures::grondak,
    carry: None,
    extras: Some(Extras {
        parts: no_parts,
        update: pulse,
    }),
    drop_angle: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::models::testing::clips_finis_et_animes;
    use bevy::prelude::Transform;

    #[test]
    fn clips_de_grondak() {
        clips_finis_et_animes(&MODEL);
    }

    #[test]
    fn les_fissures_pulsent() {
        let mut joints = [Transform::IDENTITY; JOINT_COUNT];
        let mut tint = |time: f32| {
            let mut glow = None;
            let state = ExtrasState {
                time,
                dead: false,
                moving: false,
                anim: "idle",
            };
            pulse(
                &state,
                &mut ExtrasOut {
                    parts: &mut [],
                    glow: &mut glow,
                    joints: &mut joints,
                },
            );
            glow.unwrap_or(0.0)
        };
        let peak = std::f32::consts::FRAC_PI_2 / 2.2;
        assert!((tint(peak) - 1.0).abs() < 1e-5);
        assert!((tint(3.0 * peak) - 0.5).abs() < 1e-5);
    }
}
