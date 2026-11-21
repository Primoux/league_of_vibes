//! Nyx, l'Ombre Sans Nom — assassin encapuchonné, voiles d'ombre, deux lames courbes à fil rose.
//!
//! Portage de `packages/client/src/render/champions/models/nyx.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, échelle et
//! clips.

use std::f32::consts::TAU;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;

/// Garde basse, lames vers l'avant.
fn crouch(p: &mut Pose) {
    set_rot(p, SPINE, 0.0, 0.0, -0.3);
    set_rot(p, HEAD, 0.0, 0.0, 0.2);
    set_rot(p, ARM_L, 0.4, 0.0, 0.6);
    set_rot(p, ARM_R, -0.4, 0.0, 0.6);
    set_rot(p, FORE_L, 0.0, 0.0, 0.9);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9);
    set_rot(p, LEG_L, 0.0, 0.0, 0.45);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.7);
    set_rot(p, LEG_R, 0.0, 0.0, -0.15);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.5);
    set_offset(p, 0.0, -0.14, 0.0);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    crouch(p);
    let b = (t / 2.0 * TAU).sin();
    set_rot(p, SPINE, 0.0, b * 0.08, -0.3);
}

/// Double entaille croisée (auto-attaque, Q).
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    crouch(p);
    let k = ease_out(t / 0.07) * (1.0 - ease_in((t - 0.12) / 0.18));
    set_rot(p, ARM_L, 0.4 - 1.0 * k, 0.0, 0.6 + 0.9 * k);
    set_rot(p, ARM_R, -0.4 + 1.0 * k, 0.0, 0.6 + 0.9 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 0.9 - 0.7 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9 - 0.7 * k);
    set_offset(p, 0.15 * k, -0.14, 0.0);
}

const STRIKE: Clip = Clip {
    looped: false,
    duration: 0.3,
    sample: strike,
};

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.84,
        hip_w: 0.12,
        thigh: 0.42,
        shin: 0.42,
        spine: 0.52,
        shoulder_w: 0.22,
        upper_arm: 0.31,
        forearm: 0.29,
        neck: 0.14,
    },
    hand: Some(1.0),
    energy: Some(1.25),
    clips: &[
        (
            "idle",
            Clip {
                looped: true,
                duration: 2.0,
                sample: idle,
            },
        ),
        ("strike", STRIKE),
        ("cast_Q", STRIKE),
    ],
    scale: 0.98,
    build: humanoids::nyx,
    carry: None,
    extras: None,
    drop_angle: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_nyx() {
        clips_finis_et_animes(&MODEL);
    }
}
