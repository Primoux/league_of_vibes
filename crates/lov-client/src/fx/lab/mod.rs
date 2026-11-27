//! Labo des sorts, sans serveur :
//! `LOV_SPELLLAB=<champion>` (ou `all`) `cargo run -p lov-client --features spelllab`.
//!
//! Outil de développement, compilé seulement avec la feature `spelllab` : le jeu livré n'embarque pas
//! la simulation.
//!
//! Équivalent du `labSim.ts` du fxlab web : la **vraie** simulation (`lov-sim`, systèmes d'entrées, buffs,
//! invocateurs, sorts, déplacement, combat, progression) tourne dans le client. Un champion bleu de
//! niveau 18, sorts au rang maximal, lance Q, W, E puis R en boucle sur un champion rouge (sur un allié
//! si le sort le demande), en attaquant entre deux sorts. Les événements produits sont exactement ceux du
//! jeu : ils passent par le bus et le dispatcher jusqu'aux FX, au rendu et à l'audio, comme en partie.
//!
//! - `LOV_SPELLLAB=all` enchaîne tous les champions ;
//! - `LOV_LAB_SHOTS=<dossier>` enregistre deux captures après chaque sort (`<champion>_<sort>_<n>.png`),
//!   pour vérifier les FX sans regarder la fenêtre (CI, IA) ;
//! - le journal affiche, pour chaque sort, les événements émis et la part des FX dédiés.

use std::collections::HashMap;
use std::sync::Mutex;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lov_shared::constants::TICK_DT;
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::protocol::{ClientMsg, GameStart};
use lov_shared::types::{AbilitySlot, EntityId, EntitySnapshot, GameEvent, SummonerId, Team, Vec2};
use lov_sim::abilities::engine::{CastCommand, cast_ability};
use lov_sim::abilities::registry::champion_def;
use lov_sim::systems::abilities::abilities_system;
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::combat::combat_system;
use lov_sim::systems::input::input_system;
use lov_sim::systems::leveling::leveling_system;
use lov_sim::systems::movement::movement_system;
use lov_sim::systems::summoners::summoners_system;
use lov_sim::{CreateChampion, Entity, QueuedCommand, System, World, create_champion};

use crate::fx::FxDispatcher;
use crate::net::snapshots::Pose;
use crate::net::{GameState, ServerEvent};
use crate::render::camera::CameraRig;

/// Champion demandé par `LOV_SPELLLAB` (`all` = tous), si le labo est demandé.
pub fn requested() -> Option<String> {
    std::env::var("LOV_SPELLLAB").ok().filter(|v| !v.is_empty())
}

pub fn enabled() -> bool {
    requested().is_some()
}

/// Secondes entre deux sorts.
const CAST_EVERY: f64 = 2.4;
/// Instants des captures après chaque sort (s).
const SHOTS: [f64; 2] = [0.35, 0.9];
/// Sorts ciblés qui visent un allié (les autres visent l'ennemi), comme `ALLY_TARGETED` du fxlab web.
const ALLY_TARGETED: [&str; 3] = ["ilyra_e", "solenne_w", "solenne_e"];
const SLOTS: [AbilitySlot; 4] = [
    AbilitySlot::Q,
    AbilitySlot::W,
    AbilitySlot::E,
    AbilitySlot::R,
];

const CASTER_AT: Vec2 = Vec2 {
    x: 3300.0,
    y: 3300.0,
};
const ENEMY_AT: Vec2 = Vec2 {
    x: 3750.0,
    y: 3350.0,
};
const ALLY_AT: Vec2 = Vec2 {
    x: 3150.0,
    y: 3600.0,
};

/// Un acteur du labo : entité de la simulation, place de départ et positions des deux derniers ticks.
struct Actor {
    id: EntityId,
    player: String,
    home: Vec2,
    prev: Vec2,
}

/// Le labo, en ressource : le monde de la simulation n'est pas `Sync` (ses systèmes sont des `FnMut`),
/// on le garde derrière un verrou.
#[derive(Resource)]
pub struct SpellLabRes(Mutex<SpellLab>);

