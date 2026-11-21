//! Modèle humanoïde générique aux couleurs du champion, arme selon la classe. Utilisé par les champions
//! qui n'ont pas encore leur modèle dédié.
//!
//! Portage de `packages/client/src/render/champions/models/generic.ts`.

use std::f32::consts::PI;

use lov_shared::schema::ChampionClass;

use super::anim::pose::j::*;
use super::builder::{ModelBuilder, PartOpts, css, geo};
use super::model::{ChampionModelDef, ModelColors};
use super::rig::RigSpec;

const SKIN: &str = "#e6c9a8";

fn at(x: f32, y: f32, z: f32) -> PartOpts {
    PartOpts::at(x, y, z)
}

fn weapon(b: &mut ModelBuilder, c: &ModelColors) {
    match c.class {
        ChampionClass::Tank => {
            // masse
            b.add(
                PROP,
                &geo::cyl(0.035, 0.035, 0.7, 6),
                c.secondary,
                at(0.3, 0.0, 0.0).rot(0.0, 0.0, PI / 2.0),
            );
            b.add(PROP, &geo::rock(0.16, 3.0, 0), c.accent, at(0.65, 0.0, 0.0));
        }
        ChampionClass::Fighter => {
            // épée
            b.add(
                PROP,
                &geo::cuboid(0.9, 0.06, 0.02),
                css("#d8dde6"),
                at(0.52, 0.0, 0.0),
            );
            b.add(
                PROP,
                &geo::cuboid(0.06, 0.04, 0.28),
                c.accent,
                at(0.06, 0.0, 0.0),
            );
        }
        ChampionClass::Mage => {
            // bâton + orbe
            b.add(
                PROP,
                &geo::cyl(0.025, 0.03, 1.5, 6),
                c.secondary,
                at(0.0, 0.25, 0.0),
            );
            b.glow(PROP, &geo::ball(0.09, 1), c.glow, at(0.0, 1.05, 0.0));
        }
        ChampionClass::Marksman => {
            // arme longue tenue vers l'avant
            b.add(
                PROP,
                &geo::cuboid(0.7, 0.08, 0.07),
                c.secondary,
                at(0.3, 0.02, 0.0),
            );
            b.add(
                PROP,
                &geo::cyl(0.03, 0.03, 0.4, 6),
                css("#8a8f99"),
                at(0.8, 0.04, 0.0).rot(0.0, 0.0, PI / 2.0),
            );
        }
        ChampionClass::Assassin => {
            // dague
            b.add(
                PROP,
                &geo::crystal(0.04, 0.5),
                css("#cfd4e0"),
                at(0.28, 0.0, 0.0).rot(0.0, 0.0, -PI / 2.0),
            );
        }
        ChampionClass::Support => {
            // bâton à halo
            b.add(
                PROP,
                &geo::cyl(0.025, 0.03, 1.4, 6),
                c.secondary,
                at(0.0, 0.2, 0.0),
            );
            b.glow(
                PROP,
                &geo::torus(0.12, 0.02, 14),
                c.glow,
                at(0.0, 0.98, 0.0),
            );
        }
    }
}

