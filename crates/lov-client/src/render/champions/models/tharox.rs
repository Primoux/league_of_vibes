//! Tharox, la Charge Sauvage — colosse à tête de sanglier, défenses d'ivoire, mantelet de fourrure,
//! braises.
//!
//! Portage de `packages/client/src/render/champions/models/tharox.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, échelle et
//! clips.

use std::f32::consts::TAU;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, clamp01, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::creatures;
use crate::render::champions::rig::RigSpec;

/// Charge tête baissée (dash Q/R).
fn dash(t: f32, p: &mut Pose, _: &ClipCtx) {
    let s = (t / 0.36 * TAU).sin();
    set_rot(p, BODY, 0.0, 0.0, -0.55);
    set_rot(p, HEAD, 0.0, 0.0, -0.3);
    set_rot(p, ARM_L, 0.3, 0.0, -0.6 + s * 0.4);
    set_rot(p, ARM_R, -0.3, 0.0, -0.6 - s * 0.4);
    set_rot(p, LEG_L, 0.0, 0.0, s * 1.0);
    set_rot(p, LEG_R, 0.0, 0.0, -s * 1.0);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.4);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.4);
    set_offset(p, 0.0, s.abs() * 0.08 - 0.05, 0.0);
}

/// E : piétinement, une jambe levée puis abattue.
fn cast_e(t: f32, p: &mut Pose, _: &ClipCtx) {
    let up = ease_out(t / 0.2) * (1.0 - clamp01((t - 0.2) / 0.08));
    set_rot(p, LEG_R, 0.0, 0.0, 1.2 * up);
    set_rot(p, SHIN_R, 0.0, 0.0, -up);
    set_rot(p, ARM_L, 0.6, 0.0, 0.4);
    set_rot(p, ARM_R, -0.6, 0.0, 0.4);
    set_rot(p, SPINE, 0.0, 0.0, 0.2 * up - 0.15);
    set_offset(p, 0.0, 0.1 * up - 0.06 * (1.0 - up), 0.0);
}

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.8,
        hip_w: 0.22,
        thigh: 0.38,
        shin: 0.4,
        spine: 0.7,
        shoulder_w: 0.46,
        upper_arm: 0.4,
        forearm: 0.38,
        neck: 0.12,
    },
    hand: Some(1.0),
    energy: Some(0.8),
    clips: &[
        (
            "dash",
            Clip {
                looped: true,
                duration: 0.36,
                sample: dash,
            },
        ),
        (
            "cast_E",
            Clip {
                looped: false,
                duration: 0.55,
                sample: cast_e,
            },
        ),
    ],
    scale: 1.08,
    build: creatures::tharox,
    carry: None,
    extras: None,
    drop_angle: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_tharox() {
        clips_finis_et_animes(&MODEL);
    }
}
