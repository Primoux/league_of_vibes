//! Portraits des champions non humains : Grondak, colosse de roche, et Tharox, guerrier sanglier.
//!
//! Portage de `packages/client/src/render/champions/portrait/creatures.ts`.

use super::PortraitBuild;
use super::anatomy::{SIDES, cloak};
use super::geometry::Sculpt;
use super::three::{CatmullRom, tube};
use crate::render::champions::anim::pose::j;
use crate::render::champions::builder::{ModelBuilder, PartOpts, geo};
use crate::render::champions::model::ModelColors;
use crate::render::champions::rig::RigSpec;
use crate::rings;

/// Pièce de roche à facettes (`geo.rock(r, seed, detail)`), placée et étirée.
fn rock(
    s: &mut Sculpt,
    joint: usize,
    (r, seed, detail): (f32, f32, u32),
    color: &str,
    pos: [f32; 3],
    scale: [f32; 3],
) {
    let o = PartOpts::at(pos[0], pos[1], pos[2]).scale(scale[0], scale[1], scale[2]);
    s.part(joint, &geo::rock(r, seed, detail), color, o, false);
}

pub fn grondak(b: &mut ModelBuilder, c: &ModelColors, _rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let (stone, dark, edge) = ("#827361", "#484842", "#a28e72");
    s.form(
        j::HIPS,
        dark,
        rings![[-0.19, 0.22, 0.31], [0.04, 0.3, 0.37], [0.2, 0.25, 0.32]],
    );
    rock(
        s,
        j::SPINE,
        (0.55, 2.0, 1),
        stone,
        [0.0, 0.46, 0.0],
        [0.88, 1.0, 1.05],
    );
    for side in [-1.0, 1.0] {
        rock(
            s,
            j::SPINE,
            (0.33, 8.0 + side, 0),
            edge,
            [-0.02, 0.79, side * 0.48],
            [1.15, 0.82, 1.07],
        );
        s.panel_with(
            j::SPINE,
            "#716552",
            &[
                [0.0, 0.14],
                [side * 0.26, 0.21],
                [side * 0.38, -0.08],
                [side * 0.08, -0.21],
            ],
            [0.38, 0.6, 0.0],
            false,
            0.085,
        );
        s.tube(
            j::SPINE,
            "#3d3c33",
            &[
                [0.37, 0.67, side * 0.17],
                [0.43, 0.49, side * 0.18],
                [0.4, 0.29, side * 0.28],
            ],
            &[0.024, 0.025, 0.015],
        );
        let vein = CatmullRom::new(&[
            [0.38, 0.67, side * 0.17],
            [0.445, 0.49, side * 0.18],
            [0.41, 0.29, side * 0.28],
        ]);
        s.b.glow(
            j::SPINE,
            &tube(&vein, 8, 0.009, 4),
            c.glow,
            PartOpts::default(),
        );
        for i in 0..3 {
            let fi = i as f32;
            rock(
                s,
                j::SPINE,
                (0.13, fi + 20.0 + side, 0),
                stone,
                [
                    -0.1 - fi * 0.09,
                    0.9 + fi * 0.025,
                    side * (0.27 - fi * 0.06),
                ],
                [1.0, 1.0, 1.0],
            );
        }
    }
    s.panel_with(
        j::SPINE,
        dark,
        &[
            [-0.14, 0.16],
            [0.14, 0.15],
            [0.08, -0.18],
            [0.0, -0.25],
            [-0.1, -0.12],
        ],
        [0.43, 0.38, 0.0],
        false,
        0.08,
    );
    s.gem(j::SPINE, c.glow, [0.485, 0.49, 0.0], [0.024, 0.12, 0.08]);
    // Visage anguleux, grand front, sourcils de roche et mâchoire taillée du portrait.
    rock(
        s,
        j::HEAD,
        (0.29, 7.0, 1),
        stone,
        [0.1, 0.09, 0.0],
        [1.0, 1.1, 1.05],
    );
    s.panel_with(
        j::HEAD,
        edge,
        &[
            [-0.2, 0.2],
            [-0.12, 0.32],
            [0.0, 0.27],
            [0.14, 0.32],
            [0.24, 0.17],
            [0.12, 0.1],
            [-0.12, 0.1],
        ],
        [0.28, 0.14, 0.0],
        false,
        0.11,
    );
    for side in [-1.0, 1.0] {
        s.panel_with(
            j::HEAD,
            dark,
            &[
                [side * 0.035, 0.045],
                [side * 0.2, 0.09],
                [side * 0.19, 0.015],
                [side * 0.06, -0.015],
            ],
            [0.345, 0.15, 0.0],
            false,
            0.012,
        );
        s.gem(
            j::HEAD,
            c.glow,
            [0.36, 0.18, side * 0.115],
            [0.012, 0.026, 0.066],
        );
        s.panel_with(
            j::HEAD,
            "#766752",
            &[
                [side * 0.05, 0.08],
                [side * 0.23, 0.08],
                [side * 0.21, -0.08],
                [side * 0.13, -0.14],
            ],
            [0.31, 0.03, 0.0],
            false,
            0.065,
        );
    }
    s.panel_with(
        j::HEAD,
        edge,
        &[
            [-0.035, 0.12],
            [0.035, 0.12],
            [0.06, -0.055],
            [0.0, -0.09],
            [-0.055, -0.055],
        ],
        [0.4, 0.105, 0.0],
        false,
        0.06,
    );
    s.panel_with(
        j::HEAD,
        dark,
        &[
            [-0.13, 0.025],
            [0.13, 0.025],
            [0.12, -0.055],
            [-0.12, -0.055],
        ],
        [0.32, -0.085, 0.0],
        false,
        0.012,
    );
    s.panel_with(
        j::HEAD,
        edge,
        &[
            [-0.18, 0.04],
            [0.18, 0.04],
            [0.14, -0.11],
            [0.0, -0.14],
            [-0.16, -0.08],
        ],
        [0.3, -0.14, 0.0],
        false,
        0.08,
    );
    for (arm, fore, leg, shin, side) in SIDES {
        rock(
            s,
            arm,
            (0.23, 30.0 + side, 1),
            stone,
            [0.0, -0.23, 0.0],
            [1.0, 1.25, 1.0],
        );
        rock(
            s,
            fore,
            (0.23, 40.0 + side, 0),
            dark,
            [0.0, -0.21, 0.0],
            [1.1, 1.3, 1.0],
        );
        rock(
            s,
            fore,
            (0.31, 50.0 + side, 1),
            stone,
            [0.06, -0.5, 0.0],
            [1.2, 0.9, 1.08],
        );
        for i in 0..4 {
            let fi = i as f32;
            rock(
                s,
                fore,
                (0.09, 60.0 + fi, 0),
                edge,
                [0.28, -0.49, (fi - 1.5) * 0.13],
                [0.75, 1.25, 1.0],
            );
        }
        s.gem(fore, c.glow, [0.32, -0.47, 0.0], [0.015, 0.027, 0.17]);
        rock(
            s,
            leg,
            (0.23, 70.0 + side, 0),
            stone,
            [0.0, -0.18, 0.0],
            [1.1, 1.15, 1.1],
        );
        rock(
            s,
            shin,
            (0.22, 80.0 + side, 0),
            dark,
            [0.0, -0.19, 0.0],
            [1.0, 1.15, 1.0],
        );
        rock(
            s,
            shin,
            (0.24, 90.0 + side, 0),
            stone,
            [0.1, -0.34, 0.0],
            [1.3, 0.5, 1.05],
        );
    }
}

