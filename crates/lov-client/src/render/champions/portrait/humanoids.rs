//! Portraits des champions humains : costume, visage, coiffure et arme de chacun.
//!
//! Portage de `packages/client/src/render/champions/portrait/humanoids.ts`.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use super::PortraitBuild;
use super::anatomy::{Face, Outfit, cloak, face, hair, hood, human, skirt, sun};
use super::geometry::{Facing, Paint, Point, Sculpt, loft, plate, shade};
use crate::render::champions::anim::pose::j;
use crate::render::champions::builder::{ModelBuilder, PartOpts};
use crate::render::champions::model::ModelColors;
use crate::render::champions::rig::RigSpec;
use crate::render::models::geometry as g3;
use crate::rings;

const GOLD: &str = "#c99a48";
const GOLD_LIGHT: &str = "#ead394";

fn white_glow() -> LinearRgba {
    shade("#fff1ba", 1.7)
}

/// Bras gauche ou droit selon le côté.
fn arm(side: f32) -> usize {
    if side < 0.0 { j::ARM_L } else { j::ARM_R }
}

fn fore(side: f32) -> usize {
    if side < 0.0 { j::FORE_L } else { j::FORE_R }
}

fn staff(s: &mut Sculpt, color: &str, trim: &str) {
    s.tube_with(
        j::PROP,
        color,
        &[[0.0, -0.58, 0.0], [0.0, 0.15, 0.0], [0.0, 0.98, 0.0]],
        &[0.026, 0.023, 0.02],
        true,
        8,
    );
    for y in [-0.5, -0.2, 0.35, 0.85] {
        s.ring_with(j::PROP, trim, 0.031, 0.008, [0.0, y, 0.0], Facing::Y, true);
    }
}