/// État du labo.
pub struct SpellLab {
    world: World,
    systems: Vec<System>,
    champions: Vec<String>,
    champion: usize,
    caster: Actor,
    enemy: Actor,
    ally: Actor,
    acc: f64,
    clock: f64,
    next_cast: f64,
    slot: usize,
    /// Sort en cours d'observation : nom, instant du lancement, événements comptés.
    watching: Option<(String, f64, HashMap<&'static str, u32>)>,
    shots: Option<String>,
    shots_due: Vec<(f64, String)>,
    /// Compteurs du dispatcher au lancement (pour la part des FX dédiés).
    stats_at_cast: (u32, u32),
    /// Un tick a eu lieu depuis la dernière reconstruction des snapshots.
    dirty: bool,
}

fn event_name(e: &GameEvent) -> &'static str {
    match e {
        GameEvent::Damage { .. } => "damage",
        GameEvent::Heal { .. } => "heal",
        GameEvent::Shield { .. } => "shield",
        GameEvent::AbilityCast { .. } => "abilityCast",
        GameEvent::ProjectileSpawn { .. } => "projectileSpawn",
        GameEvent::ProjectileHit { .. } => "projectileHit",
        GameEvent::Dash { .. } => "dash",
        GameEvent::AreaEffect { .. } => "areaEffect",
        GameEvent::BuffApplied { .. } => "buffApplied",
        GameEvent::BuffRemoved { .. } => "buffRemoved",
        GameEvent::AutoAttack { .. } => "autoAttack",
        _ => "autre",
    }
}

fn spawn(world: &mut World, player: &str, champion: &str, team: Team, pos: Vec2) -> Option<Actor> {
    let def = champion_def(champion)?;
    let id = create_champion(
        world,
        CreateChampion {
            player_id: player.into(),
            name: def.name.clone(),
            team,
            champion_id: champion.into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        },
        &def.base_stats,
    );
    // niveau 18, tous les sorts au rang maximal (le labo teste les FX, pas la progression)
    lov_sim::systems::leveling::grant_xp(world, id, 1.0e7);
    if let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut()) {
        c.ability_levels.q = 5;
        c.ability_levels.w = 5;
        c.ability_levels.e = 5;
        c.ability_levels.r = 3;
        c.skill_points = 0;
    }
    Some(Actor {
        id,
        player: player.into(),
        home: pos,
        prev: pos,
    })
}

/// Snapshot d'une entité pour le client (champs utiles aux FX et au rendu).
fn snapshot(e: &Entity, world: &World) -> EntitySnapshot {
    let mut s = EntitySnapshot {
        id: e.id,
        kind: e.kind,
        team: e.team,
        x: e.pos.x,
        y: e.pos.y,
        facing: e.facing,
        hp: e.hp,
        max_hp: e.stats.max_hp,
        mana: e.champ.as_ref().map(|_| e.mana),
        max_mana: e.champ.as_ref().map(|_| e.stats.max_mana),
        shield: (e.shield > 0.0).then_some(e.shield),
        dead: e.dead.then_some(true),
        subtype: e.subtype.clone(),
        owner_id: e.champ.as_ref().map(|c| c.player_id.clone()),
        level: e.champ.as_ref().map(|c| c.level),
        anim: e.anim.clone(),
        buffs: None,
        champ: None,
    };
    world.apply_entity_snapshot(e, &mut s);
    s
}

impl SpellLab {
    fn new(champions: Vec<String>, shots: Option<String>) -> Option<Self> {
        let first = champions.first()?.clone();
        let mut world = World::new(7);
        let mut systems = vec![
            input_system(),
            buffs_system(),
            summoners_system(),
            abilities_system(),
            movement_system(),
            combat_system(),
            leveling_system(),
        ];
        let caster = spawn(&mut world, "lab-lanceur", &first, Team::Blue, CASTER_AT)?;
        let enemy = spawn(&mut world, "lab-cible", "tharox", Team::Red, ENEMY_AT)?;
        let ally = spawn(&mut world, "lab-allie", "solenne", Team::Blue, ALLY_AT)?;
        world.init(&mut systems);
        world.events.drain();
        Some(Self {
            world,
            systems,
            champions,
            champion: 0,
            caster,
            enemy,
            ally,
            acc: 0.0,
            clock: 0.0,
            next_cast: 1.0,
            slot: 0,
            watching: None,
            shots,
            shots_due: Vec::new(),
            stats_at_cast: (0, 0),
            dirty: true,
        })
    }

    fn champion_id(&self) -> &str {
        &self.champions[self.champion]
    }

    /// Remet tout le monde à sa place, en pleine santé, ressources et recharges à zéro.
    fn reset(&mut self) {
        for a in [&mut self.caster, &mut self.enemy, &mut self.ally] {
            if let Some(e) = self.world.get_mut(a.id) {
                e.pos = a.home;
                e.move_target = None;
                e.path = None;
                e.attack_target = None;
                e.hp = e.stats.max_hp;
                a.prev = a.home;
            }
        }
    }

