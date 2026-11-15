//! Outils partagés des tests : port de `packages/server/test/helpers.ts`.
//!
//! Chaque fichier de `tests/` est un binaire distinct qui n'utilise qu'une partie de ces fonctions.
// `dead_code` : sans cet `allow`, chaque binaire de test avertirait des helpers qu'il n'emploie pas.
#![allow(dead_code)]

pub mod champion;

use std::sync::atomic::{AtomicU32, Ordering};

use lov_shared::data::champions::get_champion;
use lov_shared::types::{EntityId, EntityKind, Stats, SummonerId, Team, TeamOrNeutral, Vec2};
use lov_sim::systems::combat::{AttackProjectiles, combat_system_with_projectiles};
use lov_sim::systems::input::input_system;
use lov_sim::systems::movement::movement_system;
use lov_sim::systems::vision::vision_system;
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

pub fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// Un monde et ses systèmes (le TS range les systèmes dans le monde).
pub struct Sim {
    pub world: World,
    pub systems: Vec<System>,
    /// Projectiles d'auto-attaque en vol (`pendingAttackProjectiles`).
    pub projectiles: AttackProjectiles,
}

/// `coreWorld()` : input, movement, combat, vision, dans cet ordre.
pub fn core_world() -> Sim {
    let (combat, projectiles) = combat_system_with_projectiles();
    let mut systems = vec![input_system(), movement_system(), combat, vision_system()];
    let mut world = World::new(1);
    world.init(&mut systems);
    Sim {
        world,
        systems,
        projectiles,
    }
}

impl Sim {
    pub fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    /// `run(world, seconds)` : `round(seconds * 30)` ticks.
    pub fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 30.0).round() as u32 {
            self.step();
        }
    }

    pub fn e(&self, id: EntityId) -> &Entity {
        self.world.get(id).unwrap()
    }

    pub fn e_mut(&mut self, id: EntityId) -> &mut Entity {
        self.world.get_mut(id).unwrap()
    }

    pub fn champ(&mut self, team: Team, pos: Vec2) -> EntityId {
        champ(&mut self.world, team, pos)
    }

    pub fn dummy(
        &mut self,
        team: TeamOrNeutral,
        pos: Vec2,
        tweak: impl FnOnce(&mut Stats),
    ) -> EntityId {
        dummy(&mut self.world, team, pos, tweak)
    }

    pub fn dummy_kind(
        &mut self,
        team: TeamOrNeutral,
        pos: Vec2,
        tweak: impl FnOnce(&mut Stats),
        kind: EntityKind,
    ) -> EntityId {
        dummy_kind(&mut self.world, team, pos, tweak, kind)
    }
}

/// `BASE_STATS`.
pub fn base_stats() -> Stats {
    Stats {
        max_hp: 1000.0,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad: 100.0,
        ap: 0.0,
        armor: 0.0,
        mr: 0.0,
        attack_speed: 1.0,
        attack_range: 150.0,
        move_speed: 300.0,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
}

static PLAYERS: AtomicU32 = AtomicU32::new(0);

/// `champ(world, team, pos, championId = 'grondak')`. L'id doit exister dans le registre.
pub fn champ_as(world: &mut World, team: Team, pos: Vec2, champion_id: &str) -> EntityId {
    let def = get_champion(champion_id).expect("champion inconnu du registre");
    let n = PLAYERS.fetch_add(1, Ordering::Relaxed) + 1;
    let opts = CreateChampion {
        player_id: format!("p{n}"),
        name: "T".into(),
        team,
        champion_id: champion_id.into(),
        summoners: [SummonerId::Flash, SummonerId::Ignite],
        pos,
    };
    create_champion(world, opts, &def.base_stats)
}

pub fn champ(world: &mut World, team: Team, pos: Vec2) -> EntityId {
    champ_as(world, team, pos, "grondak")
}

/// Identifiant de joueur d'un champion.
pub fn player_of(world: &World, id: EntityId) -> String {
    world
        .get(id)
        .unwrap()
        .champ
        .as_ref()
        .unwrap()
        .player_id
        .clone()
}

/// `dummy(world, team, pos, stats, kind)` ; `tweak` joue le rôle de `Partial<Stats>`.
pub fn dummy_kind(
    world: &mut World,
    team: TeamOrNeutral,
    pos: Vec2,
    tweak: impl FnOnce(&mut Stats),
    kind: EntityKind,
) -> EntityId {
    let mut s = base_stats();
    tweak(&mut s);
    let hp = s.max_hp;
    world.spawn(Entity::new(kind, team, pos, 40.0, hp, 0.0, s))
}

/// `dummy` d'un sbire.
pub fn dummy(
    world: &mut World,
    team: TeamOrNeutral,
    pos: Vec2,
    tweak: impl FnOnce(&mut Stats),
) -> EntityId {
    dummy_kind(world, team, pos, tweak, EntityKind::Minion)
}
