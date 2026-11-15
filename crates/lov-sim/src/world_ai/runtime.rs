//! Adaptation des paramètres PvE aux API centrales de dégâts et de buffs.
//! Port de `packages/server/src/world-ai/runtime.ts`.
//!
//! Les données que les sbires, structures et monstres posent dans `entity.data` pour l'économie et
//! l'expérience (`pveCombat`, `xpReward`, `rewards`, `plateReward`) sont des structures typées définies
//! ici : plusieurs domaines les partagent.
//!
//! Différences avec le TypeScript :
//! - `usesCoreCombat(world)` lisait `world.systems` (la présence d'un système nommé `combat`). Les
//!   systèmes ne sont pas rangés dans le monde en Rust : l'appelant le dit explicitement (`core_combat`)
//!   à `attack` et aux constructeurs des systèmes de sbires et de structures ;
//! - `configurePveApis` (un `WeakMap<World, Apis>` qui permettait de substituer `applyDamage` et `addBuff`
//!   par monde) n'est pas porté : aucun code TypeScript ne l'appelle et le monde n'a pas de table par
//!   monde. `apply_damage` et `add_buff` sont donc toujours les implémentations par défaut ;
//! - les entités sont désignées par `EntityId` ; `stats` a deux formes (`stats` : valeurs par défaut du
//!   TypeScript pour armure, portée, vitesse et vitesse d'attaque ; `stats_with` : tous les paramètres) ;
//!   `retire` et `attack` reçoivent leurs valeurs par défaut explicitement (`RETIRE_DELAY`, `None`) ;
//! - `pveNextAttack` et `pveRemoveAt` (`entity.data`) sont les structures `PveNextAttack` et `PveRemoveAt`.

use lov_shared::math::dist2;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, PartialStats, Stats, Team, TeamOrNeutral,
};

use crate::combat::buffs::add_buff as core_add_buff;
use crate::combat::buffs::{BuffDefOverride, BuffOptions as CoreBuffOptions, BuffParams};
use crate::combat::damage::{DamageOpts, apply_damage as core_damage};
use crate::world::{Entity, World};

/// `data.pveCombat` : marque une entité gérée par l'IA du monde (jamais lue en TypeScript non plus).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PveCombat;

/// `data.xpReward` : expérience donnée à la mort.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XpReward(pub f64);

/// `data.rewards` (`RewardData` de l'économie) : tous les champs sont optionnels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rewards {
    pub gold: Option<f64>,
    pub xp: Option<f64>,
    pub last_hit_only: Option<bool>,
    pub smite_required: Option<bool>,
    pub xp_radius: Option<f64>,
    pub local_gold: Option<f64>,
    pub team_gold: Option<f64>,
}

/// `data.plateReward` : or d'une plaque de tour, lu par l'économie pendant l'événement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateReward {
    pub gold: f64,
    pub radius: f64,
    pub source: EntityId,
}

/// `data.pveNextAttack` : instant de la prochaine attaque du repli isolé (`attack`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PveNextAttack(pub f64);

/// `data.pveRemoveAt` : instant de retrait d'une entité morte (`retire`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PveRemoveAt(pub f64);

/// Délai par défaut de `retire` (s).
pub const RETIRE_DELAY: f64 = 2.0;

/// Stats d'une entité PvE avec les valeurs par défaut du TypeScript : armure 0, portée 125, vitesse 0,
/// vitesse d'attaque 1.
pub fn stats(hp: f64, ad: f64) -> Stats {
    stats_with(hp, ad, 0.0, 125.0, 0.0, 1.0)
}

/// `stats(hp, ad, armor, range, speed, attackSpeed)` ; la résistance magique égale l'armure.
pub fn stats_with(
    hp: f64,
    ad: f64,
    armor: f64,
    range: f64,
    speed: f64,
    attack_speed: f64,
) -> Stats {
    Stats {
        max_hp: hp,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad,
        ap: 0.0,
        armor,
        mr: armor,
        attack_speed,
        attack_range: range,
        move_speed: speed,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
}

/// `DamageRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRequest {
    pub source: EntityId,
    pub target: EntityId,
    pub amount: f64,
    pub damage_type: DamageType,
    pub ability: Option<String>,
}

