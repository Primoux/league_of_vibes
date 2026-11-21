//! Modèles low-poly des monstres : jungle, drakes (6 éléments + Ancien), larves, Colosse du Rift,
//! Léviathan. Tous regardent +x.
//!
//! Portage de `packages/client/src/render/models/monsters.ts`.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use lov_shared::types::DragonElement;

use super::Built;
use super::geometry::{
    cone, cuboid, cylinder, dodecahedron, icosahedron, octahedron, sphere, sphere_part, triangles,
};
use super::kit::{Kit, MatOpts, Model, N};
use super::minions::limb;
use super::rig::Rig;

struct Quadruped {
    len: f32,
    h: f32,
    w: f32,
    leg_len: f32,
    color: u32,
    belly: Option<u32>,
    head_size: f32,
    snout: Option<f32>,
    tail: Option<f32>,
    eyes: Option<u32>,
}

/// Quadrupède générique : renvoie le modèle, son rig et le pivot du corps.
fn quadruped(kit: &mut Kit, o: &Quadruped) -> (Built, N) {
    let mut m = Model::new();
    let root = Model::ROOT;
    let fur = kit.material(o.color, MatOpts::rough(0.9));
    let leg_mat = kit.material(o.belly.unwrap_or(o.color), MatOpts::rough(0.9));
    let lw = o.w * 0.28;
    let legs = [(1.0, 1.0), (1.0, -1.0), (-1.0, -1.0), (-1.0, 1.0)]
        .map(|(sx, sz)| {
            let at = Vec3::new(sx * o.len * 0.32, o.leg_len, sz * o.w * 0.3);
            limb(kit, &mut m, root, at, o.leg_len, lw, leg_mat.clone())
        })
        .to_vec();
    let body = m.pivot(root, 0.0, o.leg_len, 0.0);
    let (len, h, w) = (o.len, o.h, o.w);
    let g = kit.geo(&format!("q_body_{len}_{h}_{w}"), || {
        cuboid(len, h, w).translate(0.0, h / 2.0, 0.0)
    });
    m.part(body, g, fur.clone(), 0.0, 0.0, 0.0);
    let head = m.pivot(body, o.len / 2.0, o.h * 0.8, 0.0);
    let hs = o.head_size;
    let g = kit.geo(&format!("q_head_{hs}"), || cuboid(hs, hs * 0.85, hs * 0.9));
    m.part(head, g, fur.clone(), hs * 0.35, 0.0, 0.0);
    if let Some(s) = o.snout {
        let g = kit.geo(&format!("q_snout_{s}"), || cuboid(s, s * 0.5, s * 0.6));
        m.part(head, g, fur.clone(), hs * 0.8, -hs * 0.15, 0.0);
    }
    if let Some(eyes) = o.eyes {
        let g = kit.geo("q_eye", || cuboid(0.04, 0.05, 0.05));
        let glow = kit.glow(eyes, 2.0);
        for z in [-1.0, 1.0] {
            let n = m.part(
                head,
                g.clone(),
                glow.clone(),
                hs * 0.86,
                hs * 0.15,
                z * hs * 0.25,
            );
            m.no_shadow(n);
        }
    }
    let mut tail = None;
    if let Some(t) = o.tail {
        let p = m.pivot(body, -o.len / 2.0, o.h * 0.8, 0.0);
        let g = kit.geo(&format!("q_tail_{t}"), || {
            cone(w * 0.15, t, 4)
                .rotate_z(PI / 2.0)
                .translate(-t / 2.0, 0.0, 0.0)
        });
        m.part(p, g, fur, 0.0, 0.0, 0.0);
        tail = Some(p);
    }
    let mut rig = Rig::new(o.leg_len + o.h);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.tail = tail;
    rig.quadruped = true;
    rig.stride = 2.6;
    (Built { model: m, rig }, body)
}

pub fn wolf(kit: &mut Kit, small: bool) -> Built {
    let o = Quadruped {
        len: 0.9,
        h: 0.38,
        w: 0.36,
        leg_len: 0.35,
        color: if small { 0x6f6a72 } else { 0x4f4a58 },
        belly: Some(0x3a3640),
        head_size: 0.32,
        snout: Some(0.22),
        tail: Some(0.5),
        eyes: Some(0x9fe8ff),
    };
    let (mut b, _) = quadruped(kit, &o);
    b.model.scale(Model::ROOT, if small { 0.85 } else { 1.35 });
    b
}

