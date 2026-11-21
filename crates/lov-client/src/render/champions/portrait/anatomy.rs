//! Anatomie et costumes communs des portraits : corps habillé, mains, visage, chevelures, capuche, cape,
//! jupe et soleil.
//!
//! Portage de `packages/client/src/render/champions/portrait/anatomy.ts`.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;

use super::geometry::{Facing, Paint, Point, Sculpt, cape, loft, plate, shade};
use crate::render::champions::anim::pose::j;
use crate::render::champions::builder::PartOpts;
use crate::render::champions::model::ModelColors;
use crate::render::champions::rig::RigSpec;
use crate::rings;

/// Tenue d'un corps humain.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outfit {
    pub cloth: &'static str,
    pub pants: &'static str,
    pub skin: &'static str,
    pub trim: &'static str,
    pub armor: bool,
    pub open_hands: bool,
    pub bare_arms: bool,
    /// Largeur du torse ; défaut : 0,84 × la demi-largeur d'épaules.
    pub width: Option<f32>,
}

/// Côtés du corps : (bras, avant-bras, cuisse, tibia, signe en z).
pub const SIDES: [(usize, usize, usize, usize, f32); 2] = [
    (j::ARM_L, j::FORE_L, j::LEG_L, j::SHIN_L, -1.0),
    (j::ARM_R, j::FORE_R, j::LEG_R, j::SHIN_R, 1.0),
];

pub fn human(s: &mut Sculpt, c: &ModelColors, rig: &RigSpec, o: &Outfit) {
    let w = o.width.unwrap_or(rig.shoulder_w * 0.84);
    s.form(
        j::HIPS,
        o.pants,
        rings![[-0.12, 0.12, 0.18], [0.0, 0.145, 0.19], [0.13, 0.13, 0.17]],
    );
    s.form(
        j::SPINE,
        o.cloth,
        rings![
            [0.0, 0.13, 0.17],
            [0.12, 0.15, w * 0.78],
            [0.31, 0.17, w],
            [rig.spine - 0.08, 0.145, w * 1.02],
            [rig.spine + 0.02, 0.09, 0.12],
        ],
    );
    s.form(
        j::HIPS,
        o.trim,
        rings![[0.055, 0.151, 0.197], [0.105, 0.15, 0.193]],
    );
    s.panel(
        j::HIPS,
        c.team,
        &[[-0.06, 0.04], [0.06, 0.04], [0.05, -0.035], [-0.05, -0.035]],
        [0.16, 0.08, 0.0],
    );
    s.ell(
        j::SPINE,
        o.skin,
        [0.0, rig.spine + 0.05, 0.0],
        [0.07, 0.115, 0.075],
    );
    for (arm, fore, leg, shin, side) in SIDES {
        let (ua, fa) = (rig.upper_arm, rig.forearm);
        s.form(
            arm,
            if o.bare_arms { o.skin } else { o.cloth },
            rings![
                [-ua, 0.072, 0.075],
                [-ua * 0.6, 0.09, 0.09],
                [-0.08, 0.115, 0.12],
                [0.035, 0.09, 0.1],
            ],
        );
        s.form(
            fore,
            o.cloth,
            rings![
                [-fa + 0.025, 0.067, 0.07],
                [-fa * 0.65, 0.085, 0.09],
                [-0.02, 0.095, 0.095],
            ],
        );
        s.form_with(
            fore,
            o.trim,
            rings![[-fa + 0.015, 0.075, 0.078], [-fa + 0.065, 0.083, 0.087]],
            o.armor,
            PartOpts::default(),
        );
        if !o.open_hands {
            hand(s, fore, o.skin, fa);
        }
        let (th, sh) = (rig.thigh, rig.shin);
        s.form(
            leg,
            o.pants,
            rings![
                [-th, 0.072, 0.08],
                [-th * 0.55, 0.09, 0.105],
                [0.05, 0.105, 0.12],
            ],
        );
        s.form(
            shin,
            o.pants,
            rings![
                [-sh + 0.07, 0.065, 0.075],
                [-sh * 0.5, 0.075, 0.085],
                [0.0, 0.085, 0.095],
            ],
        );
        s.ell(
            shin,
            "#282530",
            [0.055, -sh + 0.045, 0.0],
            [0.18, 0.085, 0.105],
        );
        s.form(
            shin,
            o.trim,
            rings![
                [-sh + 0.005, 0.16, 0.105, 0.055],
                [-sh + 0.03, 0.17, 0.11, 0.055],
            ],
        );
        if o.armor {
            s.ell_with(
                arm,
                o.trim,
                [0.0, -0.015, side * 0.015],
                [0.145, 0.12, 0.16],
                12,
                true,
            );
            s.panel_with(
                fore,
                o.trim,
                &[
                    [-0.08, 0.12],
                    [0.08, 0.12],
                    [0.067, -0.12],
                    [0.0, -0.16],
                    [-0.067, -0.12],
                ],
                [0.085, -0.14, 0.0],
                true,
                0.025,
            );
            s.panel_with(
                shin,
                o.trim,
                &[
                    [-0.075, 0.09],
                    [0.075, 0.09],
                    [0.055, -0.19],
                    [0.0, -0.26],
                    [-0.055, -0.19],
                ],
                [0.083, -0.12, 0.0],
                true,
                0.025,
            );
        }
    }
}

