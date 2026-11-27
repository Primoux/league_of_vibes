//! FX d'Ilyra, la Sentinelle du Givre : glace cristalline, neige, brume froide.
//!
//! | sort | FX |
//! |------|----|
//! | P Morsure du givre | flocons en orbite par cumul ; prison de glace autour de la cible étourdie, puis éclatement |
//! | Q Éclat de glace | éclat de glace qui tournoie dans un sillage de neige, se brise à l'impact |
//! | W Nova glaciale | runes de givre qui se remplissent, couronne de pics de glace |
//! | E Égide de givre | flux de givre vers l'allié, bouclier de glace avec éclats en orbite |
//! | R Blizzard | tempête de neige tournoyante, éclats qui tombent, brume glacée |
//!
//! Portage de `packages/client/src/fx/champions/ilyra.ts`.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{Effect, Fx};
use crate::fx::effects::auras::{ShellOptions, ground_mist, shield_shell};
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, ZoneOptions, between, flash, rand, rand_range,
    shockwave, telegraph, timed, zone,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};
use crate::fx::shaders::IceMaterial;

const ICE_HDR: Vec3 = Vec3::new(1.3, 2.4, 3.2);
const FROST: Vec3 = Vec3::new(0.55, 0.8, 1.0);
const MIST: Vec3 = Vec3::new(0.75, 0.88, 1.0);

/// Maillage et matériau de l'éclat du Q (octaèdre de rayon 0,16, givre).
#[derive(Resource)]
struct ShardAssets {
    mesh: Handle<Mesh>,
    material: Handle<IceMaterial>,
}

impl FromWorld for ShardAssets {
    fn from_world(world: &mut World) -> Self {
        let r = 0.16;
        let p = vec![
            [r, 0.0, 0.0],
            [-r, 0.0, 0.0],
            [0.0, r, 0.0],
            [0.0, -r, 0.0],
            [0.0, 0.0, r],
            [0.0, 0.0, -r],
        ];
        let idx = vec![
            0, 2, 4, 0, 4, 3, 0, 3, 5, 0, 5, 2, 1, 2, 5, 1, 5, 3, 1, 3, 4, 1, 4, 2,
        ];
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
        .with_inserted_indices(Indices::U32(idx));
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let material = world
            .resource_mut::<Assets<IceMaterial>>()
            .add(IceMaterial::default());
        Self { mesh, material }
    }
}

fn shatter(fx: &mut Fx, at: Vec3, n: u32, power: f32) {
    for _ in 0..n {
        let a = rand() * TAU;
        burst(
            fx,
            BurstKind::IceDebris,
            BurstSpawn {
                pos: at,
                mode: BurstMode::Debris,
                size: rand_range(0.1, 0.2),
                life: rand_range(0.6, 1.0),
                vel: Vec3::new(
                    a.cos() * rand_range(1.0, 3.0) * power,
                    rand_range(1.5, 4.0) * power,
                    a.sin() * rand_range(1.0, 3.0) * power,
                ),
                ..default()
            },
        );
    }
    let s = fx.styles();
    fx.ps().emit(
        s.snow,
        at,
        &EmitOptions {
            count: n as f32,
            shape: EmitShape::Sphere,
            radius: 0.3,
            speed: between(1.0, 3.0),
            life: [0.5, 0.9].into(),
            size: [0.12, 0.22].into(),
            spin: between(-4.0, 4.0),
            drag: 2.0,
            tint: ICE_HDR,
            ..default()
        },
    );
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: n as f32,
            shape: EmitShape::Point,
            speed: between(3.0, 6.0),
            life: [0.2, 0.35].into(),
            size: [0.04, 0.08].into(),
            stretch: 0.04,
            tint: ICE_HDR,
            ..default()
        },
    );
}

