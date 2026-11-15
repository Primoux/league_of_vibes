//! Buffs, contrôles (CC) et recalcul des stats : port de `packages/server/src/combat/buffs.ts`.
//!
//! Contrat pour les autres domaines :
//! - `add_buff` / `remove_buff` / `get_buff` / `has_cc` ; `can_act`, `can_move`, `can_attack`, `can_cast` ;
//! - `register_stat_source` : un domaine (objets, passifs) ajoute des modificateurs de stats après la somme
//!   intégrée des objets du champion ;
//! - `recalculate_stats` : seul endroit qui écrit `entity.stats` pour les champions (et les unités buffées).
//!
//! Différences avec le TypeScript, imposées par Rust :
//! - les entités sont désignées par `EntityId` ; les prédicats (`has_cc`, `can_move`…) prennent `&Entity` ;
//! - `entity.data.buffs` est `Buffs`, et les clés ad hoc de `entity.data` lues ici deviennent des types :
//!   `Stasis`, `Casting`, `Channeling` (présence = vrai), `TauntForcedMove`, `ItemTenacity`, `BaseAd`,
//!   `BuffBaseStats`, `LastChampionDamageAt`, `Revealed`. `data.invisible`, `data.ghost` et
//!   `data.revealedUntil` sont `vision::Invisible`, `movement::Ghost` et `vision::RevealedUntil` ;
//! - `BuffParams` est une structure : les clés connues sont des champs, les autres nombres vont dans
//!   `extra` (lus par `BuffParams::number`) ;
//! - `StatSource` est un registre global du processus, comme le `Set` de module du TS (voir
//!   `register_stat_source`) ;
//! - `init_buffs` n'est pas idempotent : le TS s'appuie sur un `WeakSet<World>` et `add_buff` s'auto-initialise ;
//!   ici `World` ne porte pas ce drapeau (pas de champ ajouté), donc `init_buffs` doit être appelé une
//!   seule fois par monde (c'est l'`init` du système `buffs`) avant tout `add_buff` ;
//! - `add_buff` renvoie une copie du buff (`BuffInstance`), pas une référence ; `shieldRemaining` (vue vivante
//!   posée par le wrapper de `abilities/effects.ts`) n'existe pas : lire `combat_data(e).shields`.

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use lov_shared::data::buffs::buff as catalog_buff;
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::item;
use lov_shared::math::{clamp, stat_at_level};
use lov_shared::schema::{BuffDef, BuffKind, CcType};
use lov_shared::types::{BuffSnapshot, EntityId, GameEvent, PartialStats, Stats};

use crate::combat::damage::remove_shield;
use crate::systems::combat::cancel_attack;
use crate::systems::movement::Ghost;
use crate::systems::vision::{Invisible, RevealedUntil};
use crate::world::{Entity, World};

// ------------------------------------------------------------------------------------------------
// Données de domaine
// ------------------------------------------------------------------------------------------------

/// `data.stasis = true` (Sablier de Zhonya…) : inciblable, ne peut rien faire.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stasis;

/// `data.casting = true` : un sort à temps d'incantation est en cours.
#[derive(Debug, Clone, Copy, Default)]
pub struct Casting;

/// `data.channeling = true` : un sort ou un sort d'invocateur est canalisé.
#[derive(Debug, Clone, Copy, Default)]
pub struct Channeling;

/// `data.revealed = true` : un buff révèle l'entité (lu par le ciblage des sorts).
#[derive(Debug, Clone, Copy, Default)]
pub struct Revealed;

/// `data.tauntForcedMove` : l'unité provoquée est forcée de marcher vers son provocateur ce tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct TauntForcedMove(pub bool);

/// `data.itemTenacity` : ténacité cumulée des sources de stats (0..1).
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemTenacity(pub f64);

/// `data.baseAd` : AD de base du champion à son niveau (sans objets ni buffs).
#[derive(Debug, Clone, Copy)]
pub struct BaseAd(pub f64);

/// `data.buffBaseStats` : stats de base d'une unité sans champion tant qu'elle porte des buffs.
#[derive(Debug, Clone)]
pub struct BuffBaseStats(pub Stats);

/// `data.lastChampionDamageAt` : dernier temps de jeu où un champion ennemi a blessé ce champion.
#[derive(Debug, Clone, Copy)]
pub struct LastChampionDamageAt(pub f64);