    fn refresh(&mut self) {
        if let Some(e) = self.world.get_mut(self.caster.id) {
            e.mana = e.stats.max_mana;
            if let Some(c) = e.champ.as_mut() {
                let cd = &mut c.cooldowns;
                (cd.q, cd.w, cd.e, cd.r, cd.d, cd.f) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            }
        }
    }

    /// Champion suivant (`all`) : l'ancien lanceur est remplacé.
    fn next_champion(&mut self) {
        if self.champions.len() < 2 {
            return;
        }
        self.champion = (self.champion + 1) % self.champions.len();
        let id = self.champion_id().to_owned();
        self.world.remove(self.caster.id);
        if let Some(a) = spawn(&mut self.world, "lab-lanceur", &id, Team::Blue, CASTER_AT) {
            self.caster = a;
        }
        // les montées de niveau de `grant_xp` ne sont pas des FX à montrer (comme au démarrage)
        self.world.events.drain();
        self.reset();
    }

    fn cast(&mut self) {
        self.refresh();
        let slot = SLOTS[self.slot];
        // l'allié pour les sorts de la liste, l'ennemi sinon (comme le fxlab web) : un sort hors de portée
        // met un déplacement en file, il ne faut donc pas déduire la cible d'un refus
        let on_ally = champion_def(self.champion_id())
            .is_some_and(|d| ALLY_TARGETED.contains(&d.abilities.get(slot).id.as_str()));
        let (target, fallback) = if on_ally {
            (self.ally.id, ALLY_AT)
        } else {
            (self.enemy.id, ENEMY_AT)
        };
        let target_pos = self.world.get(target).map_or(fallback, |e| e.pos);
        let command = CastCommand {
            slot,
            target_id: Some(target),
            target_pos: Some(target_pos),
        };
        cast_ability(&mut self.world, self.caster.id, &command, true);
        let name = format!("{}_{}", self.champion_id(), slot_name(slot));
        if let Some(dir) = &self.shots {
            for (k, at) in SHOTS.iter().enumerate() {
                self.shots_due
                    .push((self.clock + at, format!("{dir}/{name}_{}.png", k + 1)));
            }
        }
        self.watching = Some((name, self.clock, HashMap::new()));
        self.slot += 1;
        if self.slot == SLOTS.len() {
            self.slot = 0;
        }
    }

    /// Un tick de simulation ; renvoie ses événements.
    fn step(&mut self) -> Vec<GameEvent> {
        for a in [&mut self.caster, &mut self.enemy, &mut self.ally] {
            if let Some(e) = self.world.get(a.id) {
                a.prev = e.pos;
            }
        }
        self.world.step(&mut self.systems);
        self.clock += TICK_DT;
        self.dirty = true;
        // la cible et l'allié restent en vie (le labo montre les FX, pas la mort)
        for id in [self.enemy.id, self.ally.id] {
            if let Some(e) = self.world.get_mut(id)
                && !e.dead
                && e.hp < e.stats.max_hp * 0.35
            {
                e.hp = e.stats.max_hp;
            }
        }
        self.world.events.drain()
    }
}

fn slot_name(s: AbilitySlot) -> &'static str {
    match s {
        AbilitySlot::Q => "q",
        AbilitySlot::W => "w",
        AbilitySlot::E => "e",
        AbilitySlot::R => "r",
    }
}

pub struct SpellLabPlugin;

impl Plugin for SpellLabPlugin {
    fn build(&self, app: &mut App) {
        let wanted = requested().unwrap_or_default();
        let champions: Vec<String> = if wanted == "all" {
            CHAMPIONS.iter().map(|c| c.id.clone()).collect()
        } else {
            vec![wanted.clone()]
        };
        let shots = std::env::var("LOV_LAB_SHOTS")
            .ok()
            .filter(|v| !v.is_empty());
        if let Some(dir) = &shots
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            warn!("labo des sorts : dossier de captures {dir} impossible : {e}");
        }
        match SpellLab::new(champions, shots) {
            Some(lab) => {
                app.insert_resource(SpellLabRes(Mutex::new(lab)))
                    .add_systems(Startup, setup)
                    .add_systems(PreUpdate, run_lab);
            }
            None => error!("labo des sorts : champion inconnu ({wanted})"),
        }
    }
}

fn setup(lab: Res<SpellLabRes>, mut state: ResMut<GameState>, mut rig: ResMut<CameraRig>) {
    let Ok(lab) = lab.0.lock() else { return };
    state.status = format!("LABO DES SORTS : {}", lab.champion_id());
    state.my_entity = Some(lab.caster.id);
    state.game = Some(GameStart {
        you: lab.caster.player.clone(),
        team: Team::Blue,
        entity_id: lab.caster.id,
        players: vec![],
        server_time: 0.0,
    });
    rig.locked = true;
    rig.distance *= 0.75;
}