/// Particules de neige qui convergent vers un point (incantation).
fn gather(fx: &mut Fx, at: Vec3, n: u32) {
    let s = fx.styles();
    for _ in 0..n {
        let a = rand() * TAU;
        let d = rand_range(0.6, 1.0);
        let life = rand_range(0.25, 0.35);
        let v = Vec3::new(
            at.x + a.cos() * d,
            at.y + rand_range(-0.3, 0.4),
            at.z + a.sin() * d,
        );
        let w = (at - v) / life;
        fx.ps().emit(
            s.ember,
            v,
            &EmitOptions {
                count: 1.0,
                shape: EmitShape::Point,
                life: life.into(),
                size: [0.08, 0.14].into(),
                inherit: Some(w),
                tint: ICE_HDR,
                ..default()
            },
        );
    }
}

// ---------------------------------------------------------------------------------- P

#[derive(Debug)]
struct Frost {
    stacks: u32,
    until: f32,
}

/// Cumuls de givre par cible : nombre de flocons en orbite (mis à jour à chaque réapplication).
#[derive(Resource, Default)]
struct FrostStacks(HashMap<EntityId, Arc<Mutex<Frost>>>);

struct FrostFx {
    target: EntityId,
    state: Arc<Mutex<Frost>>,
    t: f32,
    acc: f32,
}

impl Effect for FrostFx {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        let now = fx.time();
        let stacks = self
            .state
            .lock()
            .ok()
            .filter(|f| now <= f.until)
            .map_or(0, |f| f.stacks);
        let Some(pos) = fx.entity_pos(self.target, 0.0).filter(|_| stacks > 0) else {
            return false;
        };
        if self.acc < 0.05 {
            return true;
        }
        self.acc = 0.0;
        let s = fx.styles();
        for i in 0..stacks {
            let a = self.t * 2.5 + i as f32 / stacks as f32 * TAU;
            let v = Vec3::new(pos.x + a.cos() * 0.55, 1.9, pos.z + a.sin() * 0.55);
            fx.ps().emit(
                s.snow,
                v,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.12.into(),
                    size: 0.2.into(),
                    spin: Some(2.0.into()),
                    tint: ICE_HDR,
                    ..default()
                },
            );
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        let mut frost = fx.world().get_resource_or_init::<FrostStacks>();
        if frost
            .0
            .get(&self.target)
            .is_some_and(|s| Arc::ptr_eq(s, &self.state))
        {
            frost.0.remove(&self.target);
        }
    }
}

fn set_stacks(fx: &mut Fx, target: EntityId, stacks: u32) {
    let frost = fx.world().get_resource_or_init::<FrostStacks>();
    if let Some(s) = frost.0.get(&target)
        && let Ok(mut f) = s.lock()
    {
        f.stacks = stacks;
    }
}

fn p_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        stacks,
        ..
    } = e
    else {
        return;
    };
    if buff_id == "stun" {
        ice_prison(fx, *target, *duration as f32);
        return;
    }
    if buff_id != "ilyra_frost" {
        return;
    }
    let until = fx.time() + (*duration as f32).max(0.5);
    {
        let frost = fx.world().get_resource_or_init::<FrostStacks>();
        if let Some(prev) = frost.0.get(target)
            && let Ok(mut f) = prev.lock()
        {
            f.stacks = stacks.unwrap_or(f.stacks + 1);
            f.until = until;
            return;
        }
    }
    let state = Arc::new(Mutex::new(Frost {
        stacks: stacks.unwrap_or(1),
        until,
    }));
    fx.world()
        .get_resource_or_init::<FrostStacks>()
        .0
        .insert(*target, state.clone());
    fx.add(FrostFx {
        target: *target,
        state,
        t: 0.0,
        acc: 0.0,
    });
}

fn p_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "ilyra_frost"
    {
        set_stacks(fx, *target, 0);
    }
}

