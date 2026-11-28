//! Construction des snapshots par joueur : port de `packages/server/src/net/snapshot.ts`.
//!
//! - Filtrage par vision d'équipe (anti-triche) : structures toujours envoyées, ennemis invisibles jamais.
//! - Delta : après le premier snapshot complet, seules les entités modifiées sont envoyées (complètes si
//!   nouvelles, en `patches` sinon) avec `removed`.
//! - Événements filtrés : rien sur une unité ennemie invisible ; or, achats, objets utilisés et rappel
//!   privés à l'équipe (ou visibles si l'unité l'est).
//!
//! Différences avec le TypeScript :
//! - `SnapshotFrame` emprunte le monde (`'w`) au lieu de le garder par référence ;
//! - le `ScoreTracker` partage son état avec ses écouteurs d'événements (`Arc<Mutex>`) ;
//! - la liste `removed` suit l'ordre des ids (le TS suivait l'ordre d'insertion de sa `Map`, quasi
//!   identique puisque les ids sont croissants).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use lov_shared::constants::FOUNTAIN_RADIUS;
use lov_shared::map::FOUNTAINS;
use lov_shared::math::dist;
use lov_shared::protocol::{EntityField, EntityPatch, Score, Snapshot, TeamScore};
use lov_shared::types::{
    ChampSnapshot, Cooldowns, DragonElement, EntityId, EntityKind, EntitySnapshot, GameEvent,
    MonsterType, SelfState, Stats, StructureKind, Team, TeamOrNeutral,
};
use lov_sim::systems::vision::is_visible_to;
use lov_sim::{Entity, EventKind, World};

/// `data.hidden` : projectile jamais envoyé aux clients.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HiddenProjectile;

/// Rafraîchissement complet étalé : chaque entité est renvoyée en entier tous les `REFRESH_TICKS`
/// ticks, à un tick qui dépend de son id. Un client qui aurait manqué l'état de base d'une entité se
/// resynchronise en 2 s au plus.
pub const REFRESH_TICKS: u64 = 60;

/// `Math.round` de JavaScript (demis arrondis vers +infini).
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