/// Main fermée au bout de l'avant-bras `joint` : paume, quatre doigts, pouce.
pub fn hand(s: &mut Sculpt, joint: usize, skin: impl Paint + Copy, length: f32) {
    s.ell(
        joint,
        skin,
        [0.01, -length - 0.015, 0.0],
        [0.052, 0.07, 0.066],
    );
    for i in 0..4 {
        let z = (i as f32 - 1.5) * 0.027;
        s.tube_with(
            joint,
            skin,
            &[
                [0.033, -length - 0.025, z],
                [0.05, -length - 0.07, z],
                [0.02, -length - 0.087, z],
            ],
            &[0.016, 0.015, 0.01],
            false,
            5,
        );
    }
    s.ell_with(
        joint,
        skin,
        [0.055, -length + 0.01, -0.055],
        [0.027, 0.04, 0.025],
        8,
        false,
    );
}

/// Traits d'un visage.
#[derive(Clone, Copy, Debug, Default)]
pub struct Face {
    pub skin: &'static str,
    pub hair: &'static str,
    pub eye: Option<&'static str>,
    pub feminine: bool,
    pub smile: bool,
    pub beard: bool,
}

pub fn face(s: &mut Sculpt, o: &Face) {
    let skin = o.skin;
    let dark = shade(skin, 0.78);
    let lips = if o.feminine { "#985d5b" } else { "#704d47" };
    let width = if o.feminine { 0.147 } else { 0.16 };
    let head = loft(
        rings![
            [-0.095, 0.045, 0.055, 0.045],
            [-0.055, 0.105, width * 0.64, 0.045],
            [0.02, 0.138, width * 0.9, 0.03],
            [0.12, 0.148, width, 0.025],
            [0.24, 0.125, width * 0.93, 0.015],
            [0.3, 0.08, width * 0.65, 0.0],
            [0.32, 0.025, 0.03, 0.0],
        ],
        20,
        true,
    );
    s.part(j::HEAD, &head, skin, PartOpts::default().smooth(), false);
    for side in [-1.0, 1.0] {
        s.ell_with(
            j::HEAD,
            skin,
            [-0.003, 0.105, side * width],
            [0.036, 0.056, 0.025],
            8,
            false,
        );
        s.ell_with(
            j::HEAD,
            dark,
            [0.012, 0.11, side * (width + 0.012)],
            [0.017, 0.032, 0.01],
            8,
            false,
        );
        s.ell_with(
            j::HEAD,
            "#d8d3c9",
            [0.169, 0.145, side * 0.067],
            [0.004, 0.012, 0.033],
            10,
            false,
        );
        s.ell_with(
            j::HEAD,
            o.eye.unwrap_or("#5d3829"),
            [0.174, 0.145, side * 0.064],
            [0.003, 0.011, 0.016],
            8,
            false,
        );
        s.ell_with(
            j::HEAD,
            "#161a24",
            [0.178, 0.145, side * 0.064],
            [0.002, 0.008, 0.009],
            8,
            false,
        );
        s.tube_with(
            j::HEAD,
            "#423839",
            &[
                [0.169, 0.149, side * 0.035],
                [0.179, 0.158, side * 0.067],
                [0.151, 0.15, side * 0.1],
            ],
            &[0.003, 0.004, 0.003],
            false,
            4,
        );
        s.tube_with(
            j::HEAD,
            o.hair,
            &[
                [0.157, 0.18, side * 0.035],
                [0.17, 0.191, side * 0.067],
                [0.145, 0.18, side * 0.104],
            ],
            &[0.006, 0.01, 0.005],
            false,
            5,
        );
        s.tube_with(
            j::HEAD,
            dark,
            &[
                [0.167, 0.158, side * 0.035],
                [0.17, 0.166, side * 0.07],
                [0.143, 0.157, side * 0.105],
            ],
            &[0.004, 0.005, 0.003],
            false,
            5,
        );
    }
    s.ell_with(
        j::HEAD,
        skin,
        [0.17, 0.095, 0.0],
        [0.028, 0.051, 0.021],
        12,
        false,
    );
    s.ell_with(
        j::HEAD,
        dark,
        [0.188, 0.064, 0.0],
        [0.013, 0.011, 0.021],
        8,
        false,
    );
    s.tube_with(
        j::HEAD,
        lips,
        &[
            [0.154, 0.022, -0.053],
            [0.168, if o.smile { 0.006 } else { 0.018 }, 0.0],
            [0.154, 0.022, 0.053],
        ],
        &[0.004, 0.008, 0.004],
        false,
        6,
    );
    if o.smile {
        s.panel_with(
            j::HEAD,
            "#eadfc8",
            &[
                [-0.035, 0.009],
                [0.035, 0.009],
                [0.026, -0.004],
                [-0.026, -0.004],
            ],
            [0.172, 0.023, 0.0],
            false,
            0.003,
        );
    }
    if o.beard {
        s.tube_with(
            j::HEAD,
            o.hair,
            &[
                [0.145, -0.005, -0.064],
                [0.16, -0.052, 0.0],
                [0.145, -0.005, 0.064],
            ],
            &[0.012, 0.022, 0.012],
            false,
            6,
        );
        s.tube_with(
            j::HEAD,
            o.hair,
            &[
                [0.175, 0.039, -0.045],
                [0.19, 0.044, 0.0],
                [0.175, 0.039, 0.045],
            ],
            &[0.006, 0.009, 0.006],
            false,
            5,
        );
    }
}