/// Proc du passif : la cible est figée dans la glace (étourdissement), puis la glace éclate.
fn ice_prison(fx: &mut Fx, target: EntityId, duration: f32) {
    let Some(pos) = fx.entity_pos(target, 0.0) else {
        return;
    };
    set_stacks(fx, target, 0); // les cumuls sont consommés
    let life = (if duration == 0.0 { 1.0 } else { duration }).clamp(0.6, 2.0);
    let n = 7;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU + rand_range(-0.2, 0.2);
        burst(
            fx,
            BurstKind::IceSpikes,
            BurstSpawn {
                pos: Vec3::new(pos.x + a.cos() * 0.38, 0.0, pos.z + a.sin() * 0.38),
                mode: BurstMode::Static,
                size: rand_range(0.55, 0.75),
                life,
                heading: Some(a.cos().atan2(a.sin()) + PI),
                tilt: Some(-0.3),
                stretch: rand_range(1.6, 2.4),
                ..default()
            },
        );
    }
    burst(
        fx,
        BurstKind::IceSpikes,
        BurstSpawn {
            pos,
            mode: BurstMode::Static,
            size: 0.6,
            life,
            stretch: 2.6,
            ..default()
        },
    );
    ground_mist(fx, pos, 1.0, MIST, 6.0);
    fx.after(life, move |fx| shatter(fx, pos.with_y(1.0), 16, 1.0));
}

// ---------------------------------------------------------------------------------- Q

fn q_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast { source, .. } = e
        && let Some(v) = fx.entity_pos(*source, 1.2)
    {
        gather(fx, v, 10);
    }
}

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let (mesh, material) = {
        let a = fx.world().get_resource_or_init::<ShardAssets>();
        (a.mesh.clone(), a.material.clone())
    };
    let shard = fx
        .world()
        .spawn((
            Name::new("fx-ilyra-shard"),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::new(0.8, 0.8, 2.6)),
            Visibility::Hidden,
        ))
        .id();
    let snow = fx.styles().snow;
    let mut spin = 0.0;
    let stop = ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: FROST * 1.6,
            core: Some(ICE_HDR),
            size: 0.55,
            height: 1.1,
            trail_style: Some(snow),
            trail_density: 6.0,
            trail_size: [0.1, 0.18],
            trail_life: [0.4, 0.7],
            ribbon_width: 0.16,
            ribbon_life: 0.3,
            on_step: Some(Box::new(move |fx, head, dir, dt| {
                spin += dt * 14.0;
                if let Ok(mut e) = fx.world().get_entity_mut(shard) {
                    if let Some(mut tf) = e.get_mut::<Transform>() {
                        tf.translation = head;
                        tf.rotation = Transform::IDENTITY.looking_to(dir, Vec3::Y).rotation
                            * Quat::from_rotation_z(spin);
                    }
                    if let Some(mut v) = e.get_mut::<Visibility>() {
                        v.set_if_neq(Visibility::Inherited);
                    }
                }
            })),
            on_end: Some(Box::new(move |fx, _| {
                if let Ok(e) = fx.world().get_entity_mut(shard) {
                    e.despawn();
                }
            })),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::ProjectileHit { pos, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 1.0);
    shatter(fx, at, 12, 0.8);
    flash(fx, at, 1.6, ICE_HDR, 0.2);
    shockwave(fx, at.with_y(0.0), 0.9, 0.3, ICE_HDR, 0.08);
    ground_mist(fx, at.with_y(0.0), 0.7, MIST, 4.0);
}

// ---------------------------------------------------------------------------------- W

fn w_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast { source, .. } = e
        && let Some(v) = fx.entity_pos(*source, 1.4)
    {
        gather(fx, v, 8);
    }
}

/// Détonation de la nova.
fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        nova(fx, Fx::world_pos(*pos, 0.0), Fx::scale(*radius as f32));
    }
}