pub fn kaelen(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let glow = white_glow();
    let outfit = Outfit {
        cloth: "#263451",
        pants: "#263451",
        skin: "#a28b73",
        trim: GOLD,
        armor: true,
        width: Some(0.26),
        ..default()
    };
    human(s, c, rig, &outfit);
    // Plastron bombé, arrêtes dorées, tabard et soleil de poitrine.
    s.form_with(
        j::SPINE,
        "#bb8e3e",
        rings![
            [0.1, 0.16, 0.2],
            [0.24, 0.205, 0.25],
            [0.43, 0.17, 0.255],
            [0.53, 0.11, 0.14],
        ],
        true,
        PartOpts::default(),
    );
    s.panel(
        j::SPINE,
        "#23365b",
        &[
            [-0.12, 0.19],
            [0.12, 0.19],
            [0.1, -0.16],
            [0.0, -0.21],
            [-0.1, -0.16],
        ],
        [0.205, 0.28, 0.0],
    );
    let tabard = plate(
        &[
            [-0.12, 0.04],
            [0.12, 0.04],
            [0.15, -0.48],
            [0.0, -0.57],
            [-0.15, -0.48],
        ],
        0.018,
        0.008,
    );
    s.part(
        j::HIPS,
        &tabard,
        c.secondary,
        PartOpts::at(0.15, -0.03, 0.0),
        false,
    );
    sun(
        s,
        j::SPINE,
        GOLD_LIGHT,
        [0.226, 0.4, 0.0],
        0.067,
        glow,
        true,
    );
    for side in [-1.0, 1.0] {
        s.panel_with(
            arm(side),
            GOLD_LIGHT,
            &[
                [-0.17, 0.06],
                [-0.13, 0.17],
                [0.07, 0.18],
                [0.19, 0.02],
                [0.13, -0.12],
                [-0.12, -0.11],
            ],
            [0.03, 0.025, side * 0.025],
            true,
            0.025,
        );
        s.tube_with(
            j::SPINE,
            GOLD_LIGHT,
            &[
                [0.155, 0.51, side * 0.13],
                [0.2, 0.42, side * 0.2],
                [0.21, 0.26, side * 0.22],
            ],
            &[0.014, 0.014, 0.008],
            true,
            6,
        );
    }
    cloak(s, "#243659", "#cfaa62", 0.9, 0.37);
    // Heaume sculpté fermé, visière en V et plumet souple bleu nuit.
    s.ell_with(
        j::HEAD,
        GOLD,
        [0.0, 0.14, 0.0],
        [0.177, 0.218, 0.17],
        16,
        true,
    );
    s.panel_with(
        j::HEAD,
        "#182030",
        &[
            [-0.115, 0.08],
            [0.0, 0.038],
            [0.115, 0.08],
            [0.09, 0.035],
            [0.0, -0.025],
            [-0.09, 0.035],
        ],
        [0.17, 0.17, 0.0],
        false,
        0.008,
    );
    s.panel_with(
        j::HEAD,
        GOLD_LIGHT,
        &[
            [-0.018, 0.23],
            [0.02, 0.23],
            [0.026, -0.13],
            [0.0, -0.18],
            [-0.025, -0.13],
        ],
        [0.19, 0.12, 0.0],
        true,
        0.025,
    );
    for side in [-1.0, 1.0] {
        s.panel_with(
            j::HEAD,
            GOLD_LIGHT,
            &[
                [0.0, 0.13],
                [side * 0.15, 0.19],
                [side * 0.17, 0.12],
                [side * 0.095, 0.07],
            ],
            [0.155, 0.16, 0.0],
            true,
            0.025,
        );
    }
    sun(s, j::HEAD, GOLD_LIGHT, [0.16, 0.32, 0.0], 0.045, glow, true);
    for i in 0..5 {
        let k = i as f32 - 2.0;
        s.tube(
            j::HEAD,
            if i % 2 != 0 { "#263a68" } else { "#354f87" },
            &[
                [-0.04, 0.34, k * 0.022],
                [-0.09, 0.53 + i as f32 * 0.012, 0.0],
                [-0.3, 0.48, k * 0.035],
                [-0.41, 0.3, k * 0.04],
            ],
            &[0.035, 0.05, 0.04, 0.004],
        );
    }
    // Épée solaire : lame à quatre arêtes, garde ailée, gravure et pommeau.
    s.tube(
        j::PROP,
        "#453426",
        &[[-0.18, 0.0, 0.0], [-0.05, 0.0, 0.0], [0.1, 0.0, 0.0]],
        &[0.04, 0.035, 0.04],
    );
    s.panel_with(
        j::PROP,
        GOLD,
        &[
            [-0.27, -0.04],
            [-0.3, 0.04],
            [-0.13, 0.07],
            [0.0, 0.03],
            [0.13, 0.07],
            [0.3, 0.04],
            [0.27, -0.04],
            [0.0, -0.02],
        ],
        [0.12, 0.0, 0.0],
        true,
        0.045,
    );
    let blade = loft(
        rings![
            [0.0, 0.025, 0.09],
            [0.77, 0.018, 0.064],
            [1.07, 0.002, 0.002]
        ],
        4,
        true,
    );
    let o = PartOpts::at(0.15, 0.0, 0.0).rot(0.0, 0.0, -FRAC_PI_2);
    s.part(j::PROP, &blade, "#e7e6df", o, true);
    s.tube_with(
        j::PROP,
        GOLD_LIGHT,
        &[[0.2, 0.026, 0.0], [0.65, 0.019, 0.0], [1.1, 0.005, 0.0]],
        &[0.009, 0.008, 0.002],
        true,
        6,
    );
    s.gem(j::PROP, glow, [0.13, 0.01, 0.0], [0.045, 0.048, 0.048]);
}