fn side(team: Team) -> TeamOrNeutral {
    match team {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

// ------------------------------------------------------------------------------------------------
// Entités et état privé
// ------------------------------------------------------------------------------------------------

/// Snapshot public d'une entité (arrondi pour le réseau), complété par les hooks `entity_snapshot`.
pub fn to_entity_snapshot(e: &Entity, world: Option<&World>) -> EntitySnapshot {
    let mut s = EntitySnapshot {
        id: e.id,
        kind: e.kind,
        team: e.team,
        x: js_round(e.pos.x),
        y: js_round(e.pos.y),
        facing: js_round(e.facing * 100.0) / 100.0,
        hp: js_round(e.hp),
        max_hp: js_round(e.stats.max_hp),
        mana: None,
        max_mana: None,
        shield: (e.shield > 0.0).then(|| js_round(e.shield)),
        dead: e.dead.then_some(true),
        subtype: e.subtype.clone(),
        owner_id: None,
        level: None,
        anim: e.anim.clone(),
        buffs: None,
        champ: None,
    };
    if let Some(c) = &e.champ {
        s.mana = Some(js_round(e.mana));
        s.max_mana = Some(js_round(e.stats.max_mana));
        s.level = Some(c.level);
        s.owner_id = Some(c.player_id.clone());
        s.champ = Some(ChampSnapshot {
            player_id: c.player_id.clone(),
            name: c.name.clone(),
            kills: c.kills,
            deaths: c.deaths,
            assists: c.assists,
            cs: c.cs,
            items: c.items.clone(),
            respawn_in: e.dead.then(|| c.respawn_in.ceil()),
        });
    }
    if let Some(world) = world {
        world.apply_entity_snapshot(e, &mut s);
    }
    s
}

/// Recharges arrondies au dixième supérieur (0 si écoulées).
fn ceil1(v: f64) -> f64 {
    if v > 0.0 {
        (v * 10.0).ceil() / 10.0
    } else {
        0.0
    }
}

fn round2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}

fn round_stats(st: &Stats) -> Stats {
    Stats {
        max_hp: round2(st.max_hp),
        hp_regen: round2(st.hp_regen),
        max_mana: round2(st.max_mana),
        mana_regen: round2(st.mana_regen),
        ad: round2(st.ad),
        ap: round2(st.ap),
        armor: round2(st.armor),
        mr: round2(st.mr),
        attack_speed: round2(st.attack_speed),
        attack_range: round2(st.attack_range),
        move_speed: round2(st.move_speed),
        crit_chance: round2(st.crit_chance),
        ability_haste: round2(st.ability_haste),
        lifesteal: round2(st.lifesteal),
        armor_pen: round2(st.armor_pen),
        magic_pen: round2(st.magic_pen),
    }
}

/// État privé envoyé au seul joueur propriétaire, complété par les hooks `self_state`.
pub fn self_state(e: &Entity, world: Option<&World>) -> Option<SelfState> {
    let c = e.champ.as_ref()?;
    let at_fountain = match e.team {
        TeamOrNeutral::Blue => dist(e.pos, FOUNTAINS.blue) <= FOUNTAIN_RADIUS,
        TeamOrNeutral::Red => dist(e.pos, FOUNTAINS.red) <= FOUNTAIN_RADIUS,
        TeamOrNeutral::Neutral => false,
    };
    let cd = &c.cooldowns;
    let mut s = SelfState {
        entity_id: e.id,
        gold: c.gold.floor(),
        xp: c.xp.floor(),
        xp_to_next: 0.0,
        level: c.level,
        skill_points: c.skill_points,
        ability_levels: c.ability_levels.clone(),
        cooldowns: Cooldowns {
            q: ceil1(cd.q),
            w: ceil1(cd.w),
            e: ceil1(cd.e),
            r: ceil1(cd.r),
            d: ceil1(cd.d),
            f: ceil1(cd.f),
        },
        summoners: c.summoners.clone(),
        stats: round_stats(&e.stats),
        recalling: None,
        can_shop: e.dead || at_fountain,
        ward_charges: None,
        item_charges: None,
        item_cooldowns: None,
    };
    if let Some(world) = world {
        world.apply_self_state(e, &mut s);
    }
    Some(s)
}

// ------------------------------------------------------------------------------------------------
// Score
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct ScoreState {
    towers: [u32; 2],
    dragons: [Vec<String>; 2],
    grubs: [u32; 2],
}

fn idx(team: Team) -> usize {
    match team {
        Team::Blue => 0,
        Team::Red => 1,
    }
}

fn element_name(e: DragonElement) -> &'static str {
    match e {
        DragonElement::Infernal => "infernal",
        DragonElement::Ocean => "ocean",
        DragonElement::Mountain => "mountain",
        DragonElement::Cloud => "cloud",
        DragonElement::Hextech => "hextech",
        DragonElement::Chemtech => "chemtech",
    }
}

/// Score d'équipe tenu à jour à partir des événements (tours, drakes, Larves) et des kills.
#[derive(Debug, Clone, Default)]
pub struct ScoreTracker(Arc<Mutex<ScoreState>>);