pub fn tharox(b: &mut ModelBuilder, c: &ModelColors, rig: &RigSpec) {
    let s = &mut Sculpt::new(b);
    let (fur, tan, ivory, armor) = ("#593a2c", "#ae8458", "#dfcfaa", "#655e5a");
    s.form(
        j::HIPS,
        "#493026",
        rings![[-0.16, 0.22, 0.3], [0.1, 0.25, 0.34], [0.18, 0.2, 0.27]],
    );
    s.form(
        j::SPINE,
        "#6f4430",
        rings![
            [-0.02, 0.21, 0.29],
            [0.23, 0.31, 0.41],
            [0.55, 0.29, 0.46],
            [0.73, 0.19, 0.3],
        ],
    );
    cloak(s, "#74513b", "#916c48", 0.94, 0.48);
    for side in [-1.0, 1.0] {
        s.ell(j::SPINE, tan, [-0.08, 0.6, side * 0.36], [0.25, 0.22, 0.26]);
        s.ell_with(
            j::SPINE,
            armor,
            [0.035, 0.52, side * 0.41],
            [0.23, 0.2, 0.27],
            12,
            true,
        );
        s.tube(
            j::SPINE,
            ivory,
            &[
                [0.0, 0.66, side * 0.43],
                [0.04, 0.88, side * 0.46],
                [0.13, 0.92, side * 0.42],
            ],
            &[0.07, 0.042, 0.002],
        );
        for i in 0..8 {
            let z = side * (0.16 + i as f32 * 0.04);
            let y = 0.6 - (i % 3) as f32 * 0.06;
            s.tube_with(
                j::SPINE,
                if i % 2 != 0 { tan } else { "#77573f" },
                &[
                    [0.1, y + 0.06, z],
                    [0.16, y - 0.04, z + side * 0.06],
                    [0.18, y - 0.17, z + side * 0.035],
                ],
                &[0.055, 0.04, 0.003],
                false,
                5,
            );
        }
    }
    s.panel_with(
        j::SPINE,
        armor,
        &[
            [-0.13, 0.17],
            [0.13, 0.17],
            [0.17, -0.14],
            [0.0, -0.25],
            [-0.17, -0.14],
        ],
        [0.3, 0.27, 0.0],
        true,
        0.06,
    );
    s.gem(j::SPINE, c.glow, [0.345, 0.27, 0.0], [0.026, 0.09, 0.07]);
    // Tête de sanglier : groin modelé, narines, défenses courbes et crinière rouge.
    s.ell_with(
        j::HEAD,
        fur,
        [0.06, 0.12, 0.0],
        [0.235, 0.25, 0.235],
        16,
        false,
    );
    s.ell_with(
        j::HEAD,
        "#976550",
        [0.25, 0.055, 0.0],
        [0.15, 0.12, 0.17],
        14,
        false,
    );
    s.ell_with(
        j::HEAD,
        "#b17b65",
        [0.355, 0.065, 0.0],
        [0.03, 0.09, 0.123],
        12,
        false,
    );
    for side in [-1.0, 1.0] {
        s.ell_with(
            j::HEAD,
            "#35272b",
            [0.382, 0.075, side * 0.054],
            [0.007, 0.028, 0.022],
            8,
            false,
        );
        s.ell_with(
            j::HEAD,
            "#261f22",
            [0.245, 0.2, side * 0.125],
            [0.02, 0.032, 0.06],
            10,
            false,
        );
        s.gem(
            j::HEAD,
            c.glow,
            [0.26, 0.203, side * 0.122],
            [0.008, 0.015, 0.033],
        );
        s.tube(
            j::HEAD,
            "#3c2926",
            &[
                [0.16, 0.25, side * 0.04],
                [0.25, 0.25, side * 0.115],
                [0.22, 0.235, side * 0.19],
            ],
            &[0.014, 0.03, 0.012],
        );
        s.tube_with(
            j::HEAD,
            ivory,
            &[
                [0.25, -0.005, side * 0.135],
                [0.37, 0.045, side * 0.21],
                [0.35, 0.22, side * 0.24],
                [0.28, 0.285, side * 0.2],
            ],
            &[0.055, 0.047, 0.025, 0.003],
            false,
            8,
        );
        s.panel_with(
            j::HEAD,
            "#704533",
            &[[-0.04, 0.01], [0.0, 0.22], [0.1, 0.08], [0.065, -0.03]],
            [-0.03, 0.28, side * 0.17],
            false,
            0.035,
        );
        s.panel_with(
            j::HEAD,
            "#a27155",
            &[[0.0, 0.02], [0.025, 0.15], [0.07, 0.075]],
            [-0.005, 0.29, side * 0.18],
            false,
            0.01,
        );
    }
    for i in 0..7 {
        let k = i as f32 - 3.0;
        s.tube(
            j::HEAD,
            if i % 2 != 0 { "#853c2d" } else { "#a34c31" },
            &[
                [-0.1, 0.26, k * 0.042],
                [-0.015, 0.42 + (i % 2) as f32 * 0.04, k * 0.045],
                [-0.2, 0.42, k * 0.05],
            ],
            &[0.045, 0.05, 0.002],
        );
    }
    let (ua, fa) = (rig.upper_arm, rig.forearm);
    for (arm, fore, leg, shin, _) in SIDES {
        s.form(
            arm,
            fur,
            rings![[-ua, 0.12, 0.14], [-0.1, 0.18, 0.19], [0.06, 0.16, 0.18]],
        );
        s.form(
            fore,
            "#8c6247",
            rings![[-fa, 0.12, 0.14], [-0.16, 0.15, 0.16], [0.0, 0.145, 0.15]],
        );
        s.form_with(
            fore,
            armor,
            rings![[-fa + 0.06, 0.146, 0.16], [-fa + 0.15, 0.17, 0.18]],
            true,
            PartOpts::default(),
        );
        s.ell(fore, fur, [0.025, -fa - 0.04, 0.0], [0.14, 0.12, 0.16]);
        for i in -1..=1 {
            let z = i as f32 * 0.09;
            s.tube(
                fore,
                ivory,
                &[[0.11, -fa - 0.02, z], [0.17, -fa - 0.08, z]],
                &[0.026, 0.003],
            );
        }
        s.form(
            leg,
            fur,
            rings![[-rig.thigh, 0.12, 0.14], [0.0, 0.15, 0.17]],
        );
        s.form(
            shin,
            "#685044",
            rings![[-rig.shin + 0.08, 0.1, 0.12], [0.0, 0.13, 0.14]],
        );
        for z in [-0.07, 0.07] {
            s.ell(
                shin,
                "#302b2b",
                [0.075, -rig.shin + 0.045, z],
                [0.18, 0.095, 0.071],
            );
        }
    }
}

/// Portraits des créatures, par identifiant de champion.
pub const PORTRAITS: &[(&str, PortraitBuild)] = &[("grondak", grondak), ("tharox", tharox)];