/// `BuffOptions` du PvE : `params` est un `Record<string, number>` (paires clé, valeur).
#[derive(Debug, Clone, PartialEq)]
pub struct BuffOptions {
    pub duration: f64,
    pub source: Option<EntityId>,
    pub stacks: Option<u32>,
    pub params: Vec<(String, f64)>,
}

impl BuffOptions {
    pub fn new(duration: f64) -> Self {
        Self {
            duration,
            source: None,
            stacks: None,
            params: Vec::new(),
        }
    }
}

pub fn is_structure(e: &Entity) -> bool {
    matches!(
        e.kind,
        EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus
    )
}

/// Équipe `Blue`/`Red` d'une entité (`None` pour le neutre).
pub fn team_of(team: TeamOrNeutral) -> Option<Team> {
    match team {
        TeamOrNeutral::Blue => Some(Team::Blue),
        TeamOrNeutral::Red => Some(Team::Red),
        TeamOrNeutral::Neutral => None,
    }
}

/// `applyDamage` : délègue à l'API centrale.
pub fn apply_damage(world: &mut World, request: DamageRequest) {
    let mut opts = DamageOpts::new(
        Some(request.source),
        request.target,
        request.amount,
        request.damage_type,
    );
    opts.ability = request.ability;
    core_damage(world, opts);
}

/// `raw[key]` : 0 si absent (la valeur ne sert qu'en test de véracité ou en facteur).
fn raw_number(raw: &[(String, f64)], key: &str) -> f64 {
    raw.iter().find(|(k, _)| k == key).map_or(0.0, |(_, v)| *v)
}

/// Test de véracité JavaScript d'un nombre : ni 0 ni NaN.
fn truthy(v: f64) -> bool {
    v != 0.0 && !v.is_nan()
}

fn display_name(id: &str) -> &'static str {
    if id.starts_with("dragon_") {
        "Faveur élémentaire"
    } else if id.starts_with("soul_") {
        "Âme élémentaire"
    } else if id == "leviathan" {
        "Puissance du Léviathan"
    } else if id == "elder" {
        "Fureur ancestrale"
    } else if id == "blue_buff" {
        "Éveil azur"
    } else if id == "red_buff" {
        "Braise écarlate"
    } else {
        "Faveur de la Faille"
    }
}

/// `addBuff` : convertit les paramètres PvE (modificateurs plats, pourcentages, buffs permanents) vers
/// l'API centrale des buffs.
pub fn add_buff(world: &mut World, target: EntityId, id: &str, options: BuffOptions) {
    if world.get(target).is_none_or(|t| t.dead) {
        return;
    }
    let raw = &options.params;
    let mut flat = PartialStats::default();
    let mut percent = PartialStats::default();
    // Les huit statistiques ajustables (`keys` du TypeScript) : `key` et `keyPercent`.
    macro_rules! fill {
        ($(($key:literal, $percent_key:literal, $field:ident)),+ $(,)?) => {$(
            let v = raw_number(raw, $key);
            if truthy(v) {
                flat.$field = Some(v);
            }
            let p = raw_number(raw, $percent_key);
            if truthy(p) {
                percent.$field = Some(p);
            }
        )+};
    }
    fill!(
        ("ad", "adPercent", ad),
        ("ap", "apPercent", ap),
        ("armor", "armorPercent", armor),
        ("mr", "mrPercent", mr),
        ("abilityHaste", "abilityHastePercent", ability_haste),
        ("moveSpeed", "moveSpeedPercent", move_speed),
        ("manaRegen", "manaRegenPercent", mana_regen),
        ("attackRange", "attackRangePercent", attack_range),
    );
    let attack_speed = raw_number(raw, "attackSpeed");
    if truthy(attack_speed) {
        percent.attack_speed = Some(attack_speed);
    }
    let permanent = id.starts_with("dragon_")
        || (id.starts_with("soul_") && !id.ends_with("_speed") && !id.ends_with("_slow"))
        || id == "grub_siege";
    let mut params = BuffParams::default();
    for (k, v) in raw {
        params.set_number(k, *v);
    }
    params.stat_mods = Some(flat);
    params.stat_percent_mods = Some(percent);
    params.persist_on_death = Some(permanent);
    // `(raw.tenacity ?? 0) * (options.stacks ?? 1)`.
    params.tenacity = Some(raw_number(raw, "tenacity") * f64::from(options.stacks.unwrap_or(1)));
    let source = if permanent { None } else { options.source };
    let mut definition = BuffDefOverride {
        name: Some(display_name(id).to_owned()),
        // maxStacks borne l'ajout de l'API au total autoritaire fourni par le domaine.
        max_stacks: Some(options.stacks.unwrap_or(1).max(1)),
        ..BuffDefOverride::default()
    };
    if truthy(raw_number(raw, "slow")) {
        definition.kind = Some(BuffKind::Cc);
        definition.cc = Some(CcType::Slow);
    }
    core_add_buff(
        world,
        target,
        id,
        CoreBuffOptions {
            duration: options.duration,
            source,
            stacks: options.stacks,
            params,
            definition: Some(definition),
        },
    );
}