pub fn raptor(kit: &mut Kit, small: bool) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let skin = kit.material(if small { 0xc75a3a } else { 0xa23a2a }, MatOpts::rough(0.8));
    let dark = kit.mat(0x3a2420);
    let legs = vec![
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.42, 0.12),
            0.42,
            0.07,
            dark.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.42, -0.12),
            0.42,
            0.07,
            dark,
        ),
    ];
    let body = m.pivot(root, 0.0, 0.42, 0.0);
    let g = kit.geo("rp_body", || sphere(0.26, 7, 5).scale(1.4, 0.9, 0.9));
    m.part(body, g, skin.clone(), 0.0, 0.18, 0.0);
    let head = m.pivot(body, 0.3, 0.42, 0.0);
    let g = kit.geo("rp_neck", || cylinder(0.06, 0.09, 0.32, 5));
    let neck = m.part(head, g, skin.clone(), 0.0, -0.1, 0.0);
    m.rot(neck, 0.0, 0.0, -0.5);
    let g = kit.geo("rp_head", || cone(0.11, 0.38, 5).rotate_z(-PI / 2.0));
    m.part(head, g, skin.clone(), 0.12, 0.05, 0.0);
    let g = kit.geo("rp_crest", || cone(0.05, 0.25, 3));
    let crest_mat = kit.mat(0xf2c14a);
    let crest = m.part(head, g, crest_mat, -0.02, 0.18, 0.0);
    m.rot(crest, 0.0, 0.0, 0.6);
    let tail = m.pivot(body, -0.34, 0.2, 0.0);
    let g = kit.geo("rp_tail", || {
        cone(0.1, 0.55, 5)
            .rotate_z(PI / 2.0)
            .translate(-0.27, 0.0, 0.0)
    });
    m.part(tail, g, skin, 0.0, 0.0, 0.0);
    m.scale(root, if small { 0.85 } else { 1.25 });
    let mut rig = Rig::new(0.9);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.tail = Some(tail);
    rig.stride = 3.2;
    Built { model: m, rig }
}

pub fn krug(kit: &mut Kit, small: bool) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let rock = kit.material(0x8a7d6a, MatOpts::rough(1.0));
    let moss = kit.material(0x5c7a3a, MatOpts::rough(1.0));
    let legs = vec![
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.3, 0.2),
            0.3,
            0.18,
            rock.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.3, -0.2),
            0.3,
            0.18,
            rock.clone(),
        ),
    ];
    let body = m.pivot(root, 0.0, 0.3, 0.0);
    let g = kit.geo("kr_body", || dodecahedron(0.45, 0).scale(1.0, 0.85, 1.0));
    m.part(body, g, rock.clone(), 0.0, 0.38, 0.0);
    let g = kit.geo("kr_moss", || dodecahedron(0.3, 0).scale(1.2, 0.4, 1.2));
    m.part(body, g, moss, -0.05, 0.75, 0.0);
    let head = m.pivot(body, 0.35, 0.45, 0.0);
    let g = kit.geo("kr_eye", || cuboid(0.06, 0.06, 0.25));
    let glow = kit.glow(0xffa040, 2.0);
    let eye = m.part(head, g, glow, 0.06, 0.0, 0.0);
    m.no_shadow(eye);
    let a1 = limb(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 0.5, 0.48),
        0.45,
        0.2,
        rock.clone(),
    );
    let a2 = limb(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 0.5, -0.48),
        0.45,
        0.2,
        rock,
    );
    m.scale(root, if small { 0.7 } else { 1.3 });
    let mut rig = Rig::new(1.2);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.arms = vec![a1, a2];
    rig.stride = 1.4;
    Built { model: m, rig }
}