impl ScoreTracker {
    /// Abonne le suivi aux événements du monde.
    pub fn new(world: &mut World) -> Self {
        let tracker = Self::default();
        let st = Arc::clone(&tracker.0);
        world
            .events
            .on(EventKind::StructureDestroyed, move |_, ev| {
                if let GameEvent::StructureDestroyed {
                    kind: StructureKind::Tower,
                    team,
                    plate,
                    ..
                } = ev
                    && *plate != Some(true)
                {
                    // La tour d'une équipe tombe : un point pour l'autre.
                    let winner = match team {
                        Team::Blue => Team::Red,
                        Team::Red => Team::Blue,
                    };
                    lock(&st).towers[idx(winner)] += 1;
                }
            });
        let st = Arc::clone(&tracker.0);
        world.events.on(EventKind::ObjectiveTaken, move |_, ev| {
            let GameEvent::ObjectiveTaken {
                monster,
                team,
                element,
                ..
            } = ev
            else {
                return;
            };
            let mut s = lock(&st);
            match monster {
                MonsterType::Dragon => {
                    s.dragons[idx(*team)].push(element.map_or("dragon", element_name).to_owned())
                }
                MonsterType::ElderDragon => s.dragons[idx(*team)].push("elder".to_owned()),
                MonsterType::Grub => s.grubs[idx(*team)] += 1,
                _ => {}
            }
        });
        tracker
    }

    pub fn score(&self, world: &World) -> Score {
        let mut kills = [0u32; 2];
        for e in world.iter() {
            if let Some(c) = &e.champ {
                match e.team {
                    TeamOrNeutral::Blue => kills[0] += c.kills,
                    TeamOrNeutral::Red => kills[1] += c.kills,
                    TeamOrNeutral::Neutral => {}
                }
            }
        }
        let s = lock(&self.0);
        let team = |i: usize| TeamScore {
            kills: kills[i],
            towers: s.towers[i],
            dragons: s.dragons[i].clone(),
            grubs: s.grubs[i],
        };
        Score {
            blue: team(0),
            red: team(1),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ------------------------------------------------------------------------------------------------
// Cycle de snapshot
// ------------------------------------------------------------------------------------------------

/// JSON des champs objets (`champ`, `buffs`) d'une entité, pour comparer sans tout réencoder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nested {
    champ: Option<String>,
    buffs: Option<String>,
}

fn nested_of(s: &EntitySnapshot) -> Nested {
    Nested {
        champ: s.champ.as_ref().and_then(|c| serde_json::to_string(c).ok()),
        buffs: s.buffs.as_ref().and_then(|b| serde_json::to_string(b).ok()),
    }
}

/// Cache partagé par cycle : chaque entité n'est sérialisée qu'une fois pour tous les joueurs.
pub struct SnapshotFrame<'w> {
    pub world: &'w World,
    pub events: Vec<GameEvent>,
    pub score: Score,
    pub server_time: f64,
    entities: HashMap<EntityId, (EntitySnapshot, String)>,
    nested: RefCell<HashMap<EntityId, Nested>>,
}

impl<'w> SnapshotFrame<'w> {
    pub fn new(world: &'w World, events: Vec<GameEvent>, score: Score, server_time: f64) -> Self {
        let mut entities = HashMap::with_capacity(world.len());
        for e in world.iter() {
            if e.kind == EntityKind::Projectile && e.data.contains::<HiddenProjectile>() {
                continue;
            }
            let s = to_entity_snapshot(e, Some(world));
            let json = serde_json::to_string(&s).unwrap_or_default();
            entities.insert(e.id, (s, json));
        }
        Self {
            world,
            events,
            score,
            server_time,
            entities,
            nested: RefCell::new(HashMap::new()),
        }
    }

