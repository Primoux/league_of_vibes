//! Solenne, la Prêtresse de l'Aube — robe ivoire et ambre, auréole solaire flottante, bâton surmonté
//! d'un disque.
//!
//! Portage de `packages/client/src/render/champions/models/solenne.ts`. La géométrie est celle de son
//! portrait (`portraitModel` la remplace dans le client web) ; ce modèle apporte squelette, clips, port
//! d'arme et l'auréole.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use crate::render::champions::anim::pose::j::*;
use crate::render::champions::anim::pose::{
    Clip, ClipCtx, Pose, Sample, bell, ease_in, ease_out, set_offset, set_rot,
};
use crate::render::champions::builder::css;
use crate::render::champions::model::{
    ChampionModelDef, ExtraPart, Extras, ExtrasOut, ExtrasState, ModelColors,
};
use crate::render::champions::portrait::humanoids;
use crate::render::champions::rig::RigSpec;
use crate::render::models::geometry::torus;
use crate::render::models::kit::euler;

fn hold(p: &mut Pose) {
    set_rot(p, ARM_R, -0.15, 0.0, 0.35);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9);
    set_rot(p, PROP, 0.0, 0.0, -1.25);
    set_rot(p, ARM_L, 0.25, 0.0, 0.3);
    set_rot(p, FORE_L, 0.0, 0.0, 0.9);
}

fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    hold(p);
    let b = (t / 3.0 * TAU).sin();
    set_rot(p, HEAD, 0.0, 0.0, -0.05 + b * 0.03);
    set_offset(p, 0.0, b * 0.015, 0.0);
}

/// Bénédiction : main gauche ouverte vers la cible.
fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    hold(p);
    let k = bell(t, 0.0, 0.55);
    set_rot(p, ARM_L, 0.2, 0.0, 0.3 + 1.3 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 0.9 - 0.8 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.12 * k);
}

fn strike(t: f32, p: &mut Pose, _: &ClipCtx) {
    hold(p);
    let k = ease_out(t / 0.08) * (1.0 - ease_in((t - 0.14) / 0.24));
    set_rot(p, ARM_R, -0.15, 0.0, 0.35 + 0.9 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9 - 0.5 * k);
    set_rot(p, PROP, 0.0, 0.0, -1.25 - 0.4 * k);
}

/// R : bâton levé au ciel, bras ouverts.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    hold(p);
    let k = ease_out(t / 0.3) * (1.0 - ease_in((t - 0.7) / 0.3));
    set_rot(p, ARM_R, -0.2, 0.0, 0.35 + 2.3 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 0.9 - 0.8 * k);
    set_rot(p, PROP, 0.0, 0.0, -1.25 + 0.6 * k);
    set_rot(p, ARM_L, 1.2 * k, 0.0, 0.3 + 0.8 * k);
    set_rot(p, SPINE, 0.0, 0.0, 0.2 * k);
    set_rot(p, HEAD, 0.0, 0.0, 0.35 * k);
    set_offset(p, 0.0, 0.12 * k, 0.0);
}

/// Auréole flottante derrière la tête : accent mêlé d'or pâle, surexposé (× 1,6), translucide.
fn halo(c: &ModelColors) -> Vec<ExtraPart> {
    let (a, b) = (c.accent, css("#ffe9a8"));
    let mix = |x: f32, y: f32| (x + (y - x) * 0.5) * 1.6;
    vec![ExtraPart {
        joint: HEAD,
        geo: torus(0.2, 0.018, 4, 24),
        color: LinearRgba::rgb(
            mix(a.red, b.red),
            mix(a.green, b.green),
            mix(a.blue, b.blue),
        ),
        opacity: 0.85,
    }]
}

fn float(s: &ExtrasState, out: &mut ExtrasOut) {
    for part in out.parts.iter_mut() {
        *part = (
            Transform::from_xyz(-0.12, 0.2 + (s.time * 2.0).sin() * 0.02, 0.0)
                .with_rotation(euler(Vec3::new(s.time * 0.5, FRAC_PI_2, 0.0))),
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
        ("cast", clip(false, 0.55, cast)),
        ("strike", clip(false, 0.38, strike)),
        ("cast_R", clip(false, 1.0, cast_r)),
    ],
    scale: 1.0,
    build: humanoids::solenne,
    carry: Some(0.0),
    extras: Some(Extras {
        parts: halo,
        update: float,
    }),
    drop_angle: Some(-FRAC_PI_2),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::models::testing::clips_finis_et_animes;

    #[test]
    fn clips_de_solenne() {
        clips_finis_et_animes(&MODEL);
    }

    #[test]
    fn aureole_derriere_la_tete() {
        let Some(c) = ModelColors::of("solenne", lov_shared::types::TeamOrNeutral::Red) else {
            panic!("solenne connue");
        };
        let parts = halo(&c);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].opacity < 1.0 && parts[0].joint == HEAD);
        let mut out = [(Transform::IDENTITY, false)];
        let mut joints = [Transform::IDENTITY; JOINT_COUNT];
        let mut glow = None;
        let state = ExtrasState {
            time: 0.0,
            dead: false,
            moving: false,
            anim: "idle",
        };
        float(
            &state,
            &mut ExtrasOut {
                parts: &mut out,
                glow: &mut glow,
                joints: &mut joints,
            },
        );
        let (t, on) = out[0];
        assert!(on && t.translation == Vec3::new(-0.12, 0.2, 0.0));
        // l'anneau est dressé face à +x (vers l'avant)
        assert!((t.rotation * Vec3::Z).x.abs() > 0.99);
    }
}