/// `inRange` : la portée se mesure jusqu'au bord de `b`.
pub fn in_range(a: &Entity, b: &Entity, range: f64) -> bool {
    dist2(a.pos, b.pos) <= (range + b.radius).powi(2)
}

/// Entité vivante la plus proche de `source` dans `range` qui satisfait `predicate` ; à distance égale,
/// le plus petit id.
pub fn nearest(
    world: &World,
    source: EntityId,
    range: f64,
    mut predicate: impl FnMut(&Entity) -> bool,
) -> Option<EntityId> {
    let src = world.get(source)?;
    let mut best: Option<EntityId> = None;
    let mut distance = range * range;
    for e in world.iter() {
        if e.dead || e.id == src.id {
            continue;
        }
        let d = dist2(src.pos, e.pos);
        if d > distance || !predicate(e) {
            continue;
        }
        if d < distance || (d == distance && best.is_none_or(|b| e.id < b)) {
            best = Some(e.id);
            distance = d;
        }
    }
    best
}

/// `attack` : désigne la cible. Avec le combat central (`core_combat`), il gère armement, projectiles et
/// hooks : on ne fait que poser la cible. Sinon repli isolé pour les tests de domaine (portée, cadence,
/// dégâts physiques). `amount` vaut `source.stats.ad` par défaut. Renvoie vrai si un coup est parti.
pub fn attack(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    amount: Option<f64>,
    core_combat: bool,
) -> bool {
    let Some(src) = world.get_mut(source) else {
        return false;
    };
    let amount = amount.unwrap_or(src.stats.ad);
    src.attack_target = Some(target);
    if core_combat {
        return false;
    }
    if !world.can_attack(source) || !world.is_targetable(target, source) {
        return false;
    }
    let (Some(src), Some(tgt)) = (world.get(source), world.get(target)) else {
        return false;
    };
    let reachable = in_range(src, tgt, src.stats.attack_range);
    let target_pos = tgt.pos;
    let time = world.time;
    let Some(src) = world.get_mut(source) else {
        return false;
    };
    if !reachable {
        src.move_target = Some(target_pos);
        return false;
    }
    src.move_target = None;
    if time < src.data.get::<PveNextAttack>().map_or(0.0, |n| n.0) {
        return false;
    }
    let next = time + 1.0 / src.stats.attack_speed.max(0.1);
    src.data.insert(PveNextAttack(next));
    match &mut src.anim {
        Some(a) if a == "attack" => {}
        Some(a) => {
            a.clear();
            a.push_str("attack");
        }
        None => src.anim = Some("attack".to_owned()),
    }
    let ranged = src.stats.attack_range > 250.0;
    world.emit(GameEvent::AutoAttack {
        source,
        target,
        ranged,
        speed: None,
        crit: None,
    });
    apply_damage(
        world,
        DamageRequest {
            source,
            target,
            amount,
            damage_type: DamageType::Physical,
            ability: None,
        },
    );
    true
}

/// `retire` : retire une entité morte `delay` secondes après le premier appel.
pub fn retire(world: &mut World, id: EntityId, delay: f64) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    if !e.dead {
        return;
    }
    let at = e.data.get_or_insert_with(|| PveRemoveAt(time + delay)).0;
    if time >= at {
        world.remove(id);
    }
}
