//! Modèles low-poly des sbires (mêlée, lanceur, canon, super-sbire), couleur d'équipe.
//!
//! Portage de `packages/client/src/render/models/minions.ts`.

use std::f32::consts::PI;

use bevy::prelude::*;
use lov_shared::types::TeamOrNeutral;

use super::Built;
use super::geometry::{cone, cuboid, cylinder, dodecahedron, icosahedron, sphere};
use super::kit::{Kit, MatOpts, Model, ModelMaterial, N};
use super::rig::Rig;
use super::structures::team_hex;

fn armor(team: TeamOrNeutral) -> u32 {
    match team {
        TeamOrNeutral::Blue => 0x3567c9,
        TeamOrNeutral::Red => 0xb83a30,
        TeamOrNeutral::Neutral => 0x9a8a5a,
    }
}
const SKIN: u32 = 0xd8b48a;
const METAL: u32 = 0xb9bcc2;

/// Membre : pivot à l'articulation, géométrie vers le bas (jambe ou bras, même construction).
pub fn limb(
    kit: &mut Kit,
    m: &mut Model,
    parent: N,
    at: Vec3,
    len: f32,
    w: f32,
    mat: Handle<ModelMaterial>,
) -> N {
    let p = m.pivot(parent, at.x, at.y, at.z);
    let g = kit.geo(&format!("limb_{len}_{w}"), || {
        cuboid(w, len, w).translate(0.0, -len / 2.0, 0.0)
    });
    m.part(p, g, mat, 0.0, 0.0, 0.0);
    p
}

fn leg(kit: &mut Kit, m: &mut Model, at: Vec3, len: f32, w: f32, mat: Handle<ModelMaterial>) -> N {
    limb(kit, m, Model::ROOT, at, len, w, mat)
}

fn arm(
    kit: &mut Kit,
    m: &mut Model,
    parent: N,
    at: Vec3,
    len: f32,
    w: f32,
    mat: Handle<ModelMaterial>,
) -> N {
    limb(kit, m, parent, at, len, w, mat)
}

pub fn melee(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let armor = kit.material(armor(team), MatOpts::rm(0.6, 0.2));
    let metal = kit.material(METAL, MatOpts::rm(0.35, 0.7));
    let dark = kit.mat(0x3a3430);
    let legs = vec![
        leg(
            kit,
            &mut m,
            Vec3::new(0.0, 0.32, 0.1),
            0.32,
            0.12,
            dark.clone(),
        ),
        leg(kit, &mut m, Vec3::new(0.0, 0.32, -0.1), 0.32, 0.12, dark),
    ];
    let body = m.pivot(Model::ROOT, 0.0, 0.32, 0.0);
    let g = kit.geo("mm_torso", || {
        cuboid(0.3, 0.34, 0.36).translate(0.0, 0.17, 0.0)
    });
    m.part(body, g, armor.clone(), 0.0, 0.0, 0.0);
    let head = m.pivot(body, 0.0, 0.42, 0.0);
    let g = kit.geo("mm_helm", || sphere(0.15, 6, 5));
    m.part(head, g, armor.clone(), 0.0, 0.08, 0.0);
    let g = kit.geo("mm_crest", || cone(0.05, 0.16, 4));
    let glow = kit.glow(team_hex(team), 1.0);
    m.part(head, g, glow, 0.0, 0.26, 0.0);
    let sword = arm(
        kit,
        &mut m,
        body,
        Vec3::new(0.02, 0.3, -0.24),
        0.24,
        0.09,
        armor.clone(),
    );
    let g = kit.geo("mm_blade", || {
        cuboid(0.05, 0.4, 0.03).translate(0.0, -0.38, 0.0)
    });
    let blade = m.part(sword, g, metal.clone(), 0.0, 0.0, 0.0);
    m.rot(blade, 0.0, 0.0, 0.3);
    let shield_arm = arm(
        kit,
        &mut m,
        body,
        Vec3::new(0.05, 0.3, 0.24),
        0.22,
        0.09,
        armor,
    );
    let g = kit.geo("mm_shield", || {
        cylinder(0.16, 0.16, 0.04, 6).rotate_x(PI / 2.0)
    });
    m.part(shield_arm, g, metal, 0.08, -0.2, 0.05);
    let mut rig = Rig::new(0.9);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.arms = vec![sword, shield_arm];
    rig.weapon = Some(sword);
    rig.stride = 2.4;
    Built { model: m, rig }
}