fn run_lab(
    mut commands: Commands,
    time: Res<Time>,
    lab: Res<SpellLabRes>,
    mut state: ResMut<GameState>,
    mut events: MessageWriter<ServerEvent>,
    dispatcher: Res<FxDispatcher>,
) {
    let Ok(mut lab) = lab.0.lock() else { return };
    lab.acc = (lab.acc + f64::from(time.delta_secs())).min(4.0 * TICK_DT);
    while lab.acc >= TICK_DT {
        lab.acc -= TICK_DT;
        if lab.clock >= lab.next_cast {
            lab.next_cast = lab.clock + CAST_EVERY;
            if lab.slot == 0 && lab.watching.is_some() {
                lab.next_champion();
                state.status = format!("LABO DES SORTS : {}", lab.champion_id());
                state.my_entity = Some(lab.caster.id);
                if let Some(g) = state.game.as_mut() {
                    g.entity_id = lab.caster.id;
                }
            }
            if let Some((name, _, counts)) = lab.watching.take() {
                let custom = dispatcher.stats.custom - lab.stats_at_cast.0;
                let defaults = dispatcher.stats.defaults - lab.stats_at_cast.1;
                info!(
                    "labo des sorts : {name} → {counts:?} ; FX dédiés {custom}, génériques {defaults}"
                );
            }
            lab.stats_at_cast = (dispatcher.stats.custom, dispatcher.stats.defaults);
            lab.reset();
            lab.cast();
            let (caster, enemy) = (lab.caster.player.clone(), lab.enemy.id);
            lab.world.commands.push(QueuedCommand {
                player_id: caster,
                msg: ClientMsg::Attack { target_id: enemy },
            });
        }
        for e in lab.step() {
            if let Some((_, _, counts)) = lab.watching.as_mut() {
                *counts.entry(event_name(&e)).or_default() += 1;
            }
            events.write(ServerEvent(e));
        }
    }
    // snapshots et positions interpolées entre les deux derniers ticks
    let k = lab.acc / TICK_DT;
    let lab = &mut *lab;
    // les snapshots ne changent qu'avec un tick : inutile de les reconstruire à chaque image
    if lab.dirty {
        lab.dirty = false;
        state.buffer.entities.clear();
        for e in lab.world.iter() {
            state.buffer.entities.insert(e.id, snapshot(e, &lab.world));
        }
    }
    for a in [&lab.caster, &lab.enemy, &lab.ally] {
        if let Some(e) = lab.world.get(a.id) {
            state.poses.insert(
                a.id,
                Pose {
                    x: a.prev.x + (e.pos.x - a.prev.x) * k,
                    y: a.prev.y + (e.pos.y - a.prev.y) * k,
                    facing: e.facing,
                },
            );
        }
    }
    let clock = lab.clock;
    lab.shots_due.retain(|(at, path)| {
        if clock < *at {
            return true;
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lance le sort courant puis fait tourner la simulation `CAST_EVERY` secondes.
    fn cast_and_run(lab: &mut SpellLab) -> Vec<GameEvent> {
        lab.reset();
        lab.cast();
        let mut events = Vec::new();
        for _ in 0..(CAST_EVERY / TICK_DT) as usize {
            events.extend(lab.step());
        }
        events
    }

    #[test]
    fn chaque_sort_de_chaque_champion_part_sur_la_simulation() {
        let champions: Vec<String> = CHAMPIONS.iter().map(|c| c.id.clone()).collect();
        let mut lab = SpellLab::new(champions.clone(), None).expect("labo");
        let mut silent = Vec::new();
        for (k, champion) in champions.iter().enumerate() {
            if k > 0 {
                lab.next_champion();
            }
            assert_eq!(lab.champion_id(), champion);
            for slot in SLOTS {
                let events = cast_and_run(&mut lab);
                let cast = events.iter().any(|e| {
                    matches!(e, GameEvent::AbilityCast { source, slot: s, .. }
                        if *source == lab.caster.id && *s == slot)
                });
                if !cast {
                    silent.push(format!("{champion}_{}", slot_name(slot)));
                }
            }
        }
        assert!(silent.is_empty(), "sorts sans abilityCast : {silent:?}");
    }

    #[test]
    fn changer_de_champion_ne_rejoue_pas_les_montees_de_niveau() {
        let champions = vec!["ilyra".to_owned(), "solenne".to_owned()];
        let mut lab = SpellLab::new(champions, None).expect("labo");
        lab.next_champion();
        let events = lab.step();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, GameEvent::LevelUp { .. })),
            "montées de niveau rejouées : {events:?}"
        );
    }
}
