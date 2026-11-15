//! Contexte de lancer et aides des kits : port de `packages/server/src/abilities/context.ts`.
//!
//! Un kit TypeScript reçoit `ctx` (qui porte aussi `world` et des références d'entités). En Rust le
//! monde est un paramètre à part et `CastContext` ne contient que des ids et des copies : il est `Copy`
//! et se capture donc tel quel dans les rappels (`move |world, target| …`).

use lov_shared::math::stat_at_level;
use lov_shared::schema::{AbilityDef, AbilityParam, CcType, Scaling};
use lov_shared::types::{AbilitySlot, DamageType, EntityId, Vec2};

use crate::abilities::primitives::{
    ProjectileEndFn, ProjectileOptions, direction, spawn_projectile,
};
use crate::abilities::registry::champion_def;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, add_buff};
use crate::combat::damage::{DamageOpts, apply_damage};
use crate::world::{Entity, World};

/// `CastContext` : ce qu'un sort reçoit au moment où il s'exécute.
#[derive(Debug, Clone, Copy)]
pub struct CastContext {
    pub source: EntityId,
    pub def: &'static AbilityDef,
    /// Emplacement lancé (le `ctx.def.slot` du TS, sans le cas passif).
    pub slot: AbilitySlot,
    pub rank: u32,
    pub point: Vec2,
    pub target: Option<EntityId>,
    pub cast_index: u32,
    pub amplification: f64,
}

impl CastContext {
    /// Le lanceur, tant qu'il est dans le monde.
    pub fn source<'w>(&self, world: &'w World) -> Option<&'w Entity> {
        world.get(self.source)
    }

    /// La cible d'un sort ciblé, tant qu'elle est dans le monde.
    pub fn target<'w>(&self, world: &'w World) -> Option<&'w Entity> {
        world.get(self.target?)
    }
}

/// `atRank(value, rank, fallback)` pour un tableau par rang.
pub fn at_rank(values: &[f64], rank: u32, fallback: f64) -> f64 {
    let Some(last) = values.len().checked_sub(1) else {
        return fallback;
    };
    let index = last.min(rank.saturating_sub(1) as usize);
    values.get(index).copied().unwrap_or(fallback)
}

/// `atRank(value, rank, fallback)` pour un paramètre `number | number[]` (absent : `fallback`).
pub fn at_rank_param(value: Option<&AbilityParam>, rank: u32, fallback: f64) -> f64 {
    match value {
        Some(AbilityParam::One(v)) => *v,
        Some(AbilityParam::Many(v)) => at_rank(v, rank, fallback),
        None => fallback,
    }
}

/// `param(def, key, rank, fallback)` sans contexte (le moteur l'utilise pour `recasts`, `window`…).
pub fn def_param(def: &AbilityDef, key: &str, rank: u32, fallback: f64) -> f64 {
    at_rank_param(def.params.as_ref().and_then(|m| m.get(key)), rank, fallback)
}

/// `param(ctx, key)` : paramètre du sort au rang lancé, 0 s'il est absent.
pub fn param(ctx: &CastContext, key: &str) -> f64 {
    def_param(ctx.def, key, ctx.rank, 0.0)
}

/// `param(ctx, key, fallback)`.
pub fn param_or(ctx: &CastContext, key: &str, fallback: f64) -> f64 {
    def_param(ctx.def, key, ctx.rank, fallback)
}

/// `scaling(source, value, rank, target)` : valeur d'un ratio de sort. Un lanceur sans définition de
/// champion (le TS lèverait) utilise ses stats courantes comme stats de base.
pub fn scaling(source: &Entity, value: &Scaling, rank: u32, target: Option<&Entity>) -> f64 {
    let def = source
        .champ
        .as_ref()
        .and_then(|c| champion_def(&c.champion_id).map(|d| (d, c.level)));
    let base_ad = match def {
        Some((d, level)) => stat_at_level(
            d.base_stats.ad,
            d.per_level.ad.unwrap_or(0.0),
            f64::from(level),
        ),
        None => source.stats.ad,
    };
    let base_hp = match def {
        Some((d, level)) => stat_at_level(
            d.base_stats.max_hp,
            d.per_level.max_hp.unwrap_or(0.0),
            f64::from(level),
        ),
        None => source.stats.max_hp,
    };
    let (target_max_hp, target_hp) = target.map_or((0.0, 0.0), |t| (t.stats.max_hp, t.hp));
    at_rank(&value.base, rank, 0.0)
        + value.ad.unwrap_or(0.0) * source.stats.ad
        + value.bonus_ad.unwrap_or(0.0) * (source.stats.ad - base_ad).max(0.0)
        + value.ap.unwrap_or(0.0) * source.stats.ap
        + value.max_hp.unwrap_or(0.0) * source.stats.max_hp
        + value.bonus_hp.unwrap_or(0.0) * (source.stats.max_hp - base_hp).max(0.0)
        + value.armor.unwrap_or(0.0) * source.stats.armor
        + value.target_max_hp.unwrap_or(0.0) * target_max_hp
        + value.target_missing_hp.unwrap_or(0.0) * (target_max_hp - target_hp).max(0.0)
}

