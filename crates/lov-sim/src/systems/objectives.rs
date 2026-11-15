//! Objectifs épiques : port de `packages/server/src/systems/objectives.ts`.
//!
//! Drakes élémentaires (type de la Faille au troisième, âme au quatrième), Dragon ancestral (brûlure et
//! exécution), Larves (siège des tours, larvelins), Colosse du Rift (œil, invocation, charge),
//! Léviathan (buff d'équipe, sbires renforcés), souffle des monstres épiques.
//!
//! Différences avec le TypeScript :
//! - l'état d'une partie (un `WeakMap<World, ObjectiveState>` en TS) est partagé entre le système et sa
//!   poignée [`Objectives`] ; `getObjectiveState(world)` devient [`Objectives::lock`]. Aucun verrou n'est
//!   tenu pendant un appel qui peut redéclencher les hooks ;
//! - `getObjectiveTimers` ne publie plus que les objectifs : la jungle publie ses camps par son propre
//!   hook `objective_timers`, et le snapshot fusionne les deux comme avant ;
//! - `usesCoreCombat` et la présence du système `buffs` (lus dans `world.systems` en TS) sont des
//!   paramètres du constructeur ;
//! - `Math.random` devient `world.rng` ;
//! - les champs de `entity.data` sont des types : [`ElderBuff`], [`DragonElementTag`],
//!   [`ObjectiveClaimed`], [`PveLastCombat`], [`NextGrubling`], [`Grubling`], [`SummonedHerald`],
//!   [`NextLeviathanRefresh`], [`LeviathanMinion`] ; `heraldEye` et `leviathanBuff` sont ceux de `items`.

use std::sync::{Arc, Mutex, MutexGuard};

use lov_shared::constants::{MAP_SIZE, TIMERS, scaled};
use lov_shared::data::monsters::{DRAGON_ELEMENTS, MONSTERS};
use lov_shared::map::PITS;
use lov_shared::math::dist2;
use lov_shared::types::{
    DamageType, DragonElement, EntityId, EntityKind, GameEvent, MonsterType, ParamValue, Team,
    TeamOrNeutral, Vec2,
};

use crate::combat::buffs::{SourceMatch, remove_buff, update_buffs_with};
use crate::combat::damage::Owner;
use crate::items::{EconomyState, HeraldEye, LeviathanBuff};
use crate::systems::leveling::XpReward;
use crate::systems::structures::is_structure_attackable;
use crate::world::{EventKind, System, World};
use crate::world_ai::burns::{BurnKey, refresh_burn, take_burn_tick};
use crate::world_ai::monsters::{
    MonsterExtra, MonsterState, aggro_monster, install_monster_hooks, spawn_monster, update_monster,
};
use crate::world_ai::objective_effects::{install_objective_effects, update_objective_effects};
use crate::world_ai::runtime::{
    BuffOptions, DamageRequest, RETIRE_DELAY, Rewards, add_buff, apply_damage, retire, team_of,
};

/// Durée de l'œil du Héraut (s).
pub const HERALD_EYE_SECONDS: f64 = 240.0;
/// Durée de vie d'un Héraut invoqué (s).
pub const SUMMONED_HERALD_SECONDS: f64 = 180.0;
/// Rappel renforcé du Léviathan (s).
pub const LEVIATHAN_RECALL_SECONDS: f64 = 4.0;

// ------------------------------------------------------------------------------------------------
// État
// ------------------------------------------------------------------------------------------------

/// Un emplacement d'objectif : prochaine apparition, entités en vie, terminé pour la partie.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub next: f64,
    pub ids: Vec<EntityId>,
    pub done: bool,
}

impl Slot {
    fn new(timer: f64) -> Self {
        Self {
            next: scaled(timer),
            ids: Vec::new(),
            done: false,
        }
    }

    fn ready(&self, time: f64) -> bool {
        !self.done && self.ids.is_empty() && time >= self.next
    }
}

/// Progression d'une équipe.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamObjectives {
    pub dragons: Vec<DragonElement>,
    pub grubs: u32,
    pub soul: Option<DragonElement>,
    pub leviathan_until: f64,
    pub elder_until: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveState {
    pub dragon: Slot,
    pub grubs: Slot,
    pub herald: Slot,
    pub leviathan: Slot,
    pub dragon_count: u32,
    /// Type de la Faille, fixé au troisième drake.
    pub rift: Option<DragonElement>,
    /// L'Ancien remplace les drakes.
    pub elder: bool,
    pub blue: TeamObjectives,
    pub red: TeamObjectives,
}

impl Default for ObjectiveState {
    fn default() -> Self {
        Self {
            dragon: Slot::new(TIMERS.dragon_first_spawn),
            grubs: Slot::new(TIMERS.grubs_spawn),
            herald: Slot::new(TIMERS.herald_spawn),
            leviathan: Slot::new(TIMERS.leviathan_spawn),
            dragon_count: 0,
            rift: None,
            elder: false,
            blue: TeamObjectives::default(),
            red: TeamObjectives::default(),
        }
    }
}

