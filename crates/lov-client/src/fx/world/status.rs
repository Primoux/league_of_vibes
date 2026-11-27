//! Visuels génériques des statuts (CC, anti-soin, invisibilité…), choisis par `buffId` et colorés par la
//! fxKey du sort qui les a posés (une immobilisation de Solenne est dorée, celle de Zephyr violette). Ils
//! s'ajoutent aux FX propres au sort. Une réapplication prolonge l'effet au lieu de le recréer.
//!
//! Portage de `packages/client/src/fx/world/status.ts`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{Effect, Fx};
use crate::fx::effects::auras::STAR;
use crate::fx::effects::primitives::{between, flash, rand, shockwave};
use crate::fx::particles::{EmitOptions, EmitShape};

const TAUNT: Vec3 = Vec3::new(2.6, 0.7, 0.3);
const SILENCE: Vec3 = Vec3::new(1.6, 0.8, 2.4);
const GRIEVOUS: Vec3 = Vec3::new(1.8, 0.15, 0.2);
const DARK: Vec3 = Vec3::new(0.4, 0.3, 0.5);

/// Catégorie visuelle d'un statut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Stun,
    Suppress,
    Root,
    Slow,
    Knockup,
    Taunt,
    Fear,
    Silence,
    Blind,
    Grievous,
    Invisible,
    Revealed,
}

/// Catégorie d'un buff : `ilyra_q_slow` → slow, `stun` → stun…
pub fn status_of(buff_id: &str) -> Option<Status> {
    Some(match buff_id {
        "stun" => Status::Stun,
        "suppress" => Status::Suppress,
        "root" => Status::Root,
        "slow" | "exhaust" => Status::Slow,
        "knockup" => Status::Knockup,
        "taunt" => Status::Taunt,
        "fear" => Status::Fear,
        "silence" => Status::Silence,
        "blind" => Status::Blind,
        "grievous" => Status::Grievous,
        "invisible" => Status::Invisible,
        "revealed" => Status::Revealed,
        b if b.ends_with("_slow") => Status::Slow,
        _ => return None,
    })
}

fn emit(
    fx: &mut Fx,
    style: impl Fn(&crate::fx::particles::StdStyles) -> crate::fx::particles::system::ParticleStyle,
    at: Vec3,
    o: EmitOptions,
) {
    let s = fx.styles();
    fx.ps().emit(style(&s), at, &o);
}

fn start(kind: Status, fx: &mut Fx, pos: Vec3, c: &FxColors) {
    match kind {
        Status::Root => shockwave(fx, pos, 0.9, 0.25, c.vivid_hdr, 0.12),
        Status::Knockup => {
            emit(
                fx,
                |s| s.dust,
                pos,
                EmitOptions {
                    count: 8.0,
                    shape: EmitShape::Ring,
                    radius: 0.4,
                    speed: between(1.0, 2.0),
                    up: between(0.4, 1.0),
                    life: [0.5, 0.8].into(),
                    size: [0.6, 1.0].into(),
                    drag: 2.0,
                    ..default()
                },
            );
            emit(
                fx,
                |s| s.swirl,
                pos.with_y(0.3),
                EmitOptions {
                    count: 2.0,
                    shape: EmitShape::Point,
                    life: 0.4.into(),
                    size: [1.2, 1.6].into(),
                    spin: Some(8.0.into()),
                    tint: c.vivid,
                    ..default()
                },
            );
        }
        Status::Invisible => emit(
            fx,
            |s| s.dark_wisp,
            pos.with_y(0.8),
            EmitOptions {
                count: 12.0,
                shape: EmitShape::Sphere,
                radius: 0.5,
                speed: between(0.5, 1.2),
                life: [0.5, 0.9].into(),
                size: [0.6, 1.0].into(),
                ..default()
            },
        ),
        Status::Revealed => shockwave(fx, pos, 1.2, 0.35, GRIEVOUS, 0.06),
        _ => {}
    }
}

