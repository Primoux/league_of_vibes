//! Combat : auto-attaques, poursuite, attaque-déplacement, mort/respawn, fontaine.
//! Port de `packages/server/src/systems/combat.ts`.
//!
//! Contrat pour les autres systèmes (sbires, tours, monstres utilisent aussi ce combat) :
//! - `entity.attack_target = Some(id)` : attaque cette cible (poursuite automatique si l'entité peut bouger).
//! - `entity.data.insert(AttackConfig { .. })` : surcharge du temps d'armement (fraction de la période,
//!   défaut `champion.attack_windup` sinon 0.3) et de la vitesse du projectile (0 = mêlée ; défaut
//!   `champion.projectile_speed`, sinon 1600 si portée > 300).
//! - `entity.data.insert(KeepCorpse)` : le combat ne retire pas l'entité morte (sinon sbires/monstres/
//!   balises sont retirés `CORPSE_SECONDS` après leur mort).
//! - Hooks : `can_attack`, `is_targetable`, `before_attack_hit`, `after_attack_hit` (voir `world.rs`).
//!
//! Différences avec le TypeScript, imposées par l'emprunt Rust :
//! - les entités sont parcourues par id (copie des ids dans un tampon réutilisé) ;
//! - `entity.data.aa` devient `AttackState` (`Copy`) : le système en travaille sur une copie locale,
//!   réécrite dans l'entité avant tout appel qui peut atteindre un hook (lancement de l'attaque), pour
//!   que les hooks voient et puissent modifier l'état (`reset_attack_timer`…) ;
//! - la liste des projectiles d'auto-attaque (un `WeakMap<World, …>` en TS) appartient au système ;
//!   `pending_attack_projectiles` se lit via la poignée `AttackProjectiles` renvoyée avec lui ;
//! - `Math.random()` du crit devient `world.rng` (le crit n'est tiré que si `crit_chance > 0`, comme en TS) ;
//! - une entité créée pendant le tick (par un hook) n'est visitée qu'au tick suivant.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lov_shared::data::champions::get_champion;
use lov_shared::map::FOUNTAINS;
use lov_shared::math::{angle_to, dist};
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, TEAMS, Team, TeamOrNeutral, Vec2,
};

use crate::combat::damage::{
    CRIT_MULTIPLIER, CombatData, DamageOpts, HealOpts, apply_damage, apply_heal, expire_shields,
    respawn_champion,
};
use crate::systems::movement::DashData;
use crate::systems::vision::is_visible_to;
use crate::world::{AttackContext, Entity, System, World};

/// Délai avant retrait du cadavre d'un sbire / monstre / balise.
pub const CORPSE_SECONDS: f64 = 2.0;
/// Fontaine : soin des alliés (fraction des PV/mana max par seconde) et laser sur les ennemis (dégâts bruts / s).
pub const FOUNTAIN_HEAL_PER_SEC: f64 = 0.12;
pub const FOUNTAIN_LASER_DPS: f64 = 1800.0;
pub const FOUNTAIN_LASER_RANGE: f64 = 1200.0;
const FOUNTAIN_HEAL_RADIUS: f64 = 1000.0;
/// Distance d'acquisition d'une cible en attaque-déplacement (en plus de la portée).
pub const ATTACK_MOVE_ACQUIRE: f64 = 250.0;

/// `entity.data.attackConfig` : surcharge de l'armement et du projectile (`None` = valeur par défaut).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AttackConfig {
    pub windup: Option<f64>,
    pub projectile_speed: Option<f64>,
}

/// `entity.data.keepCorpse = true` : la présence de ce marqueur empêche le retrait du cadavre.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeepCorpse;

/// `entity.data.aa` : état d'auto-attaque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackState {
    /// Temps restant avant l'impact/lancement ; < 0 = pas d'attaque en cours.
    pub windup: f64,
    /// Temps restant avant de pouvoir démarrer une nouvelle attaque.
    pub cooldown: f64,
    /// Cible de l'attaque en cours d'armement.
    pub windup_target: Option<EntityId>,
    pub crit: bool,
    /// Poursuite en cours (`move_target` posé par le combat).
    pub chasing: bool,
    /// Destination d'attaque-déplacement.
    pub attack_move: Option<Vec2>,
}