impl ObjectiveState {
    pub fn team(&self, team: Team) -> &TeamObjectives {
        match team {
            Team::Blue => &self.blue,
            Team::Red => &self.red,
        }
    }

    pub fn team_mut(&mut self, team: Team) -> &mut TeamObjectives {
        match team {
            Team::Blue => &mut self.blue,
            Team::Red => &mut self.red,
        }
    }

    /// Timers des objectifs en attente d'apparition (`dragon` devient `elder_dragon` après une âme).
    pub fn timers(&self) -> std::collections::BTreeMap<String, f64> {
        let named = [
            (
                if self.elder { "elder_dragon" } else { "dragon" },
                &self.dragon,
            ),
            ("grubs", &self.grubs),
            ("herald", &self.herald),
            ("leviathan", &self.leviathan),
        ];
        named
            .into_iter()
            .filter(|(_, s)| !s.done && s.ids.is_empty())
            .map(|(name, s)| (name.to_owned(), s.next))
            .collect()
    }
}

/// Poignée sur l'état des objectifs d'une partie (le `getObjectiveState` du TS).
#[derive(Debug, Clone, Default)]
pub struct Objectives(Arc<Mutex<ObjectiveState>>);

impl Objectives {
    /// Verrouille l'état. Ne pas garder le verrou pendant un appel qui peut déclencher des hooks.
    pub fn lock(&self) -> MutexGuard<'_, ObjectiveState> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

// ------------------------------------------------------------------------------------------------
// Données posées sur les entités
// ------------------------------------------------------------------------------------------------

/// `data.elderBuff` : buff du Dragon ancestral (brûlure et exécution).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElderBuff {
    pub expires_at: f64,
}

/// `data.dragonElement` : élément d'un drake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragonElementTag(pub DragonElement);

/// `data.objectiveClaimed` : la récompense de ce monstre épique a été distribuée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectiveClaimed;

/// `data.pveLastCombat` : dernier échange de dégâts avec le PvE.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PveLastCombat(pub f64);

/// `data.nextGrubling` : prochaine apparition d'un larvelin pour ce champion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextGrubling(pub f64);

/// `data.grubling` : larvelin qui fonce sur une tour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grubling {
    pub target: EntityId,
    pub expires_at: f64,
}

/// `data.summonedHerald` : Colosse invoqué par un œil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SummonedHerald {
    pub owner: EntityId,
    pub expires_at: f64,
    pub next_charge: f64,
}

/// `data.nextLeviathanRefresh` : prochain rafraîchissement du renfort d'un sbire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextLeviathanRefresh(pub f64);

/// `data.leviathanMinion` : sbire renforcé par un porteur du buff du Léviathan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeviathanMinion;

// ------------------------------------------------------------------------------------------------
// Outils
// ------------------------------------------------------------------------------------------------

pub fn element_name(e: DragonElement) -> &'static str {
    match e {
        DragonElement::Infernal => "infernal",
        DragonElement::Ocean => "ocean",
        DragonElement::Mountain => "mountain",
        DragonElement::Cloud => "cloud",
        DragonElement::Hextech => "hextech",
        DragonElement::Chemtech => "chemtech",
    }
}

fn monster_type(subtype: &str) -> Option<MonsterType> {
    MONSTERS
        .iter()
        .map(|(t, _)| *t)
        .find(|t| t.as_str() == subtype)
}