pub fn vrenna(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#803638",
        pants: "#493029",
        skin: "#bd8d6b",
        trim: "#b58b54",
        bare_arms: true,
        ..default()
    };
    human(s, c, rig, &outfit);
    let traits = Face {
        skin: "#c89b79",
        hair: "#35241d",
        eye: Some("#735033"),
        feminine: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#35241d", "#5d3b2b", false, false);
    // Cuir articulé, coutures, rivets et foulard crème du portrait.
    for side in [-1.0, 1.0] {
        s.panel(
            j::SPINE,
            "#a14844",
            &[
                [0.0, 0.21],
                [side * 0.2, 0.16],
                [side * 0.16, -0.2],
                [side * 0.035, -0.25],
            ],
            [0.162, 0.3, 0.0],
        );
        for i in 0..4 {
            let i = i as f32;
            s.ell_with(
                j::SPINE,
                GOLD,
                [0.187, 0.2 + i * 0.065, side * (0.12 + i * 0.01)],
                [0.009, 0.011, 0.011],
                8,
                true,
            );
        }
        s.panel(
            arm(side),
            "#85383c",
            &[
                [-0.12, 0.05],
                [0.0, 0.14],
                [0.13, 0.05],
                [0.1, -0.1],
                [-0.1, -0.1],
            ],
            [0.0, 0.01, 0.0],
        );
    }
    s.form(
        j::SPINE,
        "#e0cda3",
        rings![[0.43, 0.16, 0.25], [0.53, 0.115, 0.16], [0.58, 0.09, 0.11]],
    );
    s.tube(
        j::SPINE,
        "#b9a77e",
        &[[0.13, 0.47, -0.2], [0.18, 0.44, 0.0], [0.13, 0.48, 0.2]],
        &[0.012, 0.012, 0.008],
    );
    s.panel(
        j::SPINE,
        "#dfcda5",
        &[
            [-0.12, 0.15],
            [0.07, 0.13],
            [0.11, -0.3],
            [0.0, -0.41],
            [-0.06, -0.25],
        ],
        [-0.22, 0.26, 0.14],
    );
    // Longue tresse avec ses deux brins, bagues et extrémité effilée.
    for i in 0..2 {
        let phase = i as f32 * PI;
        let points: Vec<Point> = (0..9)
            .map(|k| {
                let a = k as f32 * 1.7 + phase;
                [
                    0.02 + a.cos() * 0.026,
                    0.18 - k as f32 * 0.065,
                    0.16 + a.sin() * 0.026,
                ]
            })
            .collect();
        let color = if i != 0 { "#35241d" } else { "#59402c" };
        s.tube(j::HEAD, color, &points, &[0.04, 0.032, 0.017]);
    }
    for y in [0.03, -0.18, -0.32] {
        s.ring_with(
            j::HEAD,
            GOLD,
            0.042,
            0.007,
            [0.02, y, 0.16],
            Facing::Y,
            true,
        );
    }
    s.tube(
        j::PROP,
        "#b6a482",
        &[[0.0, -0.63, 0.0], [0.0, 0.4, 0.0], [0.0, 1.34, 0.0]],
        &[0.027, 0.022, 0.023],
    );
    for y in [-0.35, -0.15, 0.95, 1.2] {
        s.ring_with(j::PROP, GOLD, 0.033, 0.009, [0.0, y, 0.0], Facing::Y, true);
    }
    s.panel_with(
        j::PROP,
        "#deca99",
        &[
            [0.0, 0.26],
            [0.07, -0.01],
            [0.025, -0.13],
            [0.0, -0.08],
            [-0.025, -0.13],
            [-0.07, -0.01],
        ],
        [0.0, 1.43, 0.0],
        true,
        0.025,
    );
    s.panel_with(
        j::PROP,
        "#a84440",
        &[[0.0, 0.1], [0.18, 0.04], [0.12, -0.16], [0.0, -0.07]],
        [0.0, 1.14, 0.08],
        false,
        0.012,
    );
}

