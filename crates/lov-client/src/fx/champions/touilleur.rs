//! FX du Touilleur : moulinets magiques et étoiles burlesques, liés aux impacts et buffs réels du serveur.
//!
//! | sort | événements |
//! |------|------------|
//! | P | dégâts `touilleur_p` : éclat et tourbillons |
//! | Q | projectile `touilleur_q` : un moulinet file vers la cible |
//! | W | zone `touilleur_w_telegraph` puis `touilleur_w` : grand moulinet au sol |
//! | E | dash `touilleur_e` |
//! | R | dash `touilleur_r`, buff stun `touilleur_r_stun` : moulinet devant la cible ; canal `touilleur_r_channel` |
//!
//! Portage de `packages/client/src/fx/champions/touilleur.ts`.

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::{FxColors, linear};
use crate::fx::core::runtime::{AuraId, Effect, Fx};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, dash_trail, flash, shockwave, telegraph, timed,
};
use crate::fx::particles::EmitOptions;
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

fn mint() -> Vec3 {
    linear("#8ef4d2")
}

fn brass() -> Vec3 {
    linear("#d29a49")
}

/// Point d'une Catmull-Rom centripète (`CatmullRomCurve3` de three.js) à `t ∈ 0..1`.
fn catmull_rom(points: &[Vec3], t: f32) -> Vec3 {
    let l = points.len();
    let p = (l - 1) as f32 * t;
    let mut i = p.floor() as usize;
    let mut w = p - i as f32;
    if i >= l - 1 {
        i = l - 2;
        w = 1.0;
    }
    let (p1, p2) = (points[i], points[i + 1]);
    let p0 = if i > 0 { points[i - 1] } else { p1 * 2.0 - p2 };
    let p3 = if i + 2 < l {
        points[i + 2]
    } else {
        p2 * 2.0 - p1
    };
    let dt = |a: Vec3, b: Vec3| a.distance_squared(b).powf(0.25);
    let (mut dt0, mut dt1, mut dt2) = (dt(p0, p1), dt(p1, p2), dt(p2, p3));
    if dt1 < 1e-4 {
        dt1 = 1.0;
    }
    if dt0 < 1e-4 {
        dt0 = dt1;
    }
    if dt2 < 1e-4 {
        dt2 = dt1;
    }
    let comp = |x0: f32, x1: f32, x2: f32, x3: f32| {
        let t1 = ((x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1) * dt1;
        let t2 = ((x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2) * dt1;
        let c2 = -3.0 * x1 + 3.0 * x2 - 2.0 * t1 - t2;
        let c3 = 2.0 * x1 - 2.0 * x2 + t1 + t2;
        x1 + t1 * w + c2 * w * w + c3 * w * w * w
    };
    Vec3::new(
        comp(p0.x, p1.x, p2.x, p3.x),
        comp(p0.y, p1.y, p2.y, p3.y),
        comp(p0.z, p1.z, p2.z, p3.z),
    )
}

/// Géométrie du moulinet : spirale de 2¼ tours qui s'élargit, en tube fin (`moulinetGeometry`).
pub fn moulinet_geometry() -> Mesh {
    let points: Vec<Vec3> = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.0;
            let a = t * PI * 4.5;
            let r = 0.04 + t * 0.35;
            Vec3::new(a.cos() * r, 0.04 * t, a.sin() * r)
        })
        .collect();
    const SEGMENTS: usize = 40;
    const RADIAL: usize = 5;
    const RADIUS: f32 = 0.018;
    let path: Vec<Vec3> = (0..=SEGMENTS)
        .map(|k| catmull_rom(&points, k as f32 / SEGMENTS as f32))
        .collect();
    // repères par transport parallèle (pas de torsion le long de la spirale)
    let tangent =
        |k: usize| (path[(k + 1).min(SEGMENTS)] - path[k.saturating_sub(1)]).normalize_or(Vec3::X);
    let mut normal = tangent(0).any_orthonormal_vector();
    let mut positions = Vec::with_capacity((SEGMENTS + 1) * RADIAL);
    let mut normals = Vec::with_capacity((SEGMENTS + 1) * RADIAL);
    for (k, p) in path.iter().enumerate() {
        let t = tangent(k);
        normal = (normal - t * normal.dot(t)).normalize_or(t.any_orthonormal_vector());
        let binormal = t.cross(normal);
        for r in 0..RADIAL {
            let a = r as f32 / RADIAL as f32 * std::f32::consts::TAU;
            let n = normal * a.cos() + binormal * a.sin();
            positions.push((*p + n * RADIUS).to_array());
            normals.push(n.to_array());
        }
    }
    let mut indices = Vec::with_capacity(SEGMENTS * RADIAL * 6);
    for k in 0..SEGMENTS {
        for r in 0..RADIAL {
            let a = (k * RADIAL + r) as u32;
            let b = (k * RADIAL + (r + 1) % RADIAL) as u32;
            let (c, d) = (a + RADIAL as u32, b + RADIAL as u32);
            indices.extend([a, c, b, b, c, d]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Maillage partagé des moulinets.
#[derive(Resource)]
struct MoulinetMesh(Handle<Mesh>);

impl FromWorld for MoulinetMesh {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource_mut::<Assets<Mesh>>()
                .add(moulinet_geometry()),
        )
    }
}

/// Un moulinet affiché : entité et matériau propre (opacité).
#[derive(Clone)]
struct Spiral {
    entity: Entity,
    material: Handle<StandardMaterial>,
    disposed: Arc<AtomicBool>,
}

impl Spiral {
    fn spawn(fx: &mut Fx, at: Vec3, scale: f32) -> Spiral {
        let mesh = fx.world().get_resource_or_init::<MoulinetMesh>().0.clone();
        let c = mint() * 1.65;
        let material =
            fx.world()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, 1.0)),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
        let entity = fx
            .world()
            .spawn((
                Name::new("fx-touilleur-moulinet"),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(scale)),
            ))
            .id();
        Spiral {
            entity,
            material,
            disposed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn set(&self, fx: &mut Fx, f: impl FnOnce(&mut Transform)) {
        if let Some(mut tf) = fx.world().get_mut::<Transform>(self.entity) {
            f(&mut tf);
        }
    }

    fn opacity(&self, fx: &mut Fx, o: f32) {
        if let Some(mut m) = fx
            .world()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&self.material)
        {
            m.base_color.set_alpha(o.clamp(0.0, 1.0));
        }
    }

    /// Destruction idempotente.
    fn dispose(&self, fx: &mut Fx) {
        if self.disposed.swap(true, Ordering::Relaxed) {
            return;
        }
        let world = fx.world();
        if let Ok(e) = world.get_entity_mut(self.entity) {
            e.despawn();
        }
        world
            .resource_mut::<Assets<StandardMaterial>>()
            .remove(&self.material);
    }
}

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let from = Fx::world_pos(*from, 1.0);
    let spiral = Spiral::spawn(fx, from, 0.85);
    let s2 = spiral.clone();
    let stop = ProjectileFx::launch(
        fx,
        from,
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: mint(),
            size: 0.18,
            height: 1.0,
            ribbon_width: 0.09,
            on_step: Some(Box::new(move |fx, pos, _, _| {
                let time = fx.time();
                spiral.set(fx, |tf| {
                    tf.translation = pos;
                    tf.rotation = Quat::from_rotation_y(time * 15.0);
                });
            })),
            on_end: Some(Box::new(move |fx, _| s2.dispose(fx))),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        flash(fx, Fx::world_pos(*pos, 1.0), 0.8, mint(), 0.25);
        shockwave(fx, Fx::world_pos(*pos, 0.0), 0.7, 0.3, mint(), 0.12);
    }
}