/// `data.buffs`.
#[derive(Debug, Clone, Default)]
pub struct Buffs(pub Vec<BuffInstance>);

// ------------------------------------------------------------------------------------------------
// Types publics
// ------------------------------------------------------------------------------------------------

/// `BuffParams` : paramètres libres d'un buff.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuffParams {
    pub amount: Option<f64>,
    /// Lu en repli de `amount` pour les ralentissements.
    pub slow: Option<f64>,
    pub tenacity: Option<f64>,
    pub damage_reduction: Option<f64>,
    pub damage_dealt_reduction: Option<f64>,
    pub grievous: Option<f64>,
    pub invisible: Option<bool>,
    pub ghosted: Option<bool>,
    pub revealed: Option<bool>,
    pub persist_on_death: Option<bool>,
    pub shield_id: Option<String>,
    pub stat_mods: Option<PartialStats>,
    pub stat_percent_mods: Option<PartialStats>,
    /// Autres paramètres numériques (`stun`, `stored`…), par clé du TypeScript (camelCase).
    pub extra: Vec<(String, f64)>,
}

impl BuffParams {
    /// `Number(params[key] ?? undefined)` : `None` si la clé est absente. Les clés connues lisent
    /// leur champ, les autres `extra`.
    pub fn number(&self, key: &str) -> Option<f64> {
        match key {
            "amount" => self.amount,
            "slow" => self.slow,
            "tenacity" => self.tenacity,
            "damageReduction" => self.damage_reduction,
            "damageDealtReduction" => self.damage_dealt_reduction,
            "grievous" => self.grievous,
            _ => self.extra.iter().find(|(k, _)| k == key).map(|(_, v)| *v),
        }
    }

    /// `params[key] = v` pour un paramètre numérique.
    pub fn set_number(&mut self, key: &str, v: f64) {
        match key {
            "amount" => self.amount = Some(v),
            "slow" => self.slow = Some(v),
            "tenacity" => self.tenacity = Some(v),
            "damageReduction" => self.damage_reduction = Some(v),
            "damageDealtReduction" => self.damage_dealt_reduction = Some(v),
            "grievous" => self.grievous = Some(v),
            _ => match self.extra.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = v,
                None => self.extra.push((key.to_owned(), v)),
            },
        }
    }

    /// `{ ...self, ...other }` : les paramètres définis dans `other` écrasent ceux de `self`.
    pub fn merge_from(&mut self, other: &BuffParams) {
        macro_rules! over {
            ($($f:ident),+) => { $( if other.$f.is_some() { self.$f = other.$f.clone(); } )+ };
        }
        over!(
            amount,
            slow,
            tenacity,
            damage_reduction,
            damage_dealt_reduction,
            grievous,
            invisible,
            ghosted,
            revealed,
            persist_on_death,
            shield_id,
            stat_mods,
            stat_percent_mods
        );
        for (k, v) in &other.extra {
            self.set_number(k, *v);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffInstance {
    pub id: String,
    pub source: Option<EntityId>,
    pub remaining: f64,
    pub duration: f64,
    pub stacks: u32,
    pub params: BuffParams,
    pub def: BuffDef,
}

/// `Partial<BuffDef>` : surcharge de la définition du catalogue (`opts.definition`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuffDefOverride {
    pub id: Option<String>,
    pub name: Option<String>,
    pub kind: Option<BuffKind>,
    pub cc: Option<CcType>,
    pub max_stacks: Option<u32>,
    pub stat_mods: Option<PartialStats>,
    pub stat_percent_mods: Option<PartialStats>,
    pub fx_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffOptions {
    pub duration: f64,
    pub source: Option<EntityId>,
    /// Défaut 1.
    pub stacks: Option<u32>,
    pub params: BuffParams,
    pub definition: Option<BuffDefOverride>,
}

impl BuffOptions {
    pub fn new(duration: f64) -> Self {
        Self {
            duration,
            source: None,
            stacks: None,
            params: BuffParams::default(),
            definition: None,
        }
    }
}

/// Filtre de source de `remove_buff` : `undefined` (`Any`), `null` (`Anonymous`) ou un id (`Id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMatch {
    Any,
    Anonymous,
    Id(EntityId),
}

impl SourceMatch {
    fn matches(self, source: Option<EntityId>) -> bool {
        match self {
            SourceMatch::Any => true,
            SourceMatch::Anonymous => source.is_none(),
            SourceMatch::Id(id) => source == Some(id),
        }
    }

    /// `b.source ?? null` : cible exactement la source du buff, anonyme comprise.
    pub fn exact(source: Option<EntityId>) -> Self {
        source.map_or(SourceMatch::Anonymous, SourceMatch::Id)
    }
}

/// `StatModifiers` : plat, pourcentage (additif entre sources) et ténacité (0..1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatModifiers {
    pub flat: PartialStats,
    pub percent: PartialStats,
    pub tenacity: Option<f64>,
}