pub fn gromp(kit: &mut Kit) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let skin = kit.material(0x5f8a3a, MatOpts::rough(0.7));
    let belly = kit.material(0xc9c27a, MatOpts::rough(0.8));
    let body = m.pivot(root, 0.0, 0.0, 0.0);
    let g = kit.geo("gr_body", || sphere(0.6, 8, 6).scale(1.15, 0.75, 1.0));
    m.part(body, g, skin.clone(), 0.0, 0.5, 0.0);
    let g = kit.geo("gr_belly", || sphere(0.45, 8, 6).scale(0.8, 0.6, 0.9));
    m.part(body, g, belly, 0.25, 0.38, 0.0);
    let head = m.pivot(body, 0.5, 0.75, 0.0);
    let eyeball = kit.geo("gr_eyeball", || sphere(0.14, 6, 5));
    let pupil = kit.geo("gr_pupil", || sphere(0.06, 5, 4));
    let white = kit.mat(0xf2eed0);
    let black = kit.mat(0x111111);
    for z in [-0.25, 0.25] {
        m.part(head, eyeball.clone(), white.clone(), 0.0, 0.12, z);
        m.part(head, pupil.clone(), black.clone(), 0.11, 0.14, z);
    }
    let legs = vec![
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.3, 0.3, 0.45),
            0.3,
            0.14,
            skin.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.3, 0.3, -0.45),
            0.3,
            0.14,
            skin.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(-0.35, 0.35, -0.5),
            0.35,
            0.2,
            skin.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(-0.35, 0.35, 0.5),
            0.35,
            0.2,
            skin,
        ),
    ];
    m.scale(root, 1.3);
    let mut rig = Rig::new(1.2);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.quadruped = true;
    rig.stride = 1.2;
    Built { model: m, rig }
}

/// Golem gardien de buff : pierre + cristaux lumineux (bleu) ou magma (rouge).
pub fn buff_golem(kit: &mut Kit, blue: bool) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let c = if blue { 0x4ab0ff } else { 0xff5a2a };
    let stone = kit.material(if blue { 0x5d6a78 } else { 0x5a3a30 }, MatOpts::rough(1.0));
    let legs = vec![
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.6, 0.32),
            0.6,
            0.3,
            stone.clone(),
        ),
        limb(
            kit,
            &mut m,
            root,
            Vec3::new(0.0, 0.6, -0.32),
            0.6,
            0.3,
            stone.clone(),
        ),
    ];
    let body = m.pivot(root, 0.0, 0.6, 0.0);
    let g = kit.geo("bg_torso", || dodecahedron(0.65, 0).scale(0.9, 1.0, 1.1));
    m.part(body, g, stone.clone(), 0.0, 0.6, 0.0);
    let g = kit.geo("bg_core", || octahedron(0.22, 0));
    let glow = kit.glow(c, 2.4);
    let core = m.part(body, g, glow, 0.5, 0.65, 0.0);
    m.no_shadow(core);
    let shard = kit.geo("bg_shard", || cone(0.1, 0.5, 4));
    let glow = kit.glow(c, 1.6);
    for i in 0..5 {
        let a = i as f32 / 5.0 * PI - PI / 2.0;
        let cr = m.part(body, shard.clone(), glow.clone(), -0.2, 1.2, a.sin() * 0.4);
        m.rot(cr, a * 0.5, 0.0, 0.0);
    }
    let head = m.pivot(body, 0.35, 1.25, 0.0);
    let g = kit.geo("bg_head", || cuboid(0.4, 0.32, 0.4));
    m.part(head, g, stone.clone(), 0.0, 0.0, 0.0);
    let g = kit.geo("bg_eyes", || cuboid(0.03, 0.06, 0.28));
    let glow = kit.glow(c, 3.0);
    let eyes = m.part(head, g, glow, 0.21, 0.03, 0.0);
    m.no_shadow(eyes);
    let a1 = limb(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 1.0, 0.75),
        0.8,
        0.28,
        stone.clone(),
    );
    let a2 = limb(
        kit,
        &mut m,
        body,
        Vec3::new(0.0, 1.0, -0.75),
        0.8,
        0.28,
        stone.clone(),
    );
    let fist = kit.geo("bg_fist", || dodecahedron(0.25, 0));
    m.part(a1, fist.clone(), stone.clone(), 0.0, -0.85, 0.0);
    m.part(a2, fist, stone, 0.0, -0.85, 0.0);
    let mut rig = Rig::new(2.2);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.arms = vec![a1, a2];
    rig.stride = 1.2;
    Built { model: m, rig }
}