/// `hit(ctx, target)` : dégâts du sort (type du sort, multiplicateur 1). 0 si le sort n'inflige rien.
pub fn hit(world: &mut World, ctx: &CastContext, target: EntityId) -> f64 {
    hit_with(world, ctx, target, 1.0, None)
}

/// `hit(ctx, target, multiplier, type)` : `damage_type` remplace le type du sort.
pub fn hit_with(
    world: &mut World,
    ctx: &CastContext,
    target: EntityId,
    multiplier: f64,
    damage_type: Option<DamageType>,
) -> f64 {
    let Some(damage) = ctx.def.damage.as_ref() else {
        return 0.0;
    };
    let damage_type = damage_type.unwrap_or(damage.damage_type);
    let (Some(source), Some(victim)) = (world.get(ctx.source), world.get(target)) else {
        return 0.0;
    };
    let amount =
        scaling(source, &damage.scaling, ctx.rank, Some(victim)) * ctx.amplification * multiplier;
    let mut opts = DamageOpts::new(Some(ctx.source), target, amount, damage_type);
    opts.ability = Some(ctx.def.id.clone());
    apply_damage(world, opts)
}

/// `cc(ctx, target, type, duration, amount)` : pose un contrôle. Les ralentissements portent l'id du
/// sort (`<sort>_slow`) : deux sorts d'un même champion ralentissent indépendamment.
pub fn cc(
    world: &mut World,
    ctx: &CastContext,
    target: EntityId,
    cc_type: CcType,
    duration: f64,
    amount: Option<f64>,
) {
    let id = if cc_type == CcType::Slow {
        format!("{}_slow", ctx.def.id)
    } else {
        match cc_type {
            CcType::Stun => "stun",
            CcType::Root => "root",
            CcType::Knockup => "knockup",
            CcType::Taunt => "taunt",
            CcType::Silence => "silence",
            CcType::Suppress => "suppress",
            CcType::Fear => "fear",
            CcType::Blind => "blind",
            CcType::Grounded => "grounded",
            CcType::Slow => "slow",
        }
        .to_owned()
    };
    let mut opts = BuffOptions::new(duration);
    opts.source = Some(ctx.source);
    opts.params.amount = amount;
    opts.definition = Some(BuffDefOverride {
        kind: Some(lov_shared::schema::BuffKind::Cc),
        cc: Some(cc_type),
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, target, &id, opts);
}

/// Options de `line` (le TS les passe dans un objet).
#[derive(Default)]
pub struct LineOptions {
    pub pierce: bool,
    pub stop_on_champion: bool,
    pub only_champions: bool,
    pub from: Option<Vec2>,
    pub to: Option<Vec2>,
    /// `type` du TS : remplace la clé FX du sort.
    pub fx_key: Option<String>,
    pub on_end: Option<ProjectileEndFn>,
}

/// `line(ctx, onHit, options)` : projectile en ligne vers `ctx.point`, sur la portée du sort.
/// Renvoie l'id du projectile (`None` si le lanceur a disparu).
pub fn line(
    world: &mut World,
    ctx: &CastContext,
    options: LineOptions,
    mut on_hit: impl FnMut(&mut World, EntityId) + Send + 'static,
) -> Option<EntityId> {
    let source = world.get(ctx.source)?;
    let from = options.from.unwrap_or(source.pos);
    let dir = direction(from, ctx.point, source.facing);
    let to = options.to.unwrap_or(Vec2 {
        x: from.x + dir.x * ctx.def.range,
        y: from.y + dir.y * ctx.def.range,
    });
    let mut projectile = ProjectileOptions::new(
        ctx.source,
        to,
        ctx.def.speed.unwrap_or(1800.0),
        ctx.def.width.unwrap_or(100.0),
        options.fx_key.unwrap_or_else(|| ctx.def.fx_key.clone()),
        move |world, target, _projectile| on_hit(world, target),
    );
    projectile.from = Some(from);
    projectile.pierce = options.pierce;
    projectile.stop_on_champion = options.stop_on_champion;
    projectile.only_champions = options.only_champions;
    projectile.on_end = options.on_end;
    spawn_projectile(world, projectile)
}
