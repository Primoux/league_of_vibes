//! Outils des tests de sorts : port de `packages/server/test/champion-helpers.ts`.
//!
//! `game(id)` crée un monde avec les systèmes `buffs`, `abilities`, `movement` (le TS y ajoute
//! `summoners`, pas encore porté), un champion bleu `id` en (1000, 1000) et un champion rouge cible en
//! (1250, 1000), tous deux au niveau 6. Les kits de champions arrivent un par un : tant qu'un champion
//! n'a pas son kit, les tests du moteur enregistrent un kit de test (`register_kit`).
//!
//! Différences avec le TS :
//! - `grantXP(world, e, xpForLevel(level))` (le système `leveling` n'est pas porté) devient `grant_level`,
//!   qui reproduit ce que fait `grantXP` à chaque niveau gagné (stats, PV et mana, points de compétence) ;
//! - le TS prend `sable` pour champion cible ; `game` prend `tharox` (seule autre fiche de champion portée
//!   avec Grondak) : les tests n'en utilisent que la position, l'équipe et le type d'entité, via `game_vs`
//!   on peut choisir un autre champion ;
//! - `combat/buffs` lit les fiches dans `lov-shared` seulement : un champion dont la fiche est
//!   enregistrée à l'exécution (`register_champion_def`) ne change pas de stats avec son niveau ;
//! - les `hits` (`hitsByWorld`) sont rangés dans `Game`, pas dans un `WeakMap`.

use std::sync::{Arc, Mutex};