/// Source de stats : appelée à chaque recalcul de chaque entité.
pub type StatSource = Box<dyn Fn(&World, &Entity) -> Option<StatModifiers> + Send + Sync>;

static STAT_SOURCES: RwLock<Vec<(u64, StatSource)>> = RwLock::new(Vec::new());
static NEXT_SOURCE_ID: AtomicU64 = AtomicU64::new(1);

/// Poignée de désabonnement renvoyée par `register_stat_source` (le `() => void` du TS).
#[derive(Debug)]
pub struct StatSourceHandle(u64);

impl StatSourceHandle {
    pub fn unsubscribe(self) {
        let mut list = STAT_SOURCES.write().unwrap_or_else(|p| p.into_inner());
        list.retain(|(id, _)| *id != self.0);
    }
}

/// Ajoute des stats d'objets ou de passifs après la somme intégrée des objets du champion.
///
/// Le registre est global au processus, comme le `Set` de module du TypeScript : la source s'applique
/// à tous les mondes. Il est verrouillé en lecture pendant chaque recalcul : une source ne doit donc
/// ni s'enregistrer ni se désabonner depuis son propre corps.
pub fn register_stat_source(
    f: impl Fn(&World, &Entity) -> Option<StatModifiers> + Send + Sync + 'static,
) -> StatSourceHandle {
    let id = NEXT_SOURCE_ID.fetch_add(1, Ordering::Relaxed);
    let mut list = STAT_SOURCES.write().unwrap_or_else(|p| p.into_inner());
    list.push((id, Box::new(f)));
    StatSourceHandle(id)
}

// ------------------------------------------------------------------------------------------------
// Lecture
// ------------------------------------------------------------------------------------------------

/// `getBuffs(e)` en lecture : liste vide si l'entité n'a jamais porté de buff.
pub fn get_buffs(e: &Entity) -> &[BuffInstance] {
    e.data.get::<Buffs>().map_or(&[], |b| &b.0)
}

/// `getBuffs(e)` en écriture : crée la liste à la demande (`??= []`).
pub fn get_buffs_mut(e: &mut Entity) -> &mut Vec<BuffInstance> {
    &mut e.data.get_or_insert_with(Buffs::default).0
}

/// `getBuff(e, id, source)` : `source = None` accepte toutes les sources.
pub fn get_buff<'a>(e: &'a Entity, id: &str, source: Option<EntityId>) -> Option<&'a BuffInstance> {
    get_buffs(e)
        .iter()
        .find(|b| b.id == id && (source.is_none() || b.source == source))
}

pub fn get_buff_mut<'a>(
    e: &'a mut Entity,
    id: &str,
    source: Option<EntityId>,
) -> Option<&'a mut BuffInstance> {
    get_buffs_mut(e)
        .iter_mut()
        .find(|b| b.id == id && (source.is_none() || b.source == source))
}

pub fn has_buff(e: &Entity, id: &str) -> bool {
    get_buff(e, id, None).is_some()
}

pub fn has_cc(e: &Entity, cc: CcType) -> bool {
    get_buffs(e)
        .iter()
        .any(|b| b.remaining > 0.0 && b.def.cc == Some(cc))
}

fn has_any_cc(e: &Entity, list: &[CcType]) -> bool {
    list.iter().any(|&c| has_cc(e, c))
}

const HARD_CC: [CcType; 5] = [
    CcType::Stun,
    CcType::Knockup,
    CcType::Suppress,
    CcType::Fear,
    CcType::Taunt,
];
const MOVE_CC: [CcType; 5] = [
    CcType::Stun,
    CcType::Knockup,
    CcType::Suppress,
    CcType::Fear,
    CcType::Root,
];
const ATTACK_CC: [CcType; 4] = [
    CcType::Stun,
    CcType::Knockup,
    CcType::Suppress,
    CcType::Fear,
];

fn is_stasis(e: &Entity) -> bool {
    e.data.contains::<Stasis>()
}

