//! Clips procéduraux génériques (humanoïde). Les champions peuvent remplacer n'importe quel clip.
//! Voir les conventions d'axes dans `pose.rs` (avant = +x, membre +z = vers l'avant).
//!
//! Portage de `packages/client/src/render/champions/anim/clips.ts`.

use std::f32::consts::{PI, TAU};

use super::pose::j::*;
use super::pose::{
    Clip, ClipCtx, ClipSet, Pose, add_rot, clamp01, ease_in, ease_out, set_offset, set_rot,
};

/// Bras légèrement écartés, coudes un peu pliés : posture de base.
pub fn rest(p: &mut Pose, spread: f32) {
    set_rot(p, ARM_L, spread, 0.0, 0.05);
    set_rot(p, ARM_R, -spread, 0.0, 0.05);
    set_rot(p, FORE_L, 0.0, 0.0, 0.25);
    set_rot(p, FORE_R, 0.0, 0.0, 0.25);
}

/// Bras qui frappe et son avant-bras, selon la main du contexte.
fn hand_joints(ctx: &ClipCtx) -> (usize, usize) {
    if ctx.hand == 1.0 {
        (ARM_R, FORE_R)
    } else {
        (ARM_L, FORE_L)
    }
}

/// Repos : respiration, transfert de poids lent d'une jambe à l'autre, regard qui balaie.
fn idle(t: f32, p: &mut Pose, _: &ClipCtx) {
    rest(p, 0.12);
    let breath = (t / 2.6 * TAU).sin();
    let shift = (t / 5.2 * TAU).sin();
    add_rot(p, SPINE, 0.0, 0.0, -0.03 + breath * 0.025);
    add_rot(p, ARM_L, breath * 0.03, 0.0, 0.0);
    add_rot(p, ARM_R, -breath * 0.03, 0.0, 0.0);
    // le poids passe d'une jambe à l'autre : bassin qui roule, jambe libre légèrement fléchie
    set_rot(p, HIPS, shift * 0.05, 0.0, 0.0);
    set_rot(p, LEG_L, -shift * 0.05, 0.0, 0.05 + shift.max(0.0) * 0.08);
    set_rot(
        p,
        LEG_R,
        -shift * 0.05,
        0.0,
        0.05 + (-shift).max(0.0) * 0.08,
    );
    set_rot(p, SHIN_L, 0.0, 0.0, -0.04 - shift.max(0.0) * 0.16);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.04 - (-shift).max(0.0) * 0.16);
    add_rot(p, SPINE, -shift * 0.04, 0.0, 0.0);
    set_rot(p, HEAD, 0.0, (t / 5.2 * TAU * 0.5 + 1.0).sin() * 0.18, 0.02);
    set_offset(p, 0.0, breath * 0.012 - shift.abs() * 0.015, shift * 0.02);
}

/// Course : enjambée modérée, genou plié en phase de balancier seulement, bassin qui pivote et épaules en
/// contre-rotation, bras pliés en opposition, tête stabilisée, deux rebonds par cycle.
/// `energy` règle l'amplitude (tanks lourds < 1, assassins > 1).
fn run(t: f32, p: &mut Pose, ctx: &ClipCtx) {
    let ph = t / 0.66 * TAU;
    let s = ph.sin();
    let c = ph.cos();
    let e = ctx.energy;
    // jambes : la gauche est devant quand s > 0 ; elle avance (balancier) quand c > 0
    set_rot(p, LEG_L, 0.04, 0.0, 0.12 + s * 0.55 * e);
    set_rot(p, LEG_R, -0.04, 0.0, 0.12 - s * 0.55 * e);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.18 - c.max(0.0) * 0.95 * e);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.18 - (-c).max(0.0) * 0.95 * e);
    // bassin et épaules en contre-rotation
    set_rot(p, HIPS, (ph * 2.0).sin() * 0.03, s * 0.12 * e, 0.0);
    set_rot(p, SPINE, 0.0, -s * 0.2 * e, -0.16 * e);
    set_rot(p, HEAD, 0.0, s * 0.08 * e, 0.12 * e); // la tête compense : le regard reste devant
    // bras pliés, en opposition des jambes
    set_rot(p, ARM_L, 0.14, 0.0, -s * 0.42 * e);
    set_rot(p, ARM_R, -0.14, 0.0, s * 0.42 * e);
    set_rot(p, FORE_L, 0.0, 0.0, 0.95 + s.max(0.0) * 0.35);
    set_rot(p, FORE_R, 0.0, 0.0, 0.95 + (-s).max(0.0) * 0.35);
    // deux rebonds par cycle : point bas au contact du pied
    set_offset(
        p,
        0.0,
        (0.5 - 0.5 * (ph * 2.0).cos()) * 0.06 * e - 0.05,
        0.0,
    );
}