/// Appelé ~20 fois par seconde tant que le statut est actif.
fn tick(kind: Status, fx: &mut Fx, pos: Vec3, t: f32, c: &FxColors) {
    match kind {
        Status::Stun => {
            for i in 0..3 {
                let a = t * 5.0 + i as f32 / 3.0 * std::f32::consts::TAU;
                let p = Vec3::new(pos.x + a.cos() * 0.45, 2.3, pos.z + a.sin() * 0.45);
                emit(
                    fx,
                    |s| s.star,
                    p,
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        life: 0.12.into(),
                        size: 0.24.into(),
                        tint: STAR,
                        ..default()
                    },
                );
            }
        }
        Status::Suppress => emit(
            fx,
            |s| s.ring,
            Vec3::new(pos.x, 0.9 + (t * 6.0).sin() * 0.3, pos.z),
            EmitOptions {
                count: 1.0,
                shape: EmitShape::Point,
                life: 0.3.into(),
                size: 1.6.into(),
                tint: c.vivid_hdr,
                ..default()
            },
        ),
        Status::Root => {
            // liens qui remontent le long des jambes
            emit(
                fx,
                |s| s.shard,
                pos,
                EmitOptions {
                    count: 2.0,
                    shape: EmitShape::Ring,
                    radius: 0.45,
                    up: between(0.8, 1.6),
                    speed: between(-0.6, -0.3),
                    life: [0.3, 0.45].into(),
                    size: [0.12, 0.2].into(),
                    tint: c.vivid_hdr,
                    ..default()
                },
            );
            emit(
                fx,
                |s| s.ring,
                pos.with_y(0.06),
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.15.into(),
                    size: 1.1.into(),
                    tint: c.vivid,
                    ..default()
                },
            );
        }
        Status::Slow => {
            if rand() < 0.5 {
                emit(
                    fx,
                    |s| s.ember,
                    pos,
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Disc,
                        radius: 0.35,
                        up: between(0.2, 0.5),
                        life: [0.4, 0.6].into(),
                        size: [0.08, 0.14].into(),
                        gravity: -0.5,
                        tint: c.vivid,
                        intensity: 0.8,
                        ..default()
                    },
                );
            }
        }
        Status::Taunt => {
            if (t * 20.0).sin() > 0.3 {
                emit(
                    fx,
                    |s| s.spark,
                    pos.with_y(2.3),
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Cone,
                        angle: 0.6,
                        speed: between(1.5, 2.5),
                        life: [0.15, 0.25].into(),
                        size: [0.06, 0.1].into(),
                        stretch: 0.05,
                        tint: TAUNT,
                        ..default()
                    },
                );
            }
        }
        Status::Fear => emit(
            fx,
            |s| s.dark_wisp,
            pos.with_y(1.8),
            EmitOptions {
                count: 1.0,
                shape: EmitShape::Sphere,
                radius: 0.3,
                up: between(0.4, 0.8),
                life: [0.5, 0.8].into(),
                size: [0.4, 0.6].into(),
                ..default()
            },
        ),
        Status::Silence => {
            let a = t * 3.0;
            let p = Vec3::new(pos.x + a.cos() * 0.35, 2.25, pos.z + a.sin() * 0.35);
            emit(
                fx,
                |s| s.rune,
                p,
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.2.into(),
                    size: 0.3.into(),
                    tint: SILENCE,
                    ..default()
                },
            );
        }
        Status::Blind => emit(
            fx,
            |s| s.dark_wisp,
            pos.with_y(1.9),
            EmitOptions {
                count: 1.0,
                shape: EmitShape::Sphere,
                radius: 0.2,
                life: [0.4, 0.6].into(),
                size: [0.3, 0.5].into(),
                ..default()
            },
        ),
        Status::Grievous => {
            if rand() < 0.35 {
                emit(
                    fx,
                    |s| s.cross,
                    pos,
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Disc,
                        radius: 0.3,
                        up: between(0.6, 1.0),
                        life: [0.5, 0.7].into(),
                        size: [0.18, 0.24].into(),
                        tint: GRIEVOUS,
                        ..default()
                    },
                );
            }
        }
        Status::Knockup | Status::Invisible | Status::Revealed => {}
    }
}

fn end(kind: Status, fx: &mut Fx, pos: Vec3) {
    if kind == Status::Invisible {
        let p = pos.with_y(0.8);
        emit(
            fx,
            |s| s.dark_wisp,
            p,
            EmitOptions {
                count: 10.0,
                shape: EmitShape::Sphere,
                radius: 0.5,
                speed: between(0.5, 1.2),
                life: [0.5, 0.9].into(),
                size: [0.6, 1.0].into(),
                ..default()
            },
        );
        flash(fx, p, 1.2, DARK, 0.2);
    }
}

/// Fin d'un statut, partagée entre l'effet et le registre (réapplication, retrait).
#[derive(Debug)]
struct Shared {
    until: f32,
    removed: bool,
}