pub fn ilyra(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#234c78",
        pants: "#193553",
        skin: "#eadde1",
        trim: "#abd7ee",
        ..default()
    };
    human(s, c, rig, &outfit);
    skirt(s, "#173b62", "#a1cbdf", 0.4, 0.83);
    cloak(s, "#25517a", "#c5e8f3", 0.9, 0.42);
    let traits = Face {
        skin: "#e9dfe7",
        hair: "#9eafc6",
        eye: Some("#6195c8"),
        feminine: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#bccddd", "#e3eff3", true, false);
    for side in [-1.0, 1.0] {
        s.panel_with(
            j::SPINE,
            "#8ebbd8",
            &[
                [0.0, 0.24],
                [side * 0.2, 0.12],
                [side * 0.16, -0.14],
                [side * 0.04, -0.21],
            ],
            [0.18, 0.27, 0.0],
            true,
            0.025,
        );
        s.panel_with(
            arm(side),
            "#b9deea",
            &[
                [-0.13, 0.04],
                [0.0, 0.21],
                [0.13, 0.04],
                [0.09, -0.11],
                [-0.09, -0.11],
            ],
            [0.02, 0.0, 0.0],
            true,
            0.025,
        );
        s.tube_with(
            j::SPINE,
            "#c3e4f0",
            &[
                [0.13, 0.49, side * 0.09],
                [0.2, 0.38, side * 0.15],
                [0.16, 0.1, side * 0.1],
            ],
            &[0.012, 0.018, 0.009],
            true,
            6,
        );
    }
    s.gem(j::SPINE, c.glow, [0.207, 0.29, 0.0], [0.035, 0.1, 0.055]);
    s.ring_with(
        j::HEAD,
        "#98bfd4",
        0.165,
        0.014,
        [0.0, 0.255, 0.0],
        Facing::Y,
        true,
    );
    let ice = shade("#c6eaff", 1.1);
    for i in -3_i32..=3 {
        let (fi, a) = (i as f32, i.abs() as f32);
        s.gem(
            j::HEAD,
            ice,
            [0.015, 0.34 + (3.0 - a) * 0.025, fi * 0.052],
            [0.028, 0.12 - a * 0.014, 0.032],
        );
    }
    staff(s, "#334a6b", "#b5cbdc");
    for side in [-1.0, 1.0] {
        s.tube_with(
            j::PROP,
            "#bad7e3",
            &[
                [0.0, 0.84, side * 0.02],
                [0.0, 1.02, side * 0.14],
                [0.0, 1.18, side * 0.1],
            ],
            &[0.028, 0.022, 0.004],
            true,
            6,
        );
    }
    s.gem(j::PROP, c.glow, [0.0, 1.12, 0.0], [0.095, 0.22, 0.09]);
}

pub fn zephyr(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#5d3486",
        pants: "#241c3a",
        skin: "#c69ba6",
        trim: "#70628c",
        ..default()
    };
    human(s, c, rig, &outfit);
    skirt(s, "#2e2248", "#8364a5", 0.33, 0.76);
    cloak(s, "#4d2d73", "#8f62ac", 1.02, 0.43);
    let traits = Face {
        skin: "#cba7a8",
        hair: "#302432",
        eye: Some("#4abecb"),
        beard: true,
        ..default()
    };
    face(s, &traits);
    hood(s, "#5a3384", "#8d5ca7", true);
    // Frange asymétrique du portrait ; les mèches balayent le front vers la tempe.
    for i in 0..4 {
        let fi = i as f32;
        s.tube(
            j::HEAD,
            if i % 2 != 0 { "#392b46" } else { "#241c31" },
            &[
                [0.015, 0.29, -0.09 + fi * 0.035],
                [0.12, 0.29 - fi * 0.009, -0.04 + fi * 0.035],
                [0.15, 0.22 - fi * 0.012, 0.05 + fi * 0.028],
                [0.08, 0.11, 0.14],
            ],
            &[0.033, 0.04, 0.026, 0.002],
        );
    }
    for side in [-1.0, 1.0] {
        let fore = fore(side);
        s.form_with(
            fore,
            "#65728b",
            rings![[-0.29, 0.08, 0.09], [-0.2, 0.13, 0.13], [-0.03, 0.11, 0.11]],
            true,
            PartOpts::default(),
        );
        s.panel_with(
            fore,
            "#807399",
            &[
                [-0.09, 0.1],
                [0.0, 0.15],
                [0.09, 0.1],
                [0.07, -0.1],
                [-0.07, -0.1],
            ],
            [0.11, -0.13, 0.0],
            true,
            0.025,
        );
        s.gem(fore, c.glow, [0.135, -0.12, 0.0], [0.028, 0.066, 0.05]);
        s.tube_with(
            j::SPINE,
            "#9ba0b4",
            &[
                [0.13, 0.51, side * 0.15],
                [0.2, 0.4, side * 0.08],
                [0.17, 0.1, side * 0.12],
            ],
            &[0.02, 0.019, 0.008],
            true,
            6,
        );
    }
    s.gem(j::SPINE, c.glow, [0.19, 0.36, 0.0], [0.035, 0.095, 0.06]);
}

