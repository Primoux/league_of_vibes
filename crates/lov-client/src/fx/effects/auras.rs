//! Effets persistants attachés à une entité : coque de bouclier (fresnel), étoiles d'étourdissement, brume.
//! Annulables par clé (`fx.end_aura(key)`, typiquement sur `buffRemoved`).
//!
//! Portage de `packages/client/src/fx/effects/auras.ts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::types::EntityId;

use super::primitives::between;
use crate::fx::core::runtime::{AuraId, Effect, Fx};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::shaders::FresnelMaterial;

/// Appelé à chaque image avec le centre de la coque, le temps écoulé et le pas.
pub type ShellStep = Box<dyn FnMut(&mut Fx, Vec3, f32, f32) + Send + Sync>;
/// Appelé quand la coque commence à disparaître, avec son centre.
pub type ShellEnd = Box<dyn FnOnce(&mut Fx, Vec3) + Send + Sync>;

/// Options d'une coque de bouclier (`ShellOptions`).
pub struct ShellOptions {
    pub color: Vec3,
    pub radius: f32,
    pub height: f32,
    pub bands: f32,
    pub power: f32,
    pub on_step: Option<ShellStep>,
    pub on_end: Option<ShellEnd>,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            radius: 0.95,
            height: 1.1,
            bands: 0.15,
            power: 2.2,
            on_step: None,
            on_end: None,
        }
    }
}

/// Maillage partagé des coques (icosphère).
#[derive(Resource)]
struct ShellMesh(Handle<Mesh>);

impl FromWorld for ShellMesh {
    fn from_world(world: &mut World) -> Self {
        let mesh = Sphere::new(1.0)
            .mesh()
            .ico(3)
            .unwrap_or_else(|_| Sphere::new(1.0).mesh().uv(24, 16));
        Self(world.resource_mut::<Assets<Mesh>>().add(mesh))
    }
}

fn shell_mesh(world: &mut World) -> Handle<Mesh> {
    world.get_resource_or_init::<ShellMesh>().0.clone()
}

/// Coque de bouclier qui suit `entity` pendant `duration` s (ou jusqu'à `fx.end_aura(key)`).
pub fn shield_shell(fx: &mut Fx, entity: EntityId, duration: f32, key: &str, o: ShellOptions) {
    let mesh = shell_mesh(fx.world());
    let mut mat = FresnelMaterial::new(o.color);
    mat.u.power = o.power;
    mat.u.base = 0.03;
    mat.u.bands = o.bands;
    mat.u.opacity = 0.0;
    let material = fx
        .world()
        .resource_mut::<Assets<FresnelMaterial>>()
        .add(mat);
    let r = o.radius;
    let scale = Vec3::new(r, o.height * r, r);
    let mesh_entity = fx
        .world()
        .spawn((
            Name::new("fx-shield"),
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(scale),
            Visibility::Hidden,
        ))
        .id();
    let ending = Arc::new(AtomicBool::new(false));
    let flag = ending.clone();
    let id = fx.set_aura(key, move |_| flag.store(true, Ordering::Relaxed));
    fx.add(Shell {
        key: key.to_owned(),
        aura: id,
        entity,
        mesh: mesh_entity,
        material,
        scale,
        r,
        t: 0.0,
        duration,
        ending,
        fade: None,
        center: Vec3::ZERO,
        on_step: o.on_step,
        on_end: o.on_end,
    });
}

struct Shell {
    key: String,
    aura: AuraId,
    entity: EntityId,
    mesh: Entity,
    material: Handle<FresnelMaterial>,
    scale: Vec3,
    r: f32,
    t: f32,
    duration: f32,
    ending: Arc<AtomicBool>,
    /// Temps écoulé depuis le début de la disparition.
    fade: Option<f32>,
    center: Vec3,
    on_step: Option<ShellStep>,
    on_end: Option<ShellEnd>,
}

impl Shell {
    fn set_opacity(&self, fx: &mut Fx, opacity: f32) {
        if let Some(mut m) = fx
            .world()
            .resource_mut::<Assets<FresnelMaterial>>()
            .get_mut(&self.material)
        {
            m.u.opacity = opacity;
        }
    }
}

