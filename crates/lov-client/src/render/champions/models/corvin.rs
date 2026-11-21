//! Corvin, l'Archer des Corbeaux — archer en cape de plumes noires, masque à bec, arc recourbé, yeux
//! verts.
//!
//! Portage de `packages/client/src/render/champions/models/corvin.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette et clips.
//! L'arc est tenu à gauche : c'est sa main de frappe.

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::ChampionModelDef;
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;

/// Arc tenu main gauche ; la main droite tend la corde (k = tension).
fn draw(p: &mut Pose, k: f32) {
    set_rot(p, ARM_L, 0.2, 0.0, 0.3 + 1.25 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 0.5 - 0.45 * k);
    set_rot(p, ARM_R, -0.3, 0.0, 0.3 + 1.1 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.5 + 1.6 * k);
    set_rot(p, SPINE, 0.0, -0.5 * k, 0.0);
}

fn attack_ready(t: f32, p: &mut Pose, _: &ClipCtx) {
    draw(p, ease_out(t / 0.2));
}

/// Décoche : la main droite lâche la corde.
fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    draw(p, 1.0 - ease_in((t - 0.1) / 0.22));
    set_rot(p, FORE_R, 0.0, 0.0, 2.1 - 1.6 * ease_out(t / 0.05));
}

fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.25) * (1.0 - ease_in((t - 0.55) / 0.25));
    set_rot(p, ARM_L, 0.4, 0.0, 2.5 * k);
    set_rot(p, ARM_R, -0.6, 0.0, 2.2 * k);
    set_rot(p, SPINE, 0.0, 0.0, 0.3 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.4 * k);
    set_offset(p, 0.0, 0.05 * k, 0.0);
}

const STRIKE: Clip = Clip {
    looped: false,
    duration: 0.32,
    sample: strike,
};

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.87,
        hip_w: 0.13,
        thigh: 0.44,
        shin: 0.42,
        spine: 0.56,
        shoulder_w: 0.25,
        upper_arm: 0.33,
        forearm: 0.3,
        neck: 0.15,
    },
    hand: Some(-1.0),
    energy: Some(1.0),
    clips: &[
        (
            "attackReady",
            Clip {
                looped: true,
                duration: 1.0,
                sample: attack_ready,
            },
        ),
        ("strike", STRIKE),
        ("cast_Q", STRIKE),
        (
            "cast_R",
            Clip {
                looped: false,
                duration: 0.8,
                sample: cast_r,
            },
        ),
    ],
    scale: 1.0,
    build: humanoids::corvin,
    carry: None,
    extras: None,
    drop_angle: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_corvin() {
        clips_finis_et_animes(&MODEL);
    }
}
