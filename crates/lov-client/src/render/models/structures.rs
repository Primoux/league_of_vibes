//! Modèles low-poly des structures (tours, inhibiteurs, nexus) et balises.
//!
//! Portage de `packages/client/src/render/models/structures.ts`.

use std::f32::consts::{PI, TAU};

use lov_shared::types::{TeamOrNeutral, TowerTier};

use super::Built;
use super::geometry::{cone, cylinder, icosahedron, octahedron, torus};
use super::kit::{Kit, MatOpts, Model};
use super::rig::Rig;

/// Couleur vive de l'équipe (cristaux, liserés lumineux).
pub fn team_hex(team: TeamOrNeutral) -> u32 {
    match team {
        TeamOrNeutral::Blue => 0x3f8cff,
        TeamOrNeutral::Red => 0xff4a3a,
        TeamOrNeutral::Neutral => 0xe0b040,
    }
}

/// Teinte de pierre par équipe (bleu : pierre claire froide, rouge : pierre sombre chaude).
fn stone(team: TeamOrNeutral) -> u32 {
    match team {
        TeamOrNeutral::Blue => 0xa7a9a6,
        TeamOrNeutral::Red => 0x7c6458,
        TeamOrNeutral::Neutral => 0x8f8a7c,
    }
}

fn trim(team: TeamOrNeutral) -> u32 {
    match team {
        TeamOrNeutral::Blue => 0xd9c38a,
        TeamOrNeutral::Red => 0x8f5a3a,
        TeamOrNeutral::Neutral => 0xb8a070,
    }
}

pub fn tower(kit: &mut Kit, team: TeamOrNeutral, tier: Option<TowerTier>) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let stone = kit.material(stone(team), MatOpts::rough(0.9));
    let dark = kit.material(0x4a4642, MatOpts::rough(1.0));
    let trim = kit.material(trim(team), MatOpts::rm(0.5, 0.4));
    let c = team_hex(team);
    let scale = if tier == Some(TowerTier::Nexus) {
        0.9
    } else {
        1.0
    };

    let g = kit.geo("tw_foot", || cylinder(1.25, 1.45, 0.45, 8));
    m.part(root, g, dark, 0.0, 0.22, 0.0);
    let g = kit.geo("tw_base", || cylinder(1.0, 1.2, 0.9, 8));
    m.part(root, g, stone.clone(), 0.0, 0.9, 0.0);
    let g = kit.geo("tw_band1", || cylinder(1.06, 1.06, 0.14, 8));
    m.part(root, g, trim.clone(), 0.0, 1.38, 0.0);
    let g = kit.geo("tw_shaft", || cylinder(0.62, 0.9, 2.6, 8));
    m.part(root, g, stone.clone(), 0.0, 2.75, 0.0);
    let g = kit.geo("tw_band2", || cylinder(0.7, 0.7, 0.14, 8));
    m.part(root, g, trim.clone(), 0.0, 3.6, 0.0);
    let g = kit.geo("tw_head", || cylinder(1.0, 0.68, 0.6, 8));
    m.part(root, g, stone, 0.0, 4.3, 0.0);
    // 4 griffes qui tiennent le cristal
    let claw = kit.geo("tw_claw", || cone(0.14, 1.1, 4));
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU + PI / 4.0;
        let n = m.part(
            root,
            claw.clone(),
            trim.clone(),
            a.cos() * 0.72,
            5.0,
            a.sin() * 0.72,
        );
        m.rot(n, a.sin() * 0.35, 0.0, -a.cos() * 0.35);
    }
    let crystal_pivot = m.pivot(root, 0.0, 5.35, 0.0);
    let g = kit.geo("tw_crystal", || octahedron(0.55, 0).scale(1.0, 1.5, 1.0));
    let glow = kit.glow(c, 1.8);
    let crystal = m.part(crystal_pivot, g, glow, 0.0, 0.0, 0.0);
    m.no_shadow(crystal);
    m.scale(root, scale);
    let mut rig = Rig::new(6.0);
    rig.spinners = vec![crystal_pivot];
    rig.floaters = vec![crystal_pivot];
    rig.structure = true;
    Built { model: m, rig }
}