pub fn caster(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let robe = kit.material(armor(team), MatOpts::rough(0.85));
    let trim = kit.material(0xe9d7a8, MatOpts::rough(0.6));
    let dark = kit.mat(0x3a3430);
    let legs = vec![
        leg(
            kit,
            &mut m,
            Vec3::new(0.0, 0.22, 0.08),
            0.22,
            0.09,
            dark.clone(),
        ),
        leg(kit, &mut m, Vec3::new(0.0, 0.22, -0.08), 0.22, 0.09, dark),
    ];
    let body = m.pivot(Model::ROOT, 0.0, 0.18, 0.0);
    let g = kit.geo("mc_robe", || cone(0.26, 0.6, 7).translate(0.0, 0.3, 0.0));
    m.part(body, g, robe.clone(), 0.0, 0.0, 0.0);
    let g = kit.geo("mc_belt", || cylinder(0.17, 0.19, 0.05, 7));
    m.part(body, g, trim, 0.0, 0.32, 0.0);
    let head = m.pivot(body, 0.0, 0.62, 0.0);
    let g = kit.geo("mc_head", || sphere(0.12, 6, 5));
    let skin = kit.mat(SKIN);
    m.part(head, g, skin, 0.0, 0.0, 0.0);
    let g = kit.geo("mc_hood", || cone(0.15, 0.3, 6));
    m.part(head, g, robe.clone(), -0.02, 0.14, 0.0);
    let staff_arm = arm(
        kit,
        &mut m,
        body,
        Vec3::new(0.05, 0.48, -0.2),
        0.2,
        0.07,
        robe,
    );
    let g = kit.geo("mc_staff", || {
        cylinder(0.02, 0.02, 0.8, 4).translate(0.0, -0.15, 0.0)
    });
    let wood = kit.mat(0x6a4a2a);
    m.part(staff_arm, g, wood, 0.04, 0.0, 0.0);
    let g = kit.geo("mc_orb", || icosahedron(0.08, 0));
    let glow = kit.glow(team_hex(team), 2.0);
    let orb = m.part(staff_arm, g, glow, 0.04, 0.27, 0.0);
    m.no_shadow(orb);
    let mut rig = Rig::new(0.9);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.arms = vec![staff_arm];
    rig.weapon = Some(staff_arm);
    Built { model: m, rig }
}