fn is_busy(e: &Entity) -> bool {
    e.data.contains::<Casting>() || e.data.contains::<Channeling>()
}

pub fn can_act(e: &Entity) -> bool {
    !e.dead && !has_any_cc(e, &HARD_CC) && !is_stasis(e)
}

pub fn can_move(e: &Entity) -> bool {
    let forced = e.data.get::<TauntForcedMove>().is_some_and(|t| t.0);
    !e.dead
        && !is_stasis(e)
        && !has_any_cc(e, &MOVE_CC)
        && (!has_cc(e, CcType::Taunt) || forced)
        && !is_busy(e)
}

pub fn can_attack(e: &Entity) -> bool {
    !e.dead && !is_stasis(e) && !has_any_cc(e, &ATTACK_CC) && !is_busy(e)
}

pub fn can_cast(e: &Entity) -> bool {
    can_act(e) && !has_cc(e, CcType::Silence) && !is_busy(e)
}

/// Ténacité totale (0..0,9) : sources de stats puis buffs, multiplicatives.
pub fn get_tenacity(e: &Entity) -> f64 {
    let item = e.data.get::<ItemTenacity>().map_or(0.0, |t| t.0);
    let mut remaining = 1.0 - clamp(item, 0.0, 0.9);
    for b in get_buffs(e) {
        remaining *= 1.0 - clamp(b.params.tenacity.unwrap_or(0.0), 0.0, 0.9);
    }
    clamp(1.0 - remaining, 0.0, 0.9)
}

/// Plus forte valeur du paramètre `key` parmi les buffs (0 si aucun).
pub fn strongest_param(e: &Entity, key: &str) -> f64 {
    let mut out = 0.0f64;
    for b in get_buffs(e) {
        out = out.max(b.params.number(key).unwrap_or(0.0));
    }
    out
}

// ------------------------------------------------------------------------------------------------
// Ajout / retrait
// ------------------------------------------------------------------------------------------------

/// Pose ou retire un marqueur selon `on` (`data.x = bool`).
fn set_marker<T: std::any::Any + Send + Sync>(e: &mut Entity, on: bool, make: impl FnOnce() -> T) {
    if on {
        if !e.data.contains::<T>() {
            e.data.insert(make());
        }
    } else {
        e.data.remove::<T>();
    }
}

fn sync_flags(time: f64, e: &mut Entity) {
    let (mut invisible, mut ghosted, mut revealed) = (false, false, false);
    let mut revealed_until = f64::NEG_INFINITY;
    for b in get_buffs(e) {
        invisible |= b.id == "invisible" || b.params.invisible.unwrap_or(false);
        ghosted |= b.params.ghosted.unwrap_or(false);
        if b.params.revealed.unwrap_or(false) {
            revealed = true;
            revealed_until = revealed_until.max(time + b.remaining);
        }
    }
    set_marker(e, invisible, || Invisible);
    set_marker(e, ghosted, || Ghost);
    set_marker(e, revealed, || Revealed);
    e.data.insert(RevealedUntil(revealed_until));
}