/// Armement d'une auto-attaque, tenu en attendant l'impact (`strike`).
fn attack_ready(t: f32, p: &mut Pose, ctx: &ClipCtx) {
    rest(p, 0.12);
    let k = ease_out(t / 0.25);
    let (arm, fore) = hand_joints(ctx);
    add_rot(p, arm, 0.0, 0.0, -0.5 * k);
    add_rot(p, fore, 0.0, 0.0, 0.9 * k);
    set_rot(p, SPINE, 0.0, ctx.hand * 0.35 * k, 0.05 * k);
    set_rot(p, LEG_L, 0.0, 0.0, 0.25);
    set_rot(p, LEG_R, 0.0, 0.0, -0.2);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.25);
}

/// Frappe (au moment de l'événement autoAttack) : le corps entier participe — pas en avant, torsion du
/// buste qui traverse, bras qui balaie, puis accompagnement et retour.
fn strike(t: f32, p: &mut Pose, ctx: &ClipCtx) {
    rest(p, 0.12);
    let hit = ease_out(t / 0.09);
    let back = ease_in((t - 0.16) / 0.24);
    let k = hit * (1.0 - back);
    let h = ctx.hand;
    let (arm, fore) = hand_joints(ctx);
    let other = if h == 1.0 { ARM_L } else { ARM_R };
    add_rot(p, arm, 0.0, 0.0, -0.5 + 2.1 * k);
    add_rot(p, fore, 0.0, 0.0, 0.9 - 0.85 * k);
    add_rot(p, other, 0.0, 0.0, -0.4 * k); // l'autre bras équilibre
    set_rot(p, SPINE, 0.0, h * (0.4 - 0.9 * k), -0.25 * k);
    set_rot(p, HIPS, 0.0, h * (0.12 - 0.25 * k), 0.0);
    set_rot(p, HEAD, 0.0, -h * 0.3 * k, 0.1 * k);
    // pas en avant pendant l'impact
    set_rot(p, LEG_L, 0.0, 0.0, 0.2 + 0.35 * k);
    set_rot(p, SHIN_L, 0.0, 0.0, -0.15 - 0.3 * k);
    set_rot(p, LEG_R, 0.0, 0.0, -0.15 - 0.25 * k);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.2);
    set_offset(p, 0.12 * k, -0.07 * k, 0.0);
}

/// Incantation générique : petit élan, poussée des deux mains vers l'avant, léger recul.
fn cast(t: f32, p: &mut Pose, _: &ClipCtx) {
    rest(p, 0.12);
    let wind = ease_out(t / 0.12) * (1.0 - clamp01((t - 0.12) / 0.08));
    let push = ease_out((t - 0.12) / 0.1) * (1.0 - ease_in((t - 0.32) / 0.23));
    add_rot(p, ARM_R, -0.2 * push, 0.0, -0.3 * wind + 1.55 * push);
    add_rot(p, ARM_L, 0.2 * push, 0.0, -0.3 * wind + 1.3 * push);
    add_rot(p, FORE_R, 0.0, 0.0, 0.5 * wind - 0.25 * push);
    add_rot(p, FORE_L, 0.0, 0.0, 0.5 * wind - 0.2 * push);
    set_rot(p, SPINE, 0.0, 0.0, 0.12 * wind - 0.18 * push);
    set_rot(p, HEAD, 0.0, 0.0, 0.08 * push);
    set_rot(p, LEG_L, 0.0, 0.0, 0.25 * push);
    set_rot(p, LEG_R, 0.0, 0.0, -0.15 * push);
    set_offset(p, -0.04 * wind + 0.06 * push, -0.03 * push, 0.0);
}

/// Ultime : bras au ciel puis abattus.
fn cast_r(t: f32, p: &mut Pose, _: &ClipCtx) {
    rest(p, 0.12);
    let up = ease_out(t / 0.35);
    let down = ease_in((t - 0.4) / 0.2);
    let k = up * (1.0 - down);
    let settle = 1.0 - ease_in((t - 0.6) / 0.2);
    add_rot(p, ARM_R, -0.3, 0.0, 2.8 * k + 1.0 * down * settle);
    add_rot(p, ARM_L, 0.3, 0.0, 2.8 * k + 1.0 * down * settle);
    set_rot(p, SPINE, 0.0, 0.0, 0.25 * k - 0.3 * down * settle);
    set_rot(p, HEAD, 0.0, 0.0, 0.25 * k);
    set_offset(p, 0.0, 0.06 * k - 0.08 * down, 0.0);
}