pub fn scuttle(kit: &mut Kit) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let shell = kit.material(0x3a8a9a, MatOpts::rm(0.5, 0.2));
    let body = m.pivot(root, 0.0, 0.32, 0.0);
    let g = kit.geo("sc_shell", || {
        sphere_part(0.5, 8, 5, 0.0, TAU, 0.0, PI / 2.0).scale(1.2, 0.6, 1.0)
    });
    m.part(body, g, shell.clone(), 0.0, 0.0, 0.0);
    let g = kit.geo("sc_glow", || icosahedron(0.12, 0));
    let glow = kit.glow(0x9ff6ff, 2.0);
    let n = m.part(body, g, glow, 0.0, 0.32, 0.0);
    m.no_shadow(n);
    let head = m.pivot(body, 0.5, 0.05, 0.0);
    let g = kit.geo("sc_eye", || cylinder(0.03, 0.03, 0.22, 4));
    let black = kit.mat(0x222222);
    for z in [-0.15, 0.15] {
        m.part(head, g.clone(), black.clone(), 0.0, 0.12, z);
    }
    let mut legs = Vec::new();
    for i in 0..6 {
        let side = if i < 3 { 1.0 } else { -1.0 };
        let x = ((i % 3) as f32 - 1.0) * 0.3;
        let l = limb(
            kit,
            &mut m,
            root,
            Vec3::new(x, 0.32, side * 0.45),
            0.4,
            0.06,
            shell.clone(),
        );
        m.rot(l, side * 0.6, 0.0, 0.0);
        legs.push(l);
    }
    m.scale(root, 1.2);
    let mut rig = Rig::new(0.7);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.stride = 4.0;
    Built { model: m, rig }
}

/// Élément d'un drake, ou l'Ancien.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drake {
    Element(DragonElement),
    Elder,
}

/// Couleurs (corps, accent) d'un drake.
pub fn dragon_colors(d: Drake) -> (u32, u32) {
    match d {
        Drake::Element(DragonElement::Infernal) => (0x9a2a1a, 0xff7a1a),
        Drake::Element(DragonElement::Ocean) => (0x1a5a8a, 0x4ff0e0),
        Drake::Element(DragonElement::Mountain) => (0x6a5a40, 0xd8b060),
        Drake::Element(DragonElement::Cloud) => (0xa8c0d8, 0xf0f8ff),
        Drake::Element(DragonElement::Hextech) => (0x2a3a6a, 0x5ae0ff),
        Drake::Element(DragonElement::Chemtech) => (0x3a5a2a, 0xa8ff3a),
        Drake::Elder => (0x3a2a5a, 0xc89aff),
    }
}

pub fn dragon(kit: &mut Kit, d: Drake) -> Built {
    let (col, accent) = dragon_colors(d);
    let elder = d == Drake::Elder;
    let o = Quadruped {
        len: 1.8,
        h: 0.8,
        w: 0.9,
        leg_len: 0.65,
        color: col,
        belly: None,
        head_size: 0.55,
        snout: Some(0.45),
        tail: Some(1.6),
        eyes: Some(accent),
    };
    let (mut b, body) = quadruped(kit, &o);
    let m = &mut b.model;
    let acc = kit.glow(accent, if elder { 2.0 } else { 1.3 });
    // Le client web passe le matériau partagé du corps en double face ; ici, une variante à part.
    let wing_mat = kit.material(
        col,
        MatOpts {
            rough: 0.6,
            double_sided: true,
            ..default()
        },
    );
    let mut wings = Vec::new();
    for side in [1.0f32, -1.0] {
        let w = m.pivot(body, 0.2, 0.75, side * 0.45);
        let g = kit.geo(&format!("dr_wing_{side}"), || {
            triangles(&[
                Vec3::ZERO,
                Vec3::new(-1.2, 0.1, side * 1.6),
                Vec3::new(0.5, 0.2, side * 1.3),
                Vec3::ZERO,
                Vec3::new(-1.4, -0.1, side * 0.8),
                Vec3::new(-1.2, 0.1, side * 1.6),
            ])
        });
        m.part(w, g, wing_mat.clone(), 0.0, 0.0, 0.0);
        wings.push(w);
    }
    // crête lumineuse sur le dos
    let spike = kit.geo("dr_spike", || cone(0.08, 0.3, 4));
    for i in 0..5 {
        let n = m.part(
            body,
            spike.clone(),
            acc.clone(),
            0.6 - i as f32 * 0.3,
            0.9,
            0.0,
        );
        m.no_shadow(n);
    }
    if let Some(head) = b.rig.head {
        let horn = kit.geo("dr_horn", || cone(0.06, 0.4, 4));
        for z in [-0.18, 0.18] {
            let n = m.part(head, horn.clone(), acc.clone(), 0.1, 0.38, z);
            m.rot(n, 0.0, 0.0, 0.7);
        }
    }
    b.rig.wings = wings;
    b.rig.stride = 1.4;
    m.scale(Model::ROOT, if elder { 1.45 } else { 1.2 });
    b
}