fn dragon_params(e: DragonElement) -> Vec<(String, f64)> {
    let p: &[(&str, f64)] = match e {
        DragonElement::Infernal => &[("adPercent", 0.05), ("apPercent", 0.05)],
        DragonElement::Ocean => &[("missingHpRegen", 0.025)],
        DragonElement::Mountain => &[("armorPercent", 0.08), ("mrPercent", 0.08)],
        DragonElement::Cloud => &[("outOfCombatMoveSpeed", 0.04)],
        DragonElement::Hextech => &[("abilityHaste", 5.0), ("attackSpeed", 0.05)],
        DragonElement::Chemtech => &[("tenacity", 0.06), ("healPower", 0.06)],
    };
    p.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

fn soul_params(soul: DragonElement) -> Vec<(String, f64)> {
    if soul == DragonElement::Cloud {
        vec![("moveSpeedPercent".into(), 0.15)]
    } else {
        vec![("soul".into(), 1.0)]
    }
}

fn team_champions(world: &World, team: TeamOrNeutral, alive_only: bool) -> Vec<EntityId> {
    world
        .query(Some(EntityKind::Champion))
        .filter(|c| c.team == team && (!alive_only || !c.dead))
        .map(|c| c.id)
        .collect()
}

fn to_side(team: Team) -> TeamOrNeutral {
    match team {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

fn announce(world: &mut World, key: &str, team: Team, params: Option<(&str, &str)>) {
    world.emit(GameEvent::Announcement {
        key: key.into(),
        team: Some(team),
        params: params.map(|(k, v)| {
            [(k.to_owned(), ParamValue::Str(v.to_owned()))]
                .into_iter()
                .collect()
        }),
    });
}

/// Monstres épiques qui soufflent sur leur cible.
fn breathes(subtype: Option<&str>) -> bool {
    matches!(subtype, Some("dragon" | "elder_dragon" | "leviathan"))
}

// ------------------------------------------------------------------------------------------------
// Récompenses
// ------------------------------------------------------------------------------------------------

fn grant_dragon(
    world: &mut World,
    objectives: &Objectives,
    team: Team,
    element: DragonElement,
    killer: EntityId,
) {
    let (stacks, new_soul) = {
        let mut st = objectives.lock();
        let rift = st.rift;
        let progress = st.team_mut(team);
        progress.dragons.push(element);
        let stacks = progress.dragons.iter().filter(|&&d| d == element).count();
        let new_soul = if progress.dragons.len() == 4 && progress.soul.is_none() {
            let soul = rift.unwrap_or(element);
            progress.soul = Some(soul);
            Some(soul)
        } else {
            None
        };
        if new_soul.is_some() {
            st.elder = true;
        }
        (stacks, new_soul)
    };
    let champions = team_champions(world, to_side(team), false);
    for &c in &champions {
        add_buff(
            world,
            c,
            &format!("dragon_{}", element_name(element)),
            BuffOptions {
                source: Some(killer),
                stacks: Some(stacks as u32),
                params: dragon_params(element),
                ..BuffOptions::new(1e9)
            },
        );
    }
    let Some(soul) = new_soul else { return };
    for &c in &champions {
        add_buff(
            world,
            c,
            &format!("soul_{}", element_name(soul)),
            BuffOptions {
                source: Some(killer),
                params: soul_params(soul),
                ..BuffOptions::new(1e9)
            },
        );
    }
    announce(
        world,
        "dragon_soul",
        team,
        Some(("element", element_name(soul))),
    );
}

/// Remet les buffs permanents d'équipe (drakes, âme, Larves) à un champion réanimé.
fn sync_persistent_objectives(world: &mut World, objectives: &Objectives, champion: EntityId) {
    let Some(team) = world.get(champion).and_then(|c| team_of(c.team)) else {
        return;
    };
    let progress = objectives.lock().team(team).clone();
    for element in DRAGON_ELEMENTS {
        let stacks = progress.dragons.iter().filter(|&&d| d == element).count();
        if stacks > 0 {
            add_buff(
                world,
                champion,
                &format!("dragon_{}", element_name(element)),
                BuffOptions {
                    stacks: Some(stacks as u32),
                    params: dragon_params(element),
                    ..BuffOptions::new(1e9)
                },
            );
        }
    }
    if let Some(soul) = progress.soul {
        add_buff(
            world,
            champion,
            &format!("soul_{}", element_name(soul)),
            BuffOptions {
                params: soul_params(soul),
                ..BuffOptions::new(1e9)
            },
        );
    }
    if progress.grubs > 0 {
        add_buff(
            world,
            champion,
            "grub_siege",
            BuffOptions {
                stacks: Some(progress.grubs),
                params: vec![("structureDamage".into(), 8.0)],
                ..BuffOptions::new(1e9)
            },
        );
    }
}

/// Utilise l'œil du Héraut : invoque le Colosse sur le porteur. Ne consomme l'œil qu'après validation.
pub fn use_herald_eye(world: &mut World, owner: EntityId) -> bool {
    let time = world.time;
    let finished = world.winner.is_some();
    let Some(o) = world.get(owner) else {
        return false;
    };
    let Some(eye) = o.data.get::<HeraldEye>() else {
        return false;
    };
    if o.dead || o.team == TeamOrNeutral::Neutral || time >= eye.expires_at || finished {
        return false;
    }
    let pos = o.pos;
    summon_herald(world, owner, pos);
    if let Some(o) = world.get_mut(owner) {
        o.data.remove::<HeraldEye>();
    }
    true
}

fn summon_herald(world: &mut World, owner: EntityId, pos: Vec2) {
    let time = world.time;
    let Some(team) = world.get(owner).and_then(|o| team_of(o.team)) else {
        return;
    };
    let herald = spawn_monster(
        world,
        MonsterType::Herald,
        pos,
        30000.0,
        MonsterExtra::default(),
    );
    if let Some(h) = world.get_mut(herald) {
        h.team = to_side(team);
        h.subtype = Some("summoned_herald".into());
        h.data.insert(Owner(owner));
        h.data.insert(SummonedHerald {
            owner,
            expires_at: time + SUMMONED_HERALD_SECONDS,
            next_charge: 0.0,
        });
        h.data.remove::<MonsterState>();
        h.data.remove::<Rewards>();
    }
    announce(world, "herald_summoned", team, None);
}

fn spawn_epic(
    world: &mut World,
    t: MonsterType,
    x: f64,
    y: f64,
    element: Option<DragonElement>,
) -> EntityId {
    let pit = if matches!(t, MonsterType::Dragon | MonsterType::ElderDragon) {
        PITS.dragon
    } else {
        PITS.leviathan
    };
    let pos = Vec2 {
        x: pit.pos.x + x,
        y: pit.pos.y + y,
    };
    let id = spawn_monster(
        world,
        t,
        pos,
        pit.radius,
        MonsterExtra {
            epic: true,
            home: Some(pit.pos),
            camp: None,
        },
    );
    if let Some(element) = element
        && let Some(e) = world.get_mut(id)
    {
        e.data.insert(DragonElementTag(element));
    }
    world.emit(GameEvent::ObjectiveSpawn {
        monster: t,
        element,
        pos,
    });
    id
}

/// Tour ennemie attaquable la plus proche du Colosse invoqué (à distance égale, le plus petit id).
fn herald_tower(world: &World, herald: EntityId) -> Option<EntityId> {
    let h = world.get(herald)?;
    let mut best: Option<EntityId> = None;
    let mut distance = 30000.0f64 * 30000.0;
    for t in world.query(Some(EntityKind::Tower)) {
        if t.dead || t.team == h.team {
            continue;
        }
        let d = dist2(h.pos, t.pos);
        if d > distance
            || (d == distance && best.is_some_and(|b| t.id > b))
            || !is_structure_attackable(world, t.id)
        {
            continue;
        }
        best = Some(t.id);
        distance = d;
    }
    best
}

fn epic_breath(world: &mut World, entity: EntityId, target: EntityId) {
    let Some(e) = world.get(entity) else { return };
    if !breathes(e.subtype.as_deref()) {
        return;
    }
    let fx_key = match e.subtype.as_deref() {
        Some("dragon") => format!(
            "dragon_{}",
            e.data
                .get::<DragonElementTag>()
                .map_or("undefined", |t| element_name(t.0))
        ),
        other => other.unwrap_or_default().to_owned(),
    };
    let amount = e.stats.ad * 0.5;
    let Some(target_pos) = world.get(target).map(|t| t.pos) else {
        return;
    };
    world.emit(GameEvent::AreaEffect {
        source: entity,
        fx_key: fx_key.clone(),
        pos: target_pos,
        radius: 300.0,
        duration: 0.5,
    });
    let victims: Vec<EntityId> = world
        .query(Some(EntityKind::Champion))
        .filter(|v| !v.dead && v.id != target && dist2(v.pos, target_pos) <= 300.0 * 300.0)
        .map(|v| v.id)
        .collect();
    for victim in victims {
        apply_damage(
            world,
            DamageRequest {
                source: entity,
                target: victim,
                amount,
                damage_type: DamageType::Magic,
                ability: Some(fx_key.clone()),
            },
        );
    }
}

fn execute(world: &mut World, source: EntityId, target: EntityId) {
    let Some(t) = world.get(target) else { return };
    let amount = t.hp + t.shield + 1.0;
    apply_damage(
        world,
        DamageRequest {
            source,
            target,
            amount,
            damage_type: DamageType::True,
            ability: Some("elder_execute".into()),
        },
    );
}

// ------------------------------------------------------------------------------------------------
// Initialisation
// ------------------------------------------------------------------------------------------------

fn on_damage(
    world: &mut World,
    objectives: &Objectives,
    ctx: &crate::world::DamageContext,
    dealt: f64,
) {
    if dealt <= 0.0 {
        return;
    }
    let time = world.time;
    let Some(source) = ctx.credit.or(ctx.source) else {
        return;
    };
    let target = ctx.target;
    if world.get(target).is_none_or(|t| t.hp <= 0.0) || !world.contains(source) {
        return;
    }
    for id in [source, target] {
        if let Some(e) = world.get_mut(id) {
            e.data.insert(PveLastCombat(time));
        }
    }
    let epic = world
        .get(target)
        .and_then(|t| t.data.get::<MonsterState>())
        .is_some_and(|m| m.epic);
    if epic {
        aggro_monster(world, target, source);
    }
    let (Some(src), Some(tgt)) = (world.get(source), world.get(target)) else {
        return;
    };
    let Some(team) = team_of(src.team).filter(|_| src.champ.is_some()) else {
        return;
    };
    if tgt.team == src.team {
        return;
    }
    let (grubs, elder_until) = {
        let st = objectives.lock();
        (st.team(team).grubs, st.team(team).elder_until)
    };
    let (tgt_kind, tgt_dead, src_pos) = (tgt.kind, tgt.dead, src.pos);
    let has_elder = src.data.contains::<ElderBuff>();
    if tgt_kind == EntityKind::Tower && grubs > 0 && ctx.ability.as_deref() != Some("grub_siege") {
        apply_damage(
            world,
            DamageRequest {
                source,
                target,
                amount: 8.0 * f64::from(grubs),
                damage_type: DamageType::True,
                ability: Some("grub_siege".into()),
            },
        );
        let next = world
            .get(source)
            .and_then(|s| s.data.get::<NextGrubling>())
            .map_or(0.0, |n| n.0);
        if grubs >= 3 && time >= next {
            if let Some(s) = world.get_mut(source) {
                s.data.insert(NextGrubling(time + 15.0));
            }
            spawn_grubling(world, source, team, src_pos, target);
        }
    }
    let elder_hit = !ctx
        .ability
        .as_deref()
        .is_some_and(|a| a.starts_with("elder_"));
    if tgt_kind == EntityKind::Champion && elder_until > time && has_elder && !tgt_dead && elder_hit
    {
        refresh_burn(world, target, BurnKey::Elder, source);
        if world
            .get(target)
            .is_some_and(|t| t.hp / t.stats.max_hp < 0.2)
        {
            execute(world, source, target);
        }
    }
}

fn spawn_grubling(world: &mut World, owner: EntityId, team: Team, pos: Vec2, tower: EntityId) {
    let time = world.time;
    let id = spawn_monster(
        world,
        MonsterType::Grub,
        pos,
        30000.0,
        MonsterExtra::default(),
    );
    let Some(g) = world.get_mut(id) else { return };
    g.kind = EntityKind::Minion;
    g.team = to_side(team);
    g.subtype = Some("grubling".into());
    g.data.remove::<MonsterState>();
    g.data.remove::<Rewards>();
    g.data.insert(XpReward(0.0));
    g.hp = 250.0;
    g.stats.max_hp = 250.0;
    g.stats.ad = 25.0;
    g.stats.move_speed = 400.0;
    g.data.insert(Owner(owner));
    g.data.insert(Grubling {
        target: tower,
        expires_at: time + 15.0,
    });
}

fn on_death(
    world: &mut World,
    objectives: &Objectives,
    target: EntityId,
    killer: Option<EntityId>,
) {
    let time = world.time;
    let Some(t) = world.get_mut(target) else {
        return;
    };
    if t.champ.is_some() {
        t.data.remove::<LeviathanBuff>();
        t.data.remove::<ElderBuff>();
        return;
    }
    let epic = t.data.get::<MonsterState>().is_some_and(|m| m.epic);
    if !epic || t.data.contains::<ObjectiveClaimed>() {
        return;
    }
    t.data.insert(ObjectiveClaimed);
    let Some(kind) = t.subtype.as_deref().and_then(monster_type) else {
        return;
    };
    let element = t.data.get::<DragonElementTag>().map(|e| e.0);
    {
        let mut st = objectives.lock();
        let elder = st.elder;
        let slot = match kind {
            MonsterType::Dragon | MonsterType::ElderDragon => &mut st.dragon,
            MonsterType::Grub => &mut st.grubs,
            MonsterType::Herald => &mut st.herald,
            _ => &mut st.leviathan,
        };
        slot.ids.retain(|&id| id != target);
        match kind {
            MonsterType::Grub | MonsterType::Herald => {
                if slot.ids.is_empty() {
                    slot.done = true;
                }
            }
            MonsterType::Dragon | MonsterType::ElderDragon => {
                let respawn = if elder || kind == MonsterType::ElderDragon {
                    TIMERS.elder_respawn
                } else {
                    TIMERS.dragon_respawn
                };
                slot.next = time + scaled(respawn);
            }
            _ => slot.next = time + scaled(TIMERS.leviathan_respawn),
        }
    }
    let Some((killer, team)) = world
        .get_opt(killer)
        .and_then(|k| team_of(k.team).map(|t| (k.id, t)))
    else {
        return;
    };
    if kind == MonsterType::Dragon
        && let Some(element) = element
    {
        grant_dragon(world, objectives, team, element, killer);
        let mut st = objectives.lock();
        if st.elder {
            st.dragon.next = time + scaled(TIMERS.elder_respawn);
        }
    }
    if kind == MonsterType::Grub {
        let grubs = {
            let mut st = objectives.lock();
            st.team_mut(team).grubs += 1;
            st.team(team).grubs
        };
        for c in team_champions(world, to_side(team), false) {
            add_buff(
                world,
                c,
                "grub_siege",
                BuffOptions {
                    stacks: Some(grubs),
                    params: vec![("structureDamage".into(), 8.0)],
                    ..BuffOptions::new(1e9)
                },
            );
        }
    }
    if kind == MonsterType::Herald
        && let Some(k) = world.get_mut(killer).filter(|k| k.champ.is_some())
    {
        k.data.insert(HeraldEye {
            expires_at: time + HERALD_EYE_SECONDS,
        });
    }
    if matches!(kind, MonsterType::Leviathan | MonsterType::ElderDragon) {
        let leviathan = kind == MonsterType::Leviathan;
        let duration = scaled(if leviathan {
            TIMERS.leviathan_buff_duration
        } else {
            TIMERS.elder_buff_duration
        });
        {
            let mut st = objectives.lock();
            let p = st.team_mut(team);
            if leviathan {
                p.leviathan_until = time + duration;
            } else {
                p.elder_until = time + duration;
            }
        }
        for c in team_champions(world, to_side(team), true) {
            if let Some(e) = world.get_mut(c) {
                if leviathan {
                    e.data.insert(LeviathanBuff {
                        expires_at: time + duration,
                        recall_seconds: Some(LEVIATHAN_RECALL_SECONDS),
                    });
                } else {
                    e.data.insert(ElderBuff {
                        expires_at: time + duration,
                    });
                }
            }
            let (id, params) = if leviathan {
                (
                    "leviathan",
                    vec![
                        ("ad".into(), 35.0),
                        ("ap".into(), 50.0),
                        ("recallSeconds".into(), LEVIATHAN_RECALL_SECONDS),
                    ],
                )
            } else {
                ("elder", vec![("executeThreshold".into(), 0.2)])
            };
            add_buff(
                world,
                c,
                id,
                BuffOptions {
                    source: Some(killer),
                    params,
                    ..BuffOptions::new(duration)
                },
            );
        }
    }
    world.emit(GameEvent::ObjectiveTaken {
        monster: kind,
        team,
        killer: Some(killer),
        element,
    });
    announce(
        world,
        "objective_taken",
        team,
        Some(("monster", kind.as_str())),
    );
}

fn init_objectives(world: &mut World, objectives: &Objectives) {
    install_monster_hooks(world);
    *objectives.lock() = ObjectiveState::default();

    let obj = objectives.clone();
    world
        .hooks
        .objective_timers
        .push(Box::new(move |_, _| Some(obj.lock().timers())));

    let obj = objectives.clone();
    world.events.on(EventKind::Respawn, move |world, ev| {
        if let GameEvent::Respawn { target, .. } = ev {
            sync_persistent_objectives(world, &obj, *target);
        }
    });

    install_objective_effects(world, objectives);

    world.hooks.after_attack_hit.push(Box::new(|world, ctx, _| {
        let epic = world
            .get(ctx.attacker)
            .and_then(|a| a.data.get::<MonsterState>())
            .is_some_and(|m| m.epic);
        if epic {
            epic_breath(world, ctx.attacker, ctx.target);
        }
    }));

    let obj = objectives.clone();
    world
        .hooks
        .after_damage
        .push(Box::new(move |world, ctx, dealt| {
            on_damage(world, &obj, ctx, dealt)
        }));

    let obj = objectives.clone();
    world.events.on(EventKind::Death, move |world, ev| {
        if let GameEvent::Death { target, killer, .. } = ev {
            on_death(world, &obj, *target, *killer);
        }
    });
}

// ------------------------------------------------------------------------------------------------
// Mise à jour
// ------------------------------------------------------------------------------------------------

fn spawn_ready_objectives(world: &mut World, objectives: &Objectives) {
    let time = world.time;
    let dragon = {
        let mut st = objectives.lock();
        if st.dragon.ready(time) {
            let mut element = None;
            if !st.elder {
                st.dragon_count += 1;
                let e = match st.rift {
                    Some(r) => r,
                    None => DRAGON_ELEMENTS[world.rng.below(DRAGON_ELEMENTS.len())],
                };
                if st.dragon_count == 3 {
                    st.rift = Some(e);
                }
                element = Some(e);
            }
            Some((st.elder, element))
        } else {
            None
        }
    };
    if let Some((elder, element)) = dragon {
        let t = if elder {
            MonsterType::ElderDragon
        } else {
            MonsterType::Dragon
        };
        let id = spawn_epic(world, t, 0.0, 0.0, element);
        objectives.lock().dragon.ids = vec![id];
    }

    if time >= scaled(TIMERS.herald_spawn) {
        let ids = std::mem::take(&mut objectives.lock().grubs.ids);
        for id in ids {
            world.remove(id);
        }
        objectives.lock().grubs.done = true;
    }
    if objectives.lock().grubs.ready(time) {
        let ids = vec![
            spawn_epic(world, MonsterType::Grub, -180.0, -100.0, None),
            spawn_epic(world, MonsterType::Grub, 180.0, -100.0, None),
            spawn_epic(world, MonsterType::Grub, 0.0, 200.0, None),
        ];
        objectives.lock().grubs.ids = ids;
    }
    if time >= scaled(TIMERS.herald_despawn) {
        let ids = std::mem::take(&mut objectives.lock().herald.ids);
        for id in ids {
            world.remove(id);
        }
        objectives.lock().herald.done = true;
    } else if objectives.lock().herald.ready(time) {
        let id = spawn_epic(world, MonsterType::Herald, 0.0, 0.0, None);
        objectives.lock().herald.ids = vec![id];
    }
    if objectives.lock().leviathan.ready(time) {
        let id = spawn_epic(world, MonsterType::Leviathan, 0.0, 0.0, None);
        objectives.lock().leviathan.ids = vec![id];
    }
}

/// Un tick des données d'objectif d'une entité. Renvoie faux si l'entité a été retirée.
fn update_entity(
    world: &mut World,
    id: EntityId,
    dt: f64,
    core_combat: bool,
    bearers: &[(TeamOrNeutral, Vec2)],
) -> bool {
    let time = world.time;
    let Some(e) = world.get_mut(id) else {
        return false;
    };

    if e.data
        .get::<HeraldEye>()
        .is_some_and(|eye| time >= eye.expires_at)
    {
        e.data.remove::<HeraldEye>();
    }
    let call = e
        .data
        .get_mut::<EconomyState>()
        .and_then(|st| st.colossus_call.take());
    if let Some(call) = call {
        let in_map = call.x.is_finite()
            && call.y.is_finite()
            && (0.0..=MAP_SIZE).contains(&call.x)
            && (0.0..=MAP_SIZE).contains(&call.y);
        if !e.dead && e.champ.is_some() && to_side(call.team) == e.team && in_map {
            summon_herald(
                world,
                id,
                Vec2 {
                    x: call.x,
                    y: call.y,
                },
            );
        }
    }

    let Some(e) = world.get(id) else { return false };
    if e.data.get::<MonsterState>().is_some_and(|m| m.epic) {
        if e.dead {
            retire(world, id, RETIRE_DELAY);
            return world.contains(id);
        }
        let target = e.attack_target;
        let attacked = update_monster(world, id, dt, core_combat);
        let breath = world
            .get(id)
            .is_some_and(|e| breathes(e.subtype.as_deref()));
        if attacked
            && breath
            && let Some(target) = target.filter(|&t| world.contains(t))
        {
            epic_breath(world, id, target);
        }
    }

    let Some(e) = world.get(id) else { return false };
    if let Some(summon) = e.data.get::<SummonedHerald>().copied() {
        if e.dead || time >= summon.expires_at {
            world.remove(id);
            return false;
        }
        if let Some(tower) = herald_tower(world, id) {
            let Some(tower_pos) = world.get(tower).map(|t| t.pos) else {
                return true;
            };
            let Some(e) = world.get_mut(id) else {
                return false;
            };
            e.move_target = Some(tower_pos);
            if dist2(e.pos, tower_pos) <= 450.0 * 450.0 && time >= summon.next_charge {
                if let Some(s) = e.data.get_mut::<SummonedHerald>() {
                    s.next_charge = time + 10.0;
                }
                let from = e.pos;
                world.emit(GameEvent::Dash {
                    source: id,
                    from,
                    to: tower_pos,
                    duration: 0.4,
                    fx_key: Some("herald_charge".into()),
                });
                apply_damage(
                    world,
                    DamageRequest {
                        source: id,
                        target: tower,
                        amount: 2000.0,
                        damage_type: DamageType::True,
                        ability: Some("herald_charge".into()),
                    },
                );
                if let Some(e) = world.get_mut(id) {
                    e.hp -= e.stats.max_hp * 0.35;
                    if e.hp <= 0.0 {
                        e.dead = true;
                    }
                }
            }
        }
    }

    let Some(e) = world.get(id) else { return false };
    if let Some(grub) = e.data.get::<Grubling>().copied() {
        let target = world.get(grub.target).filter(|t| !t.dead).map(|t| t.pos);
        let Some(target_pos) = target.filter(|_| !e.dead && time < grub.expires_at) else {
            world.remove(id);
            return false;
        };
        let next = e
            .data
            .get::<crate::world_ai::runtime::PveNextAttack>()
            .map_or(0.0, |n| n.0);
        let in_reach = dist2(e.pos, target_pos) <= 200.0 * 200.0;
        let ad = e.stats.ad;
        if let Some(e) = world.get_mut(id) {
            e.move_target = Some(target_pos);
        }
        if in_reach && time >= next {
            if let Some(e) = world.get_mut(id) {
                e.data
                    .insert(crate::world_ai::runtime::PveNextAttack(time + 1.0));
            }
            apply_damage(
                world,
                DamageRequest {
                    source: id,
                    target: grub.target,
                    amount: ad,
                    damage_type: DamageType::Physical,
                    ability: Some("grubling".into()),
                },
            );
        }
    }

    while let Some(source) = take_burn_tick(world, id, BurnKey::Elder) {
        let active = world.get(source).is_some_and(|s| {
            !s.dead
                && s.data
                    .get::<ElderBuff>()
                    .is_some_and(|b| b.expires_at > world.time)
        });
        if !active {
            continue;
        }
        apply_damage(
            world,
            DamageRequest {
                source,
                target: id,
                amount: 40.0,
                damage_type: DamageType::True,
                ability: Some("elder_burn".into()),
            },
        );
        if world
            .get(id)
            .is_some_and(|e| !e.dead && e.hp > 0.0 && e.hp / e.stats.max_hp < 0.2)
        {
            execute(world, source, id);
        }
    }

    let Some(e) = world.get_mut(id) else {
        return false;
    };
    let dead = e.dead;
    if e.data
        .get::<LeviathanBuff>()
        .is_some_and(|b| dead || time >= b.expires_at)
    {
        e.data.remove::<LeviathanBuff>();
    }
    if e.data
        .get::<ElderBuff>()
        .is_some_and(|b| dead || time >= b.expires_at)
    {
        e.data.remove::<ElderBuff>();
    }

    if e.kind == EntityKind::Minion
        && e.team != TeamOrNeutral::Neutral
        && !e.dead
        && !e.data.contains::<Grubling>()
    {
        let empowered = bearers
            .iter()
            .any(|&(team, pos)| team == e.team && dist2(pos, e.pos) <= 1400.0 * 1400.0);
        let next = e.data.get::<NextLeviathanRefresh>().map_or(0.0, |n| n.0);
        if empowered && time >= next {
            e.data.insert(LeviathanMinion);
            e.data.insert(NextLeviathanRefresh(time + 0.75));
            add_buff(
                world,
                id,
                "leviathan_minion",
                BuffOptions {
                    params: vec![
                        ("adPercent".into(), 0.5),
                        ("armor".into(), 50.0),
                        ("mr".into(), 50.0),
                        ("attackRange".into(), 100.0),
                    ],
                    ..BuffOptions::new(1.0)
                },
            );
        } else if !empowered && e.data.contains::<LeviathanMinion>() {
            e.data.remove::<LeviathanMinion>();
            e.data.remove::<NextLeviathanRefresh>();
            remove_buff(world, id, "leviathan_minion", SourceMatch::Any);
        }
    }
    true
}

struct Scratch {
    ids: Vec<EntityId>,
    champions: Vec<EntityId>,
    buffs: Vec<EntityId>,
    bearers: Vec<(TeamOrNeutral, Vec2)>,
}

fn update(
    world: &mut World,
    dt: f64,
    objectives: &Objectives,
    core_combat: bool,
    run_buffs: bool,
    sc: &mut Scratch,
) {
    if world.winner.is_some() {
        return;
    }
    spawn_ready_objectives(world, objectives);

    let time = world.time;
    sc.bearers.clear();
    sc.bearers.extend(
        world
            .query(Some(EntityKind::Champion))
            .filter(|c| {
                !c.dead
                    && c.data
                        .get::<LeviathanBuff>()
                        .is_some_and(|b| b.expires_at > time)
            })
            .map(|c| (c.team, c.pos)),
    );

    world.ids_into(None, &mut sc.ids);
    for i in 0..sc.ids.len() {
        update_entity(world, sc.ids[i], dt, core_combat, &sc.bearers);
    }
    if !run_buffs {
        update_buffs_with(world, dt, &mut sc.buffs);
    }
    update_objective_effects(world, objectives, dt, &mut sc.champions);
}

/// Le système des objectifs et la poignée sur son état.
///
/// `core_combat` : le système `combat` fait partie de la partie (voir `runtime::attack`).
/// `buffs_system` : le système `buffs` fait partie de la partie ; sinon les objectifs font avancer
/// les buffs eux-mêmes, comme en TS.
pub fn objectives_system_with_state(core_combat: bool, buffs_system: bool) -> (System, Objectives) {
    let objectives = Objectives::default();
    let for_init = objectives.clone();
    let for_update = objectives.clone();
    let mut sc = Scratch {
        ids: Vec::new(),
        champions: Vec::new(),
        buffs: Vec::new(),
        bearers: Vec::new(),
    };
    let system = System::new("objectives", move |world, dt| {
        update(world, dt, &for_update, core_combat, buffs_system, &mut sc);
    })
    .with_init(move |world| init_objectives(world, &for_init));
    (system, objectives)
}

/// Le système des objectifs, dans une partie complète (combat et buffs présents).
pub fn objectives_system() -> System {
    objectives_system_with_state(true, true).0
}