pub fn sable(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#71432d",
        pants: "#3c2c29",
        skin: "#ba8967",
        trim: "#b28c50",
        ..default()
    };
    human(s, c, rig, &outfit);
    cloak(s, "#77472e", "#b68d57", 0.98, 0.34);
    let traits = Face {
        skin: "#c08b68",
        hair: "#3b251e",
        eye: Some("#a46b2e"),
        feminine: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#3b251e", "#6a432a", true, false);
    // Foulard enveloppant, chapeau creusé, ceinture et boucle de laiton.
    s.form(
        j::HEAD,
        "#ba975e",
        rings![
            [-0.045, 0.14, 0.14, 0.06],
            [0.035, 0.173, 0.165, 0.05],
            [0.075, 0.142, 0.16, 0.04],
        ],
    );
    s.tube(
        j::HEAD,
        "#907345",
        &[
            [0.196, 0.045, -0.11],
            [0.217, 0.013, 0.0],
            [0.196, 0.045, 0.11],
        ],
        &[0.007, 0.01, 0.007],
    );
    s.form(
        j::HEAD,
        "#4a3025",
        rings![
            [0.26, 0.37, 0.35, -0.025],
            [0.28, 0.38, 0.36, -0.025],
            [0.3, 0.17, 0.17, -0.02],
            [0.45, 0.15, 0.15, -0.035],
            [0.48, 0.115, 0.125, -0.04],
        ],
    );
    s.form(
        j::HEAD,
        "#8e6538",
        rings![[0.3, 0.172, 0.173, -0.02], [0.34, 0.17, 0.17, -0.025]],
    );
    s.panel_with(
        j::HEAD,
        GOLD,
        &[
            [-0.042, 0.03],
            [0.042, 0.03],
            [0.042, -0.03],
            [-0.042, -0.03],
        ],
        [0.153, 0.32, 0.0],
        true,
        0.025,
    );
    s.tube(
        j::SPINE,
        "#473328",
        &[[0.16, 0.52, -0.18], [0.195, 0.27, 0.0], [0.14, 0.05, 0.17]],
        &[0.035, 0.037, 0.035],
    );
    for i in 0..6 {
        let i = i as f32;
        s.ell_with(
            j::SPINE,
            "#cba66a",
            [0.21, 0.42 - i * 0.055, -0.11 + i * 0.045],
            [0.02, 0.035, 0.013],
            8,
            true,
        );
    }
    s.panel_with(
        j::PROP,
        "#533726",
        &[
            [-0.055, 0.2],
            [0.06, 0.2],
            [0.07, -0.2],
            [0.025, -0.3],
            [-0.065, -0.25],
        ],
        [0.0, -0.03, 0.0],
        false,
        0.065,
    );
    let barrel = g3::cylinder(0.032, 0.037, 0.86, 12);
    let o = PartOpts::at(0.0, 0.63, 0.0).smooth();
    s.part(j::PROP, &barrel, "#b7834e", o, true);
    s.form_with(
        j::PROP,
        "#6b5140",
        rings![[0.18, 0.055, 0.06], [0.36, 0.05, 0.055]],
        true,
        PartOpts::default(),
    );
    s.ell(j::PROP, "#18232d", [0.0, 1.064, 0.0], [0.027, 0.003, 0.027]);
    for y in [0.25, 0.8, 1.02] {
        s.ring_with(
            j::PROP,
            "#d3ac70",
            0.039,
            0.009,
            [0.0, y, 0.0],
            Facing::Y,
            true,
        );
    }
    s.ring_with(
        j::PROP,
        "#c3a16d",
        0.07,
        0.009,
        [0.0, 0.15, 0.04],
        Facing::X,
        true,
    );
}