fn build(b: &mut ModelBuilder, c: &ModelColors, _rig: &RigSpec) {
    let skin = css(SKIN);
    let heavy = matches!(c.class, ChampionClass::Tank | ChampionClass::Fighter);
    // bassin + ceinture d'équipe
    b.add(
        HIPS,
        &geo::cuboid(0.26, 0.22, 0.4),
        c.secondary,
        at(0.0, 0.02, 0.0),
    );
    b.add(
        HIPS,
        &geo::cuboid(0.28, 0.06, 0.43),
        c.team,
        at(0.0, 0.1, 0.0),
    );
    // torse
    b.add(
        SPINE,
        &geo::cuboid(0.3, 0.42, 0.46),
        c.primary,
        at(0.0, 0.32, 0.0),
    );
    b.add(
        SPINE,
        &geo::cuboid(0.32, 0.12, 0.5),
        c.secondary,
        at(0.0, 0.1, 0.0),
    );
    b.add(
        SPINE,
        &geo::cuboid(0.06, 0.2, 0.2),
        c.accent,
        at(0.15, 0.38, 0.0),
    );
    // épaulières
    for z in [-0.28, 0.28] {
        let col = if heavy { c.accent } else { c.primary };
        b.add(
            SPINE,
            &geo::ball(0.12, 0),
            col,
            at(0.0, 0.56, z).scale(1.0, 0.7, 1.0),
        );
    }
    // tête
    b.add(HEAD, &geo::ball(0.16, 1), skin, at(0.02, 0.12, 0.0));
    // cheveux / capuche
    b.add(
        HEAD,
        &geo::ball(0.17, 1),
        c.secondary,
        at(-0.03, 0.17, 0.0).scale(1.0, 0.75, 1.02),
    );
    b.glow(
        HEAD,
        &geo::cuboid(0.03, 0.03, 0.04),
        c.glow,
        at(0.16, 0.13, -0.06),
    );
    b.glow(
        HEAD,
        &geo::cuboid(0.03, 0.03, 0.04),
        c.glow,
        at(0.16, 0.13, 0.06),
    );
    // bras
    for (arm, fore) in [(ARM_L, FORE_L), (ARM_R, FORE_R)] {
        b.add(
            arm,
            &geo::cuboid(0.13, 0.34, 0.13),
            c.primary,
            at(0.0, -0.16, 0.0),
        );
        b.add(
            fore,
            &geo::cuboid(0.12, 0.3, 0.12),
            c.secondary,
            at(0.0, -0.14, 0.0),
        );
        b.add(fore, &geo::ball(0.07, 0), skin, at(0.0, -0.31, 0.0));
    }
    // jambes
    for (leg, shin) in [(LEG_L, SHIN_L), (LEG_R, SHIN_R)] {
        b.add(
            leg,
            &geo::cuboid(0.16, 0.42, 0.16),
            c.secondary,
            at(0.0, -0.2, 0.0),
        );
        b.add(
            shin,
            &geo::cuboid(0.14, 0.38, 0.14),
            c.primary,
            at(0.0, -0.19, 0.0),
        );
        b.add(
            shin,
            &geo::cuboid(0.24, 0.08, 0.15),
            c.secondary,
            at(0.04, -0.38, 0.0),
        );
    }
    weapon(b, c);
}

pub const GENERIC_RIG: RigSpec = RigSpec {
    hip_y: 0.85,
    hip_w: 0.15,
    thigh: 0.42,
    shin: 0.42,
    spine: 0.58,
    shoulder_w: 0.28,
    upper_arm: 0.33,
    forearm: 0.31,
    neck: 0.15,
};

/// `genericFor` : proportions et port d'arme selon la classe (tanks plus massifs, assassins plus fins).
pub fn generic_for(class: ChampionClass) -> ChampionModelDef {
    use ChampionClass::*;
    let scale = match class {
        Tank => 1.12,
        Assassin => 0.96,
        _ => 1.0,
    };
    let energy = match class {
        Tank => 0.75,
        Assassin => 1.2,
        _ => 1.0,
    };
    // port d'arme selon la classe : arme basse vers l'avant, bâtons droits
    let carry = match class {
        Mage | Support => 0.0,
        Assassin => -0.7,
        _ => -0.3,
    };
    let drop_angle = match class {
        Mage | Support => -PI / 2.0,
        _ => 0.0,
    };
    ChampionModelDef {
        rig: GENERIC_RIG,
        hand: None,
        energy: Some(energy),
        clips: &[],
        scale,
        build,
        carry: Some(carry),
        extras: None,
        drop_angle: Some(drop_angle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::model::build_model;
    use bevy::prelude::LinearRgba;

    fn colors(class: ChampionClass) -> ModelColors {
        ModelColors {
            primary: LinearRgba::RED,
            secondary: LinearRgba::GREEN,
            accent: LinearRgba::BLUE,
            team: LinearRgba::WHITE,
            glow: LinearRgba::rgb(2.0, 2.0, 2.0),
            class,
        }
    }

    #[test]
    fn chaque_classe_a_son_arme() {
        for class in [
            ChampionClass::Tank,
            ChampionClass::Fighter,
            ChampionClass::Mage,
            ChampionClass::Marksman,
            ChampionClass::Assassin,
            ChampionClass::Support,
        ] {
            let def = generic_for(class);
            let m = build_model(&def, &colors(class));
            assert!(
                m.body[PROP].is_some() || m.glow[PROP].is_some(),
                "{class:?}"
            );
            for j in [HIPS, SPINE, HEAD, ARM_L, FORE_R, LEG_L, SHIN_R] {
                assert!(m.body[j].is_some(), "{class:?} : articulation {j} vide");
            }
            assert!(m.glow[HEAD].is_some(), "yeux lumineux");
        }
        assert_eq!(generic_for(ChampionClass::Tank).scale, 1.12);
        assert_eq!(generic_for(ChampionClass::Mage).drop_angle, Some(-PI / 2.0));
    }
}