fn w_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    {
        telegraph(
            fx,
            Fx::world_pos(*pos, 0.0),
            Fx::scale(*radius as f32),
            *duration as f32,
            mint(),
            TelegraphOptions {
                fill: Some(0.07),
                ..default()
            },
        );
    }
}

fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect { pos, radius, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.08);
    let r = Fx::scale(*radius as f32);
    let spiral = Spiral::spawn(fx, at, r * 2.0);
    shockwave(fx, at, r, 0.55, brass(), 0.12);
    let s2 = spiral.clone();
    timed(
        fx,
        0.65,
        move |fx, t, _| {
            spiral.set(fx, |tf| tf.rotation = Quat::from_rotation_y(t * 13.0));
            spiral.opacity(fx, 1.0 - t);
        },
        move |fx| s2.dispose(fx),
    );
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash {
        source, from, to, ..
    } = e
    {
        dash_trail(
            fx,
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            0.15,
            mint(),
            false,
        );
        flash(fx, Fx::world_pos(*to, 0.7), 1.0, brass(), 0.25);
    }
}

fn r_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash {
        source, from, to, ..
    } = e
    {
        dash_trail(
            fx,
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            0.18,
            brass(),
            false,
        );
    }
}

/// Moulinet devant la cible étourdie par l'ultime, jusqu'à la fin du stun.
struct StunSpiral {
    key: String,
    aura: AuraId,
    target: EntityId,
    spiral: Spiral,
    ended: Arc<AtomicBool>,
    duration: f32,
    age: f32,
}