pub fn corvin(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#303343",
        pants: "#202634",
        skin: "#333540",
        trim: "#7d8a94",
        ..default()
    };
    human(s, c, rig, &outfit);
    cloak(s, "#202735", "#3c4657", 1.0, 0.38);
    hood(s, "#252b39", "#525c6c", false);
    s.ell_with(
        j::HEAD,
        "#292e3b",
        [0.025, 0.13, 0.0],
        [0.15, 0.2, 0.15],
        12,
        false,
    );
    s.panel_with(
        j::HEAD,
        "#a2abb2",
        &[
            [0.0, 0.21],
            [0.14, 0.12],
            [0.09, -0.05],
            [0.0, -0.21],
            [-0.09, -0.05],
            [-0.14, 0.12],
        ],
        [0.145, 0.14, 0.0],
        true,
        0.045,
    );
    s.tube_with(
        j::HEAD,
        "#7b8793",
        &[[0.16, 0.26, 0.0], [0.27, 0.08, 0.0], [0.32, -0.04, 0.0]],
        &[0.035, 0.032, 0.003],
        true,
        6,
    );
    for side in [-1.0, 1.0] {
        s.panel(
            j::HEAD,
            "#111c24",
            &[
                [side * 0.025, 0.024],
                [side * 0.115, 0.06],
                [side * 0.1, 0.005],
                [side * 0.035, -0.01],
            ],
            [0.18, 0.2, 0.0],
        );
        s.gem(
            j::HEAD,
            c.glow,
            [0.187, 0.218, side * 0.069],
            [0.008, 0.013, 0.037],
        );
    }
    for side in [-1.0, 1.0] {
        for i in 0..8 {
            let (col, row) = ((i % 3) as f32, (i / 3) as f32);
            let pos = [
                -0.08 - col * 0.035,
                0.49 - row * 0.095,
                side * (0.2 + col * 0.08),
            ];
            s.panel(
                j::SPINE,
                if i % 2 != 0 { "#363e4d" } else { "#202734" },
                &[
                    [-0.035, 0.07],
                    [0.05, 0.055],
                    [0.045, -0.08],
                    [0.0, -0.2],
                    [-0.04, -0.07],
                ],
                pos,
            );
        }
    }
    // Véritable arc recourbé ouvert, poignée et corde (plus de cercle fermé).
    s.tube_with(
        j::FORE_L,
        "#594b44",
        &[
            [0.02, -0.92, 0.0],
            [-0.16, -0.73, 0.0],
            [-0.28, -0.33, 0.0],
            [-0.16, 0.07, 0.0],
            [0.02, 0.27, 0.0],
        ],
        &[0.018, 0.028, 0.038, 0.028, 0.018],
        true,
        6,
    );
    s.tube_with(
        j::FORE_L,
        "#9ca6aa",
        &[[0.02, -0.92, 0.0], [0.075, -0.83, 0.0], [0.04, -0.76, 0.0]],
        &[0.012, 0.02, 0.012],
        true,
        6,
    );
    s.tube_with(
        j::FORE_L,
        "#9ca6aa",
        &[[0.02, 0.27, 0.0], [0.075, 0.18, 0.0], [0.04, 0.11, 0.0]],
        &[0.012, 0.02, 0.012],
        true,
        6,
    );
    let string = g3::cylinder(0.004, 0.004, 1.18, 4);
    s.b.glow(j::FORE_L, &string, c.glow, PartOpts::at(0.025, -0.325, 0.0));
}

pub fn nyx(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#26233b",
        pants: "#171e2b",
        skin: "#28263a",
        trim: "#594169",
        ..default()
    };
    human(s, c, rig, &outfit);
    cloak(s, "#372547", "#79527d", 0.85, 0.34);
    hood(s, "#392b54", "#805887", true);
    s.ell_with(
        j::HEAD,
        "#0d1421",
        [0.05, 0.12, 0.0],
        [0.13, 0.178, 0.135],
        14,
        false,
    );
    for side in [-1.0, 1.0] {
        s.gem(
            j::HEAD,
            c.glow,
            [0.18, 0.16, side * 0.06],
            [0.008, 0.014, 0.041],
        );
    }
    let edge = plate(
        &[
            [0.085, -0.22],
            [0.08, -0.39],
            [-0.07, -0.57],
            [-0.03, -0.44],
            [0.048, -0.35],
        ],
        0.006,
        0.001,
    );
    for side in [-1.0, 1.0] {
        let fore = fore(side);
        s.panel(
            j::SPINE,
            "#5e426b",
            &[
                [0.0, 0.18],
                [side * 0.18, 0.09],
                [side * 0.11, -0.16],
                [0.0, -0.2],
            ],
            [0.15, 0.3, 0.0],
        );
        s.panel_with(
            fore,
            "#6b6a80",
            &[
                [-0.035, -0.05],
                [0.045, -0.04],
                [0.1, -0.22],
                [0.08, -0.39],
                [-0.07, -0.57],
                [-0.025, -0.34],
            ],
            [0.075, -0.16, 0.0],
            true,
            0.025,
        );
        s.b.glow(fore, &edge, c.glow, PartOpts::at(0.1, -0.16, 0.0));
        s.ring_with(
            fore,
            "#796281",
            0.065,
            0.012,
            [0.0, -0.28, 0.0],
            Facing::Y,
            true,
        );
    }
}