pub fn cannon(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let plate = kit.material(armor(team), MatOpts::rm(0.55, 0.3));
    let wood = kit.mat(0x6a4a2e);
    let iron = kit.material(0x3d3d42, MatOpts::rm(0.4, 0.7));
    let body = m.pivot(Model::ROOT, 0.0, 0.32, 0.0);
    let g = kit.geo("cn_cart", || cuboid(0.85, 0.3, 0.6));
    m.part(body, g, wood.clone(), 0.0, 0.05, 0.0);
    let g = kit.geo("cn_plate", || cuboid(0.88, 0.1, 0.64));
    m.part(body, g, plate, 0.0, 0.24, 0.0);
    let barrel = m.pivot(body, 0.05, 0.4, 0.0);
    let g = kit.geo("cn_barrel", || {
        cylinder(0.11, 0.15, 0.75, 8)
            .rotate_z(-PI / 2.0)
            .translate(0.3, 0.0, 0.0)
    });
    m.part(barrel, g, iron, 0.0, 0.0, 0.0);
    m.rot(barrel, 0.0, 0.0, 0.25);
    // roues = « jambes » (rotation continue simulée par le balancement)
    let wheel = kit.geo("cn_wheel", || {
        cylinder(0.18, 0.18, 0.07, 8).rotate_x(PI / 2.0)
    });
    let mut legs = Vec::new();
    for (x, z) in [(0.28, 0.33), (-0.28, 0.33), (0.28, -0.33), (-0.28, -0.33)] {
        let w = m.pivot(Model::ROOT, x, 0.18, z);
        m.part(w, wheel.clone(), wood.clone(), 0.0, 0.0, 0.0);
        legs.push(w);
    }
    let g = kit.geo("cn_flag", || {
        cuboid(0.02, 0.22, 0.16).translate(0.0, 0.0, 0.08)
    });
    let glow = kit.glow(team_hex(team), 0.8);
    m.part(body, g, glow, -0.38, 0.75, 0.0);
    let mut rig = Rig::new(0.9);
    rig.body = Some(body);
    rig.legs = legs;
    rig.weapon = Some(barrel);
    rig.stride = 3.0;
    rig.quadruped = true;
    Built { model: m, rig }
}

pub fn super_minion(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let armor = kit.material(armor(team), MatOpts::rm(0.5, 0.35));
    let dark = kit.mat(0x2e2a28);
    let metal = kit.material(METAL, MatOpts::rm(0.3, 0.8));
    let legs = vec![
        leg(
            kit,
            &mut m,
            Vec3::new(0.0, 0.55, 0.2),
            0.55,
            0.22,
            dark.clone(),
        ),
        leg(kit, &mut m, Vec3::new(0.0, 0.55, -0.2), 0.55, 0.22, dark),
    ];
    let body = m.pivot(Model::ROOT, 0.0, 0.55, 0.0);
    let g = kit.geo("sm_torso", || {
        cuboid(0.6, 0.7, 0.75).translate(0.0, 0.35, 0.0)
    });
    m.part(body, g, armor.clone(), 0.0, 0.0, 0.0);
    let pauldron = kit.geo("sm_pauldron", || sphere(0.22, 6, 4));
    m.part(body, pauldron.clone(), metal.clone(), 0.0, 0.68, 0.42);
    m.part(body, pauldron, metal.clone(), 0.0, 0.68, -0.42);
    let head = m.pivot(body, 0.05, 0.8, 0.0);
    let g = kit.geo("sm_head", || cuboid(0.3, 0.26, 0.28));
    m.part(head, g, armor.clone(), 0.0, 0.1, 0.0);
    let g = kit.geo("sm_eyes", || cuboid(0.02, 0.05, 0.2));
    let glow = kit.glow(team_hex(team), 2.5);
    m.part(head, g, glow, 0.16, 0.12, 0.0);
    let fist = kit.geo("sm_fist", || dodecahedron(0.17, 0));
    let fist1 = arm(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 0.62, -0.5),
        0.55,
        0.2,
        armor.clone(),
    );
    m.part(fist1, fist.clone(), metal.clone(), 0.0, -0.6, 0.0);
    let fist2 = arm(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 0.62, 0.5),
        0.55,
        0.2,
        armor,
    );
    m.part(fist2, fist, metal, 0.0, -0.6, 0.0);
    let mut rig = Rig::new(1.6);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.arms = vec![fist1, fist2];
    rig.weapon = Some(fist1);
    rig.stride = 1.6;
    Built { model: m, rig }
}

/// `minionModel` : mêlée par défaut.
pub fn minion(kit: &mut Kit, subtype: Option<&str>, team: TeamOrNeutral) -> Built {
    match subtype {
        Some("caster") => caster(kit, team),
        Some("cannon") => cannon(kit, team),
        Some("super") => super_minion(kit, team),
        _ => melee(kit, team),
    }
}