/// Pendant le délai : runes de givre qui se remplissent, flocons qui se figent au sol.
fn w_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    let delay = (*duration as f32).max(0.15);
    telegraph(
        fx,
        center,
        r,
        delay,
        FROST,
        TelegraphOptions {
            color2: Some(ICE_HDR),
            fill: Some(0.16),
            ..default()
        },
    );
    let snow = fx.styles().snow;
    timed(
        fx,
        delay,
        move |fx, _, _| {
            if rand() < 0.6 {
                fx.ps().emit(
                    snow,
                    center,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Disc,
                        radius: r * 0.9,
                        up: between(0.1, 0.4),
                        life: [0.4, 0.7].into(),
                        size: [0.12, 0.2].into(),
                        spin: between(-2.0, 2.0),
                        tint: ICE_HDR,
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

fn nova(fx: &mut Fx, center: Vec3, r: f32) {
    shockwave(fx, center, r * 1.1, 0.4, ICE_HDR, 0.16);
    flash(fx, center.with_y(0.6), r * 1.2, FROST, 0.3);
    let n = 12;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU + rand_range(-0.12, 0.12);
        let d = r * rand_range(0.5, 0.85);
        burst(
            fx,
            BurstKind::IceSpikes,
            BurstSpawn {
                pos: Vec3::new(center.x + a.cos() * d, 0.0, center.z + a.sin() * d),
                mode: BurstMode::Spike,
                size: rand_range(0.4, 0.6),
                life: rand_range(0.8, 1.1),
                heading: Some(a.cos().atan2(a.sin()) + PI),
                tilt: Some(-0.45),
                stretch: rand_range(1.2, 1.8),
                ..default()
            },
        );
    }
    shatter(fx, center.with_y(0.4), 14, 1.2);
    ground_mist(fx, center, r, MIST, 12.0);
}

// ---------------------------------------------------------------------------------- E

/// Flux de givre vers l'allié protégé.
fn e_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast {
        source, target_id, ..
    } = e
    else {
        return;
    };
    let Some(from) = fx.entity_pos(*source, 1.2) else {
        return;
    };
    gather(fx, from, 6);
    let Some(target) = target_id.filter(|t| t != source) else {
        return;
    };
    let to = fx.entity_pos(target, 1.1).unwrap_or(from);
    let sparkles = fx.styles().sparkles;
    let _ = ProjectileFx::launch(
        fx,
        from,
        to,
        18.0,
        ProjectileLook {
            color: FROST * 1.8,
            core: Some(ICE_HDR),
            size: 0.4,
            height: 1.1,
            trail_style: Some(sparkles),
            trail_density: 5.0,
            trail_size: [0.2, 0.3],
            ribbon_width: 0.22,
            ribbon_life: 0.35,
            target: Some(target),
            ..default()
        },
    );
}

/// Le bouclier lui-même (buff `ilyra_e`), jusqu'à expiration ou rupture.
fn e_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "ilyra_e" {
        return;
    }
    let snow = fx.styles().snow;
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.3),
        &format!("ilyra_e:{target}"),
        ShellOptions {
            color: Vec3::new(0.5, 0.95, 1.4),
            radius: 1.05,
            bands: 0.25,
            on_step: Some(Box::new(move |fx, center, t, _| {
                for i in 0..3 {
                    let a = t * 2.4 + i as f32 / 3.0 * TAU;
                    let orbit = Vec3::new(
                        center.x + a.cos() * 1.05,
                        center.y + (t * 3.0 + i as f32).sin() * 0.3,
                        center.z + a.sin() * 1.05,
                    );
                    fx.ps().emit(
                        snow,
                        orbit,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Point,
                            life: 0.25.into(),
                            size: 0.18.into(),
                            spin: Some(3.0.into()),
                            tint: ICE_HDR,
                            ..default()
                        },
                    );
                }
            })),
            on_end: Some(Box::new(|fx, center| shatter(fx, center, 10, 0.7))),
            ..default()
        },
    );
    if let Some(v) = fx.entity_pos(*target, 1.0) {
        flash(fx, v, 2.0, ICE_HDR, 0.25);
    }
}

fn e_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "ilyra_e"
    {
        fx.end_aura(&format!("ilyra_e:{target}"));
    }
}

// ---------------------------------------------------------------------------------- R

fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast { source, .. } = e
        && let Some(v) = fx.entity_pos(*source, 2.4)
    {
        gather(fx, v, 16);
        flash(fx, v, 2.4, ICE_HDR, 0.4);
    }
}

