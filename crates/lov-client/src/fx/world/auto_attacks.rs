//! FX des auto-attaques (événement `autoAttack`, émis au lancement : impact pour la mêlée, départ du
//! projectile à distance). Variantes par champion via [`AutoAttackFx`], sinon FX générique aux couleurs du
//! champion ; tirs de tours et de sbires aux couleurs d'équipe ; souffle des drakes, coups lourds des
//! monstres épiques.
//!
//! Portage de `packages/client/src/fx/world/autoAttacks.ts`.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::types::{
    DragonElement, EntityKind, EntitySnapshot, GameEvent, MonsterType, TeamOrNeutral,
};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::Fx;
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    LightningOptions, ProjectileFx, ProjectileLook, between, flash, lightning, rand, rand_range,
    shockwave,
};
use crate::fx::particles::system::ParticleStyle;
use crate::fx::particles::{EmitOptions, EmitShape, StdStyles};

/// Données d'une auto-attaque (champs de l'événement `autoAttack`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoAttack {
    pub source: lov_shared::types::EntityId,
    pub target: lov_shared::types::EntityId,
    pub ranged: bool,
    /// Vitesse du projectile (unités de jeu/s).
    pub speed: Option<f64>,
    pub crit: bool,
}

/// Tir à distance : part de `from` (monde) vers la cible.
pub type RangedFn = fn(&mut Fx, &AutoAttack, Vec3, &FxColors);
/// Coup au corps à corps, au point d'impact `at` (monde).
pub type MeleeFn = fn(&mut Fx, &AutoAttack, Vec3, &FxColors);

/// Variante d'auto-attaque d'un champion (`AutoAttackFx`).
#[derive(Clone, Copy, Default)]
pub struct AutoAttackFx {
    pub ranged: Option<RangedFn>,
    pub melee: Option<MeleeFn>,
}

/// Variantes par champion et élément du drake courant.
#[derive(Resource)]
pub struct AutoAttacks {
    custom: HashMap<String, AutoAttackFx>,
    /// Élément du drake courant (comme le rendu : appris par `objectiveSpawn`).
    dragon: DragonElement,
}

impl Default for AutoAttacks {
    fn default() -> Self {
        let mut custom = HashMap::new();
        custom.insert(
            "grondak".to_owned(),
            AutoAttackFx {
                melee: Some(grondak_melee),
                ..default()
            },
        );
        custom.insert(
            "ilyra".to_owned(),
            AutoAttackFx {
                ranged: Some(ilyra_ranged),
                ..default()
            },
        );
        Self {
            custom,
            dragon: DragonElement::Infernal,
        }
    }
}

/// Enregistrement d'une variante depuis le plugin d'un champion (`registerAutoAttackFx`).
pub trait RegisterAutoAttackFx {
    fn register_auto_attack_fx(&mut self, champion: &str, fx: AutoAttackFx) -> &mut Self;
}

impl RegisterAutoAttackFx for App {
    fn register_auto_attack_fx(&mut self, champion: &str, fx: AutoAttackFx) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<AutoAttacks>()
            .custom
            .insert(champion.to_owned(), fx);
        self
    }
}

const TEAM_BLUE: Vec3 = Vec3::new(0.5, 0.8, 2.4);
const TEAM_RED: Vec3 = Vec3::new(2.4, 0.6, 0.5);
const NEUTRAL: Vec3 = Vec3::new(2.0, 1.6, 0.6);

fn team_color(t: TeamOrNeutral) -> Vec3 {
    match t {
        TeamOrNeutral::Blue => TEAM_BLUE,
        TeamOrNeutral::Red => TEAM_RED,
        TeamOrNeutral::Neutral => NEUTRAL,
    }
}

fn emit(fx: &mut Fx, style: impl Fn(&StdStyles) -> ParticleStyle, at: Vec3, o: EmitOptions) {
    let s = fx.styles();
    fx.ps().emit(style(&s), at, &o);
}

/// Projectile générique à tête chercheuse ; `on_hit` à l'arrivée.
pub fn homing_shot(
    fx: &mut Fx,
    e: &AutoAttack,
    from: Vec3,
    color: Vec3,
    core: Vec3,
    size: f32,
    on_hit: impl FnOnce(&mut Fx, Vec3) + Send + Sync + 'static,
) {
    let to = fx.entity_pos(e.target, 1.0).unwrap_or(from);
    let speed = Fx::scale(e.speed.unwrap_or(1600.0) as f32);
    let trail = fx.styles().glow;
    let _ = ProjectileFx::launch(
        fx,
        from,
        to,
        speed,
        ProjectileLook {
            color,
            core: Some(core),
            size,
            height: from.y,
            trail_style: Some(trail),
            trail_density: 6.0,
            trail_size: [size * 0.25, size * 0.45],
            trail_life: [0.15, 0.25],
            ribbon_width: size * 0.35,
            ribbon_life: 0.15,
            target: Some(e.target),
            on_end: Some(Box::new(on_hit)),
            ..default()
        },
    );
}

