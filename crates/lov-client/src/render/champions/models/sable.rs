//! Sable, la Tireuse des Dunes — pistolera du désert : chapeau à large bord, cache-poussière, long fusil à
//! canon cuivré.
//!
//! Portage de `packages/client/src/render/champions/models/sable.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, clips et port
//! d'arme.

use std::f32::consts::{FRAC_PI_2, TAU};

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;

/// Fusil épaulé : les deux bras vers l'avant.
fn aim(p: &mut Pose, k: f32) {
    set_rot(p, ARM_R, -0.15, 0.0, 0.3 + 1.1 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.6 - 0.5 * k);
    set_rot(p, PROP, 0.0, 0.0, -0.9 + 0.75 * k);
    set_rot(p, ARM_L, 0.5, 0.0, 0.3 + 1.2 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 0.9 - 0.4 * k);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    aim(p, 0.0);
    let b = (t / 2.6 * TAU).sin();
    set_rot(p, SPINE, 0.0, 0.0, b * 0.02);
    set_rot(p, HEAD, 0.0, (t / 5.2 * TAU).sin() * 0.2, 0.0);
    set_offset(p, 0.0, b * 0.015, 0.0);
}

fn attack_ready(t: f32, p: &mut Pose, _: &ClipCtx) {
    aim(p, ease_out(t / 0.15));
    set_rot(p, SPINE, 0.0, -0.2, 0.0);
}

/// Tir : recul du fusil.
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    aim(p, 1.0);
    let kick = ease_out(t / 0.04) * (1.0 - ease_in((t - 0.05) / 0.2));
    set_rot(p, ARM_R, -0.15, 0.0, 1.4 + 0.35 * kick);
    set_rot(p, SPINE, 0.0, -0.2, 0.12 * kick);
    set_offset(p, -0.06 * kick, 0.0, 0.0);
}

/// R : visée longue, un genou à terre.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.25);
    aim(p, k);
    set_rot(p, LEG_L, 0.0, 0.0, 1.0 * k);
    set_rot(p, SHIN_L, 0.0, 0.0, -1.4 * k);
    set_rot(p, LEG_R, 0.0, 0.0, -0.5 * k);
    set_rot(p, SHIN_R, 0.0, 0.0, -1.5 * k);
    let kick = if t > 1.0 {
        ease_out((t - 1.0) / 0.05) * (1.0 - ease_in((t - 1.05) / 0.2))
    } else {
        0.0
    };
    set_rot(p, SPINE, 0.0, -0.2, 0.2 * kick);
    set_offset(p, -0.1 * kick, -0.38 * k, 0.0);
}

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.86,
        hip_w: 0.13,
        thigh: 0.43,
        shin: 0.42,
        spine: 0.55,
        shoulder_w: 0.24,
        upper_arm: 0.32,
        forearm: 0.3,
        neck: 0.15,
    },
    hand: Some(1.0),
    energy: Some(1.0),
    clips: &[
        (
            "idle",
            Clip {
                looped: true,
                duration: 2.6,
                sample: idle,
            },
        ),
        (
            "attackReady",
            Clip {
                looped: true,
                duration: 1.0,
                sample: attack_ready,
            },
        ),
        (
            "strike",
            Clip {
                looped: false,
                duration: 0.3,
                sample: strike,
            },
        ),
        (
            "cast_R",
            Clip {
                looped: false,
                duration: 1.25,
                sample: cast_r,
            },
        ),
    ],
    scale: 1.0,
    build: humanoids::sable,
    carry: Some(-1.9),
    extras: None,
    drop_angle: Some(-FRAC_PI_2),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_sable() {
        clips_finis_et_animes(&MODEL);
    }
}
