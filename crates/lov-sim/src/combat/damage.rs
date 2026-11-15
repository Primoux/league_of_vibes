//! API de dégâts / soins / boucliers : port de `packages/server/src/combat/damage.ts`.
//!
//! TOUS les dégâts du jeu passent par `apply_damage` (sorts, AA, tours, monstres, fontaine, DoT...).
//!
//! Extension sans éditer ce fichier : `world.hooks.before_damage`, `after_damage`, `before_heal`,
//! `prevent_death`, `bounty`, `respawn_time` (voir `WorldHooks`).
//!
//! Différences avec le TypeScript, imposées par l'emprunt Rust :
//! - les entités sont désignées par `EntityId` (source, cible, crédit) ; une entité retirée du monde est
//!   traitée comme absente (le TS continuerait avec l'objet orphelin) ;
//! - `entity.data.combat` est la structure `CombatData` ; `entity.data.owner` est `Owner` ;
//! - `championDamagers` (un `Map` ordonné par insertion en TS) est un `Vec` qui garde cet ordre ;
//! - `expire_shields` reçoit le temps de jeu plutôt que le monde.

use lov_shared::map::FOUNTAINS;
use lov_shared::math::damage_multiplier;
use lov_shared::types::{DamageType, EntityId, EntityKind, GameEvent, TeamOrNeutral, Vec2};

use crate::systems::movement::DashData;
use crate::world::{DamageContext, DataTable, Entity, HealContext, World};

/// Fenêtre (s) pendant laquelle un champion ayant infligé des dégâts compte pour un assist.
pub const ASSIST_WINDOW: f64 = 10.0;
/// Multiplicateur des coups critiques.
pub const CRIT_MULTIPLIER: f64 = 1.75;

/// `Math.round` de JavaScript (arrondi vers +infini sur les demis).
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `entity.data.owner` : entité créditée pour les invocations, zones, projectiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner(pub EntityId);

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldInstance {
    pub amount: f64,
    /// Temps de jeu (s) d'expiration ; `INFINITY` = permanent jusqu'à consommation.
    pub until: f64,
    pub source: Option<EntityId>,
    pub id: Option<String>,
}

/// État de combat namespacé (`entity.data.combat`). Lecture libre pour les autres domaines.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatData {
    /// Dernier temps (s) où l'entité a subi / infligé des dégâts (hors combat, rappel, régénération...).
    pub last_damaged_at: f64,
    pub last_dealt_at: f64,
    /// Dernière fois que l'entité a été touchée par un champion ennemi, par id de champion, dans
    /// l'ordre de première insertion (comme un `Map` JS).
    pub champion_damagers: Vec<(EntityId, f64)>,
    pub shields: Vec<ShieldInstance>,
    /// Mort : temps de jeu de la mort.
    pub died_at: Option<f64>,
}

impl Default for CombatData {
    fn default() -> Self {
        Self {
            last_damaged_at: f64::NEG_INFINITY,
            last_dealt_at: f64::NEG_INFINITY,
            champion_damagers: Vec::new(),
            shields: Vec::new(),
            died_at: None,
        }
    }
}

impl CombatData {
    /// `championDamagers.set(id, t)` : met à jour sur place si présent, sinon ajoute à la fin.
    pub fn set_damager(&mut self, id: EntityId, t: f64) {
        match self.champion_damagers.iter_mut().find(|(i, _)| *i == id) {
            Some(slot) => slot.1 = t,
            None => self.champion_damagers.push((id, t)),
        }
    }
}

fn combat_in(data: &mut DataTable) -> &mut CombatData {
    data.get_or_insert_with(CombatData::default)
}

/// `combatData(e)` : état de combat de l'entité, créé à la demande.
pub fn combat_data(e: &mut Entity) -> &mut CombatData {
    combat_in(&mut e.data)
}

/// Secondes depuis le dernier échange de dégâts (subis ou infligés). 0 si l'entité n'existe pas.
pub fn time_since_combat(world: &mut World, id: EntityId) -> f64 {
    let time = world.time;
    let Some(e) = world.get_mut(id) else {
        return 0.0;
    };
    let c = combat_data(e);
    time - c.last_damaged_at.max(c.last_dealt_at)
}