/// Impact d'auto-attaque à distance : volontairement sobre (plusieurs par seconde en combat). Un
/// empilement de halos HDR sature en blanc sous le bloom.
fn aa_impact(fx: &mut Fx, at: Vec3, c: &FxColors, crit: bool) {
    flash(fx, at, if crit { 1.1 } else { 0.7 }, c.vivid, 0.14);
    emit(
        fx,
        |s| s.spark,
        at,
        EmitOptions {
            count: if crit { 10.0 } else { 5.0 },
            shape: EmitShape::Point,
            speed: between(2.0, 4.0),
            life: [0.12, 0.22].into(),
            size: [0.04, 0.07].into(),
            stretch: 0.04,
            tint: c.vivid_hdr,
            ..default()
        },
    );
    if crit {
        shockwave(fx, at.with_y(0.0), 1.0, 0.25, c.vivid, 0.08);
    }
}

pub fn generic_ranged(fx: &mut Fx, e: &AutoAttack, from: Vec3, c: &FxColors) {
    let (c2, crit) = (*c, e.crit);
    homing_shot(
        fx,
        e,
        from,
        c.vivid,
        c.accent,
        if crit { 0.45 } else { 0.3 },
        move |fx, at| {
            aa_impact(fx, at, &c2, crit);
        },
    );
}

pub fn generic_melee(fx: &mut Fx, e: &AutoAttack, at: Vec3, c: &FxColors) {
    let k = if e.crit { 1.5 } else { 1.0 };
    let p = at.with_y(1.0);
    flash(fx, p, 1.1 * k, c.vivid_hdr, 0.15);
    emit(
        fx,
        |s| s.spark,
        p,
        EmitOptions {
            count: (8.0 * k).round(),
            shape: EmitShape::Point,
            speed: between(3.0, 6.0),
            life: [0.15, 0.3].into(),
            size: [0.05, 0.09].into(),
            stretch: 0.04,
            gravity: 6.0,
            tint: c.accent_hdr,
            ..default()
        },
    );
    if e.crit {
        shockwave(fx, at, 1.2, 0.3, c.vivid_hdr, 0.1);
    }
}

fn structure_or_minion(fx: &mut Fx, e: &AutoAttack, src: &EntitySnapshot) {
    let team = team_color(src.team);
    if !e.ranged {
        let Some(at) = fx.entity_pos(e.target, 0.7) else {
            return;
        };
        emit(
            fx,
            |s| s.spark,
            at,
            EmitOptions {
                count: 4.0,
                shape: EmitShape::Point,
                speed: between(2.0, 4.0),
                life: [0.12, 0.2].into(),
                size: [0.04, 0.07].into(),
                stretch: 0.03,
                tint: team,
                ..default()
            },
        );
        return;
    }
    let tower = src.kind == EntityKind::Tower;
    let Some(from) = fx.entity_pos(e.source, if tower { 4.8 } else { 0.9 }) else {
        return;
    };
    homing_shot(
        fx,
        e,
        from,
        team,
        team,
        if tower { 0.9 } else { 0.3 },
        move |fx, at| {
            flash(fx, at, if tower { 1.8 } else { 0.7 }, team, 0.15);
            if tower {
                emit(
                    fx,
                    |s| s.spark,
                    at,
                    EmitOptions {
                        count: 10.0,
                        shape: EmitShape::Point,
                        speed: between(2.0, 5.0),
                        life: [0.2, 0.35].into(),
                        size: [0.05, 0.1].into(),
                        stretch: 0.04,
                        tint: team,
                        ..default()
                    },
                );
            }
        },
    );
}

