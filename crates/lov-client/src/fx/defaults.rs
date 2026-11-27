//! FX génériques, aux couleurs de la palette du sort. Ils s'appliquent à toute fxKey sans handler dédié,
//! pour que chaque sort ait dès le départ une anticipation, un projectile, un impact et une zone lisibles.
//!
//! Portage de `packages/client/src/fx/defaults.ts`.

use bevy::prelude::*;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::Fx;
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, ZoneEmit, ZoneOptions, between, dash_trail,
    flash, shockwave, telegraph, zone,
};
use crate::fx::keys::is_telegraph_key;
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{AllHandlers, ProjectileHandle};

/// Petite explosion colorée : éclat, étincelles, onde. Réutilisée par plusieurs FX.
pub fn impact_burst(fx: &mut Fx, at: Vec3, c: &FxColors, scale: f32) {
    let q = at.with_y(1.0);
    flash(fx, q, 1.6 * scale, c.accent_hdr, 0.22);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        q,
        &EmitOptions {
            count: 14.0 * scale,
            shape: EmitShape::Point,
            speed: between(3.0, 7.0),
            life: [0.2, 0.4].into(),
            size: [0.06, 0.12].into(),
            stretch: 0.05,
            gravity: 6.0,
            tint: c.vivid_hdr,
            ..default()
        },
    );
    fx.ps().emit(
        s.glow_pop,
        q,
        &EmitOptions {
            count: 3.0,
            shape: EmitShape::Sphere,
            radius: 0.2,
            speed: between(0.3, 1.0),
            life: [0.25, 0.4].into(),
            size: [0.5 * scale, 0.9 * scale].into(),
            tint: c.vivid,
            ..default()
        },
    );
    shockwave(fx, q.with_y(0.0), 1.1 * scale, 0.35, c.vivid_hdr, 0.1);
}

/// Explosion de zone (fin d'une télégraphie).
pub fn area_burst(fx: &mut Fx, center: Vec3, radius: f32, c: &FxColors) {
    shockwave(fx, center, radius * 1.05, 0.45, c.vivid_hdr, 0.18);
    let q = center.with_y(0.3);
    let s = fx.styles();
    fx.ps().emit(
        s.glow_pop,
        q,
        &EmitOptions {
            count: 10.0,
            shape: EmitShape::Disc,
            radius: radius * 0.7,
            speed: between(0.5, 1.5),
            up: between(1.0, 2.5),
            life: [0.3, 0.6].into(),
            size: [0.6, 1.2].into(),
            tint: c.vivid,
            intensity: 1.8,
            ..default()
        },
    );
    fx.ps().emit(
        s.spark,
        q,
        &EmitOptions {
            count: 24.0,
            shape: EmitShape::Disc,
            radius: radius * 0.6,
            speed: between(2.0, 5.0),
            up: between(2.0, 6.0),
            life: [0.3, 0.6].into(),
            size: [0.06, 0.12].into(),
            stretch: 0.05,
            gravity: 9.0,
            tint: c.accent_hdr,
            ..default()
        },
    );
    fx.ps().emit(
        s.dust,
        center,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Ring,
            radius: radius * 0.8,
            speed: between(0.5, 1.2),
            up: between(0.2, 0.6),
            life: [0.6, 1.0].into(),
            size: [0.8, 1.4].into(),
            ..default()
        },
    );
}

fn cast(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let Some(p) = fx.entity_pos(*source, 0.0) else {
        return;
    };
    shockwave(fx, p, 0.9, 0.3, c.vivid_hdr, 0.08);
    let s = fx.styles();
    fx.ps().emit(
        s.sparkles,
        p.with_y(0.2),
        &EmitOptions {
            count: 6.0,
            shape: EmitShape::Disc,
            radius: 0.5,
            up: between(1.5, 3.0),
            life: [0.35, 0.6].into(),
            size: [0.25, 0.45].into(),
            tint: c.accent,
            intensity: 1.5,
            ..default()
        },
    );
}

fn projectile(fx: &mut Fx, e: &GameEvent, c: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from,
        to,
        speed,
        width,
        ..
    } = e
    else {
        return None;
    };
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let size = (Fx::scale(*width as f32) * 1.4).max(0.45);
    let trail = fx.styles().glow;
    let stop = ProjectileFx::launch(
        fx,
        from,
        to,
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: c.vivid_hdr,
            core: Some(c.accent_hdr),
            size,
            height: 1.0,
            trail_style: Some(trail),
            trail_density: 10.0,
            trail_size: [size * 0.3, size * 0.55],
            ribbon_width: size * 0.5,
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn hit(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        impact_burst(fx, Fx::world_pos(*pos, 0.0), c, 0.8);
    }
}