/// Chevelure : calotte, puis mèches ou boucles, et longues mèches tombantes (`long`).
pub fn hair(s: &mut Sculpt, dark: &str, light: &str, long: bool, curly: bool) {
    s.ell_with(
        j::HEAD,
        dark,
        [-0.04, 0.25, 0.0],
        [0.15, 0.12, 0.17],
        12,
        false,
    );
    if curly {
        // Boucles décalées et serrées : une masse irrégulière, pas des rouleaux parallèles.
        for i in 0..12 {
            let row = if i < 7 { 0.0 } else { 1.0 };
            let n = if i < 7 { i } else { i - 7 } as f32;
            let fi = i as f32;
            s.ell_with(
                j::HEAD,
                if i % 4 != 0 { dark } else { light },
                [
                    0.11 - row * 0.09 + (fi * 2.4).sin() * 0.012,
                    0.25 + row * 0.07 + (fi * 1.8).sin() * 0.017,
                    (n - if i < 7 { 3.0 } else { 2.0 }) * 0.044,
                ],
                [0.041, 0.041 + (i % 3) as f32 * 0.007, 0.034],
                8,
                false,
            );
        }
    } else {
        for i in 0..5 {
            let z = (i as f32 - 2.0) * 0.053;
            s.tube(
                j::HEAD,
                if i == 2 { light } else { dark },
                &[
                    [-0.1, 0.24, z * 0.65],
                    [-0.025, 0.33, z * 0.65],
                    [0.09, 0.29, z + 0.025],
                    [0.1, 0.23 + i as f32 * 0.007, z + 0.055],
                ],
                &[0.034, 0.042, 0.035, 0.003],
            );
        }
    }
    if long {
        for side in [-1.0, 1.0] {
            for i in 0..5 {
                let fi = i as f32;
                let z = side * (0.13 + fi * 0.017);
                s.tube(
                    j::HEAD,
                    if i % 2 != 0 { dark } else { light },
                    &[
                        [-0.055 - fi * 0.014, 0.22, z],
                        [-0.1, 0.03, z + side * 0.025],
                        [-0.04 - fi * 0.02, -0.18, z + side * 0.045],
                        [-0.14, -0.36 - fi * 0.017, z + side * 0.035],
                    ],
                    &[0.032, 0.039, 0.027, 0.004],
                );
            }
        }
    }
}