impl Default for AttackState {
    fn default() -> Self {
        Self {
            windup: -1.0,
            cooldown: 0.0,
            windup_target: None,
            crit: false,
            chasing: false,
            attack_move: None,
        }
    }
}

/// Projectile d'auto-attaque en vol.
#[derive(Debug, Clone, Copy)]
struct AaProjectile {
    attacker: EntityId,
    target: EntityId,
    x: f64,
    y: f64,
    speed: f64,
    crit: bool,
}

/// `attackState(e)` : état d'auto-attaque de l'entité, créé à la demande.
pub fn attack_state(e: &mut Entity) -> &mut AttackState {
    e.data.get_or_insert_with(AttackState::default)
}

/// Démarre une attaque-déplacement vers `pos` (appelé par input).
pub fn order_attack_move(e: &mut Entity, pos: Vec2) {
    attack_state(e).attack_move = Some(pos);
    e.attack_target = None;
    e.move_target = Some(pos);
    e.path = None;
}

fn set_anim(anim: &mut Option<String>, value: &str) {
    match anim {
        Some(a) if a == value => {}
        Some(a) => {
            a.clear();
            a.push_str(value);
        }
        None => *anim = Some(value.to_owned()),
    }
}

fn anim_is(anim: &Option<String>, value: &str) -> bool {
    anim.as_deref() == Some(value)
}

/// Corps de `cancelAttack` sur un état et une animation.
fn cancel_state(st: &mut AttackState, anim: &mut Option<String>) {
    if st.windup < 0.0 {
        return;
    }
    st.windup = -1.0;
    st.cooldown = 0.0;
    st.windup_target = None;
    if anim_is(anim, "attack") {
        set_anim(anim, "idle");
    }
}

/// Annule l'attaque en cours d'armement (déplacement, sort, CC). Le timer d'attaque est rendu.
pub fn cancel_attack(e: &mut Entity) {
    let Some(st) = e.data.get_mut::<AttackState>() else {
        return;
    };
    cancel_state(st, &mut e.anim);
}

/// Réinitialise le timer d'auto-attaque (sorts « reset d'AA »).
pub fn reset_attack_timer(e: &mut Entity) {
    attack_state(e).cooldown = 0.0;
}

/// Armement (fraction de la période) et vitesse de projectile (0 = mêlée) d'une entité.
///
/// Un champion dont l'id est absent du registre (`get_champion` renvoie `None`) est traité comme une
/// entité sans définition : défauts 0.3 et (1600 si portée > 300, sinon 0). Le TS lèverait une erreur.
pub fn attack_config(e: &Entity) -> (f64, f64) {
    let cfg = e.data.get::<AttackConfig>().copied().unwrap_or_default();
    let mut windup = cfg.windup;
    let mut speed = cfg.projectile_speed;
    if let Some(champ) = &e.champ
        && (windup.is_none() || speed.is_none())
        && let Some(def) = get_champion(&champ.champion_id)
    {
        windup = windup.or(Some(def.attack_windup));
        speed = speed.or(Some(if def.ranged {
            def.projectile_speed.unwrap_or(2000.0)
        } else {
            0.0
        }));
    }
    (
        windup.unwrap_or(0.3),
        speed.unwrap_or(if e.stats.attack_range > 300.0 {
            1600.0
        } else {
            0.0
        }),
    )
}

/// Distance bord à bord.
pub fn edge_distance(a: &Entity, b: &Entity) -> f64 {
    dist(a.pos, b.pos) - a.radius - b.radius
}

/// Vrai si `attacker` peut viser `target` : vivant, ennemi, visible, ciblable.
pub fn is_valid_attack_target(world: &World, attacker: EntityId, target: Option<EntityId>) -> bool {
    let (Some(t), Some(a)) = (world.get_opt(target), world.get(attacker)) else {
        return false;
    };
    if t.dead
        || t.id == a.id
        || matches!(t.kind, EntityKind::Projectile | EntityKind::Zone)
        || t.team == a.team
    {
        return false;
    }
    if a.team != TeamOrNeutral::Neutral && !is_visible_to(t, a.team, true) {
        return false;
    }
    world.is_targetable(t.id, a.id)
}