pub fn grub(kit: &mut Kit) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let skin = kit.material(0x7a4ab0, MatOpts::rough(0.7));
    let body = m.pivot(root, 0.0, 0.0, 0.0);
    let mut segs = Vec::new();
    for i in 0..4 {
        let s = 0.24 - i as f32 * 0.03;
        let g = kit.geo(&format!("gb_seg_{i}"), || sphere(s, 6, 5));
        segs.push(m.part(body, g, skin.clone(), -(i as f32) * 0.28, s, 0.0));
    }
    let head = m.pivot(body, 0.2, 0.28, 0.0);
    let g = kit.geo("gb_eye", || sphere(0.06, 5, 4));
    let glow = kit.glow(0xf0a0ff, 2.5);
    let n = m.part(head, g, glow, 0.1, 0.05, 0.0);
    m.no_shadow(n);
    // les segments ondulent : on les traite comme « jambes » (rotation z sur leur centre)
    let mut rig = Rig::new(0.5);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = segs;
    rig.stride = 2.5;
    Built { model: m, rig }
}

/// Colosse du Rift : grand scarabée cuirassé à œil unique.
pub fn herald(kit: &mut Kit) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let shell = kit.material(0x4a2a6a, MatOpts::rm(0.45, 0.3));
    let plate = kit.material(0x7a5aa0, MatOpts::rm(0.5, 0.3));
    let eye = kit.glow(0xff4ad8, 3.0);
    let mut legs = Vec::new();
    for i in 0..6 {
        let side = if i < 3 { 1.0 } else { -1.0 };
        let at = Vec3::new(((i % 3) as f32 - 1.0) * 0.6, 0.8, side * 0.8);
        let l = limb(kit, &mut m, root, at, 0.85, 0.16, shell.clone());
        m.rot(l, side * 0.45, 0.0, 0.0);
        legs.push(l);
    }
    let body = m.pivot(root, 0.0, 0.75, 0.0);
    let g = kit.geo("hr_shell", || {
        sphere_part(1.0, 9, 6, 0.0, TAU, 0.0, PI / 2.0).scale(1.35, 0.9, 1.0)
    });
    m.part(body, g, shell, 0.0, 0.0, 0.0);
    let g = kit.geo("hr_plate", || cuboid(0.2, 0.15, 1.6));
    for i in 0..3 {
        let y = 0.82 - (i as f32 - 1.0).abs() * 0.12;
        m.part(
            body,
            g.clone(),
            plate.clone(),
            -0.6 + i as f32 * 0.5,
            y,
            0.0,
        );
    }
    let head = m.pivot(body, 1.15, 0.25, 0.0);
    let g = kit.geo("hr_head", || dodecahedron(0.45, 0));
    m.part(head, g, plate.clone(), 0.0, 0.0, 0.0);
    let g = kit.geo("hr_eye", || sphere(0.2, 8, 6));
    let n = m.part(head, g, eye, 0.35, 0.05, 0.0);
    m.no_shadow(n);
    let g = kit.geo("hr_mandible", || cone(0.08, 0.6, 4).rotate_z(-PI / 2.0));
    for z in [-0.3, 0.3] {
        m.part(head, g.clone(), plate.clone(), 0.45, -0.25, z);
    }
    m.scale(root, 1.3);
    let mut rig = Rig::new(2.2);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.stride = 1.6;
    Built { model: m, rig }
}