use lov_shared::constants::TICK_DT;
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{
    AbilitySlot, EntityId, EntityKind, Stats, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::abilities::engine::{CastCommand, cast_ability};
use lov_sim::abilities::registry::champion_def;
use lov_sim::combat::buffs::recalculate_stats;
use lov_sim::combat::damage::{DamageOpts, apply_damage};
use lov_sim::systems::abilities::abilities_system;
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::movement::movement_system;
use lov_sim::{CreateChampion, Entity, QueuedCommand, System, World, create_champion};

/// Un coup enregistré par `damage`.
#[derive(Debug, Clone)]
pub struct Hit {
    pub ability: Option<String>,
    pub target: EntityId,
    pub amount: f64,
}

pub struct Game {
    pub world: World,
    pub systems: Vec<System>,
    pub source: EntityId,
    pub target: EntityId,
    hits: Arc<Mutex<Vec<Hit>>>,
}

fn point(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// `grantXP(world, e, xpForLevel(level))` : monte `id` au niveau `level`.
pub fn grant_level(world: &mut World, id: EntityId, level: u32) {
    while world
        .get(id)
        .and_then(|e| e.champ.as_ref())
        .is_some_and(|c| c.level < level)
    {
        let Some(e) = world.get_mut(id) else { return };
        let (old_hp, old_mana) = (e.stats.max_hp, e.stats.max_mana);
        if let Some(c) = e.champ.as_mut() {
            c.level += 1;
            c.skill_points += 1;
        }
        recalculate_stats(world, id);
        let Some(e) = world.get_mut(id) else { return };
        if !e.dead {
            e.hp = e.stats.max_hp.min(e.hp + e.stats.max_hp - old_hp);
        }
        e.mana = e.stats.max_mana.min(e.mana + e.stats.max_mana - old_mana);
    }
}

fn spawn_champion(
    world: &mut World,
    player: &str,
    name: &str,
    champion_id: &str,
    team: Team,
    pos: Vec2,
) -> EntityId {
    let def = champion_def(champion_id).expect("champion inconnu du registre");
    create_champion(
        world,
        CreateChampion {
            player_id: player.into(),
            name: name.into(),
            team,
            champion_id: champion_id.into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        },
        &def.base_stats,
    )
}

/// `game(id)` au niveau 6, contre un champion rouge `tharox`.
pub fn game(id: &str) -> Game {
    game_vs(id, "tharox", 6)
}

/// `game(id, level)` avec le champion cible de son choix.
pub fn game_vs(id: &str, target_champion: &str, level: u32) -> Game {
    let mut world = World::new(1);
    let mut systems = vec![buffs_system(), abilities_system(), movement_system()];
    let source = spawn_champion(&mut world, "p1", id, id, Team::Blue, point(1000.0, 1000.0));
    let target = spawn_champion(
        &mut world,
        "p2",
        "Target",
        target_champion,
        Team::Red,
        point(1250.0, 1000.0),
    );
    grant_level(&mut world, source, level);
    grant_level(&mut world, target, level);
    if let Some(c) = world.get_mut(source).and_then(|e| e.champ.as_mut()) {
        c.ability_levels.q = 1;
        c.ability_levels.w = 1;
        c.ability_levels.e = 1;
        c.ability_levels.r = u32::from(level >= 6);
    }
    world.init(&mut systems);
    world.events.drain();
    let hits: Arc<Mutex<Vec<Hit>>> = Arc::default();
    let sink = Arc::clone(&hits);
    world
        .hooks
        .after_damage
        .push(Box::new(move |_, ctx, amount| {
            if let Ok(mut list) = sink.lock() {
                list.push(Hit {
                    ability: ctx.ability.clone(),
                    target: ctx.target,
                    amount,
                });
            }
        }));
    Game {
        world,
        systems,
        source,
        target,
        hits,
    }
}

impl Game {
    pub fn e(&self, id: EntityId) -> &Entity {
        self.world.get(id).unwrap()
    }

    pub fn e_mut(&mut self, id: EntityId) -> &mut Entity {
        self.world.get_mut(id).unwrap()
    }

    /// `world.step()`.
    pub fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    /// `world.step(dt)`.
    pub fn step_dt(&mut self, dt: f64) {
        self.world.step_dt(&mut self.systems, dt);
    }

    /// `advance(world, seconds)`.
    pub fn advance(&mut self, seconds: f64) {
        let until = self.world.time + seconds;
        while self.world.time + 1e-8 < until {
            self.step_dt(TICK_DT.min(until - self.world.time));
        }
    }

    /// `cast(world, source, slot, target, point)` : `point` par défaut la position de la cible, sinon
    /// (1400, 1000).
    pub fn cast(&mut self, slot: AbilitySlot, target: Option<EntityId>, at: Option<Vec2>) -> bool {
        let at = at
            .or_else(|| target.and_then(|t| self.world.get(t)).map(|t| t.pos))
            .unwrap_or(point(1400.0, 1000.0));
        let command = CastCommand {
            slot,
            target_id: target,
            target_pos: Some(at),
        };
        cast_ability(&mut self.world, self.source, &command, true)
    }

    /// Une commande joueur (`world.commands.push`).
    pub fn command(&mut self, player: &str, msg: ClientMsg) {
        self.world.commands.push(QueuedCommand {
            player_id: player.into(),
            msg,
        });
    }

    /// `auto(world, source, target, crit)`.
    pub fn auto(&mut self, source: EntityId, target: EntityId, crit: bool) -> f64 {
        self.world.emit(lov_shared::types::GameEvent::AutoAttack {
            source,
            target,
            ranged: false,
            speed: None,
            crit: crit.then_some(true),
        });
        let ad = self.e(source).stats.ad;
        let mut opts = DamageOpts::new(
            Some(source),
            target,
            ad * if crit { 1.75 } else { 1.0 },
            lov_shared::types::DamageType::Physical,
        );
        opts.crit = crit;
        opts.is_auto_attack = true;
        apply_damage(&mut self.world, opts)
    }

    /// `damage(world, ability, target)` : somme des dégâts enregistrés.
    pub fn damage(&self, ability: Option<&str>, target: Option<EntityId>) -> f64 {
        let list = self.hits.lock().map(|l| l.clone()).unwrap_or_default();
        list.iter()
            .filter(|h| {
                ability.is_none_or(|a| h.ability.as_deref() == Some(a))
                    && target.is_none_or(|t| h.target == t)
            })
            .map(|h| h.amount)
            .sum()
    }

    /// `dummy(world, source, pos, stats)` : sbire rouge de 10 000 PV, sans résistances.
    pub fn dummy(&mut self, pos: Vec2, tweak: impl FnOnce(&mut Stats)) -> EntityId {
        let mut stats = self.e(self.source).stats.clone();
        stats.max_hp = 10000.0;
        stats.armor = 0.0;
        stats.mr = 0.0;
        tweak(&mut stats);
        self.world.spawn(Entity::new(
            EntityKind::Minion,
            TeamOrNeutral::Red,
            pos,
            30.0,
            10000.0,
            0.0,
            stats,
        ))
    }
}