/// Capuche : coque, doublure sombre (enroulement inversé), bordure ; pointe tombante si `point`.
pub fn hood(s: &mut Sculpt, color: &str, trim: &str, point: bool) {
    let o = PartOpts::at(0.0, 0.16, 0.0)
        .rot(0.0, 0.0, -FRAC_PI_2)
        .smooth();
    let shell = loft(
        rings![
            [-0.24, 0.025, 0.025],
            [-0.18, 0.18, 0.18],
            [-0.05, 0.24, 0.215],
            [0.085, 0.23, 0.2],
        ],
        16,
        false,
    );
    s.part(j::HEAD, &shell, color, o, false);
    let mut inside = loft(
        rings![
            [-0.235, 0.02, 0.02],
            [-0.175, 0.169, 0.169],
            [-0.045, 0.228, 0.202],
            [0.084, 0.218, 0.187],
        ],
        16,
        false,
    );
    if let Some(index) = inside.indices.as_mut() {
        for t in index.as_chunks_mut::<3>().0 {
            t.swap(0, 1);
        }
    }
    s.part(j::HEAD, &inside, shade(color, 0.45), o, false);
    s.tube(
        j::HEAD,
        trim,
        &[
            [0.05, -0.065, -0.17],
            [0.095, 0.16, -0.19],
            [0.015, 0.36, 0.0],
            [0.095, 0.16, 0.19],
            [0.05, -0.065, 0.17],
        ],
        &[0.025, 0.023, 0.027, 0.023, 0.025],
    );
    if point {
        s.tube_with(
            j::HEAD,
            color,
            &[[-0.05, 0.3, 0.0], [-0.11, 0.46, 0.015], [-0.24, 0.48, 0.02]],
            &[0.12, 0.075, 0.003],
            false,
            8,
        );
    }
}

/// Cape dans le dos, bordée de deux galons.
pub fn cloak(s: &mut Sculpt, color: &str, trim: &str, height: f32, width: f32) {
    s.part(
        j::SPINE,
        &cape(height, width, 0.25),
        color,
        PartOpts::default().smooth(),
        false,
    );
    for side in [-1.0, 1.0] {
        s.tube_with(
            j::SPINE,
            trim,
            &[
                [-0.19, 0.48, side * 0.25],
                [-0.23, 0.05, side * width * 0.9],
                [-0.28, 0.48 - height, side * width],
            ],
            &[0.013, 0.014, 0.016],
            false,
            5,
        );
    }
}

/// Jupe longue évasée, ourlet et plis.
pub fn skirt(s: &mut Sculpt, color: &str, trim: &str, width: f32, length: f32) {
    s.form(
        j::HIPS,
        color,
        rings![
            [-length, 0.27, width],
            [-length * 0.7, 0.23, width * 0.87],
            [-length * 0.3, 0.17, width * 0.6],
            [0.08, 0.135, 0.18],
        ],
    );
    s.form(
        j::HIPS,
        trim,
        rings![
            [-length - 0.004, 0.273, width + 0.005],
            [-length + 0.035, 0.266, width],
        ],
    );
    let fold = shade(color, 0.83);
    for i in 0..7 {
        let a = i as f32 / 6.0 * PI - FRAC_PI_2;
        let (sin, cos) = a.sin_cos();
        s.tube_with(
            j::HIPS,
            fold,
            &[
                [cos * 0.14, -0.04, sin * 0.18],
                [cos * 0.235, -length * 0.65, sin * width * 0.83],
                [cos * 0.276, -length + 0.03, sin * width],
            ],
            &[0.006, 0.012, 0.008],
            false,
            4,
        );
    }
}