fn dash(_: f32, p: &mut Pose, _: &ClipCtx) {
    set_rot(p, BODY, 0.0, 0.0, -0.35);
    set_rot(p, ARM_L, 0.4, 0.0, -0.9);
    set_rot(p, ARM_R, -0.4, 0.0, -0.9);
    set_rot(p, LEG_L, 0.0, 0.0, 0.7);
    set_rot(p, SHIN_L, 0.0, 0.0, -1.2);
    set_rot(p, LEG_R, 0.0, 0.0, -0.5);
    set_rot(p, SHIN_R, 0.0, 0.0, -0.3);
    set_offset(p, 0.0, 0.05, 0.0);
}

/// Mort : les genoux cèdent, puis chute en arrière avec un petit rebond au sol.
fn death(t: f32, p: &mut Pose, _: &ClipCtx) {
    let buckle = ease_out(t / 0.22);
    let fall = clamp01((t - 0.15) / 0.5);
    let f = fall * fall * (3.0 - 2.0 * fall);
    let bounce = if t > 0.65 {
        let b = clamp01((t - 0.65) / 0.3);
        (b * PI).sin() * 0.08 * (1.0 - b)
    } else {
        0.0
    };
    set_rot(p, SHIN_L, 0.0, 0.0, -0.9 * buckle * (1.0 - f * 0.5));
    set_rot(p, SHIN_R, 0.0, 0.0, -0.7 * buckle * (1.0 - f * 0.5));
    set_rot(p, LEG_L, 0.0, 0.0, 0.5 * buckle);
    set_rot(p, LEG_R, 0.0, 0.0, 0.35 * buckle + 0.3 * f);
    set_rot(p, HEAD, 0.0, 0.3 * f, -0.3 * buckle + 0.5 * f);
    set_rot(p, BODY, 0.0, 0.0, 1.4 * f - bounce);
    set_rot(p, ARM_L, 0.3 + 0.8 * f, 0.0, 0.4 * buckle + 0.4 * f);
    set_rot(p, ARM_R, -0.3 - 0.8 * f, 0.0, 0.4 * buckle + 0.4 * f);
    set_rot(p, FORE_L, 0.0, 0.0, 0.5 * buckle);
    set_rot(p, FORE_R, 0.0, 0.0, 0.5 * buckle);
    // le pivot du corps est aux pieds : on ne l'abaisse que pendant la flexion des genoux, sinon le corps
    // couché (≈ 80°) finirait sous le sol
    set_offset(p, -0.25 * f, -0.22 * buckle * (1.0 - f) + 0.04 * f, 0.0);
}

/// Rappel : genou à terre, mains jointes (public : certains modèles le reprennent en le corrigeant).
pub fn recall(t: f32, p: &mut Pose, _: &ClipCtx) {
    let k = ease_out(t / 0.5);
    let b = (t / 1.5 * TAU).sin() * 0.03;
    set_rot(p, LEG_L, 0.0, 0.0, 1.2 * k);
    set_rot(p, SHIN_L, 0.0, 0.0, -1.5 * k);
    set_rot(p, LEG_R, 0.0, 0.0, -0.3 * k);
    set_rot(p, SHIN_R, 0.0, 0.0, -1.7 * k);
    set_rot(p, ARM_L, 0.5 * k, 0.0, 0.9 * k);
    set_rot(p, ARM_R, -0.5 * k, 0.0, 0.9 * k);
    set_rot(p, FORE_L, 0.0, 0.0, 1.1 * k);
    set_rot(p, FORE_R, 0.0, 0.0, 1.1 * k);
    set_rot(p, SPINE, 0.0, 0.0, -0.15 * k + b);
    set_rot(p, HEAD, 0.0, 0.0, -0.25 * k);
    set_offset(p, 0.0, -0.42 * k, 0.0);
}

/// Étourdi : vacille (public : certains modèles le reprennent en le corrigeant).
pub fn stunned(t: f32, p: &mut Pose, _: &ClipCtx) {
    rest(p, 0.3);
    let s = (t / 1.2 * TAU).sin();
    set_rot(p, BODY, s * 0.08, 0.0, 0.1);
    set_rot(p, HEAD, 0.0, s * 0.3, 0.25);
    set_offset(p, 0.0, -0.05, 0.0);
}

const fn looped(duration: f32, sample: super::pose::Sample) -> Clip {
    Clip {
        looped: true,
        duration,
        sample,
    }
}

const fn once(duration: f32, sample: super::pose::Sample) -> Clip {
    Clip {
        looped: false,
        duration,
        sample,
    }
}

