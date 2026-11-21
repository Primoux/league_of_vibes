//! Vrenna, la Lance des Steppes — guerrière nomade : armure de cuir rouge, foulard crème, longue lance à
//! pointe dorée.
//!
//! Portage de `packages/client/src/render/champions/models/vrenna.ts`. La géométrie est celle de son
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

/// Lance tenue à deux mains, pointe vers l'avant.
fn guard(p: &mut Pose) {
    set_rot(p, ARM_R, -0.2, 0.0, 0.5);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9);
    set_rot(p, PROP, 0.0, 0.0, -1.4);
    set_rot(p, ARM_L, 0.35, 0.0, 0.9);
    set_rot(p, FORE_L, 0.0, 0.0, 0.8);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    guard(p);
    let b = (t / 2.4 * TAU).sin();
    set_rot(p, SPINE, 0.0, 0.15, b * 0.03);
    set_rot(p, LEG_L, 0.0, 0.0, 0.2);
    set_rot(p, LEG_R, 0.0, 0.0, -0.15);
    set_offset(p, 0.0, b * 0.015 - 0.02, 0.0);
}

/// Estoc (auto-attaque, Q) : la lance part vers l'avant.
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    guard(p);
    let k = ease_out(t / 0.08) * (1.0 - ease_in((t - 0.14) / 0.22));
    set_rot(p, ARM_R, -0.2, 0.0, 0.5 + 0.9 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9 - 0.85 * k);
    set_rot(p, PROP, 0.0, 0.0, -1.4 - 0.05 * k);
    set_rot(p, SPINE, 0.0, 0.15 - 0.4 * k, -0.15 * k);
    set_rot(p, LEG_L, 0.0, 0.0, 0.45 * k + 0.2);
    set_rot(p, LEG_R, 0.0, 0.0, -0.35 * k);
    set_offset(p, 0.15 * k, -0.04 * k, 0.0);
}

/// W : cri de guerre, lance brandie.
fn cast_w(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.15) * (1.0 - ease_in((t - 0.45) / 0.25));
    set_rot(p, ARM_R, -0.3, 0.0, 2.6 * k + 0.4);
    set_rot(p, FORE_R, 0.0, 0.0, 0.3);
    set_rot(p, PROP, 0.0, 0.0, -0.5);
    set_rot(p, ARM_L, 0.9 * k, 0.0, 0.3);
    set_rot(p, SPINE, 0.0, 0.0, 0.3 * k);
    set_rot(p, HEAD, 0.0, 0.0, -0.4 * k);
}

const STRIKE: Clip = Clip {
    looped: false,
    duration: 0.36,
    sample: strike,
};

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.88,
        hip_w: 0.14,
        thigh: 0.44,
        shin: 0.43,
        spine: 0.56,
        shoulder_w: 0.26,
        upper_arm: 0.33,
        forearm: 0.3,
        neck: 0.15,
    },
    hand: Some(1.0),
    energy: Some(1.1),
    clips: &[
        (
            "idle",
            Clip {
                looped: true,
                duration: 2.4,
                sample: idle,
            },
        ),
        ("strike", STRIKE),
        ("cast_Q", STRIKE),
        (
            "cast_W",
            Clip {
                looped: false,
                duration: 0.7,
                sample: cast_w,
            },
        ),
    ],
    scale: 1.0,
    build: humanoids::vrenna,
    carry: Some(-0.5),
    extras: None,
    drop_angle: Some(-FRAC_PI_2),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_vrenna() {
        clips_finis_et_animes(&MODEL);
    }
}