/// Poignée de lecture du nombre de projectiles d'auto-attaque en vol (`pendingAttackProjectiles`).
#[derive(Debug, Clone)]
pub struct AttackProjectiles(Arc<AtomicUsize>);

impl AttackProjectiles {
    /// Projectiles d'auto-attaque en vol (tests, debug).
    pub fn pending(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

/// Tampons et projectiles propres au système.
struct State {
    ids: Vec<EntityId>,
    projectiles: Vec<AaProjectile>,
    pending: Arc<AtomicUsize>,
}

/// Le système de combat (à placer après `movement`, avant `vision`).
pub fn combat_system() -> System {
    combat_system_with_projectiles().0
}

/// `combat_system` et la poignée de lecture des projectiles en vol.
pub fn combat_system_with_projectiles() -> (System, AttackProjectiles) {
    let pending = Arc::new(AtomicUsize::new(0));
    let handle = AttackProjectiles(Arc::clone(&pending));
    let mut st = State {
        ids: Vec::new(),
        projectiles: Vec::new(),
        pending,
    };
    (
        System::new("combat", move |world, dt| update(world, dt, &mut st)),
        handle,
    )
}

fn update(world: &mut World, dt: f64, st: &mut State) {
    world.ids_into(None, &mut st.ids);
    for k in 0..st.ids.len() {
        let id = st.ids[k];
        let time = world.time;
        let Some(e) = world.get_mut(id) else { continue };
        expire_shields(time, e);
        if e.dead {
            handle_dead(world, id, dt);
            continue;
        }
        if matches!(
            e.kind,
            EntityKind::Projectile | EntityKind::Zone | EntityKind::Ward
        ) {
            continue;
        }
        update_attacker(world, st, id, dt);
    }
    update_projectiles(world, st, dt);
    update_fountains(world, &mut st.ids, dt);
}

fn handle_dead(world: &mut World, id: EntityId, dt: f64) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    if let Some(c) = e.champ.as_mut() {
        c.respawn_in -= dt;
        if c.respawn_in <= 0.0 {
            respawn_champion(world, id);
        }
        return;
    }
    if matches!(
        e.kind,
        EntityKind::Champion | EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus
    ) || e.data.contains::<KeepCorpse>()
    {
        return;
    }
    let died_at = e
        .data
        .get::<CombatData>()
        .and_then(|c| c.died_at)
        .unwrap_or(time);
    if time - died_at >= CORPSE_SECONDS {
        world.remove(id);
    }
}

fn update_attacker(world: &mut World, st: &mut State, id: EntityId, dt: f64) {
    let Some(e) = world.get_mut(id) else { return };
    if !e.data.contains::<AttackState>() && e.attack_target.is_none() {
        return;
    }
    let mut s = *attack_state(e);
    // Après un lancement, l'état a déjà été écrit (et peut avoir été modifié par des hooks).
    if !step_attacker(world, st, id, dt, &mut s)
        && let Some(e) = world.get_mut(id)
    {
        *attack_state(e) = s;
    }
}

/// Corps de `updateAttacker`. Renvoie vrai si une attaque a été lancée (état déjà réécrit).
fn step_attacker(
    world: &mut World,
    st: &mut State,
    id: EntityId,
    dt: f64,
    s: &mut AttackState,
) -> bool {
    let (range, move_speed, attack_speed, crit_chance) = {
        let Some(e) = world.get_mut(id) else {
            return false;
        };
        s.cooldown -= dt;
        if anim_is(&e.anim, "attack") && s.windup < 0.0 && s.cooldown <= 0.0 {
            set_anim(&mut e.anim, "idle");
        }
        if e.data.contains::<DashData>() {
            cancel_state(s, &mut e.anim);
            return false;
        }
        (
            e.stats.attack_range,
            e.stats.move_speed,
            e.stats.attack_speed,
            e.stats.crit_chance,
        )
    };

    // Attaque-déplacement : acquisition de la cible la plus proche.
    let has_target = world.get(id).is_some_and(|e| e.attack_target.is_some());
    if !has_target && s.attack_move.is_some() {
        let found = acquire_target(world, id, range + ATTACK_MOVE_ACQUIRE);
        let Some(e) = world.get_mut(id) else {
            return false;
        };
        if let Some(f) = found {
            e.attack_target = Some(f);
        } else if e.move_target.is_none() {
            s.attack_move = None;
        }
    }

    let target = world.get(id).and_then(|e| e.attack_target);
    let valid = is_valid_attack_target(world, id, target);
    let Some(tid) = target.filter(|_| valid) else {
        if let Some(e) = world.get_mut(id) {
            if e.attack_target.is_some() {
                drop_target(e, s);
            }
            if s.windup >= 0.0 {
                cancel_state(s, &mut e.anim);
            }
        }
        return false;
    };
    let Some((tpos, tradius)) = world.get(tid).map(|t| (t.pos, t.radius)) else {
        return false;
    };

    let Some(e) = world.get_mut(id) else {
        return false;
    };
    let in_range = dist(e.pos, tpos) - e.radius - tradius <= range;
    if s.windup >= 0.0 {
        // Armement en cours : se termine même si la cible s'éloigne un peu (comme dans LoL).
        if s.windup_target != Some(tid) {
            cancel_state(s, &mut e.anim);
        } else {
            e.facing = angle_to(e.pos, tpos);
            s.windup -= dt;
            if s.windup <= 0.0 {
                launch(world, st, id, tid, s);
                return true;
            }
            return false;
        }
    }

    if !in_range {
        if move_speed > 0.0 {
            s.chasing = true;
            let stale = e.move_target.is_none_or(|m| m.x != tpos.x || m.y != tpos.y);
            if stale {
                e.move_target = Some(tpos);
            }
        }
        return false;
    }

    if e.move_target.is_some() {
        e.move_target = None;
        e.path = None;
        s.chasing = false;
    }
    if s.cooldown > 0.0 || attack_speed <= 0.0 || !world.can_attack(id) {
        return false;
    }

    // Début de l'armement.
    let period = 1.0 / attack_speed;
    let Some(e) = world.get(id) else {
        return false;
    };
    let (windup, _) = attack_config(e);
    s.cooldown = period;
    s.windup = period * windup;
    s.windup_target = Some(tid);
    s.crit = crit_chance > 0.0 && world.rng.chance(crit_chance);
    let Some(e) = world.get_mut(id) else {
        return false;
    };
    set_anim(&mut e.anim, "attack");
    e.facing = angle_to(e.pos, tpos);
    s.windup -= dt;
    if s.windup <= 0.0 {
        launch(world, st, id, tid, s);
        return true;
    }
    false
}

fn drop_target(e: &mut Entity, s: &mut AttackState) {
    e.attack_target = None;
    if s.chasing {
        e.move_target = None;
        e.path = None;
        s.chasing = false;
    }
    if let Some(am) = s.attack_move {
        e.move_target = Some(am);
    }
}

fn acquire_target(world: &World, id: EntityId, range: f64) -> Option<EntityId> {
    let e = world.get(id)?;
    let mut best = None;
    let mut best_d = f64::INFINITY;
    for o in world.iter() {
        if matches!(o.kind, EntityKind::Projectile | EntityKind::Zone) {
            continue;
        }
        let d = edge_distance(e, o);
        if d > range || d >= best_d {
            continue;
        }
        if !is_valid_attack_target(world, id, Some(o.id)) {
            continue;
        }
        best = Some(o.id);
        best_d = d;
    }
    best
}

fn launch(world: &mut World, st: &mut State, id: EntityId, tid: EntityId, s: &mut AttackState) {
    s.windup = -1.0;
    s.windup_target = None;
    let Some(e) = world.get_mut(id) else { return };
    *attack_state(e) = *s;
    let (_, speed) = attack_config(e);
    let (x, y) = (e.pos.x, e.pos.y);
    let ranged = speed > 0.0;
    world.emit(GameEvent::AutoAttack {
        source: id,
        target: tid,
        ranged,
        speed: ranged.then_some(speed),
        crit: s.crit.then_some(true),
    });
    if ranged {
        st.projectiles.push(AaProjectile {
            attacker: id,
            target: tid,
            x,
            y,
            speed,
            crit: s.crit,
        });
        st.pending.store(st.projectiles.len(), Ordering::Relaxed);
    } else {
        hit(world, id, tid, s.crit);
    }
}

/// Résout l'impact d'une auto-attaque (hooks `before_attack_hit` / `after_attack_hit`). Exporté pour les
/// sorts « prochaine AA ». Renvoie les dégâts infligés.
pub fn hit(world: &mut World, attacker: EntityId, target: EntityId, crit: bool) -> f64 {
    if world.get(target).is_none_or(|t| t.dead) {
        return 0.0;
    }
    let Some(ad) = world.get(attacker).map(|a| a.stats.ad) else {
        return 0.0;
    };
    let mut ctx = AttackContext {
        attacker,
        target,
        amount: ad,
        crit,
        crit_multiplier: CRIT_MULTIPLIER,
        bonus: 0.0,
        damage_type: DamageType::Physical,
    };
    world.run_before_attack_hit(&mut ctx);
    let amount = ctx.amount * (if ctx.crit { ctx.crit_multiplier } else { 1.0 }) + ctx.bonus;
    let mut opts = DamageOpts::new(Some(attacker), target, amount, ctx.damage_type);
    opts.is_auto_attack = true;
    opts.crit = ctx.crit;
    let dealt = apply_damage(world, opts);
    world.run_after_attack_hit(&mut ctx, dealt);
    dealt
}

fn update_projectiles(world: &mut World, st: &mut State, dt: f64) {
    if st.projectiles.is_empty() {
        return;
    }
    let mut w = 0;
    for i in 0..st.projectiles.len() {
        let mut p = st.projectiles[i];
        let Some((tpos, tradius, tdead)) = world.get(p.target).map(|t| (t.pos, t.radius, t.dead))
        else {
            continue;
        };
        if tdead || !world.contains(p.attacker) {
            continue;
        }
        let d = (tpos.x - p.x).hypot(tpos.y - p.y);
        let step = p.speed * dt;
        if d <= step + tradius * 0.5 {
            hit(world, p.attacker, p.target, p.crit);
            continue;
        }
        p.x += ((tpos.x - p.x) / d) * step;
        p.y += ((tpos.y - p.y) / d) * step;
        st.projectiles[w] = p;
        w += 1;
    }
    st.projectiles.truncate(w);
    st.pending.store(w, Ordering::Relaxed);
}

fn team_of(t: Team) -> TeamOrNeutral {
    match t {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

fn update_fountains(world: &mut World, ids: &mut Vec<EntityId>, dt: f64) {
    world.ids_into(Some(EntityKind::Champion), ids);
    for team in TEAMS {
        let f = *FOUNTAINS.get(team);
        let own = team_of(team);
        for &id in ids.iter() {
            let Some(e) = world.get(id) else { continue };
            if e.dead {
                continue;
            }
            let d = dist(e.pos, f);
            if e.team == own {
                if d > FOUNTAIN_HEAL_RADIUS {
                    continue;
                }
                if e.hp < e.stats.max_hp {
                    let amount = e.stats.max_hp * FOUNTAIN_HEAL_PER_SEC * dt;
                    let mut o = HealOpts::new(None, id, amount);
                    o.silent = true;
                    apply_heal(world, o);
                }
                if let Some(e) = world.get_mut(id)
                    && e.mana < e.stats.max_mana
                {
                    e.mana = e
                        .stats
                        .max_mana
                        .min(e.mana + e.stats.max_mana * FOUNTAIN_HEAL_PER_SEC * dt);
                }
            } else if d <= FOUNTAIN_LASER_RANGE {
                let mut o = DamageOpts::new(None, id, FOUNTAIN_LASER_DPS * dt, DamageType::True);
                o.ability = Some("fountain_laser".to_owned());
                apply_damage(world, o);
            }
        }
    }
}