/// `GENERIC_CLIPS`.
pub fn generic_clips() -> ClipSet {
    ClipSet::from([
        ("idle", looped(5.2, idle)),
        ("run", looped(0.66, run)),
        ("attackReady", looped(1.0, attack_ready)),
        ("strike", once(0.4, strike)),
        ("cast", once(0.55, cast)),
        ("cast_R", once(0.8, cast_r)),
        ("dash", looped(0.4, dash)),
        ("death", once(1.0, death)),
        ("recall", looped(3.0, recall)),
        ("stunned", looped(1.2, stunned)),
    ])
}

/// Portage de `packages/client/test/fx/anim.test.ts`.
#[cfg(test)]
mod tests {
    use super::super::pose::*;
    use super::*;

    #[test]
    fn blend_pose_interpole_chaque_composante() {
        let a = [0.0; POSE_SIZE];
        let b = [2.0; POSE_SIZE];
        let mut out = [0.0; POSE_SIZE];
        blend_pose(&a, &b, 0.25, &mut out);
        assert!(out.iter().all(|&v| v == 0.5));
        assert_eq!(out.len(), POSE_SIZE);
    }

    #[test]
    fn resolve_clip_suit_la_chaine_de_repli() {
        let clips = generic_clips();
        assert_eq!(resolve_clip(&clips, "cast_R"), "cast_R");
        assert_eq!(resolve_clip(&clips, "cast_Q"), "cast");
        assert_eq!(resolve_clip(&clips, "inconnu"), "idle");
    }

    #[test]
    fn tous_les_clips_generiques_produisent_des_valeurs_finies() {
        for (name, clip) in generic_clips() {
            let mut t = 0.0;
            while t <= clip.duration * 2.0 {
                let mut p = [0.0; POSE_SIZE];
                (clip.sample)(t, &mut p, &DEFAULT_CTX);
                assert!(p.iter().all(|v| v.is_finite()), "{name}");
                t += 0.05;
            }
        }
    }

    fn animator() -> Animator {
        Animator::new(generic_clips(), DEFAULT_CTX)
    }

    #[test]
    fn revient_au_clip_de_base_apres_un_one_shot() {
        let mut a = animator();
        a.set_base("run");
        a.update(0.5);
        assert_eq!(a.current(), "run");
        a.play("cast_Q", None, None);
        assert_eq!(a.current(), "cast");
        a.update(0.3);
        assert_eq!(a.current(), "cast");
        a.update(0.3); // > 0.5 s
        assert_eq!(a.current(), "run");
    }

    #[test]
    fn changer_la_base_pendant_un_one_shot_ne_l_interrompt_pas() {
        let mut a = animator();
        a.play("strike", None, None);
        a.set_base("attackReady");
        assert_eq!(a.current(), "strike");
        a.update(0.5);
        assert_eq!(a.current(), "attackReady");
    }

    #[test]
    fn la_mort_est_un_etat_final() {
        let mut a = animator();
        a.play("death", None, None);
        a.update(3.0);
        assert_eq!(a.current(), "death");
        assert!(a.pose[j::BODY * 3 + 2] > 1.2, "couché (≈ 80°)");
        assert!(
            a.pose[BODY_OFFSET + 1] >= 0.0,
            "pivot aux pieds : le corps couché ne s'enfonce pas"
        );
    }

    #[test]
    fn fondu_la_pose_part_de_la_precedente() {
        let mut a = animator();
        a.play("death", None, None);
        a.update(3.0);
        let fallen = a.pose[j::BODY * 3 + 2];
        a.set_base("idle");
        a.play("idle", Some(0.2), None);
        a.update(0.01);
        assert!(
            a.pose[j::BODY * 3 + 2] > fallen * 0.9,
            "encore presque couché"
        );
        a.update(0.5);
        assert!(a.pose[j::BODY * 3 + 2].abs() < 0.01);
    }

    #[test]
    fn rate_accelere_les_boucles() {
        let mut a = animator();
        let mut b = animator();
        a.set_base("run");
        b.set_base("run");
        a.update(1.0);
        b.update(1.0);
        b.rate = 2.0;
        a.update(0.1);
        b.update(0.05);
        for (x, y) in a.pose.iter().zip(b.pose.iter()) {
            assert!((x - y).abs() < 1e-5, "{x} ≠ {y}");
        }
    }

    #[test]
    fn easings() {
        assert_eq!(ease_out(-1.0), 0.0);
        assert_eq!(ease_out(2.0), 1.0);
        assert!((ease_in(0.5) - 0.25).abs() < 1e-6);
        assert!((bell(0.5, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(bell(0.0, 0.0, 1.0).abs() < 1e-6);
    }
}