    fn nested(&self, id: EntityId) -> Nested {
        if let Some(n) = self.nested.borrow().get(&id) {
            return n.clone();
        }
        let n = self
            .entities
            .get(&id)
            .map(|(s, _)| nested_of(s))
            .unwrap_or_default();
        self.nested.borrow_mut().insert(id, n.clone());
        n
    }
}

/// Dernière version envoyée d'une entité.
#[derive(Debug, Clone)]
struct Sent {
    json: String,
    snap: EntitySnapshot,
    nested: Nested,
}

/// Patch des champs qui diffèrent entre deux versions d'une entité (objets comparés par leur JSON).
fn diff_entity(prev: &Sent, snap: &EntitySnapshot, nested: &Nested) -> EntityPatch {
    let a = &prev.snap;
    let mut p = EntityPatch::new(snap.id);
    macro_rules! required {
        ($($f:ident),+) => { $( if a.$f != snap.$f { p.$f = Some(snap.$f.clone()); } )+ };
    }
    required!(kind, team, x, y, facing, hp, max_hp);
    let mut unset = Vec::new();
    macro_rules! optional {
        ($(($f:ident, $field:ident)),+) => {$(
            match (&a.$f, &snap.$f) {
                (_, Some(v)) if a.$f.as_ref() != Some(v) => p.$f = Some(v.clone()),
                (Some(_), None) => unset.push(EntityField::$field),
                _ => {}
            }
        )+};
    }
    optional!(
        (mana, Mana),
        (max_mana, MaxMana),
        (shield, Shield),
        (dead, Dead),
        (subtype, Subtype),
        (owner_id, OwnerId),
        (level, Level),
        (anim, Anim)
    );
    match (&a.champ, &snap.champ) {
        (_, Some(c)) if prev.nested.champ != nested.champ => p.champ = Some(c.clone()),
        (Some(_), None) => unset.push(EntityField::Champ),
        _ => {}
    }
    match (&a.buffs, &snap.buffs) {
        (_, Some(b)) if prev.nested.buffs != nested.buffs => p.buffs = Some(b.clone()),
        (Some(_), None) => unset.push(EntityField::Buffs),
        _ => {}
    }
    if !unset.is_empty() {
        p.u = Some(unset);
    }
    p
}

/// État de delta d'un client (dernière version envoyée de chaque entité). Remettre à zéro : le
/// prochain snapshot est complet.
#[derive(Debug, Clone)]
pub struct ClientView {
    sent: BTreeMap<EntityId, Sent>,
    full: bool,
}

impl Default for ClientView {
    fn default() -> Self {
        Self {
            sent: BTreeMap::new(),
            full: true,
        }
    }
}

impl ClientView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.sent.clear();
        self.full = true;
    }

    pub fn build(&mut self, frame: &SnapshotFrame<'_>, player_id: &str, team: Team) -> Snapshot {
        let world = frame.world;
        let self_id = world.champion_id_of(player_id);
        let mut entities = Vec::new();
        let mut patches = Vec::new();
        let mut seen = std::collections::HashSet::with_capacity(self.sent.len());
        for e in world.iter() {
            if Some(e.id) != self_id && !is_visible_to(e, side(team), false) {
                continue;
            }
            let Some((snap, json)) = frame.entities.get(&e.id) else {
                continue;
            };
            seen.insert(e.id);
            let prev = if self.full {
                None
            } else {
                self.sent.get(&e.id)
            };
            let refresh = (u64::from(e.id) + world.tick).is_multiple_of(REFRESH_TICKS);
            if prev.is_some_and(|p| &p.json == json) && !refresh {
                continue;
            }
            let nested = frame.nested(e.id);
            match prev {
                Some(prev) if !refresh => patches.push(diff_entity(prev, snap, &nested)),
                _ => entities.push(snap.clone()),
            }
            self.sent.insert(
                e.id,
                Sent {
                    json: json.clone(),
                    snap: snap.clone(),
                    nested,
                },
            );
        }
        let mut removed = Vec::new();
        self.sent.retain(|id, _| {
            let keep = seen.contains(id);
            if !keep {
                removed.push(*id);
            }
            keep
        });
        let mut snap = Snapshot {
            tick: world.tick,
            time: js_round(world.time * 1000.0) / 1000.0,
            server_time: frame.server_time,
            entities,
            removed: None,
            patches: None,
            events: filter_events(world, &frame.events, team),
            self_state: None,
            objective_timers: None,
            score: frame.score.clone(),
        };
        if !self.full {
            snap.removed = Some(removed);
            if !patches.is_empty() {
                snap.patches = Some(patches);
            }
        }
        self.full = false;
        snap.self_state = self_id
            .and_then(|id| world.get(id))
            .and_then(|e| self_state(e, Some(world)));
        let mut timers: Option<BTreeMap<String, f64>> = None;
        for hook in world.hooks.objective_timers.iter() {
            if let Some(t) = hook(world, team) {
                timers.get_or_insert_with(BTreeMap::new).extend(t);
            }
        }
        snap.objective_timers = timers;
        snap
    }
}

