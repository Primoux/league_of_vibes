//! FX des événements du monde : chiffres flottants (dégâts, soins, or), montée de niveau, mort, rappel,
//! réapparition, destruction de structures, objectifs, buffs de jungle et d'objectifs.
//!
//! Portage de `packages/client/src/fx/world/events.ts`. La réapparition, que le web déclenchait depuis la
//! vue du champion (`setRespawnHook`), part ici de l'événement serveur `respawn`, qui porte la position.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::constants::RECALL_SECONDS;
use lov_shared::types::{
    DamageType, DragonElement, EntityId, EntityKind, GameEvent, MonsterType, StructureKind, Team,
    TeamOrNeutral,
};

use super::damage_numbers::FloatingNumbers;
use crate::fx::core::runtime::{AuraId, Effect, Fx, FxDecal};
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    ZoneOptions, between, decal_circle, edit_decal, flash, rand, rand_range, shockwave, timed, zone,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::shaders::DecalShape;
use crate::net::GameState;

/// Couleurs linéaires : un bleu (0.35, 0.6, 1) paraît presque blanc une fois converti en sRGB.
pub mod colors {
    use bevy::math::Vec3;

    pub const PHYSICAL: Vec3 = Vec3::new(1.0, 0.32, 0.04);
    pub const MAGIC: Vec3 = Vec3::new(0.1, 0.3, 1.0);
    pub const TRUE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
    pub const TAKEN: Vec3 = Vec3::new(1.0, 0.18, 0.15);
    pub const HEAL: Vec3 = Vec3::new(0.3, 1.0, 0.4);
    pub const GOLD: Vec3 = Vec3::new(1.0, 0.82, 0.25);
    pub const GOLD_HDR: Vec3 = Vec3::new(2.6, 2.0, 0.6);
    pub const LEVEL: Vec3 = Vec3::new(2.4, 2.0, 0.9);
    pub const RECALL: Vec3 = Vec3::new(0.6, 1.4, 2.6);
    pub const SOUL: Vec3 = Vec3::new(0.9, 1.1, 1.6);
    pub const BLUE: Vec3 = Vec3::new(0.45, 0.75, 2.2);
    pub const RED: Vec3 = Vec3::new(2.2, 0.55, 0.35);
    pub const TEAM_BLUE: Vec3 = Vec3::new(0.6, 1.0, 2.6);
    pub const TEAM_RED: Vec3 = Vec3::new(2.6, 0.7, 0.5);
    pub const ELDER: Vec3 = Vec3::new(1.8, 0.9, 2.6);
    pub const LEVIATHAN: Vec3 = Vec3::new(1.6, 0.6, 2.6);
}

use colors as C;

pub fn team_hdr(team: Team) -> Vec3 {
    match team {
        Team::Blue => C::TEAM_BLUE,
        Team::Red => C::TEAM_RED,
    }
}

pub fn element_color(e: DragonElement) -> Vec3 {
    match e {
        DragonElement::Infernal => Vec3::new(2.6, 0.8, 0.2),
        DragonElement::Ocean => Vec3::new(0.3, 1.4, 2.4),
        DragonElement::Mountain => Vec3::new(1.6, 1.1, 0.6),
        DragonElement::Cloud => Vec3::new(1.6, 2.2, 2.4),
        DragonElement::Hextech => Vec3::new(0.6, 1.6, 2.6),
        DragonElement::Chemtech => Vec3::new(1.0, 2.4, 0.4),
    }
}

/// Le joueur local voit les chiffres qui le concernent (sans partie, en démo : tous).
fn concerns_me(fx: &mut Fx, ids: &[EntityId]) -> bool {
    my_entity(fx).is_none_or(|me| ids.contains(&me))
}