/// Ajoute un buff (ou le rafraîchit s'il existe pour la même source). Renvoie une copie du buff, ou
/// `None` si la cible est morte ou la durée invalide. Les CC (hors projection, suppression,
/// ralentissement, ancrage) sont raccourcis par la ténacité.
pub fn add_buff(
    world: &mut World,
    target: EntityId,
    buff_id: &str,
    opts: BuffOptions,
) -> Option<BuffInstance> {
    let t = world.get(target)?;
    if t.dead || !opts.duration.is_finite() || opts.duration <= 0.0 {
        return None;
    }
    let source = opts.source;
    let mut def = catalog_buff(buff_id).cloned().unwrap_or_else(|| BuffDef {
        id: buff_id.to_owned(),
        name: buff_id.to_owned(),
        kind: BuffKind::Buff,
        cc: None,
        max_stacks: None,
        stat_mods: None,
        stat_percent_mods: None,
        fx_key: None,
    });
    if let Some(o) = opts.definition {
        if let Some(v) = o.id {
            def.id = v;
        }
        if let Some(v) = o.name {
            def.name = v;
        }
        if let Some(v) = o.kind {
            def.kind = v;
        }
        if o.cc.is_some() {
            def.cc = o.cc;
        }
        if o.max_stacks.is_some() {
            def.max_stacks = o.max_stacks;
        }
        if o.stat_mods.is_some() {
            def.stat_mods = o.stat_mods;
        }
        if o.stat_percent_mods.is_some() {
            def.stat_percent_mods = o.stat_percent_mods;
        }
        if o.fx_key.is_some() {
            def.fx_key = o.fx_key;
        }
    }
    let reducible = def.cc.is_some_and(|c| {
        !matches!(
            c,
            CcType::Knockup | CcType::Suppress | CcType::Slow | CcType::Grounded
        )
    });
    let duration = opts.duration
        * if reducible {
            1.0 - get_tenacity(t)
        } else {
            1.0
        };
    let max_stacks = def.max_stacks.unwrap_or(1);
    let fx_key = def.fx_key.clone().unwrap_or_else(|| buff_id.to_owned());

    let time = world.time;
    let t = world.get_mut(target)?;
    let list = get_buffs_mut(t);
    let applied = match list
        .iter_mut()
        .find(|b| b.id == buff_id && b.source == source)
    {
        Some(b) => {
            b.remaining = b.remaining.max(duration);
            b.duration = b.duration.max(duration);
            b.stacks = max_stacks.min(b.stacks + opts.stacks.unwrap_or(1));
            b.params.merge_from(&opts.params);
            b.clone()
        }
        None => {
            let b = BuffInstance {
                id: buff_id.to_owned(),
                source,
                remaining: duration,
                duration,
                stacks: max_stacks.min(opts.stacks.unwrap_or(1)),
                params: opts.params,
                def,
            };
            list.push(b.clone());
            b
        }
    };
    sync_flags(time, t);
    recalculate_stats(world, target);
    world.emit(GameEvent::BuffApplied {
        target,
        buff_id: buff_id.to_owned(),
        duration,
        source,
        fx_key: Some(fx_key),
        stacks: Some(applied.stacks),
    });
    Some(applied)
}

/// Retire tous les buffs `buff_id` correspondant au filtre de source ; leur bouclier lié est retiré.
pub fn remove_buff(world: &mut World, target: EntityId, buff_id: &str, source: SourceMatch) {
    let Some(len) = world.get(target).map(|e| get_buffs(e).len()) else {
        return;
    };
    let mut i = len;
    while i > 0 {
        i -= 1;
        let Some(e) = world.get_mut(target) else {
            break;
        };
        let list = get_buffs_mut(e);
        // Un écouteur d'événement a pu raccourcir la liste entre-temps (le TS planterait).
        let Some(b) = list.get(i) else { continue };
        if b.id != buff_id || !source.matches(b.source) {
            continue;
        }
        if let Some(shield_id) = b.params.shield_id.clone() {
            remove_shield(e, Some(&shield_id));
        }
        let removed = get_buffs_mut(e).remove(i);
        world.emit(GameEvent::BuffRemoved {
            target,
            buff_id: buff_id.to_owned(),
            fx_key: Some(removed.def.fx_key.unwrap_or_else(|| buff_id.to_owned())),
        });
    }
    let time = world.time;
    if let Some(e) = world.get_mut(target) {
        sync_flags(time, e);
    }
    recalculate_stats(world, target);
}

/// Retire les CC (sauf projection et suppression).
pub fn cleanse(world: &mut World, id: EntityId) {
    let Some(e) = world.get(id) else { return };
    let victims: Vec<(String, Option<EntityId>)> = get_buffs(e)
        .iter()
        .filter(|b| {
            b.def
                .cc
                .is_some_and(|c| c != CcType::Knockup && c != CcType::Suppress)
        })
        .map(|b| (b.id.clone(), b.source))
        .collect();
    for (buff_id, source) in victims {
        remove_buff(world, id, &buff_id, SourceMatch::exact(source));
    }
}

pub fn break_invisibility(world: &mut World, id: EntityId) {
    let Some(e) = world.get(id) else { return };
    let victims: Vec<(String, Option<EntityId>)> = get_buffs(e)
        .iter()
        .filter(|b| b.id == "invisible" || b.params.invisible.unwrap_or(false))
        .map(|b| (b.id.clone(), b.source))
        .collect();
    for (buff_id, source) in victims {
        remove_buff(world, id, &buff_id, SourceMatch::exact(source));
    }
}

// ------------------------------------------------------------------------------------------------
// Stats
// ------------------------------------------------------------------------------------------------