impl Effect for Shell {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        if self.t >= self.duration {
            self.ending.store(true, Ordering::Relaxed);
        }
        if self.fade.is_none() && self.ending.load(Ordering::Relaxed) {
            self.fade = Some(0.0);
            if let Some(end) = self.on_end.take() {
                end(fx, self.center);
            }
        }
        if let Some(p) = fx.entity_pos(self.entity, self.r * 0.9) {
            self.center = p;
        }
        if let Some(f) = self.fade.as_mut() {
            *f += dt;
            let f = *f;
            self.scale *= 1.0 + dt * 1.5;
            self.set_opacity(fx, (1.0 - f / 0.2).max(0.0));
            if let Some(mut tf) = fx.world().get_mut::<Transform>(self.mesh) {
                tf.translation = self.center;
                tf.scale = self.scale;
            }
            return f < 0.2;
        }
        self.set_opacity(fx, (self.t / 0.15).min(1.0));
        let (center, scale) = (self.center, self.scale);
        if let Ok(mut e) = fx.world().get_entity_mut(self.mesh) {
            if let Some(mut tf) = e.get_mut::<Transform>() {
                tf.translation = center;
                tf.scale = scale;
            }
            if let Some(mut v) = e.get_mut::<Visibility>() {
                v.set_if_neq(Visibility::Inherited);
            }
        }
        if let Some(step) = self.on_step.as_mut() {
            step(fx, center, self.t, dt);
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.forget_aura(&self.key, self.aura);
        let world = fx.world();
        if let Ok(e) = world.get_entity_mut(self.mesh) {
            e.despawn();
        }
        world
            .resource_mut::<Assets<FresnelMaterial>>()
            .remove(&self.material);
    }
}

/// Couleur des étoiles d'étourdissement.
pub const STAR: Vec3 = Vec3::new(2.4, 2.1, 0.8);

/// Étoiles qui tournent au-dessus de la tête (étourdissement).
pub fn stun_stars(fx: &mut Fx, entity: EntityId, duration: f32, height: f32) {
    fx.add(Stars {
        entity,
        duration,
        height,
        t: 0.0,
        acc: 0.0,
    });
}

struct Stars {
    entity: EntityId,
    duration: f32,
    height: f32,
    t: f32,
    acc: f32,
}

impl Effect for Stars {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        if self.acc > 0.05
            && let Some(pos) = fx.entity_pos(self.entity, self.height)
        {
            self.acc = 0.0;
            let s = fx.styles();
            for i in 0..3 {
                let a = self.t * 5.0 + i as f32 / 3.0 * std::f32::consts::TAU;
                let p = Vec3::new(pos.x + a.cos() * 0.45, pos.y, pos.z + a.sin() * 0.45);
                fx.ps().emit(
                    s.star,
                    p,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        life: 0.12.into(),
                        size: 0.22.into(),
                        tint: STAR,
                        ..default()
                    },
                );
            }
        }
        self.t < self.duration
    }
}

/// Brume basse (mélange normal) qui s'étale autour d'un point.
pub fn ground_mist(fx: &mut Fx, at: Vec3, radius: f32, color: Vec3, count: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.smoke,
        at,
        &EmitOptions {
            count,
            shape: EmitShape::Disc,
            radius: radius * 0.6,
            speed: between(0.3, 1.0),
            up: between(0.05, 0.3),
            life: [0.9, 1.5].into(),
            size: [radius * 0.5, radius * 0.9].into(),
            tint: color,
            drag: 1.5,
            ..default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    fn shells(w: &mut World) -> usize {
        w.query::<&MeshMaterial3d<FresnelMaterial>>()
            .iter(w)
            .count()
    }

    #[test]
    fn bouclier_termine_par_sa_cle_puis_retire() {
        let mut w = fx_world();
        shield_shell(
            &mut Fx::new(&mut w),
            1,
            10.0,
            "bouclier:1",
            ShellOptions::default(),
        );
        FxRuntime::update(&mut w, 0.1);
        assert_eq!(shells(&mut w), 1);
        Fx::new(&mut w).end_aura("bouclier:1");
        for _ in 0..10 {
            FxRuntime::update(&mut w, 0.05);
        }
        assert_eq!(shells(&mut w), 0, "coque retirée après le fondu");
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
        assert_eq!(w.resource::<Assets<FresnelMaterial>>().len(), 0);
    }

    #[test]
    fn bouclier_expire_seul() {
        let mut w = fx_world();
        shield_shell(&mut Fx::new(&mut w), 1, 0.3, "b", ShellOptions::default());
        for _ in 0..20 {
            FxRuntime::update(&mut w, 0.05);
        }
        assert_eq!(shells(&mut w), 0);
    }
}