impl Effect for StunSpiral {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.age += dt;
        if self.ended.load(Ordering::Relaxed) || self.age >= self.duration {
            return false;
        }
        let info = fx
            .entity(self.target)
            .map(|i| (i.dead == Some(true), i.facing));
        if info.is_some_and(|(dead, _)| dead) {
            return false;
        }
        let Some(mut p) = fx.entity_pos(self.target, 1.4) else {
            return false;
        };
        let facing = fx
            .world()
            .get_resource::<crate::net::GameState>()
            .and_then(|s| s.pose(self.target))
            .map_or(info.map_or(0.0, |i| i.1), |p| p.facing) as f32;
        p.x -= facing.cos() * 0.35;
        p.z += facing.sin() * 0.35;
        let age = self.age;
        self.spiral.set(fx, |tf| {
            tf.translation = p;
            tf.rotation = Quat::from_euler(EulerRot::XYZ, FRAC_PI_2, age * 12.0, -facing);
        });
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        self.spiral.dispose(fx);
        fx.forget_aura(&self.key, self.aura);
    }
}

fn r_stun_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target, duration, ..
    } = e
    else {
        return;
    };
    let key = format!("touilleur:r:{target}");
    let spiral = Spiral::spawn(fx, Vec3::ZERO, 1.25);
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    fx.add(StunSpiral {
        key,
        aura,
        target: *target,
        spiral,
        ended,
        duration: *duration as f32,
        age: 0.0,
    });
}

fn r_stun_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved { target, .. } = e {
        fx.end_aura(&format!("touilleur:r:{target}"));
    }
}

/// L'index du modèle est déjà animé ; le buff de la cible possède le moulinet.
fn nothing(_: &mut Fx, _: &GameEvent, _: &FxColors) {}

fn p_hit(fx: &mut Fx, e: &GameEvent) {
    if let GameEvent::Damage { target, .. } = e
        && let Some(at) = fx.entity_pos(*target, 1.4)
    {
        flash(fx, at, 1.2, brass(), 0.25);
        let s = fx.styles();
        fx.ps().emit(
            s.swirl,
            at,
            &EmitOptions {
                count: 3.0,
                life: [0.3, 0.5].into(),
                size: [0.4, 0.7].into(),
                spin: Some(9.0.into()),
                tint: mint(),
                ..default()
            },
        );
    }
}