/// `mergeStats(out, add, stacks)` : `out[k] = (out[k] ?? 0) + (add[k] ?? 0) * stacks`, clé par clé.
pub fn merge_stats(out: &mut PartialStats, add: &PartialStats, stacks: f64) {
    macro_rules! m {
        ($($f:ident),+) => {
            $( if let Some(v) = add.$f { out.$f = Some(out.$f.unwrap_or(0.0) + v * stacks); } )+
        };
    }
    m!(
        max_hp,
        hp_regen,
        max_mana,
        mana_regen,
        ad,
        ap,
        armor,
        mr,
        attack_speed,
        attack_range,
        move_speed,
        crit_chance,
        ability_haste,
        lifesteal,
        armor_pen,
        magic_pen
    );
}

/// Modificateurs des seuls buffs de l'entité (plat, pourcentage, ténacité).
pub fn get_stat_mods(e: &Entity) -> StatModifiers {
    let mut m = StatModifiers::default();
    fill_buff_mods(e, &mut m.flat, &mut m.percent);
    m.tenacity = Some(get_tenacity(e));
    m
}

fn fill_buff_mods(e: &Entity, flat: &mut PartialStats, percent: &mut PartialStats) {
    for b in get_buffs(e) {
        let s = f64::from(b.stacks);
        if let Some(m) = &b.def.stat_mods {
            merge_stats(flat, m, s);
        }
        if let Some(m) = &b.params.stat_mods {
            merge_stats(flat, m, s);
        }
        if let Some(m) = &b.def.stat_percent_mods {
            merge_stats(percent, m, s);
        }
        if let Some(m) = &b.params.stat_percent_mods {
            merge_stats(percent, m, s);
        }
    }
}

/// Garde séparées les stats de base d'un sbire ou monstre et les modificateurs temporaires.
pub fn set_base_stats(world: &mut World, id: EntityId, stats: Stats) {
    let Some(e) = world.get_mut(id) else { return };
    if e.champ.is_some() {
        return;
    }
    if !get_buffs(e).is_empty() {
        e.data.insert(BuffBaseStats(stats));
    } else {
        e.stats = stats;
    }
    recalculate_stats(world, id);
}

/// Recalcule `entity.stats` : base (niveau) + objets + sources + buffs, ralentissement, bornes.
/// Sans accumulation d'un tick à l'autre : tout est recalculé depuis la base.
pub fn recalculate_stats(world: &mut World, id: EntityId) {
    let Some(e) = world.get(id) else { return };
    // Un id de champion hors registre (le TS lèverait) est traité comme une entité sans définition.
    let def = e.champ.as_ref().and_then(|c| get_champion(&c.champion_id));
    if def.is_none() && get_buffs(e).is_empty() {
        let Some(e) = world.get_mut(id) else { return };
        if let Some(BuffBaseStats(original)) = e.data.remove::<BuffBaseStats>() {
            e.stats = original;
            e.hp = e.hp.min(e.stats.max_hp);
        }
        return;
    }
    let base_stats: Stats = match def {
        Some(d) => d.base_stats.clone(),
        None => {
            let Some(e) = world.get_mut(id) else { return };
            let snapshot = e.stats.clone();
            e.data
                .get_or_insert_with(|| BuffBaseStats(snapshot))
                .0
                .clone()
        }
    };
    let Some(e) = world.get(id) else { return };

    let mut flat = PartialStats::default();
    let mut percent = PartialStats::default();
    fill_buff_mods(e, &mut flat, &mut percent);
    let level = e.champ.as_ref().map_or(1.0, |c| f64::from(c.level));
    let base_ad = match def {
        Some(d) => stat_at_level(d.base_stats.ad, d.per_level.ad.unwrap_or(0.0), level),
        None => base_stats.ad,
    };
    if let Some(champ) = &e.champ {
        for slot in &champ.items {
            if let Some(it) = slot.as_deref().filter(|s| !s.is_empty()).and_then(item) {
                merge_stats(&mut flat, &it.stats, 1.0);
            }
        }
    }
    let mut tenacity = 0.0f64;
    {
        let sources = STAT_SOURCES.read().unwrap_or_else(|p| p.into_inner());
        for (_, f) in sources.iter() {
            let Some(mods) = f(world, e) else { continue };
            merge_stats(&mut flat, &mods.flat, 1.0);
            merge_stats(&mut percent, &mods.percent, 1.0);
            tenacity = 1.0 - (1.0 - tenacity) * (1.0 - mods.tenacity.unwrap_or(0.0));
        }
    }

    let mut stats = base_stats.clone();
    macro_rules! calc {
        ($($f:ident),+) => {
            $(
                let base = match def {
                    Some(d) => stat_at_level(base_stats.$f, d.per_level.$f.unwrap_or(0.0), level),
                    None => base_stats.$f,
                };
                stats.$f = (base + flat.$f.unwrap_or(0.0)) * (1.0 + percent.$f.unwrap_or(0.0));
            )+
        };
    }
    calc!(
        max_hp,
        hp_regen,
        max_mana,
        mana_regen,
        ad,
        ap,
        armor,
        mr,
        attack_speed,
        attack_range,
        move_speed,
        crit_chance,
        ability_haste,
        lifesteal,
        armor_pen,
        magic_pen
    );
    let mut slow = 0.0f64;
    for b in get_buffs(e) {
        if b.def.cc == Some(CcType::Slow) {
            slow = slow.max(b.params.amount.or(b.params.slow).unwrap_or(0.0));
        }
    }
    stats.move_speed = (stats.move_speed * (1.0 - clamp(slow, 0.0, 0.99))).max(0.0);
    stats.attack_speed = if stats.attack_speed > 0.0 {
        clamp(stats.attack_speed, 0.1, 2.5)
    } else {
        0.0
    };
    stats.crit_chance = clamp(stats.crit_chance, 0.0, 1.0);
    stats.max_hp = stats.max_hp.max(1.0);
    stats.max_mana = stats.max_mana.max(0.0);

    let Some(e) = world.get_mut(id) else { return };
    e.data.insert(BaseAd(base_ad));
    e.data.insert(ItemTenacity(tenacity));
    e.hp = e.hp.min(stats.max_hp);
    e.mana = clamp(e.mana, 0.0, stats.max_mana);
    e.stats = stats;
}