/// Soleil : anneau, disque, cœur lumineux et dix rayons sculptés.
pub fn sun(
    s: &mut Sculpt,
    joint: usize,
    color: &str,
    pos: Point,
    r: f32,
    glow: LinearRgba,
    metal: bool,
) {
    s.ring_with(joint, color, r, r * 0.12, pos, Facing::X, metal);
    s.ell_with(joint, color, pos, [r * 0.16, r * 0.68, r * 0.68], 10, metal);
    s.gem(
        joint,
        glow,
        [pos[0] + r * 0.18, pos[1], pos[2]],
        [r * 0.11, r * 0.35, r * 0.35],
    );
    let ray = plate(
        &[
            [0.0, 0.0],
            [0.022, r * 0.18],
            [0.0, r * 0.5],
            [-0.022, r * 0.18],
        ],
        0.012,
        0.002,
    );
    for i in 0..10 {
        let a = i as f32 / 10.0 * PI * 2.0;
        let o = PartOpts::at(
            pos[0],
            pos[1] + a.cos() * r * 1.03,
            pos[2] + a.sin() * r * 1.03,
        )
        .rot(a, 0.0, 0.0);
        s.part(joint, &ray, color, o, metal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::champions::builder::ModelBuilder;
    use crate::render::models::geometry::Geo;
    use lov_shared::types::TeamOrNeutral;

    const RIG: RigSpec = RigSpec {
        hip_y: 0.88,
        hip_w: 0.14,
        thigh: 0.44,
        shin: 0.43,
        spine: 0.56,
        shoulder_w: 0.26,
        upper_arm: 0.33,
        forearm: 0.3,
        neck: 0.15,
    };

    fn colors() -> Option<ModelColors> {
        ModelColors::of("vrenna", TeamOrNeutral::Blue)
    }

    #[test]
    fn corps_complet_sur_chaque_articulation() {
        let Some(c) = colors() else {
            panic!("vrenna connue");
        };
        let mut b = ModelBuilder::new();
        let mut s = Sculpt::new(&mut b);
        let outfit = Outfit {
            cloth: "#803638",
            pants: "#493029",
            skin: "#bd8d6b",
            trim: "#b58b54",
            armor: true,
            ..default()
        };
        human(&mut s, &c, &RIG, &outfit);
        face(
            &mut s,
            &Face {
                skin: "#c89b79",
                hair: "#35241d",
                beard: true,
                smile: true,
                ..default()
            },
        );
        hair(&mut s, "#35241d", "#5d3b2b", true, false);
        hair(&mut s, "#35241d", "#5d3b2b", false, true);
        hood(&mut s, "#5a3384", "#8d5ca7", true);
        cloak(&mut s, "#243659", "#cfaa62", 0.9, 0.37);
        skirt(&mut s, "#173b62", "#a1cbdf", 0.4, 0.83);
        sun(
            &mut s,
            j::SPINE,
            "#ead394",
            [0.226, 0.4, 0.0],
            0.067,
            c.glow,
            true,
        );
        let m = b.build(&RIG, true);
        for joint in [
            j::HIPS,
            j::SPINE,
            j::HEAD,
            j::ARM_L,
            j::FORE_R,
            j::LEG_L,
            j::SHIN_R,
        ] {
            assert!(m.body[joint].is_some(), "articulation {joint} vide");
        }
        // armure : métal aux épaules, avant-bras et tibias ; soleil lumineux
        assert!(m.metal[j::ARM_L].is_some() && m.metal[j::SHIN_R].is_some());
        assert!(m.glow[j::SPINE].is_some());
        assert!(m.sculpted);
        for mesh in m.body.iter().chain(&m.metal).flatten() {
            let n = mesh
                .attribute(Mesh::ATTRIBUTE_NORMAL)
                .and_then(|a| a.as_float3())
                .map(<[_]>::to_vec)
                .unwrap_or_default();
            assert_eq!(n.len(), mesh.count_vertices());
            assert!(n.iter().flatten().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn doublure_de_capuche_retournee() {
        // la doublure regarde vers l'axe : enroulement inverse de la coque
        let mut b = ModelBuilder::new();
        let mut s = Sculpt::new(&mut b);
        hood(&mut s, "#5a3384", "#8d5ca7", false);
        let shell = loft(rings![[-0.18, 0.18, 0.18], [-0.05, 0.24, 0.215]], 16, false);
        let facing = |g: &Geo| {
            let v = g.triangle_vertices();
            let t = &v[0..3];
            (t[1] - t[0])
                .cross(t[2] - t[0])
                .dot(Vec3::new(t[0].x, 0.0, t[0].z))
        };
        let mut inside = shell.clone();
        if let Some(i) = inside.indices.as_mut() {
            i.as_chunks_mut::<3>()
                .0
                .iter_mut()
                .for_each(|t| t.swap(0, 1));
        }
        assert!(facing(&shell) > 0.0 && facing(&inside) < 0.0);
    }
}