fn area(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AreaEffect {
        fx_key,
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let radius = Fx::scale(*radius as f32);
    let duration = *duration as f32;
    // zone retardée : le serveur émet d'abord `<clé>_telegraph` (durée = délai), puis `<clé>` à la détonation
    if is_telegraph_key(fx_key) {
        telegraph(
            fx,
            center,
            radius,
            duration.max(0.1),
            c.vivid,
            TelegraphOptions {
                color2: Some(c.vivid_hdr),
                ..default()
            },
        );
        return;
    }
    if duration <= 0.35 {
        area_burst(fx, center, radius, c);
        return;
    }
    // zone persistante
    let sparkles = fx.styles().sparkles;
    zone(
        fx,
        center,
        radius,
        duration,
        c.vivid_hdr,
        ZoneOptions {
            fill: Some(0.12),
            pattern: Some(0.5),
            emit: Some(ZoneEmit {
                style: sparkles,
                rate: 18.0 * radius,
                size: [0.25, 0.45],
                life: [0.5, 0.9],
                up: Some([0.4, 1.2]),
                tint: Some(c.accent),
                intensity: 1.4,
            }),
            ..default()
        },
    );
}

fn dash(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::Dash {
        source,
        from,
        to,
        duration,
        ..
    } = e
    else {
        return;
    };
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let duration = *duration as f32;
    let s = fx.styles();
    if duration < 0.08 {
        // blink : disparition, apparition
        for p in [from, to] {
            let q = p.with_y(1.0);
            flash(fx, q, 2.0, c.vivid_hdr, 0.3);
            fx.ps().emit(
                s.sparkles,
                q,
                &EmitOptions {
                    count: 10.0,
                    shape: EmitShape::Sphere,
                    radius: 0.5,
                    speed: between(0.5, 1.5),
                    life: [0.3, 0.6].into(),
                    size: [0.25, 0.4].into(),
                    tint: c.accent,
                    intensity: 1.6,
                    ..default()
                },
            );
        }
        return;
    }
    fx.ps().emit(
        s.dust,
        from,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Disc,
            radius: 0.4,
            speed: between(0.5, 1.5),
            up: between(0.3, 0.8),
            life: [0.5, 0.8].into(),
            size: [0.6, 1.1].into(),
            ..default()
        },
    );
    dash_trail(fx, *source, from, to, duration, c.vivid_hdr, true);
    let color = c.vivid_hdr;
    fx.after(duration, move |fx| shockwave(fx, to, 1.2, 0.3, color, 0.1));
}

/// Buffs génériques : rien par défaut (trop de bruit). Les CC et buffs notables ont des FX dédiés.
fn nothing(_: &mut Fx, _: &GameEvent, _: &FxColors) {}

fn summoner(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::SummonerCast { from, to, .. } = e else {
        return;
    };
    let from = Fx::world_pos(*from, 0.0);
    flash(fx, from.with_y(1.0), 2.2, c.vivid_hdr, 0.3);
    shockwave(fx, from, 1.3, 0.35, c.vivid_hdr, 0.12);
    if let Some(to) = to {
        flash(fx, Fx::world_pos(*to, 1.0), 2.2, c.accent_hdr, 0.3);
    }
}

/// Effets génériques de toutes les phases (`defaultFx`).
pub fn default_fx() -> AllHandlers {
    AllHandlers {
        cast,
        projectile,
        hit,
        area,
        dash,
        buff: nothing,
        buff_end: nothing,
        summoner,
    }
}

#[cfg(test)]
mod tests {
    use lov_shared::types::Vec2;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    const O: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    fn colors() -> FxColors {
        FxRuntime::default().colors("ilyra_q")
    }

    fn area_event(key: &str, duration: f64) -> GameEvent {
        GameEvent::AreaEffect {
            source: 1,
            fx_key: key.into(),
            pos: O,
            radius: 300.0,
            duration,
        }
    }

    #[test]
    fn zone_retardee_instantanee_ou_persistante() {
        let c = colors();
        let d = default_fx();
        for (key, duration) in [
            ("grondak_r_telegraph", 1.0),
            ("ilyra_w", 0.0),
            ("ilyra_r", 3.0),
        ] {
            let mut w = fx_world();
            (d.area)(&mut Fx::new(&mut w), &area_event(key, duration), &c);
            assert!(
                w.resource::<FxRuntime>().active_effects() >= 1,
                "{key} : au moins un effet"
            );
        }
    }

    #[test]
    fn projectile_generique_rend_sa_poignee() {
        let mut w = fx_world();
        let e = GameEvent::ProjectileSpawn {
            id: 4,
            source: 1,
            fx_key: "ilyra_q".into(),
            from: O,
            to: Vec2 { x: 900.0, y: 0.0 },
            speed: 1500.0,
            width: 80.0,
        };
        let h = (default_fx().projectile)(&mut Fx::new(&mut w), &e, &colors());
        assert!(h.is_some());
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 1);
    }

    #[test]
    fn blink_ou_ruee() {
        let c = colors();
        for (duration, effects) in [(0.0, 2), (0.3, 1)] {
            let mut w = fx_world();
            let e = GameEvent::Dash {
                source: 1,
                from: O,
                to: Vec2 { x: 400.0, y: 0.0 },
                duration,
                fx_key: None,
            };
            (default_fx().dash)(&mut Fx::new(&mut w), &e, &c);
            assert_eq!(w.resource::<FxRuntime>().active_effects(), effects);
        }
    }
}