/// Léviathan : serpent abyssal géant dressé, tête cornue.
pub fn leviathan(kit: &mut Kit) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let skin = kit.material(0x2a1a4a, MatOpts::rm(0.55, 0.15));
    let scale = kit.material(0x5a3a8a, MatOpts::rough(0.5));
    let acc = kit.glow(0xb06aff, 2.0);
    let body = m.pivot(root, 0.0, 0.0, 0.0);
    // corps en arc : anneaux de tailles décroissantes
    let segs = 9;
    let fin = kit.geo("lv_fin", || cone(0.12, 0.55, 4));
    for i in 0..segs {
        let t = i as f32 / (segs - 1) as f32;
        let r = 0.75 - t * 0.35;
        let x = -1.8 + t * 2.6;
        let y = 0.5 + (t * PI * 0.85).sin() * 2.2;
        let g = kit.geo(&format!("lv_seg_{i}"), || dodecahedron(r, 0));
        let mat = if i % 2 == 1 {
            scale.clone()
        } else {
            skin.clone()
        };
        m.part(body, g, mat, x, y, 0.0);
        if i % 2 == 0 {
            let n = m.part(body, fin.clone(), acc.clone(), x, y + r * 0.9, 0.0);
            m.no_shadow(n);
        }
    }
    let head = m.pivot(body, 1.1, 2.6, 0.0);
    let g = kit.geo("lv_head", || cuboid(1.1, 0.6, 0.8));
    m.part(head, g, skin, 0.3, 0.0, 0.0);
    let g = kit.geo("lv_jaw", || cuboid(0.9, 0.2, 0.7));
    m.part(head, g, scale.clone(), 0.4, -0.35, 0.0);
    let eye = kit.geo("lv_eye", || cuboid(0.1, 0.1, 0.12));
    let eye_mat = kit.glow(0xffe04a, 3.0);
    let horn = kit.geo("lv_horn", || cone(0.12, 0.9, 5));
    let bone = kit.mat(0xd8cfb8);
    for z in [-0.3, 0.3] {
        let n = m.part(head, eye.clone(), eye_mat.clone(), 0.86, 0.12, z);
        m.no_shadow(n);
        let n = m.part(head, horn.clone(), bone.clone(), -0.1, 0.5, z);
        m.rot(n, 0.0, 0.0, 0.9);
    }
    // tentacules à la base = « jambes »
    let mut legs = Vec::new();
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU + 0.4;
        let at = Vec3::new(-1.8 + a.cos() * 0.7, 0.6, a.sin() * 0.7);
        let l = limb(kit, &mut m, root, at, 0.9, 0.18, scale.clone());
        m.rot(l, a.sin() * 0.7, 0.0, 0.0);
        legs.push(l);
    }
    m.scale(root, 1.35);
    let mut rig = Rig::new(4.0);
    rig.body = Some(body);
    rig.head = Some(head);
    rig.legs = legs;
    rig.stride = 0.8;
    rig.floaters = vec![head];
    Built { model: m, rig }
}

/// `monsterModel` : `None` pour un sous-type inconnu (la vue par défaut prend le relais).
pub fn monster(
    kit: &mut Kit,
    subtype: Option<&str>,
    element: Option<DragonElement>,
) -> Option<Built> {
    Some(match subtype? {
        "wolves" => wolf(kit, false),
        "wolf_small" => wolf(kit, true),
        "raptors" => raptor(kit, false),
        "raptor_small" => raptor(kit, true),
        "krugs" => krug(kit, false),
        "krug_small" => krug(kit, true),
        "gromp" => gromp(kit),
        "blue_buff" => buff_golem(kit, true),
        "red_buff" => buff_golem(kit, false),
        "scuttle" => scuttle(kit),
        "dragon" => dragon(
            kit,
            Drake::Element(element.unwrap_or(DragonElement::Infernal)),
        ),
        "elder_dragon" => dragon(kit, Drake::Elder),
        "grub" => grub(kit),
        "herald" => herald(kit),
        "leviathan" => leviathan(kit),
        _ => return None,
    })
}