pub fn register(app: &mut App) {
    app.register_fx(
        "touilleur_q",
        FxHandlers {
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "touilleur_w_telegraph",
        FxHandlers {
            area: Some(w_telegraph),
            ..default()
        },
    )
    .register_fx(
        "touilleur_w",
        FxHandlers {
            area: Some(w_area),
            ..default()
        },
    )
    .register_fx(
        "touilleur_e",
        FxHandlers {
            dash: Some(e_dash),
            ..default()
        },
    )
    .register_fx(
        "touilleur_r",
        FxHandlers {
            dash: Some(r_dash),
            ..default()
        },
    )
    .register_fx(
        "touilleur_r_stun",
        FxHandlers {
            buff: Some(r_stun_buff),
            buff_end: Some(r_stun_end),
            ..default()
        },
    )
    .register_fx(
        "touilleur_r_channel",
        FxHandlers {
            buff: Some(nothing),
            buff_end: Some(nothing),
            ..default()
        },
    )
    .on_ability_hit("touilleur_p", p_hit);
}

/// Port de la partie « moulinets FX » de `packages/client/test/touilleur-model.test.ts`.
#[cfg(test)]
mod tests {
    use bevy::mesh::VertexAttributeValues;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;
    use crate::fx::keys::FxPhase;
    use crate::fx::registry::FxRegistry;
    use crate::net::GameState;
    use crate::net::snapshots::Pose;

    #[test]
    fn dispose_de_handlers_pour_les_vrais_evenements_de_chaque_sort() {
        let mut app = App::new();
        register(&mut app);
        let reg = app.world().resource::<FxRegistry>();
        for (key, phase) in [
            ("touilleur_q", FxPhase::Projectile),
            ("touilleur_q", FxPhase::Hit),
            ("touilleur_w_telegraph", FxPhase::Area),
            ("touilleur_w", FxPhase::Area),
            ("touilleur_e", FxPhase::Dash),
            ("touilleur_r", FxPhase::Dash),
            ("touilleur_r_stun", FxPhase::Buff),
            ("touilleur_r_stun", FxPhase::BuffEnd),
        ] {
            assert!(
                reg.get(key).is_some_and(|h| h.has(phase)),
                "{key} {phase:?}"
            );
        }
        let g = moulinet_geometry();
        let Some(VertexAttributeValues::Float32x3(p)) = g.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        assert!(p.iter().flatten().all(|v| v.is_finite()));
        assert!(g.indices().map_or(0, |i| i.len()) / 3 < 500);
    }

    #[test]
    fn arrete_le_moulinet_a_la_suppression_du_stun_et_libere_ses_ressources_une_seule_fois() {
        let mut w = fx_world();
        let mut state = GameState::default();
        state.poses.insert(
            1,
            Pose {
                x: 0.0,
                y: 0.0,
                facing: 0.0,
            },
        );
        w.insert_resource(state);
        let c = FxRuntime::default().colors("touilleur_r_stun");
        let applied = GameEvent::BuffApplied {
            target: 1,
            buff_id: "stun".into(),
            duration: 2.0,
            source: Some(2),
            fx_key: Some("touilleur_r_stun".into()),
            stacks: None,
        };
        r_stun_buff(&mut Fx::new(&mut w), &applied, &c);
        let spirals = |w: &mut World| w.query::<&Name>().iter(w).count();
        FxRuntime::update(&mut w, 0.1);
        assert_eq!(spirals(&mut w), 1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 1);
        let removed = GameEvent::BuffRemoved {
            target: 1,
            buff_id: "stun".into(),
            fx_key: Some("touilleur_r_stun".into()),
        };
        r_stun_end(&mut Fx::new(&mut w), &removed, &c);
        FxRuntime::update(&mut w, 0.1);
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
        assert_eq!(spirals(&mut w), 0);
        assert_eq!(
            w.resource::<Assets<StandardMaterial>>().len(),
            0,
            "matériau libéré"
        );
        // une seconde fin n'a plus rien à libérer
        r_stun_end(&mut Fx::new(&mut w), &removed, &c);
        FxRuntime::update(&mut w, 0.1);
    }
}