/// Statuts actifs par catégorie et cible.
#[derive(Resource, Default)]
pub struct ActiveStatuses(HashMap<(Status, EntityId), Arc<Mutex<Shared>>>);

impl ActiveStatuses {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

struct StatusEffect {
    kind: Status,
    target: EntityId,
    shared: Arc<Mutex<Shared>>,
    colors: FxColors,
    pos: Vec3,
    t: f32,
    acc: f32,
}

impl Effect for StatusEffect {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        let Some(pos) = fx.entity_pos(self.target, 0.0) else {
            return false;
        };
        self.pos = pos;
        if self.acc >= 0.05 {
            self.acc = 0.0;
            tick(self.kind, fx, pos, self.t, &self.colors);
        }
        let now = fx.time();
        self.shared
            .lock()
            .map(|s| !s.removed && now < s.until)
            .unwrap_or(false)
    }

    fn end(&mut self, fx: &mut Fx) {
        let key = (self.kind, self.target);
        let mut active = fx.world().get_resource_or_init::<ActiveStatuses>();
        if active
            .0
            .get(&key)
            .is_some_and(|s| Arc::ptr_eq(s, &self.shared))
        {
            active.0.remove(&key);
        }
        end(self.kind, fx, self.pos);
    }
}

/// Statuts (`installStatusFx`).
pub fn status_events(fx: &mut Fx, e: &GameEvent) {
    match e {
        GameEvent::BuffApplied {
            target,
            buff_id,
            duration,
            fx_key,
            ..
        } => {
            let Some(kind) = status_of(buff_id) else {
                return;
            };
            let key = (kind, *target);
            let until = fx.time() + (*duration as f32).clamp(0.15, 60.0);
            {
                let active = fx.world().get_resource_or_init::<ActiveStatuses>();
                if let Some(prev) = active.0.get(&key)
                    && let Ok(mut p) = prev.lock()
                    && !p.removed
                {
                    // réapplication : on prolonge
                    p.until = p.until.max(until);
                    return;
                }
            }
            // cible inconnue : rien à suivre (le web enregistrait quand même le statut, qui bloquait
            // ensuite ses réapplications)
            let Some(pos) = fx.entity_pos(*target, 0.0) else {
                return;
            };
            let shared = Arc::new(Mutex::new(Shared {
                until,
                removed: false,
            }));
            fx.world()
                .get_resource_or_init::<ActiveStatuses>()
                .0
                .insert(key, shared.clone());
            let colors = fx.colors(fx_key.as_deref().unwrap_or(buff_id));
            start(kind, fx, pos, &colors);
            fx.add(StatusEffect {
                kind,
                target: *target,
                shared,
                colors,
                pos,
                t: 0.0,
                acc: 0.0,
            });
        }
        GameEvent::BuffRemoved {
            target, buff_id, ..
        } => {
            let Some(kind) = status_of(buff_id) else {
                return;
            };
            let active = fx.world().get_resource_or_init::<ActiveStatuses>();
            if let Some(s) = active.0.get(&(kind, *target))
                && let Ok(mut s) = s.lock()
            {
                s.removed = true;
            }
        }
        _ => {}
    }
}

/// Port de `packages/client/test/fx/status.test.ts` (classement) et du cycle de vie.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn classe_les_buffs_par_categorie_visuelle() {
        assert_eq!(status_of("stun"), Some(Status::Stun));
        assert_eq!(status_of("ilyra_q_slow"), Some(Status::Slow));
        assert_eq!(status_of("exhaust"), Some(Status::Slow));
        assert_eq!(status_of("knockup"), Some(Status::Knockup));
        assert_eq!(status_of("grievous"), Some(Status::Grievous));
        assert_eq!(status_of("grondak_w"), None);
    }

    fn applied(buff: &str, duration: f64) -> GameEvent {
        GameEvent::BuffApplied {
            target: 1,
            buff_id: buff.into(),
            duration,
            source: None,
            fx_key: None,
            stacks: None,
        }
    }

    #[test]
    fn sans_position_connue_rien_ne_tourne() {
        let mut w = fx_world();
        status_events(&mut Fx::new(&mut w), &applied("stun", 1.0));
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
    }

    #[test]
    fn cible_inconnue_non_enregistree() {
        let mut w = fx_world();
        status_events(&mut Fx::new(&mut w), &applied("stun", 1.0));
        assert_eq!(w.get_resource_or_init::<ActiveStatuses>().len(), 0);
    }
}