/// Entité créditée pour une source : suit `data.owner` pour les invocations, zones, projectiles.
pub fn credit_of(world: &World, source: Option<EntityId>) -> Option<EntityId> {
    let mut cur = source;
    for _ in 0..4 {
        let c = cur?;
        let Some(Owner(owner)) = world.get(c).and_then(|e| e.data.get::<Owner>()).copied() else {
            return cur;
        };
        if !world.contains(owner) {
            return cur;
        }
        cur = Some(owner);
    }
    cur
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageOpts {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub amount: f64,
    pub damage_type: DamageType,
    /// Id du sort (ex. `ilyra_q`) : transmis dans l'événement `damage`.
    pub ability: Option<String>,
    pub is_auto_attack: bool,
    pub crit: bool,
    /// N'émet pas d'événement `damage` (DoT très fréquents agrégés ailleurs). Défaut false.
    pub silent: bool,
}

impl DamageOpts {
    pub fn new(
        source: Option<EntityId>,
        target: EntityId,
        amount: f64,
        damage_type: DamageType,
    ) -> Self {
        Self {
            source,
            target,
            amount,
            damage_type,
            ability: None,
            is_auto_attack: false,
            crit: false,
            silent: false,
        }
    }
}

/// Applique des dégâts : hooks before_damage -> résistances/pénétration -> boucliers -> PV -> lifesteal
/// (AA) -> mort. Retourne les dégâts effectivement infligés (bouclier absorbé inclus).
pub fn apply_damage(world: &mut World, opts: DamageOpts) -> f64 {
    let target = opts.target;
    match world.get(target) {
        Some(t) if !t.dead && opts.amount > 0.0 => {}
        _ => return 0.0,
    }
    let source = opts.source.filter(|&s| world.contains(s));
    let credit = credit_of(world, source);
    let mut ctx = DamageContext {
        source,
        credit,
        target,
        amount: opts.amount,
        damage_type: opts.damage_type,
        ability: opts.ability,
        is_auto_attack: opts.is_auto_attack,
        crit: opts.crit,
        flat_pen: 0.0,
        percent_pen: 0.0,
        multiplier: 1.0,
    };
    world.run_before_damage(&mut ctx);
    if ctx.amount <= 0.0 || ctx.multiplier <= 0.0 {
        return 0.0;
    }

    let mut amount = ctx.amount;
    if ctx.damage_type != DamageType::True {
        let physical = ctx.damage_type == DamageType::Physical;
        let Some(t) = world.get(target) else {
            return 0.0;
        };
        let resist = if physical { t.stats.armor } else { t.stats.mr };
        let pen = credit.and_then(|c| world.get(c)).map(|c| {
            if physical {
                c.stats.armor_pen
            } else {
                c.stats.magic_pen
            }
        });
        let flat = pen.unwrap_or(0.0) + ctx.flat_pen;
        amount *= damage_multiplier(effective_resist(resist, ctx.percent_pen, flat));
    }
    amount *= ctx.multiplier;
    if amount <= 0.0 {
        return 0.0;
    }

    // Boucliers d'abord (les plus proches de l'expiration en premier).
    let time = world.time;
    let Some(t) = world.get_mut(target) else {
        return 0.0;
    };
    let cd = combat_in(&mut t.data);
    let mut remaining = amount;
    if t.shield > 0.0 && !cd.shields.is_empty() {
        cd.shields.sort_by(|a, b| a.until.total_cmp(&b.until));
        for s in cd.shields.iter_mut() {
            let absorbed = s.amount.min(remaining);
            s.amount -= absorbed;
            remaining -= absorbed;
            if remaining <= 0.0 {
                break;
            }
        }
        cd.shields.retain(|s| s.amount > 0.0);
        t.shield = sum_shields(&cd.shields);
    }
    let hp_loss = t.hp.min(remaining);
    t.hp -= remaining;
    let dealt = amount - remaining + hp_loss;

    cd.last_damaged_at = time;
    let target_team = t.team;
    let target_is_champion = t.kind == EntityKind::Champion;
    let mut enemy_champion_credit = false;
    if let Some(c) = credit
        && let Some(ce) = world.get_mut(c)
    {
        combat_in(&mut ce.data).last_dealt_at = time;
        if ce.team != target_team
            && target_is_champion
            && let Some(champ) = ce.champ.as_mut()
        {
            enemy_champion_credit = true;
            champ.damage_to_champions += dealt;
        }
    }
    if enemy_champion_credit
        && let Some(c) = credit
        && let Some(t) = world.get_mut(target)
    {
        combat_in(&mut t.data).set_damager(c, time);
    }

    if !opts.silent {
        world.emit(GameEvent::Damage {
            source: credit.or(source).unwrap_or(0),
            target,
            amount: js_round(dealt),
            damage_type: ctx.damage_type,
            crit: ctx.crit.then_some(true),
            ability: ctx.ability.clone().filter(|a| !a.is_empty()),
        });
    }

    if ctx.is_auto_attack
        && let Some(c) = credit
        && let Some(ce) = world.get(c)
        && ce.stats.lifesteal > 0.0
        && !ce.dead
    {
        let heal = dealt * ce.stats.lifesteal;
        apply_heal(world, HealOpts::new(Some(c), c, heal));
    }

    world.run_after_damage(&mut ctx, dealt);

    if let Some(t) = world.get(target)
        && t.hp <= 0.0
        && !t.dead
    {
        let prevented = world.run_prevent_death(target, credit);
        if prevented && world.get(target).is_some_and(|t| t.hp > 0.0) {
            return dealt;
        }
        kill(world, target, credit);
    }
    dealt
}

/// Résistance effective : % de pénétration puis pénétration fixe, la fixe ne descend pas sous 0.
pub fn effective_resist(resist: f64, percent_pen: f64, flat_pen: f64) -> f64 {
    if resist <= 0.0 {
        return resist;
    }
    let after_percent = resist * (1.0 - 1.0f64.min(0.0f64.max(percent_pen)));
    0.0f64.max(after_percent - flat_pen)
}

/// Dégâts après résistances, sans effet de bord (prévisualisation, tests, IA).
pub fn preview_damage(
    source: Option<&Entity>,
    target: &Entity,
    amount: f64,
    damage_type: DamageType,
) -> f64 {
    if damage_type == DamageType::True {
        return amount;
    }
    let physical = damage_type == DamageType::Physical;
    let resist = if physical {
        target.stats.armor
    } else {
        target.stats.mr
    };
    let flat = source.map_or(0.0, |s| {
        if physical {
            s.stats.armor_pen
        } else {
            s.stats.magic_pen
        }
    });
    amount * damage_multiplier(effective_resist(resist, 0.0, flat))
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealOpts {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub amount: f64,
    pub silent: bool,
}

impl HealOpts {
    pub fn new(source: Option<EntityId>, target: EntityId, amount: f64) -> Self {
        Self {
            source,
            target,
            amount,
            silent: false,
        }
    }
}

/// Soigne (plafonné aux PV max). Retourne le soin effectif. Hook : before_heal (anti-soin, amplification).
pub fn apply_heal(world: &mut World, opts: HealOpts) -> f64 {
    let target = opts.target;
    match world.get(target) {
        Some(t) if !t.dead && opts.amount > 0.0 => {}
        _ => return 0.0,
    }
    let mut ctx = HealContext {
        source: opts.source.filter(|&s| world.contains(s)),
        target,
        amount: opts.amount,
    };
    world.run_before_heal(&mut ctx);
    let Some(t) = world.get_mut(target) else {
        return 0.0;
    };
    let healed = 0.0f64.max(ctx.amount.min(t.stats.max_hp - t.hp));
    if healed <= 0.0 {
        return 0.0;
    }
    t.hp += healed;
    if !opts.silent && healed >= 1.0 {
        world.emit(GameEvent::Heal {
            source: ctx.source.unwrap_or(target),
            target,
            amount: js_round(healed),
        });
    }
    healed
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldOpts {
    pub source: Option<EntityId>,
    pub target: EntityId,
    pub amount: f64,
    /// Durée en secondes (défaut : jusqu'à consommation).
    pub duration: Option<f64>,
    /// Id du bouclier : un nouveau bouclier avec le même id remplace l'ancien (ex. `grondak_p`).
    pub id: Option<String>,
}

impl ShieldOpts {
    pub fn new(source: Option<EntityId>, target: EntityId, amount: f64) -> Self {
        Self {
            source,
            target,
            amount,
            duration: None,
            id: None,
        }
    }
}

/// Ajoute un bouclier. `entity.shield` est la somme des boucliers actifs (expirés par le système combat).
pub fn apply_shield(world: &mut World, opts: ShieldOpts) -> f64 {
    let target = opts.target;
    match world.get(target) {
        Some(t) if !t.dead && opts.amount > 0.0 => {}
        _ => return 0.0,
    }
    let source = opts.source.filter(|&s| world.contains(s));
    let time = world.time;
    let Some(t) = world.get_mut(target) else {
        return 0.0;
    };
    let cd = combat_in(&mut t.data);
    if let Some(id) = opts.id.as_deref().filter(|i| !i.is_empty()) {
        cd.shields.retain(|s| s.id.as_deref() != Some(id));
    }
    cd.shields.push(ShieldInstance {
        amount: opts.amount,
        until: opts.duration.map_or(f64::INFINITY, |d| time + d),
        source,
        id: opts.id,
    });
    t.shield = sum_shields(&cd.shields);
    world.emit(GameEvent::Shield {
        source: source.unwrap_or(target),
        target,
        amount: js_round(opts.amount),
    });
    opts.amount
}

/// Retire un bouclier par id (ou tous si id absent).
pub fn remove_shield(target: &mut Entity, id: Option<&str>) {
    let cd = combat_in(&mut target.data);
    match id.filter(|i| !i.is_empty()) {
        Some(id) => cd.shields.retain(|s| s.id.as_deref() != Some(id)),
        None => cd.shields.clear(),
    }
    target.shield = sum_shields(&cd.shields);
}

/// Expire les boucliers échus à `time` (appelé chaque tick par le système combat).
pub fn expire_shields(time: f64, e: &mut Entity) {
    let Some(cd) = e.data.get_mut::<CombatData>() else {
        if e.shield != 0.0 {
            e.shield = 0.0;
        }
        return;
    };
    if cd.shields.is_empty() {
        return;
    }
    if cd.shields.iter().any(|s| s.until <= time) {
        cd.shields.retain(|s| s.until > time);
        e.shield = sum_shields(&cd.shields);
    }
}

fn sum_shields(list: &[ShieldInstance]) -> f64 {
    let mut t = 0.0;
    for s in list {
        t += s.amount;
    }
    t
}

/// Durée de mort par défaut : 6 + 2,5 x niveau, +2 % par minute au-delà de 15:00 (max +50 %).
pub fn default_respawn_time(level: u32, game_time: f64) -> f64 {
    let base = 6.0 + f64::from(level) * 2.5;
    let late_factor = 0.5f64.min(0.0f64.max((game_time - 15.0 * 60.0) / 60.0) * 0.02);
    base * (1.0 + late_factor)
}

/// Assistants d'un kill : champions ennemis de la victime ayant infligé des dégâts dans les
/// `ASSIST_WINDOW` secondes (dans l'ordre de première touche).
pub fn assists_for(world: &World, victim: EntityId, killer: Option<EntityId>) -> Vec<EntityId> {
    let mut out = Vec::new();
    let Some(v) = world.get(victim) else {
        return out;
    };
    let Some(cd) = v.data.get::<CombatData>() else {
        return out;
    };
    for &(id, t) in &cd.champion_damagers {
        if world.time - t > ASSIST_WINDOW || Some(id) == killer {
            continue;
        }
        if let Some(e) = world.get(id)
            && e.champ.is_some()
            && e.team != v.team
        {
            out.push(id);
        }
    }
    out
}

/// Donne un crédit d'assist à `helper` sur `victim` (CC, soins d'un allié qui combat...).
pub fn mark_assist(world: &mut World, helper: EntityId, victim: EntityId) {
    let time = world.time;
    let Some(h) = world.get(helper) else { return };
    if h.champ.is_none() {
        return;
    }
    let team = h.team;
    if let Some(v) = world.get_mut(victim)
        && team != v.team
    {
        combat_in(&mut v.data).set_damager(helper, time);
    }
}

/// Tue une entité (sans passer par les dégâts : exécutions, laser de fontaine, despawn...).
/// Champion : K/D/A, timer de respawn. Émet `death` (killer = champion crédité si un champion a touché
/// la victime dans les 10 dernières secondes et que le coup final vient d'une unité non-champion).
pub fn kill(world: &mut World, target: EntityId, killer: Option<EntityId>) {
    let time = world.time;
    let Some(t) = world.get_mut(target) else {
        return;
    };
    if t.dead {
        return;
    }
    t.hp = 0.0;
    t.dead = true;
    t.anim = Some("death".to_owned());
    t.move_target = None;
    t.path = None;
    t.attack_target = None;
    t.data.remove::<DashData>();
    remove_shield(t, None);
    combat_in(&mut t.data).died_at = Some(time);
    let target_team = t.team;
    let target_is_champ = t.champ.is_some();

    let mut credited = killer;
    let killer_is_champ = killer
        .and_then(|k| world.get(k))
        .is_some_and(|k| k.champ.is_some());
    if target_is_champ && !killer_is_champ {
        // Exécuté par une tour / un sbire : le dernier champion ennemi à l'avoir touché récupère le kill.
        let mut best: Option<EntityId> = None;
        let mut best_t = f64::NEG_INFINITY;
        if let Some(cd) = world.get(target).and_then(|t| t.data.get::<CombatData>()) {
            for &(id, t) in &cd.champion_damagers {
                if time - t > ASSIST_WINDOW || t <= best_t {
                    continue;
                }
                if let Some(e) = world.get(id)
                    && e.champ.is_some()
                    && e.team != target_team
                {
                    best = Some(id);
                    best_t = t;
                }
            }
        }
        if best.is_some() {
            credited = best;
        }
    }

    let assists = if target_is_champ {
        assists_for(world, target, credited)
    } else {
        Vec::new()
    };
    let bounty = world.hook_bounty(target, credited, &assists);

    if target_is_champ {
        // `deaths++` avant les hooks de respawn, comme en TypeScript : un hook peut lire le compteur.
        if let Some(ch) = world.get_mut(target).and_then(|t| t.champ.as_mut()) {
            ch.deaths += 1;
        }
        let respawn = world.hook_respawn_time(target);
        let credited_champ_enemy = credited
            .and_then(|c| world.get(c))
            .is_some_and(|c| c.champ.is_some() && c.team != target_team);
        if let Some(t) = world.get_mut(target) {
            if let Some(ch) = t.champ.as_mut() {
                ch.respawn_in = respawn.unwrap_or_else(|| default_respawn_time(ch.level, time));
            }
            combat_in(&mut t.data).champion_damagers.clear();
        }
        if credited_champ_enemy
            && let Some(ch) = credited
                .and_then(|c| world.get_mut(c))
                .and_then(|c| c.champ.as_mut())
        {
            ch.kills += 1;
        }
        for &a in &assists {
            if let Some(ch) = world.get_mut(a).and_then(|a| a.champ.as_mut()) {
                ch.assists += 1;
            }
        }
    }

    world.emit(GameEvent::Death {
        target,
        killer: credited,
        assists: (!assists.is_empty()).then_some(assists),
        bounty,
    });
}

/// Ressuscite un champion à sa fontaine (PV/mana pleins). Émet `respawn`.
pub fn respawn_champion(world: &mut World, id: EntityId) {
    let Some(e) = world.get_mut(id) else { return };
    let f = if e.team == TeamOrNeutral::Red {
        FOUNTAINS.red
    } else {
        FOUNTAINS.blue
    };
    e.dead = false;
    e.hp = e.stats.max_hp;
    e.mana = e.stats.max_mana;
    e.pos = Vec2 { x: f.x, y: f.y };
    e.anim = Some("idle".to_owned());
    e.move_target = None;
    e.path = None;
    e.attack_target = None;
    if let Some(c) = e.champ.as_mut() {
        c.respawn_in = 0.0;
    }
    let cd = combat_in(&mut e.data);
    cd.died_at = None;
    cd.last_damaged_at = f64::NEG_INFINITY;
    cd.last_dealt_at = f64::NEG_INFINITY;
    world.emit(GameEvent::Respawn {
        target: id,
        pos: Vec2 { x: f.x, y: f.y },
    });
}