/// Snapshot complet et autonome (tests, outils). Sans `score`, un score vide.
pub fn build_snapshot(
    world: &World,
    player_id: &str,
    team: Team,
    events: Vec<GameEvent>,
    score: Option<Score>,
) -> Snapshot {
    let score = score.unwrap_or_else(|| ScoreTracker::default().score(world));
    let frame = SnapshotFrame::new(world, events, score, 0.0);
    ClientView::new().build(&frame, player_id, team)
}

// ------------------------------------------------------------------------------------------------
// Événements
// ------------------------------------------------------------------------------------------------

/// L'entité est visible de l'équipe (une entité disparue l'est aussi : rien à cacher).
fn visible_id(world: &World, id: Option<EntityId>, team: Team) -> bool {
    match id {
        None | Some(0) => false,
        Some(id) => world
            .get(id)
            .is_none_or(|e| is_visible_to(e, side(team), false)),
    }
}

fn same_team(world: &World, id: EntityId, team: Team) -> bool {
    world.get(id).is_some_and(|e| e.team == side(team))
}

fn player_team(world: &World, player_id: &str) -> Option<TeamOrNeutral> {
    world.champion_of(player_id).map(|e| e.team)
}

/// Filtre les événements du cycle pour une équipe.
pub fn filter_events(world: &World, events: &[GameEvent], team: Team) -> Vec<GameEvent> {
    let vis = |id: EntityId| visible_id(world, Some(id), team);
    events
        .iter()
        .filter(|ev| match ev {
            GameEvent::Damage { source, target, .. }
            | GameEvent::Heal { source, target, .. }
            | GameEvent::Shield { source, target, .. }
            | GameEvent::AutoAttack { source, target, .. } => vis(*source) || vis(*target),
            GameEvent::Death { target, killer, .. } => {
                vis(*target) || killer.is_some_and(|k| same_team(world, k, team))
            }
            GameEvent::AbilityCast { source, .. }
            | GameEvent::SummonerCast { source, .. }
            | GameEvent::Dash { source, .. } => vis(*source),
            GameEvent::ProjectileSpawn { id, source, .. } => vis(*source) || vis(*id),
            GameEvent::ProjectileHit { id, target, .. } => {
                vis(*id) || visible_id(world, *target, team)
            }
            GameEvent::AreaEffect { source, .. } => *source == 0 || vis(*source),
            GameEvent::BuffApplied { target, .. }
            | GameEvent::BuffRemoved { target, .. }
            | GameEvent::LevelUp { target, .. } => vis(*target),
            GameEvent::GoldGain { target, .. } => same_team(world, *target, team),
            GameEvent::ItemBought { target, .. }
            | GameEvent::ItemSold { target, .. }
            // Un objet utilisé (balise posée…) ne trahit pas un ennemi hors vision (#317).
            | GameEvent::ItemUsed { target, .. }
            | GameEvent::RecallStart { target }
            | GameEvent::RecallEnd { target, .. }
            | GameEvent::Respawn { target, .. } => same_team(world, *target, team) || vis(*target),
            GameEvent::Ping { player_id, .. } => player_team(world, player_id) == Some(side(team)),
            GameEvent::Chat {
                player_id,
                team_only,
                ..
            } => !*team_only || player_team(world, player_id) == Some(side(team)),
            // structureDestroyed, objectiveSpawn/Taken, announcement : publics.
            _ => true,
        })
        .cloned()
        .collect()
}
