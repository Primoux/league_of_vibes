//! Zephyr, l'Arcaniste Instable — mage en longue cape violette, capuche, gantelets cyan, éclats
//! arcaniques qui gravitent autour de lui (de plus en plus vite quand il incante).
//!
//! Portage de `packages/client/src/render/champions/models/zephyr.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, clips et les
//! éclats en orbite.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, Sample, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::model::{
    ChampionModelDef, ExtraPart, Extras, ExtrasOut, ExtrasState, ModelColors,
};
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;
use crate::render::models::geometry::octahedron;
use crate::render::models::kit::euler;

/// Lévitation légère, mains ouvertes.
fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    let b = (t / 2.2 * TAU).sin();
    set_rot(p, ARM_L, 0.4, 0.0, 0.35 + b * 0.05);
    set_rot(p, ARM_R, -0.4, 0.0, 0.35 - b * 0.05);
    set_rot(p, FORE_L, 0.0, 0.0, 0.7);
    set_rot(p, FORE_R, 0.0, 0.0, 0.7);
    set_rot(p, LEG_L, 0.0, 0.0, 0.1);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.25);
    set_offset(p, 0.0, 0.12 + b * 0.04, 0.0);
}

/// Incantation : deux mains projetées vers l'avant.
fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.1) * (1.0 - ease_in((t - 0.25) / 0.2));
    set_rot(p, ARM_L, 0.3, 0.0, 0.35 + 1.2 * k);
    set_rot(p, ARM_R, -0.3, 0.0, 0.35 + 1.2 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 0.7 - 0.6 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.7 - 0.6 * k);
    set_rot(p, SPINE, 0.0, 0.0, -0.15 * k);
    set_offset(p, -0.06 * k, 0.12, 0.0);
}

/// R : bras levés, le corps s'élève.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.35) * (1.0 - ease_in((t - 0.65) / 0.25));
    set_rot(p, ARM_L, 0.6, 0.0, 2.6 * k);
    set_rot(p, ARM_R, -0.6, 0.0, 2.6 * k);
    set_rot(p, SPINE, 0.0, 0.0, 0.3 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.35 * k);
    set_offset(p, 0.0, 0.12 + 0.35 * k, 0.0);
}

/// Éclats arcaniques : accent surexposé (× 1,8), non éclairés.
fn shards(c: &ModelColors) -> Vec<ExtraPart> {
    let a = c.accent;
    (0..4)
        .map(|_| ExtraPart {
            joint: BODY,
            geo: octahedron(0.06, 0),
            color: LinearRgba::rgb(a.red * 1.8, a.green * 1.8, a.blue * 1.8),
            opacity: 1.0,
        })
        .collect()
}

/// Orbite irrégulière, plus rapide pendant les incantations.
fn orbit(s: &ExtrasState, out: &mut ExtrasOut) {
    let speed = if s.anim.starts_with("cast") { 5.0 } else { 1.6 };
    for (i, part) in out.parts.iter_mut().enumerate() {
        let fi = i as f32;
        let a = s.time * speed * if i % 2 != 0 { 1.0 } else { -1.3 } + fi * 1.7;
        let r = 0.45 + fi * 0.05;
        let y = 1.0 + (s.time * 2.3 + fi).sin() * 0.35;
        *part = (
            Transform::from_xyz(a.cos() * r, y, a.sin() * r).with_rotation(euler(Vec3::new(
                s.time * 3.0,
                s.time * 2.0,
                0.0,
            ))),
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

const CAST: Clip = clip(false, 0.45, cast);

pub const MODEL: ChampionModelDef = ChampionModelDef {
    rig: RigSpec {
        hip_y: 0.88,
        hip_w: 0.12,
        thigh: 0.44,
        shin: 0.43,
        spine: 0.56,
        shoulder_w: 0.25,
        upper_arm: 0.31,
        forearm: 0.3,
        neck: 0.15,
    },
    hand: Some(1.0),
    energy: Some(0.9),
    clips: &[
        ("idle", clip(true, 2.2, idle)),
        ("cast", CAST),
        ("cast_R", clip(false, 0.9, cast_r)),
        ("strike", CAST),
    ],
    scale: 1.0,
    build: humanoids::zephyr,
    carry: None,
    extras: Some(Extras {
        parts: shards,
        update: orbit,
    }),
    drop_angle: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    fn angle_after(anim: &str) -> f32 {
        let mut parts = [(Transform::IDENTITY, false); 4];
        let mut joints = [Transform::IDENTITY; JOINT_COUNT];
        let mut glow = None;
        let state = ExtrasState {
            time: 0.1,
            dead: false,
            moving: false,
            anim,
        };
        orbit(
            &state,
            &mut ExtrasOut {
                parts: &mut parts,
                glow: &mut glow,
                joints: &mut joints,
            },
        );
        let p = parts[1].0.translation;
        p.z.atan2(p.x) - 1.7
    }

    #[test]
    fn clips_de_zephyr() {
        clips_finis_et_animes(&MODEL);
    }

    #[test]
    fn eclats_plus_rapides_en_incantation() {
        assert!((angle_after("idle") - 0.16).abs() < 1e-4);
        assert!((angle_after("cast_Q") - 0.5).abs() < 1e-4);
    }
}