// ------------------------------------------------------------------------------------------------
// Tick
// ------------------------------------------------------------------------------------------------

/// `updateBuffs(world, dt)` ; le système `buffs` réutilise un tampon d'ids (voir `update_buffs_with`).
pub fn update_buffs(world: &mut World, dt: f64) {
    let mut ids = Vec::new();
    update_buffs_with(world, dt, &mut ids);
}

/// Un tick de buffs : expiration, recalcul des stats, provocation, puis annulation des ordres
/// que le CC interdit. `ids` est un tampon réutilisable (vidé d'abord).
pub fn update_buffs_with(world: &mut World, dt: f64, ids: &mut Vec<EntityId>) {
    world.ids_into(None, ids);
    for &id in ids.iter() {
        let Some(len) = world.get(id).map(|e| get_buffs(e).len()) else {
            continue;
        };
        let mut i = len;
        while i > 0 {
            i -= 1;
            let Some(e) = world.get_mut(id) else { break };
            let dead = e.dead;
            let Some(b) = get_buffs_mut(e).get_mut(i) else {
                continue;
            };
            b.remaining -= dt;
            let expired = b.remaining <= 1e-8 || (dead && b.params.persist_on_death != Some(true));
            if expired {
                let (buff_id, source) = (b.id.clone(), b.source);
                remove_buff(world, id, &buff_id, SourceMatch::exact(source));
            }
        }
        let Some(e) = world.get(id) else { continue };
        if e.champ.is_some() || !get_buffs(e).is_empty() || e.data.contains::<BuffBaseStats>() {
            recalculate_stats(world, id);
        }

        let Some(e) = world.get(id) else { continue };
        let taunt_source = get_buffs(e)
            .iter()
            .find(|b| b.def.cc == Some(CcType::Taunt))
            .and_then(|b| b.source);
        let taunter = world
            .get_opt(taunt_source)
            .filter(|t| !t.dead)
            .map(|t| (t.id, t.pos, t.radius));
        let attack_range = e.stats.attack_range;
        let (pos, radius, can_hit) = (e.pos, e.radius, can_attack(e));
        let Some(e) = world.get_mut(id) else { continue };
        e.data.insert(TauntForcedMove(false));
        if let Some((tid, tpos, tradius)) = taunter
            && can_hit
        {
            e.attack_target = Some(tid);
            e.data.insert(TauntForcedMove(true));
            let out_of_range =
                (tpos.x - pos.x).hypot(tpos.y - pos.y) > attack_range + radius + tradius;
            e.move_target = out_of_range.then_some(tpos);
            e.path = None;
        }
        if !can_move(e) {
            e.move_target = None;
            e.path = None;
        }
        if !can_attack(e) {
            cancel_attack(e);
            e.attack_target = None;
        }
    }
}

fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Branche contrôles et mitigation sur les hooks du monde (aucune édition d'un autre système).
/// À appeler une seule fois par monde, avant tout `add_buff` (voir l'en-tête du module).
pub fn init_buffs(world: &mut World) {
    world.hooks.entity_snapshot.push(Box::new(|_, e, snapshot| {
        let buffs = get_buffs(e);
        if !buffs.is_empty() {
            snapshot.buffs = Some(
                buffs
                    .iter()
                    .map(|b| BuffSnapshot {
                        id: b.id.clone(),
                        remaining: js_round(b.remaining.max(0.0) * 10.0) / 10.0,
                        stacks: (b.stacks > 1).then_some(b.stacks),
                    })
                    .collect(),
            );
        }
    }));
    world
        .hooks
        .can_move
        .push(Box::new(|w, id| w.get(id).is_none_or(can_move)));
    world
        .hooks
        .can_attack
        .push(Box::new(|w, id| w.get(id).is_none_or(can_attack)));
    world.hooks.is_targetable.push(Box::new(|w, target, _| {
        w.get(target).is_none_or(|e| !is_stasis(e))
    }));
    world.hooks.before_attack_hit.push(Box::new(|w, ctx| {
        if w.get(ctx.attacker)
            .is_some_and(|a| has_cc(a, CcType::Blind))
        {
            ctx.amount = 0.0;
            ctx.bonus = 0.0;
        }
    }));
    world.hooks.before_damage.push(Box::new(|world, ctx| {
        let target = ctx.target;
        if !ctx.amount.is_finite() || world.get(target).is_some_and(is_stasis) {
            ctx.amount = 0.0;
            return;
        }
        let source = ctx.credit.or(ctx.source);
        let parry = world
            .get(target)
            .and_then(|t| get_buff(t, "kaelen_w", None))
            .map(|p| (p.source, p.params.number("stun").unwrap_or(1.0)));
        let enemy_champion = source
            .and_then(|s| world.get(s))
            .filter(|s| s.champ.is_some() && world.get(target).is_some_and(|t| s.team != t.team));
        if let (Some((parry_source, stun)), Some(src)) = (parry, enemy_champion) {
            let in_reach = world.get(target).is_some_and(|t| {
                (src.pos.x - t.pos.x).hypot(src.pos.y - t.pos.y) <= 250.0 + src.radius + t.radius
            });
            let src_id = src.id;
            // `parry.source` absent : `removeBuff` reçoit `undefined`, donc toute source.
            let filter = parry_source.map_or(SourceMatch::Any, SourceMatch::Id);
            remove_buff(world, target, "kaelen_w", filter);
            ctx.amount = 0.0;
            if in_reach {
                let mut o = BuffOptions::new(stun);
                o.source = Some(target);
                add_buff(world, src_id, "stun", o);
            }
            return;
        }
        if ctx.damage_type != lov_shared::types::DamageType::True {
            let dealt = source
                .and_then(|s| world.get(s))
                .map_or(0.0, |s| strongest_param(s, "damageDealtReduction"));
            ctx.multiplier *= 1.0 - dealt.min(0.95);
            let taken = world
                .get(target)
                .map_or(0.0, |t| strongest_param(t, "damageReduction"));
            ctx.multiplier *= 1.0 - taken.min(0.95);
        }
    }));
    world.hooks.after_damage.push(Box::new(|world, ctx, _| {
        let source = ctx.credit.or(ctx.source);
        let time = world.time;
        let enemy_champion = source.and_then(|s| world.get(s)).is_some_and(|s| {
            s.champ.is_some()
                && world
                    .get(ctx.target)
                    .is_some_and(|t| t.champ.is_some() && s.team != t.team)
        });
        if enemy_champion && let Some(t) = world.get_mut(ctx.target) {
            t.data.insert(LastChampionDamageAt(time));
        }
    }));
    world.hooks.before_heal.push(Box::new(|world, ctx| {
        let grievous = world
            .get(ctx.target)
            .map_or(0.0, |t| strongest_param(t, "grievous"));
        ctx.amount *= 1.0 - grievous.min(1.0);
    }));
}