pub fn solenne(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let glow = white_glow();
    let outfit = Outfit {
        cloth: "#f0e1bc",
        pants: "#c4ab78",
        skin: "#e8c5ab",
        trim: "#c5a15c",
        ..default()
    };
    human(s, c, rig, &outfit);
    skirt(s, "#e8ddbf", "#ba9654", 0.42, 0.83);
    cloak(s, "#f0e5c8", "#c5a464", 0.95, 0.36);
    let traits = Face {
        skin: "#e7c3aa",
        hair: "#a97e3d",
        eye: Some("#8b6b35"),
        feminine: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#c2a263", "#e4ce97", true, false);
    for side in [-1.0, 1.0] {
        s.panel_with(
            j::SPINE,
            "#d3b16d",
            &[
                [0.0, 0.23],
                [side * 0.2, 0.12],
                [side * 0.13, -0.13],
                [side * 0.04, -0.22],
            ],
            [0.17, 0.3, 0.0],
            true,
            0.025,
        );
        s.tube_with(
            j::HEAD,
            GOLD,
            &[
                [0.1, 0.24, side * 0.04],
                [0.04, 0.27, side * 0.13],
                [-0.02, 0.2, side * 0.16],
            ],
            &[0.008, 0.01, 0.008],
            true,
            6,
        );
        s.panel_with(
            arm(side),
            "#e2c78f",
            &[[-0.1, 0.04], [0.0, 0.14], [0.1, 0.04], [0.0, -0.04]],
            [0.02, 0.0, 0.0],
            true,
            0.025,
        );
    }
    sun(
        s,
        j::SPINE,
        "#d3ad62",
        [0.204, 0.34, 0.0],
        0.067,
        glow,
        true,
    );
    s.gem(
        j::HEAD,
        "#74d7dc".linear(),
        [0.16, 0.25, 0.0],
        [0.012, 0.034, 0.02],
    );
    s.ring_with(
        j::HEAD,
        "#d7b66e",
        0.29,
        0.012,
        [-0.12, 0.23, 0.0],
        Facing::X,
        true,
    );
    for i in 0..12 {
        let (sin, cos) = (i as f32 / 12.0 * PI * 2.0).sin_cos();
        s.tube_with(
            j::HEAD,
            "#dfc684",
            &[
                [-0.12, 0.23 + cos * 0.3, sin * 0.3],
                [-0.12, 0.23 + cos * 0.37, sin * 0.37],
            ],
            &[0.014, 0.003],
            true,
            5,
        );
    }
    staff(s, "#ad8548", "#dac591");
    sun(s, j::PROP, "#ddbd76", [0.0, 1.03, 0.0], 0.13, glow, true);
}

pub fn master67(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let outfit = Outfit {
        cloth: "#7653d2",
        pants: "#1e2b43",
        skin: "#ad7655",
        trim: "#3ed9d4",
        open_hands: true,
        width: Some(0.27),
        ..default()
    };
    human(s, c, rig, &outfit);
    let traits = Face {
        skin: "#b27d59",
        hair: "#282034",
        eye: Some("#4c3028"),
        beard: true,
        smile: true,
        ..default()
    };
    face(s, &traits);
    hair(s, "#282034", "#44304a", false, true);
    hood(s, "#7250c8", "#3ed9d4", false);
    // Coutures, plis de hoodie, poche, cordons, chaîne et bijoux : tous dans le lot principal.
    for side in [-1.0, 1.0] {
        s.tube(
            j::SPINE,
            "#ad8ae7",
            &[
                [0.115, 0.48, side * 0.14],
                [0.17, 0.25, side * 0.19],
                [0.14, 0.08, side * 0.2],
            ],
            &[0.009, 0.014, 0.009],
        );
        s.tube(
            j::SPINE,
            "#3ed9d4",
            &[
                [0.115, 0.51, side * 0.1],
                [0.193, 0.38, side * 0.09],
                [0.19, 0.23, side * 0.085],
            ],
            &[0.012, 0.014, 0.009],
        );
        s.tube(
            j::SPINE,
            "#c3a153",
            &[
                [0.105, 0.5, side * 0.13],
                [0.18, 0.36, side * 0.09],
                [0.183, 0.32, 0.0],
            ],
            &[0.007, 0.009, 0.007],
        );
        s.panel_with(
            j::SPINE,
            "#6645ac",
            &[
                [0.0, 0.06],
                [side * 0.14, 0.07],
                [side * 0.12, -0.04],
                [0.0, -0.05],
            ],
            [0.17, 0.13, 0.0],
            false,
            0.012,
        );
    }
    s.ring(j::HEAD, "#e1bf6f", 0.027, 0.007, [0.0, 0.07, 0.17]);
    s.gem(
        j::SPINE,
        "#80eff0".linear(),
        [0.19, 0.29, 0.0],
        [0.013, 0.027, 0.025],
    );
    let skin = "#b98562";
    for fore in [j::FORE_L, j::FORE_R] {
        s.ell_with(
            fore,
            skin,
            [0.02, -0.355, 0.0],
            [0.037, 0.095, 0.114],
            12,
            false,
        );
        for i in 0..4 {
            let k = i as f32 - 1.5;
            let (z, len) = (k * 0.057, 0.11 - k.abs() * 0.012);
            s.tube(
                fore,
                skin,
                &[[0.015, -0.4, z], [0.035, -0.46, z], [0.075, -0.44 - len, z]],
                &[0.024, 0.022, 0.014],
            );
        }
        let side = if fore == j::FORE_L { -1.0 } else { 1.0 };
        s.tube(
            fore,
            skin,
            &[
                [0.02, -0.34, side * 0.08],
                [0.045, -0.36, side * 0.15],
                [0.09, -0.42, side * 0.17],
            ],
            &[0.03, 0.024, 0.016],
        );
    }
    // 67 brodé sur le torse, dans le plan de face, lisible dans le bon sens.
    let shapes: [&[[f32; 2]]; 2] = [
        &[
            [0.09, 0.34],
            [0.065, 0.37],
            [0.02, 0.35],
            [0.015, 0.28],
            [0.035, 0.255],
            [0.075, 0.26],
            [0.09, 0.29],
            [0.065, 0.315],
            [0.02, 0.3],
        ],
        &[[-0.025, 0.36], [-0.095, 0.36], [-0.04, 0.255]],
    ];
    for (i, points) in shapes.iter().enumerate() {
        let points: Vec<Point> = points.iter().map(|[z, y]| [0.185, *y, *z]).collect();
        let color = if i != 0 { "#dcb9ef" } else { "#56e5df" };
        s.tube(j::SPINE, color, &points, &[0.008, 0.008, 0.008]);
    }
    for shin in [j::SHIN_L, j::SHIN_R] {
        s.ell(
            shin,
            "#eff4ed",
            [0.07, -rig.shin + 0.05, 0.0],
            [0.18, 0.085, 0.11],
        );
        s.form(
            shin,
            "#3ed9d4",
            rings![
                [-rig.shin + 0.005, 0.165, 0.112, 0.06],
                [-rig.shin + 0.022, 0.17, 0.112, 0.06],
            ],
        );
    }
}

/// Portraits humains, par identifiant de champion.
pub const PORTRAITS: &[(&str, PortraitBuild)] = &[
    ("kaelen", kaelen),
    ("vrenna", vrenna),
    ("ilyra", ilyra),
    ("zephyr", zephyr),
    ("sable", sable),
    ("corvin", corvin),
    ("nyx", nyx),
    ("solenne", solenne),
    ("master67", master67),
];