// ---------------------------------------------------------------------------------- monstres épiques

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Breath {
    Element(DragonElement),
    Elder,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BreathStyle {
    Flame,
    Glow,
    Dust,
    Sparkles,
    Smoke,
}

fn breath_look(b: Breath) -> (Vec3, BreathStyle) {
    match b {
        Breath::Element(DragonElement::Infernal) => (Vec3::new(2.6, 0.9, 0.25), BreathStyle::Flame),
        Breath::Element(DragonElement::Ocean) => (Vec3::new(0.3, 1.3, 2.6), BreathStyle::Glow),
        Breath::Element(DragonElement::Mountain) => (Vec3::new(0.7, 0.55, 0.4), BreathStyle::Dust),
        Breath::Element(DragonElement::Cloud) => (Vec3::new(1.8, 2.3, 2.6), BreathStyle::Sparkles),
        Breath::Element(DragonElement::Hextech) => {
            (Vec3::new(0.5, 1.8, 2.8), BreathStyle::Sparkles)
        }
        Breath::Element(DragonElement::Chemtech) => (Vec3::new(0.7, 2.2, 0.3), BreathStyle::Smoke),
        Breath::Elder => (Vec3::new(1.8, 0.8, 2.8), BreathStyle::Flame),
    }
}

fn breath_style(s: &StdStyles, b: BreathStyle) -> ParticleStyle {
    match b {
        BreathStyle::Flame => s.flame,
        BreathStyle::Glow => s.glow,
        BreathStyle::Dust => s.dust,
        BreathStyle::Sparkles => s.sparkles,
        BreathStyle::Smoke => s.smoke,
    }
}

/// Souffle de drake : jet de particules élémentaires de la gueule vers la cible, impact coloré.
fn breath(fx: &mut Fx, e: &AutoAttack, b: Breath) {
    let (color, look) = breath_look(b);
    let (Some(mut from), Some(at)) = (fx.entity_pos(e.source, 1.8), fx.entity_pos(e.target, 0.9))
    else {
        return;
    };
    let d = at - from;
    let len = d.length();
    let dir = if len > 0.0 { d / len } else { Vec3::X };
    from += dir * 1.2; // gueule, devant le corps
    let speed = (len * 2.5).max(6.0);
    let s = fx.styles();
    let style = breath_style(&s, look);
    fx.ps().emit(
        style,
        from,
        &EmitOptions {
            count: 26.0,
            shape: EmitShape::Cone,
            dir: Some(dir),
            angle: 0.18,
            speed: between(speed * 0.8, speed * 1.1),
            life: [len / speed, len / speed * 1.2].into(),
            size: [0.3, 0.6].into(),
            drag: 0.5,
            tint: color,
            ..default()
        },
    );
    if b == Breath::Element(DragonElement::Hextech) {
        lightning(
            fx,
            from,
            at,
            color,
            LightningOptions {
                width: 0.16,
                life: 0.2,
                jag: 0.25,
                ..default()
            },
        );
    }
    fx.after(len / speed, move |fx| {
        flash(fx, at, 1.8, color, 0.2);
        fx.ps().emit(
            style,
            at,
            &EmitOptions {
                count: 10.0,
                shape: EmitShape::Sphere,
                radius: 0.3,
                speed: between(1.0, 2.5),
                life: [0.3, 0.6].into(),
                size: [0.3, 0.6].into(),
                tint: color,
                ..default()
            },
        );
        if b == Breath::Element(DragonElement::Mountain) {
            emit(
                fx,
                |s| s.debris,
                at,
                EmitOptions {
                    count: 6.0,
                    shape: EmitShape::Sphere,
                    radius: 0.2,
                    speed: between(2.0, 4.0),
                    up: between(1.0, 3.0),
                    gravity: 9.0,
                    life: [0.6, 0.9].into(),
                    size: [0.12, 0.2].into(),
                    spin: between(-6.0, 6.0),
                    tint: color,
                    ..default()
                },
            );
        }
    });
}

/// Coup lourd d'un monstre épique (Léviathan, Colosse) : onde de choc et poussière.
fn heavy_hit(fx: &mut Fx, e: &AutoAttack, color: Vec3) {
    let Some(at) = fx.entity_pos(e.target, 0.0) else {
        return;
    };
    shockwave(fx, at, 1.8, 0.4, color, 0.14);
    flash(fx, at.with_y(1.0), 2.0, color, 0.2);
    emit(
        fx,
        |s| s.dust,
        at.with_y(0.1),
        EmitOptions {
            count: 8.0,
            shape: EmitShape::Ring,
            radius: 0.5,
            speed: between(1.0, 2.0),
            up: between(0.3, 0.8),
            life: [0.6, 0.9].into(),
            size: [0.6, 1.0].into(),
            ..default()
        },
    );
}

const LEVIATHAN: Vec3 = Vec3::new(1.6, 0.6, 2.6);
const HERALD: Vec3 = Vec3::new(1.2, 0.7, 2.4);

fn monster(fx: &mut Fx, e: &AutoAttack, src: &EntitySnapshot, dragon: DragonElement) -> bool {
    let sub = src.subtype.as_deref().unwrap_or("");
    if sub == MonsterType::Dragon.as_str() {
        breath(fx, e, Breath::Element(dragon));
    } else if sub == MonsterType::ElderDragon.as_str() {
        breath(fx, e, Breath::Elder);
    } else if sub == MonsterType::Leviathan.as_str() {
        heavy_hit(fx, e, LEVIATHAN);
    } else if sub == MonsterType::Herald.as_str() {
        heavy_hit(fx, e, HERALD);
    } else {
        return false;
    }
    true
}

/// Auto-attaques (`installAutoAttackFx`).
pub fn auto_attack_events(fx: &mut Fx, ev: &GameEvent) {
    match ev {
        GameEvent::ObjectiveSpawn {
            monster: MonsterType::Dragon,
            element: Some(el),
            ..
        } => {
            fx.world().get_resource_or_init::<AutoAttacks>().dragon = *el;
        }
        GameEvent::AutoAttack {
            source,
            target,
            ranged,
            speed,
            crit,
        } => {
            let e = AutoAttack {
                source: *source,
                target: *target,
                ranged: *ranged,
                speed: *speed,
                crit: crit.unwrap_or(false),
            };
            let Some(src) = fx.entity(e.source).cloned() else {
                return;
            };
            let champ = src
                .subtype
                .as_deref()
                .filter(|s| src.kind == EntityKind::Champion && get_champion(s).is_some())
                .map(str::to_owned);
            let (custom, dragon) = {
                let aa = fx.world().get_resource_or_init::<AutoAttacks>();
                (
                    champ.as_ref().and_then(|c| aa.custom.get(c).copied()),
                    aa.dragon,
                )
            };
            let Some(champ) = champ else {
                if !(src.kind == EntityKind::Monster && monster(fx, &e, &src, dragon)) {
                    structure_or_minion(fx, &e, &src);
                }
                return;
            };
            let c = fx.colors(&format!("{champ}_aa"));
            let custom = custom.unwrap_or_default();
            if e.ranged {
                let Some(mut from) = fx.entity_pos(e.source, 1.25) else {
                    return;
                };
                // départ légèrement devant le champion, du côté de la cible
                if let Some(at) = fx.entity_pos(e.target, 1.25) {
                    from += (at - from).with_y(0.0).normalize_or_zero() * 0.45;
                }
                custom.ranged.unwrap_or(generic_ranged)(fx, &e, from, &c);
            } else {
                let Some(at) = fx.entity_pos(e.target, 0.0) else {
                    return;
                };
                custom.melee.unwrap_or(generic_melee)(fx, &e, at, &c);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------- variantes

const STONE: Vec3 = Vec3::new(0.195, 0.144, 0.098);

fn grondak_melee(fx: &mut Fx, e: &AutoAttack, at: Vec3, c: &FxColors) {
    generic_melee(fx, e, at, c);
    emit(
        fx,
        |s| s.dust,
        at,
        EmitOptions {
            count: 4.0,
            shape: EmitShape::Disc,
            radius: 0.3,
            up: between(0.3, 0.8),
            speed: between(0.5, 1.2),
            life: [0.5, 0.8].into(),
            size: [0.5, 0.8].into(),
            ..default()
        },
    );
    for _ in 0..3 {
        let a = rand() * std::f32::consts::TAU;
        burst(
            fx,
            BurstKind::RockDebris,
            BurstSpawn {
                pos: at.with_y(0.4),
                mode: BurstMode::Debris,
                size: rand_range(0.07, 0.12),
                life: 0.8,
                vel: Vec3::new(a.cos() * 1.5, 3.0, a.sin() * 1.5),
                color: Some(STONE),
                ..default()
            },
        );
    }
}

const ICE_HDR: Vec3 = Vec3::new(1.3, 2.4, 3.2);

fn ilyra_ranged(fx: &mut Fx, e: &AutoAttack, from: Vec3, _: &FxColors) {
    homing_shot(
        fx,
        e,
        from,
        Vec3::new(0.7, 1.4, 2.0),
        ICE_HDR,
        if e.crit { 0.5 } else { 0.36 },
        |fx, at| {
            emit(
                fx,
                |s| s.snow,
                at,
                EmitOptions {
                    count: 6.0,
                    shape: EmitShape::Sphere,
                    radius: 0.15,
                    speed: between(1.0, 2.5),
                    life: [0.35, 0.6].into(),
                    size: [0.1, 0.16].into(),
                    spin: between(-4.0, 4.0),
                    drag: 2.0,
                    tint: ICE_HDR,
                    ..default()
                },
            );
            flash(fx, at, 0.9, ICE_HDR, 0.15);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variantes_enregistrees() {
        let aa = AutoAttacks::default();
        assert!(aa.custom.get("grondak").and_then(|f| f.melee).is_some());
        assert!(aa.custom.get("ilyra").and_then(|f| f.ranged).is_some());
        assert_eq!(breath_look(Breath::Elder).1, BreathStyle::Flame);
    }

    #[test]
    fn le_drake_courant_suit_l_apparition() {
        let mut w = crate::fx::effects::primitives::tests::fx_world();
        auto_attack_events(
            &mut Fx::new(&mut w),
            &GameEvent::ObjectiveSpawn {
                monster: MonsterType::Dragon,
                element: Some(DragonElement::Ocean),
                pos: lov_shared::types::Vec2 { x: 0.0, y: 0.0 },
            },
        );
        assert_eq!(w.resource::<AutoAttacks>().dragon, DragonElement::Ocean);
    }
}