fn r_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    let duration = (*duration as f32).max(0.5);
    zone(
        fx,
        center,
        r,
        duration,
        FROST * 1.3,
        ZoneOptions {
            fill: Some(0.14),
            pattern: Some(0.7),
            ..default()
        },
    );
    ground_mist(fx, center, r, MIST, 14.0);
    let s = fx.styles();
    let mut acc = 0.0;
    let mut shard_acc = 0.0;
    timed(
        fx,
        duration,
        move |fx, t, dt| {
            let fade = ((1.0 - t) * duration * 2.0).min(1.0);
            // neige tournoyante : vitesse tangentielle selon la position (tourbillon)
            acc += dt * 90.0 * fade;
            while acc >= 1.0 {
                acc -= 1.0;
                let a = rand() * TAU;
                let d = rand().sqrt() * r;
                let v = Vec3::new(
                    center.x + a.cos() * d,
                    rand_range(0.2, 2.2),
                    center.z + a.sin() * d,
                );
                let wind = Vec3::new(-a.sin() * 3.5, -0.6, a.cos() * 3.5);
                let style = if rand() < 0.75 { s.snow } else { s.spark };
                fx.ps().emit(
                    style,
                    v,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        life: [0.5, 0.9].into(),
                        size: [0.1, 0.2].into(),
                        inherit: Some(wind),
                        spin: between(-5.0, 5.0),
                        stretch: 0.03,
                        tint: ICE_HDR,
                        intensity: 0.8,
                        ..default()
                    },
                );
            }
            // éclats de glace qui tombent du ciel
            shard_acc += dt * 7.0 * fade;
            while shard_acc >= 1.0 {
                shard_acc -= 1.0;
                let a = rand() * TAU;
                let d = rand().sqrt() * r * 0.9;
                burst(
                    fx,
                    BurstKind::IceDebris,
                    BurstSpawn {
                        pos: Vec3::new(
                            center.x + a.cos() * d,
                            rand_range(3.0, 4.5),
                            center.z + a.sin() * d,
                        ),
                        mode: BurstMode::Debris,
                        size: rand_range(0.16, 0.26),
                        life: 1.1,
                        vel: Vec3::new(rand_range(-0.5, 0.5), -6.0, rand_range(-0.5, 0.5)),
                        ..default()
                    },
                );
            }
            if rand() < dt * 3.0 {
                ground_mist(fx, center, r * 0.8, MIST, 2.0);
            }
        },
        |_| {},
    );
}

pub fn register(app: &mut App) {
    app.init_resource::<FrostStacks>()
        .register_fx(
            "ilyra_p",
            FxHandlers {
                buff: Some(p_buff),
                buff_end: Some(p_buff_end),
                ..default()
            },
        )
        .register_fx(
            "ilyra_q",
            FxHandlers {
                cast: Some(q_cast),
                projectile: Some(q_projectile),
                hit: Some(q_hit),
                ..default()
            },
        )
        .register_fx(
            "ilyra_w",
            FxHandlers {
                cast: Some(w_cast),
                area: Some(w_area),
                ..default()
            },
        )
        .register_fx(
            "ilyra_w_telegraph",
            FxHandlers {
                area: Some(w_telegraph),
                ..default()
            },
        )
        .register_fx(
            "ilyra_e",
            FxHandlers {
                cast: Some(e_cast),
                buff: Some(e_buff),
                buff_end: Some(e_buff_end),
                ..default()
            },
        )
        .register_fx(
            "ilyra_r",
            FxHandlers {
                cast: Some(r_cast),
                area: Some(r_area),
                ..default()
            },
        );
}

#[cfg(test)]
mod tests {
    use lov_shared::types::Vec2;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn l_eclat_du_q_disparait_a_l_arrivee() {
        let mut w = fx_world();
        let c = FxRuntime::default().colors("ilyra_q");
        let e = GameEvent::ProjectileSpawn {
            id: 2,
            source: 1,
            fx_key: "ilyra_q".into(),
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 300.0, y: 0.0 },
            speed: 3000.0,
            width: 80.0,
        };
        assert!(q_projectile(&mut Fx::new(&mut w), &e, &c).is_some());
        let shards = |w: &mut World| w.query::<&MeshMaterial3d<IceMaterial>>().iter(w).count();
        assert_eq!(shards(&mut w), 1);
        for _ in 0..10 {
            FxRuntime::update(&mut w, 0.05);
        }
        assert_eq!(shards(&mut w), 0);
    }
}
