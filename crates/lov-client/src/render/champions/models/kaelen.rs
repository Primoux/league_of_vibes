//! Kaelen, la Lame de l'Aube — chevalier solaire : armure dorée, tabard bleu nuit, plumet, grande épée
//! lumineuse.
//!
//! Portage de `packages/client/src/render/champions/models/kaelen.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, clips et port
//! d'arme.

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;

/// Taille horizontale (auto-attaque) : l'épée balaie devant lui.
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.1) * (1.0 - ease_in((t - 0.16) / 0.22));
    set_rot(p, ARM_R, -1.2 + 0.4 * k, 0.0, 0.9 + 0.4 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.2);
    set_rot(p, SPINE, 0.0, 0.6 - 1.4 * k, -0.1);
    set_rot(p, ARM_L, 0.3, 0.0, 0.2);
    set_rot(p, LEG_L, 0.0, 0.0, 0.3);
    set_rot(p, LEG_R, 0.0, 0.0, -0.25);
    set_offset(p, 0.08 * k, -0.03, 0.0);
}

/// R : épée levée à deux mains puis abattue.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    let up = ease_out(t / 0.3);
    let down = ease_in((t - 0.32) / 0.1);
    let rec = ease_in((t - 0.55) / 0.2);
    let raise = (2.9 * up - 2.2 * down) * (1.0 - rec);
    set_rot(p, ARM_R, -0.3, 0.0, raise);
    set_rot(p, ARM_L, 0.3, 0.0, raise);
    set_rot(p, FORE_R, 0.0, 0.0, 0.3 * up);
    set_rot(p, FORE_L, 0.0, 0.0, 0.3 * up);
    set_rot(p, SPINE, 0.0, 0.0, (0.25 * up - 0.5 * down) * (1.0 - rec));
    set_offset(
        p,
        0.1 * down * (1.0 - rec),
        (0.08 * up - 0.15 * down) * (1.0 - rec),
        0.0,
    );
}

/// W : parade, épée en travers.
fn cast_w(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.12) * (1.0 - ease_in((t - 1.0) / 0.2));
    set_rot(p, ARM_R, -0.6 * k, 0.0, 1.3 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 1.2 * k);
    set_rot(p, PROP, 0.0, 0.0, -0.9 * k);
    set_rot(p, ARM_L, 0.4 * k, 0.0, 1.0 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 1.3 * k);
    set_rot(p, LEG_L, 0.0, 0.0, 0.35 * k);
    set_rot(p, LEG_R, 0.0, 0.0, -0.3 * k);
    set_offset(p, 0.0, -0.08 * k, 0.0);
}

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.88,
        hip_w: 0.15,
        thigh: 0.44,
        shin: 0.43,
        spine: 0.6,
        shoulder_w: 0.3,
        upper_arm: 0.34,
        forearm: 0.31,
        neck: 0.15,
    },
    hand: Some(1.0),
    energy: Some(1.05),
    clips: &[
        (
            "strike",
            Clip {
                looped: false,
                duration: 0.38,
                sample: strike,
            },
        ),
        (
            "cast_R",
            Clip {
                looped: false,
                duration: 0.75,
                sample: cast_r,
            },
        ),
        (
            "cast_W",
            Clip {
                looped: false,
                duration: 1.2,
                sample: cast_w,
            },
        ),
    ],
    scale: 1.0,
    build: humanoids::kaelen,
    carry: Some(-0.25),
    extras: None,
    drop_angle: Some(0.0),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_kaelen() {
        clips_finis_et_animes(&MODEL);
        // l'épée s'abat : le bras monte puis redescend
        let mut p = [0.0; crate::render::champions::anim::pose::POSE_SIZE];
        let ctx = ClipCtx {
            hand: 1.0,
            energy: 1.0,
        };
        cast_r(0.3, &mut p, &ctx);
        let up = p[ARM_R * 3 + 2];
        cast_r(0.45, &mut p, &ctx);
        assert!(up > 2.5 && p[ARM_R * 3 + 2] < 1.0);
    }
}