fn my_entity(fx: &mut Fx) -> Option<EntityId> {
    fx.world()
        .get_resource::<GameState>()
        .and_then(|s| s.my_entity)
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

/// Événements du monde (`installWorldFx`).
pub fn world_events(fx: &mut Fx, e: &GameEvent) {
    match e {
        GameEvent::Damage {
            source,
            target,
            amount,
            damage_type,
            crit,
            ..
        } => {
            if *amount < 1.0 || !concerns_me(fx, &[*source, *target]) {
                return;
            }
            let Some(p) = fx.entity_pos(*target, 2.1) else {
                return;
            };
            let color = if my_entity(fx) == Some(*target) {
                C::TAKEN
            } else {
                match damage_type {
                    DamageType::Physical => C::PHYSICAL,
                    DamageType::Magic => C::MAGIC,
                    DamageType::True => C::TRUE,
                }
            };
            let crit = crit.unwrap_or(false);
            let big = if crit { 1.45 } else { 1.0 };
            let text = format!("{}{}", amount.round(), if crit { "!" } else { "" });
            FloatingNumbers::spawn(
                fx,
                p,
                &text,
                color,
                0.42 * big,
                if crit { 1.1 } else { 0.9 },
            );
        }
        GameEvent::Heal {
            source,
            target,
            amount,
        } => {
            if *amount < 1.0 {
                return;
            }
            let Some(p) = fx.entity_pos(*target, 0.0) else {
                return;
            };
            emit(
                fx,
                |s| s.cross,
                p,
                EmitOptions {
                    count: 5.0,
                    shape: EmitShape::Disc,
                    radius: 0.45,
                    up: between(1.0, 2.0),
                    life: [0.6, 0.9].into(),
                    size: [0.18, 0.28].into(),
                    tint: C::HEAL,
                    intensity: 1.6,
                    ..default()
                },
            );
            if concerns_me(fx, &[*source, *target]) {
                let text = format!("+{}", amount.round());
                FloatingNumbers::spawn(fx, p.with_y(2.1), &text, C::HEAL, 0.36, 0.9);
            }
        }
        GameEvent::GoldGain {
            target,
            amount,
            pos,
        } => {
            if !concerns_me(fx, &[*target]) {
                return;
            }
            let p = match pos {
                Some(pos) => Fx::world_pos(*pos, 1.0),
                None => match fx.entity_pos(*target, 1.6) {
                    Some(p) => p,
                    None => return,
                },
            };
            emit(
                fx,
                |s| s.coin,
                p,
                EmitOptions {
                    count: (2.0 + (amount / 20.0).round() as f32).min(8.0),
                    shape: EmitShape::Sphere,
                    radius: 0.2,
                    speed: between(1.0, 2.5),
                    up: between(2.0, 3.5),
                    gravity: 9.0,
                    life: [0.6, 0.9].into(),
                    size: [0.16, 0.22].into(),
                    spin: between(-8.0, 8.0),
                    tint: C::GOLD_HDR,
                    ..default()
                },
            );
            let text = format!("+{}", amount.round());
            FloatingNumbers::spawn(fx, p + Vec3::Y * 0.6, &text, C::GOLD, 0.34, 1.1);
        }
        GameEvent::Shield { target, .. } => {
            let Some(p) = fx.entity_pos(*target, 1.0) else {
                return;
            };
            emit(
                fx,
                |s| s.sparkles,
                p,
                EmitOptions {
                    count: 4.0,
                    shape: EmitShape::Sphere,
                    radius: 0.7,
                    life: [0.4, 0.6].into(),
                    size: [0.3, 0.45].into(),
                    tint: C::SOUL,
                    intensity: 1.2,
                    ..default()
                },
            );
        }
        GameEvent::LevelUp { target, .. } => level_up(fx, *target),
        GameEvent::Death { target, .. } => death(fx, *target),
        GameEvent::RecallStart { target } => recall_start(fx, *target),
        GameEvent::RecallEnd { target, completed } => recall_end(fx, *target, *completed),
        GameEvent::Respawn { target, pos } => respawn_fx(fx, *target, Fx::world_pos(*pos, 0.0)),
        GameEvent::StructureDestroyed {
            target,
            kind,
            team,
            plate,
        } => structure_destroyed(fx, *target, *kind, *team, plate.unwrap_or(false)),
        GameEvent::ObjectiveTaken {
            monster,
            team,
            killer,
            element,
        } => {
            let color = if *monster == MonsterType::ElderDragon {
                C::ELDER
            } else {
                element.map_or(team_hdr(*team), element_color)
            };
            let Some(pos) = killer.and_then(|k| fx.entity_pos(k, 0.0)) else {
                return;
            };
            flash(fx, pos.with_y(1.5), 4.0, color, 0.5);
            fx.shake(pos, 30.0, 0.5);
            shockwave(fx, pos, 3.0, 0.7, color, 0.2);
            emit(
                fx,
                |s| s.spark,
                pos,
                EmitOptions {
                    count: 40.0,
                    shape: EmitShape::Ring,
                    radius: 1.0,
                    up: between(6.0, 12.0),
                    life: [0.4, 0.7].into(),
                    size: [0.08, 0.14].into(),
                    stretch: 0.08,
                    tint: color,
                    ..default()
                },
            );
        }
        GameEvent::ObjectiveSpawn {
            monster,
            element,
            pos,
        } => {
            let color = if *monster == MonsterType::ElderDragon {
                C::ELDER
            } else {
                element.map_or(C::SOUL, element_color)
            };
            let pos = Fx::world_pos(*pos, 0.0);
            shockwave(fx, pos, 4.0, 1.0, color, 0.3);
            emit(
                fx,
                |s| s.glow,
                pos,
                EmitOptions {
                    count: 20.0,
                    shape: EmitShape::Disc,
                    radius: 2.5,
                    up: between(1.0, 3.0),
                    life: [0.8, 1.4].into(),
                    size: [0.6, 1.2].into(),
                    tint: color,
                    ..default()
                },
            );
        }
        GameEvent::BuffApplied {
            target,
            buff_id,
            duration,
            ..
        } => {
            if let Some((color, style)) = objective_aura(buff_id) {
                aura(fx, *target, buff_id, *duration as f32, color, style);
                return;
            }
            match buff_id.as_str() {
                "red_burn" => burn(fx, *target, *duration as f32, C::RED),
                "elder_burn" => burn(fx, *target, *duration as f32, C::ELDER),
                "elder_execute" => {
                    if let Some(p) = fx.entity_pos(*target, 1.0) {
                        flash(fx, p, 4.0, C::ELDER, 0.5);
                        emit(
                            fx,
                            |s| s.flame,
                            p.with_y(0.2),
                            EmitOptions {
                                count: 30.0,
                                shape: EmitShape::Disc,
                                radius: 0.6,
                                up: between(3.0, 6.0),
                                life: [0.5, 0.9].into(),
                                size: [0.6, 1.0].into(),
                                tint: C::ELDER,
                                ..default()
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        GameEvent::BuffRemoved {
            target, buff_id, ..
        } if objective_aura(buff_id).is_some() => {
            fx.end_aura(&format!("obj:{buff_id}:{target}"));
        }
        _ => {}
    }
}

fn level_up(fx: &mut Fx, target: EntityId) {
    let Some(pos) = fx.entity_pos(target, 0.0) else {
        return;
    };
    shockwave(fx, pos, 1.5, 0.5, C::LEVEL, 0.12);
    flash(fx, pos.with_y(1.2), 2.6, C::LEVEL, 0.4);
    // colonne de lumière qui suit le champion
    timed(
        fx,
        0.9,
        move |fx, t, _| {
            let Some(q) = fx.entity_pos(target, 0.0) else {
                return;
            };
            emit(
                fx,
                |s| s.spark,
                q,
                EmitOptions {
                    count: 3.0,
                    shape: EmitShape::Ring,
                    radius: 0.55 * (1.0 - t * 0.5),
                    up: between(5.0, 8.0),
                    life: [0.3, 0.5].into(),
                    size: [0.06, 0.1].into(),
                    stretch: 0.06,
                    tint: C::LEVEL,
                    ..default()
                },
            );
            emit(
                fx,
                |s| s.glow,
                q.with_y(0.2),
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Disc,
                    radius: 0.3,
                    up: between(3.0, 5.0),
                    life: [0.3, 0.5].into(),
                    size: [0.5, 0.8].into(),
                    tint: C::LEVEL,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

fn death(fx: &mut Fx, target: EntityId) {
    let kind = fx.entity(target).map(|i| i.kind);
    let Some(p) = fx.entity_pos(target, 0.8) else {
        return;
    };
    match kind {
        Some(EntityKind::Champion) => {
            // âme qui s'élève pendant la chute
            fx.after(0.7, move |fx| {
                let Some(q) = fx.entity_pos(target, 0.6) else {
                    return;
                };
                emit(
                    fx,
                    |s| s.glow,
                    q,
                    EmitOptions {
                        count: 10.0,
                        shape: EmitShape::Sphere,
                        radius: 0.4,
                        up: between(1.0, 2.2),
                        speed: between(0.1, 0.4),
                        life: [0.8, 1.3].into(),
                        size: [0.3, 0.6].into(),
                        tint: C::SOUL,
                        intensity: 1.4,
                        ..default()
                    },
                );
                emit(
                    fx,
                    |s| s.sparkles,
                    q,
                    EmitOptions {
                        count: 6.0,
                        shape: EmitShape::Sphere,
                        radius: 0.5,
                        up: between(1.5, 2.5),
                        life: [0.8, 1.2].into(),
                        size: [0.3, 0.5].into(),
                        tint: C::SOUL,
                        ..default()
                    },
                );
            });
        }
        Some(EntityKind::Minion | EntityKind::Monster) => {
            emit(
                fx,
                |s| s.dust,
                p.with_y(0.2),
                EmitOptions {
                    count: 5.0,
                    shape: EmitShape::Disc,
                    radius: 0.3,
                    up: between(0.3, 0.8),
                    speed: between(0.3, 0.8),
                    life: [0.5, 0.8].into(),
                    size: [0.5, 0.8].into(),
                    ..default()
                },
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------- rappel

struct Recall {
    key: String,
    aura: AuraId,
    target: EntityId,
    decal: FxDecal,
    ended: Arc<AtomicBool>,
    opacity: f32,
    t: f32,
    acc: f32,
}

impl Effect for Recall {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        let Some(pos) = fx.entity_pos(self.target, 0.0) else {
            return false;
        };
        let ended = self.ended.load(Ordering::Relaxed);
        self.opacity = if ended {
            (self.opacity - dt * 5.0).max(0.0)
        } else {
            (self.t * 3.0).min(1.0)
        };
        let (opacity, r) = (self.opacity, 1.0 + (self.t * 3.0).sin() * 0.05);
        edit_decal(fx, &self.decal, |m, g| {
            m.u.opacity = opacity;
            Some(g.circle(m, pos, r, DecalShape::Circle))
        });
        if ended {
            return self.opacity > 0.0;
        }
        if self.acc > 0.05 {
            self.acc = 0.0;
            let k = (self.t / RECALL_SECONDS as f32).min(1.0);
            // anneau de runes qui monte et se resserre à mesure que le rappel avance
            for i in 0..2 {
                let a = self.t * 2.0 + i as f32 * std::f32::consts::PI;
                let p = Vec3::new(
                    pos.x + a.cos() * (0.9 - k * 0.4),
                    0.1 + k * 1.6,
                    pos.z + a.sin() * (0.9 - k * 0.4),
                );
                emit(
                    fx,
                    |s| s.rune,
                    p,
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        life: 0.4.into(),
                        size: 0.28.into(),
                        tint: C::RECALL,
                        ..default()
                    },
                );
            }
            emit(
                fx,
                |s| s.spark,
                pos,
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Ring,
                    radius: 0.8,
                    up: between(2.0, 4.0),
                    life: [0.4, 0.7].into(),
                    size: [0.05, 0.08].into(),
                    stretch: 0.05,
                    tint: C::RECALL,
                    ..default()
                },
            );
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.forget_aura(&self.key, self.aura);
        fx.release_decal(self.decal.clone());
    }
}

fn recall_start(fx: &mut Fx, target: EntityId) {
    let key = format!("recall:{target}");
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    let decal = fx.acquire_decal();
    edit_decal(fx, &decal, |m, _| {
        m.set_color(C::RECALL, C::RECALL);
        m.u.fill = 0.1;
        m.u.pattern = 0.8;
        m.u.opacity = 0.0;
        None
    });
    if let Some(p) = fx.entity_pos(target, 0.0) {
        decal_circle(fx, &decal, p, 1.0, DecalShape::Circle);
    }
    fx.add(Recall {
        key,
        aura,
        target,
        decal,
        ended,
        opacity: 0.0,
        t: 0.0,
        acc: 0.0,
    });
}

fn recall_end(fx: &mut Fx, target: EntityId, completed: bool) {
    let had = fx.entity_pos(target, 0.0);
    fx.end_aura(&format!("recall:{target}"));
    let Some(pos) = had else {
        return;
    };
    if completed {
        flash(fx, pos.with_y(1.2), 3.2, C::RECALL, 0.4);
        emit(
            fx,
            |s| s.spark,
            pos,
            EmitOptions {
                count: 30.0,
                shape: EmitShape::Disc,
                radius: 0.5,
                up: between(8.0, 14.0),
                life: [0.3, 0.5].into(),
                size: [0.08, 0.14].into(),
                stretch: 0.08,
                tint: C::RECALL,
                ..default()
            },
        );
        shockwave(fx, pos, 1.6, 0.35, C::RECALL, 0.1);
    } else {
        emit(
            fx,
            |s| s.sparkles,
            pos.with_y(0.6),
            EmitOptions {
                count: 6.0,
                shape: EmitShape::Sphere,
                radius: 0.6,
                life: [0.3, 0.5].into(),
                size: [0.3, 0.4].into(),
                tint: C::RECALL,
                intensity: 0.7,
                ..default()
            },
        );
    }
}

/// Réapparition : colonne de lumière à la couleur d'équipe.
pub fn respawn_fx(fx: &mut Fx, id: EntityId, pos: Vec3) {
    let color = match fx.entity(id).map(|i| i.team) {
        Some(TeamOrNeutral::Red) => C::TEAM_RED,
        _ => C::TEAM_BLUE,
    };
    flash(fx, pos.with_y(1.2), 3.0, color, 0.45);
    shockwave(fx, pos, 1.8, 0.5, color, 0.12);
    emit(
        fx,
        |s| s.spark,
        pos,
        EmitOptions {
            count: 26.0,
            shape: EmitShape::Disc,
            radius: 0.5,
            up: between(6.0, 11.0),
            life: [0.3, 0.5].into(),
            size: [0.07, 0.12].into(),
            stretch: 0.07,
            tint: color,
            ..default()
        },
    );
    emit(
        fx,
        |s| s.sparkles,
        pos,
        EmitOptions {
            count: 10.0,
            shape: EmitShape::Disc,
            radius: 0.7,
            up: between(0.5, 1.5),
            life: [0.6, 1.0].into(),
            size: [0.3, 0.45].into(),
            tint: color,
            ..default()
        },
    );
}

// ---------------------------------------------------------------------------------- structures

fn structure_destroyed(
    fx: &mut Fx,
    target: EntityId,
    kind: StructureKind,
    team: Team,
    plate: bool,
) {
    let Some(pos) = fx.entity_pos(target, 0.0) else {
        return;
    };
    let team = team_hdr(team);
    if plate {
        emit(
            fx,
            |s| s.coin,
            pos.with_y(2.5),
            EmitOptions {
                count: 10.0,
                shape: EmitShape::Sphere,
                radius: 0.5,
                speed: between(1.5, 3.0),
                up: between(2.0, 4.0),
                gravity: 9.0,
                life: [0.8, 1.1].into(),
                size: [0.2, 0.28].into(),
                spin: between(-8.0, 8.0),
                tint: C::GOLD_HDR,
                ..default()
            },
        );
        debris(fx, pos, 8, 0.8, team);
        return;
    }
    let scale = match kind {
        StructureKind::Nexus => 2.2,
        StructureKind::Inhibitor => 1.3,
        StructureKind::Tower => 1.0,
    };
    flash(fx, pos.with_y(2.5 * scale), 6.0 * scale, team, 0.5);
    let (shake, shake_t) = match kind {
        StructureKind::Nexus => (70.0, 1.2),
        StructureKind::Inhibitor => (45.0, 0.6),
        StructureKind::Tower => (35.0, 0.6),
    };
    fx.shake(pos, shake, shake_t);
    shockwave(fx, pos, 3.5 * scale, 0.8, team, 0.3);
    debris(fx, pos, (26.0 * scale).round() as u32, 1.6 * scale, team);
    emit(
        fx,
        |s| s.smoke,
        pos,
        EmitOptions {
            count: (24.0 * scale).round(),
            shape: EmitShape::Disc,
            radius: 1.2 * scale,
            speed: between(0.5, 1.5),
            up: between(1.0, 3.0),
            life: [1.5, 2.5].into(),
            size: [1.5, 3.0].into(),
            drag: 0.8,
            ..default()
        },
    );
    emit(
        fx,
        |s| s.spark,
        pos.with_y(2.0),
        EmitOptions {
            count: (40.0 * scale).round(),
            shape: EmitShape::Sphere,
            radius: 0.8,
            speed: between(4.0, 10.0),
            life: [0.5, 1.0].into(),
            size: [0.08, 0.14].into(),
            stretch: 0.06,
            gravity: 9.0,
            tint: team,
            ..default()
        },
    );
    if kind == StructureKind::Nexus {
        // le nexus explose en plusieurs temps
        for i in 1..=3 {
            fx.after(i as f32 * 0.35, move |fx| {
                let q = Vec3::new(
                    pos.x + rand_range(-1.5, 1.5),
                    2.0 + rand_range(0.0, 2.0),
                    pos.z + rand_range(-1.5, 1.5),
                );
                flash(fx, q, 5.0, team, 0.4);
                emit(
                    fx,
                    |s| s.spark,
                    q,
                    EmitOptions {
                        count: 30.0,
                        shape: EmitShape::Sphere,
                        radius: 0.5,
                        speed: between(4.0, 9.0),
                        life: [0.5, 0.9].into(),
                        size: [0.08, 0.14].into(),
                        stretch: 0.06,
                        gravity: 9.0,
                        tint: team,
                        ..default()
                    },
                );
            });
        }
        zone(
            fx,
            pos,
            5.0,
            3.0,
            team,
            ZoneOptions {
                fill: Some(0.15),
                pattern: Some(0.8),
                ..default()
            },
        );
    }
}

/// Débris de pierre teintés vers la couleur d'équipe.
pub fn debris(fx: &mut Fx, at: Vec3, n: u32, power: f32, tint: Vec3) {
    let color = Vec3::new(0.55, 0.52, 0.5).lerp(tint * 0.3, 0.2);
    for _ in 0..n {
        let a = rand() * std::f32::consts::TAU;
        burst(
            fx,
            BurstKind::RockDebris,
            BurstSpawn {
                pos: Vec3::new(
                    at.x + a.cos() * 0.5,
                    1.0 + rand() * 2.0,
                    at.z + a.sin() * 0.5,
                ),
                mode: BurstMode::Debris,
                size: rand_range(0.15, 0.35) * power.sqrt(),
                life: rand_range(1.2, 1.8),
                vel: Vec3::new(
                    a.cos() * rand_range(1.0, 4.0) * power,
                    rand_range(3.0, 7.0) * power,
                    a.sin() * rand_range(1.0, 4.0) * power,
                ),
                color: Some(color),
                ..default()
            },
        );
    }
}

// ---------------------------------------------------------------------------------- buffs d'objectifs

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuraStyle {
    Rune,
    Flame,
    Wisp,
}

/// Aura d'un buff de jungle ou d'objectif.
pub fn objective_aura(buff_id: &str) -> Option<(Vec3, AuraStyle)> {
    Some(match buff_id {
        "blue_buff" | "blue" => (C::BLUE, AuraStyle::Rune),
        "red_buff" | "red" => (C::RED, AuraStyle::Flame),
        "leviathan" | "leviathan_buff" => (C::LEVIATHAN, AuraStyle::Wisp),
        "elder" | "elder_buff" => (C::ELDER, AuraStyle::Flame),
        _ => return None,
    })
}

struct ObjectiveAura {
    key: String,
    aura: AuraId,
    target: EntityId,
    ended: Arc<AtomicBool>,
    until: f32,
    color: Vec3,
    style: AuraStyle,
    t: f32,
    acc: f32,
}

impl Effect for ObjectiveAura {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        if self.ended.load(Ordering::Relaxed) || fx.time() > self.until {
            return false;
        }
        let Some(pos) = fx.entity_pos(self.target, 0.0) else {
            return false;
        };
        if self.acc < 0.08 {
            return true;
        }
        self.acc = 0.0;
        let color = self.color;
        match self.style {
            AuraStyle::Rune => {
                let a = self.t * 2.2;
                let p = Vec3::new(pos.x + a.cos() * 0.6, 0.15, pos.z + a.sin() * 0.6);
                emit(
                    fx,
                    |s| s.rune,
                    p,
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        up: Some(0.6.into()),
                        life: 0.6.into(),
                        size: 0.22.into(),
                        tint: color,
                        ..default()
                    },
                );
            }
            AuraStyle::Flame => emit(
                fx,
                |s| s.flame,
                pos,
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Ring,
                    radius: 0.5,
                    up: between(0.8, 1.4),
                    life: [0.35, 0.55].into(),
                    size: [0.25, 0.4].into(),
                    tint: color,
                    intensity: 0.6,
                    ..default()
                },
            ),
            AuraStyle::Wisp => emit(
                fx,
                |s| s.glow,
                pos.with_y(1.4),
                EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.6,
                    up: between(0.3, 0.8),
                    life: [0.6, 0.9].into(),
                    size: [0.3, 0.5].into(),
                    tint: color,
                    ..default()
                },
            ),
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.forget_aura(&self.key, self.aura);
    }
}

fn aura(
    fx: &mut Fx,
    target: EntityId,
    buff_id: &str,
    duration: f32,
    color: Vec3,
    style: AuraStyle,
) {
    let key = format!("obj:{buff_id}:{target}");
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    let until = fx.time() + duration.clamp(1.0, 600.0);
    fx.add(ObjectiveAura {
        key,
        aura,
        target,
        ended,
        until,
        color,
        style,
        t: 0.0,
        acc: 0.0,
    });
}

/// Brûlure (buff rouge, Drake Ancien, Embrasement) : flammes sur la cible.
pub fn burn(fx: &mut Fx, target: EntityId, duration: f32, color: Vec3) {
    let mut acc = 0.0;
    timed(
        fx,
        duration.max(0.3),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.06 {
                return;
            }
            let Some(pos) = fx.entity_pos(target, 0.0) else {
                return;
            };
            acc = 0.0;
            emit(
                fx,
                |s| s.flame,
                pos,
                EmitOptions {
                    count: 2.0,
                    shape: EmitShape::Disc,
                    radius: 0.35,
                    up: between(1.0, 2.0),
                    life: [0.35, 0.6].into(),
                    size: [0.35, 0.6].into(),
                    tint: color,
                    intensity: 1.1,
                    ..default()
                },
            );
            if rand() < 0.3 {
                emit(
                    fx,
                    |s| s.smoke,
                    pos.with_y(1.5),
                    EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        up: between(0.5, 1.0),
                        life: [0.6, 0.9].into(),
                        size: [0.3, 0.5].into(),
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auras_d_objectifs() {
        assert_eq!(
            objective_aura("blue_buff").map(|a| a.1),
            Some(AuraStyle::Rune)
        );
        assert_eq!(objective_aura("elder").map(|a| a.1), Some(AuraStyle::Flame));
        assert_eq!(
            objective_aura("leviathan_buff").map(|a| a.1),
            Some(AuraStyle::Wisp)
        );
        assert!(objective_aura("stun").is_none());
    }
}