pub fn inhibitor(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let stone = kit.material(stone(team), MatOpts::rough(0.9));
    let trim = kit.material(trim(team), MatOpts::rm(0.5, 0.4));
    let c = team_hex(team);
    let g = kit.geo("ih_base", || cylinder(1.6, 1.8, 0.4, 10));
    m.part(root, g, stone.clone(), 0.0, 0.2, 0.0);
    let g = kit.geo("ih_step", || cylinder(1.2, 1.4, 0.3, 10));
    m.part(root, g, trim, 0.0, 0.55, 0.0);
    let post = kit.geo("ih_post", || super::geometry::cuboid(0.22, 1.2, 0.22));
    for i in 0..5 {
        let a = i as f32 / 5.0 * TAU;
        let p = m.part(
            root,
            post.clone(),
            stone.clone(),
            a.cos() * 1.35,
            1.0,
            a.sin() * 1.35,
        );
        m.rot(p, 0.0, -a, 0.0);
    }
    let core = m.pivot(root, 0.0, 1.7, 0.0);
    let g = kit.geo("ih_core", || icosahedron(0.55, 0));
    let glow = kit.glow(c, 2.0);
    let n = m.part(core, g, glow, 0.0, 0.0, 0.0);
    m.no_shadow(n);
    let g = kit.geo("ih_ring", || torus(0.95, 0.07, 6, 28));
    let glow = kit.glow(c, 1.2);
    let ring = m.part(core, g, glow, 0.0, 0.0, 0.0);
    m.rot(ring, PI / 2.4, 0.0, 0.0);
    let mut rig = Rig::new(2.4);
    rig.spinners = vec![core];
    rig.floaters = vec![core];
    rig.structure = true;
    Built { model: m, rig }
}

pub fn nexus(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let stone = kit.material(stone(team), MatOpts::rough(0.9));
    let trim = kit.material(trim(team), MatOpts::rm(0.45, 0.5));
    let c = team_hex(team);
    let g = kit.geo("nx_base", || cylinder(2.6, 3.0, 0.6, 12));
    m.part(root, g, stone.clone(), 0.0, 0.3, 0.0);
    let g = kit.geo("nx_step", || cylinder(2.1, 2.4, 0.5, 12));
    m.part(root, g, trim.clone(), 0.0, 0.85, 0.0);
    let g = kit.geo("nx_ped", || cylinder(1.2, 1.7, 0.9, 8));
    m.part(root, g, stone, 0.0, 1.5, 0.0);
    let spike = kit.geo("nx_spike", || cone(0.28, 2.2, 5));
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU;
        let s = m.part(
            root,
            spike.clone(),
            trim.clone(),
            a.cos() * 2.0,
            1.9,
            a.sin() * 2.0,
        );
        m.rot(s, a.sin() * 0.25, 0.0, -a.cos() * 0.25);
    }
    let core = m.pivot(root, 0.0, 3.4, 0.0);
    let g = kit.geo("nx_core", || octahedron(1.1, 0).scale(1.0, 1.6, 1.0));
    let glow = kit.glow(c, 2.0);
    let n = m.part(core, g, glow, 0.0, 0.0, 0.0);
    m.no_shadow(n);
    let orbit = m.pivot(root, 0.0, 3.4, 0.0);
    let shard = kit.geo("nx_shard", || octahedron(0.28, 0).scale(1.0, 1.8, 1.0));
    let glow = kit.glow(c, 1.4);
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU;
        let y = (i as f32 * 2.0).sin() * 0.3;
        let n = m.part(
            orbit,
            shard.clone(),
            glow.clone(),
            a.cos() * 1.8,
            y,
            a.sin() * 1.8,
        );
        m.no_shadow(n);
    }
    let mut rig = Rig::new(4.5);
    rig.spinners = vec![core, orbit];
    rig.floaters = vec![core];
    rig.structure = true;
    Built { model: m, rig }
}

pub fn ward(kit: &mut Kit, team: TeamOrNeutral) -> Built {
    let mut m = Model::new();
    let root = Model::ROOT;
    let c = team_hex(team);
    let g = kit.geo("wd_post", || cylinder(0.04, 0.06, 0.7, 5));
    let wood = kit.mat(0x5a4632);
    m.part(root, g, wood, 0.0, 0.35, 0.0);
    let lantern = m.pivot(root, 0.0, 0.8, 0.0);
    let g = kit.geo("wd_gem", || octahedron(0.14, 0).scale(1.0, 1.4, 1.0));
    let glow = kit.glow(c, 2.2);
    let n = m.part(lantern, g, glow, 0.0, 0.0, 0.0);
    m.no_shadow(n);
    let mut rig = Rig::new(0.9);
    rig.spinners = vec![lantern];
    rig.floaters = vec![lantern];
    rig.structure = true;
    Built { model: m, rig }
}
