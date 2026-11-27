//! Aiguillage `GameEvent` → handler FX : handler de la fxKey, sinon handler générique. Garde la trace des
//! projectiles en vol pour que `projectileHit` les arrête au bon endroit.
//!
//! Portage de `packages/client/src/fx/dispatcher.ts`.

use std::collections::HashMap;

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::runtime::{Fx, FxRuntime};
use crate::fx::keys::{FxPhase, resolve_fx};
use crate::fx::registry::{FxFn, FxRegistry, ProjectileHandle};

/// Durée max de suivi d'un projectile sans impact (skillshot raté…), en secondes.
const PROJECTILE_TTL: f32 = 8.0;

/// Compteurs de debug (fxlab).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchStats {
    pub events: u32,
    pub defaults: u32,
    pub custom: u32,
}

/// État du dispatcher (`FxDispatcher`) : projectiles en vol et compteurs.
#[derive(Resource, Default)]
pub struct FxDispatcher {
    projectiles: HashMap<EntityId, (Box<dyn ProjectileHandle>, f32)>,
    pub stats: DispatchStats,
}

/// Auteur d'un événement de sort (pour la teinte ennemie).
fn source_of(e: &GameEvent) -> Option<EntityId> {
    match e {
        GameEvent::AbilityCast { source, .. }
        | GameEvent::ProjectileSpawn { source, .. }
        | GameEvent::AreaEffect { source, .. }
        | GameEvent::Dash { source, .. }
        | GameEvent::SummonerCast { source, .. } => Some(*source),
        GameEvent::BuffApplied { source, .. } => *source,
        _ => None,
    }
}

impl FxDispatcher {
    /// Projectiles suivis.
    pub fn in_flight(&self) -> usize {
        self.projectiles.len()
    }

    /// Traite un événement : handlers du monde, impact de sort, puis handler de la phase.
    pub fn handle(world: &mut World, e: &GameEvent) {
        world.resource_scope(|world, mut d: Mut<FxDispatcher>| d.run(world, e));
    }

    fn run(&mut self, world: &mut World, e: &GameEvent) {
        self.stats.events += 1;
        let (world_count, ability_hit, buff_fx) = {
            let reg = world.resource::<FxRegistry>();
            let hit = match e {
                GameEvent::Damage {
                    ability: Some(a), ..
                } => reg.ability_hit(a),
                _ => None,
            };
            (reg.world_handlers().len(), hit, reg.buff_fx)
        };
        // par index : un handler a besoin du monde entier, on ne garde pas d'emprunt sur le registre
        for i in 0..world_count {
            let f = world.resource::<FxRegistry>().world_handlers()[i];
            f(&mut Fx::new(world), e);
        }
        if let Some(f) = ability_hit {
            f(&mut Fx::new(world), e);
        }
        let Some(resolved) = resolve_fx(e, buff_fx) else {
            return;
        };
        let (custom, defaults) = {
            let reg = world.resource::<FxRegistry>();
            (
                reg.get(&resolved.key).copied().unwrap_or_default(),
                reg.defaults,
            )
        };
        if custom.has(resolved.phase) {
            self.stats.custom += 1;
        } else {
            self.stats.defaults += 1;
        }
        let colors = world.resource_mut::<FxRuntime>().colors(&resolved.key);
        let enemy = source_of(e).is_some_and(|s| Fx::new(world).is_enemy(s));
        world.resource_mut::<FxRuntime>().enemy_context = enemy;
        let mut fx = Fx::new(world);
        let phase_fn = |custom: Option<FxFn>, default: FxFn| custom.unwrap_or(default);
        match resolved.phase {
            FxPhase::Cast => phase_fn(custom.cast, defaults.cast)(&mut fx, e, &colors),
            FxPhase::Projectile => {
                let f = custom.projectile.unwrap_or(defaults.projectile);
                if let (Some(h), GameEvent::ProjectileSpawn { id, .. }) =
                    (f(&mut fx, e, &colors), e)
                {
                    let now = fx.time();
                    if self.projectiles.len() > 64 {
                        self.prune(now);
                    }
                    self.projectiles.insert(*id, (h, now));
                }
            }
            FxPhase::Hit => {
                if let GameEvent::ProjectileHit { id, pos, .. } = e
                    && let Some((mut h, _)) = self.projectiles.remove(id)
                {
                    h.finish(&mut fx, Fx::world_pos(*pos, 0.0));
                }
                phase_fn(custom.hit, defaults.hit)(&mut fx, e, &colors);
            }
            FxPhase::Area => phase_fn(custom.area, defaults.area)(&mut fx, e, &colors),
            FxPhase::Dash => phase_fn(custom.dash, defaults.dash)(&mut fx, e, &colors),
            FxPhase::Buff => phase_fn(custom.buff, defaults.buff)(&mut fx, e, &colors),
            FxPhase::BuffEnd => phase_fn(custom.buff_end, defaults.buff_end)(&mut fx, e, &colors),
            FxPhase::Summoner => phase_fn(custom.summoner, defaults.summoner)(&mut fx, e, &colors),
        }
        world.resource_mut::<FxRuntime>().enemy_context = false;
    }

    fn prune(&mut self, now: f32) {
        self.projectiles
            .retain(|_, (_, at)| now - *at <= PROJECTILE_TTL);
    }
}

/// Port de `packages/client/test/fx/dispatcher.test.ts`.
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use lov_shared::protocol::GameStart;
    use lov_shared::types::{AbilitySlot, EntityKind, EntitySnapshot, Team, TeamOrNeutral, Vec2};

    use super::*;
    use crate::fx::core::palette::FxColors;
    use crate::fx::registry::{AllHandlers, FxHandlers};
    use crate::net::GameState;

    const O: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// Appels reçus par les handlers de test (les handlers sont des `fn` : pas de capture possible).
    static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn log(s: &str) {
        if let Ok(mut c) = CALLS.lock() {
            c.push(s.to_owned());
        }
    }

    fn take_calls() -> Vec<String> {
        CALLS
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default()
    }

    /// Les tests partagent `CALLS` : ils ne doivent pas tourner en même temps.
    static SERIAL: Mutex<()> = Mutex::new(());

    macro_rules! logger {
        ($name:ident, $label:literal) => {
            fn $name(_: &mut Fx, _: &GameEvent, _: &FxColors) {
                log($label);
            }
        };
    }
    logger!(d_cast, "default cast");
    logger!(d_hit, "default hit");
    logger!(d_area, "default area");
    logger!(d_other, "default other");
    logger!(custom_cast, "custom cast");
    logger!(merge_a, "a");
    logger!(merge_b, "b");

    fn d_projectile(_: &mut Fx, _: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
        log("default projectile");
        Some(Box::new(Finish))
    }

    struct Finish;
    impl ProjectileHandle for Finish {
        fn finish(&mut self, _: &mut Fx, at: Vec3) {
            log(&format!("finish {}", at.x));
        }
    }

    fn defaults() -> AllHandlers {
        AllHandlers {
            cast: d_cast,
            projectile: d_projectile,
            hit: d_hit,
            area: d_area,
            dash: d_other,
            buff: d_other,
            buff_end: d_other,
            summoner: d_other,
        }
    }

    /// Monde de test : runtime, registre, dispatcher, et un état réseau où l'entité 666 est ennemie.
    fn world() -> World {
        let mut w = World::new();
        w.init_resource::<FxRuntime>();
        w.init_resource::<FxDispatcher>();
        let mut reg = FxRegistry::default();
        reg.defaults = defaults();
        w.insert_resource(reg);
        let mut state = GameState::default();
        state.game = Some(GameStart {
            you: "p1".into(),
            team: Team::Blue,
            entity_id: 1,
            players: vec![],
            server_time: 0.0,
        });
        for (id, team) in [(1, TeamOrNeutral::Blue), (666, TeamOrNeutral::Red)] {
            state.buffer.entities.insert(
                id,
                EntitySnapshot {
                    id,
                    kind: EntityKind::Champion,
                    team,
                    x: 0.0,
                    y: 0.0,
                    facing: 0.0,
                    hp: 1.0,
                    max_hp: 1.0,
                    mana: None,
                    max_mana: None,
                    shield: None,
                    dead: None,
                    subtype: None,
                    owner_id: None,
                    level: None,
                    anim: None,
                    buffs: None,
                    champ: None,
                },
            );
        }
        w.insert_resource(state);
        w
    }

    fn cast(champion: &str) -> GameEvent {
        GameEvent::AbilityCast {
            source: 1,
            champion_id: champion.into(),
            slot: AbilitySlot::Q,
            ability_id: "x".into(),
            target_pos: None,
            target_id: None,
            fx_key: None,
        }
    }

    fn area(source: EntityId, key: &str) -> GameEvent {
        GameEvent::AreaEffect {
            source,
            fx_key: key.into(),
            pos: O,
            radius: 100.0,
            duration: 0.5,
        }
    }

    #[test]
    fn handler_de_la_fx_key_puis_generique_pour_les_autres_phases() {
        let _g = SERIAL.lock();
        take_calls();
        let mut w = world();
        w.resource_mut::<FxRegistry>().register(
            "testchamp_q",
            FxHandlers {
                cast: Some(custom_cast),
                ..default()
            },
        );
        // championId inconnu des données : la clé reste `testchamp_q`
        FxDispatcher::handle(&mut w, &cast("testchamp"));
        FxDispatcher::handle(&mut w, &area(1, "testchamp_q"));
        assert_eq!(take_calls(), ["custom cast", "default area"]);
        assert_eq!(
            w.resource::<FxDispatcher>().stats,
            DispatchStats {
                events: 2,
                defaults: 1,
                custom: 1
            }
        );
    }

    #[test]
    fn register_complete_une_entree_existante() {
        let _g = SERIAL.lock();
        take_calls();
        let mut w = world();
        let mut reg = w.resource_mut::<FxRegistry>();
        reg.register(
            "merge_q",
            FxHandlers {
                cast: Some(merge_a),
                ..default()
            },
        );
        reg.register(
            "merge_q",
            FxHandlers {
                area: Some(merge_b),
                ..default()
            },
        );
        FxDispatcher::handle(&mut w, &cast("merge"));
        FxDispatcher::handle(&mut w, &area(1, "merge_q"));
        assert_eq!(take_calls(), ["a", "b"]);
    }

    #[test]
    fn arrete_le_projectile_a_l_impact_a_la_position_de_l_impact() {
        let _g = SERIAL.lock();
        take_calls();
        let mut w = world();
        let spawn = GameEvent::ProjectileSpawn {
            id: 7,
            source: 1,
            fx_key: "p_q".into(),
            from: O,
            to: Vec2 { x: 1000.0, y: 0.0 },
            speed: 1000.0,
            width: 80.0,
        };
        FxDispatcher::handle(&mut w, &spawn);
        assert_eq!(w.resource::<FxDispatcher>().in_flight(), 1);
        let hit = GameEvent::ProjectileHit {
            id: 7,
            target: None,
            pos: Vec2 { x: 500.0, y: 0.0 },
            fx_key: "p_q".into(),
        };
        FxDispatcher::handle(&mut w, &hit);
        assert_eq!(
            take_calls(),
            ["default projectile", "finish 5", "default hit"]
        );
        assert_eq!(w.resource::<FxDispatcher>().in_flight(), 0);
    }

    fn record_enemy(fx: &mut Fx, _: &GameEvent, _: &FxColors) {
        log(&format!("enemy {}", fx.enemy_context()));
    }

    #[test]
    fn contexte_ennemi_pendant_le_handler_d_un_evenement_ennemi() {
        let _g = SERIAL.lock();
        take_calls();
        let mut w = world();
        w.resource_mut::<FxRegistry>().defaults.area = record_enemy;
        FxDispatcher::handle(&mut w, &area(666, "x_r"));
        FxDispatcher::handle(&mut w, &area(1, "x_r"));
        assert_eq!(take_calls(), ["enemy true", "enemy false"]);
        assert!(!w.resource::<FxRuntime>().enemy_context);
    }

    fn on_level(_: &mut Fx, e: &GameEvent) {
        if let GameEvent::LevelUp { target, level, .. } = e {
            log(&format!("level {target} {level}"));
        }
    }

    fn on_hit(_: &mut Fx, e: &GameEvent) {
        if let GameEvent::Damage { target, .. } = e {
            log(&format!("ability hit {target}"));
        }
    }

    #[test]
    fn transmet_les_evenements_du_monde_et_les_impacts_de_sort() {
        let _g = SERIAL.lock();
        take_calls();
        let mut w = world();
        w.resource_mut::<FxRegistry>().on_world_event(on_level);
        w.resource_mut::<FxRegistry>()
            .on_ability_hit("ilyra_q", on_hit);
        FxDispatcher::handle(
            &mut w,
            &GameEvent::LevelUp {
                target: 3,
                level: 6,
                fx_key: None,
            },
        );
        let damage = |ability: &str| GameEvent::Damage {
            source: 1,
            target: 9,
            amount: 10.0,
            damage_type: lov_shared::types::DamageType::Magic,
            crit: None,
            ability: Some(ability.into()),
        };
        FxDispatcher::handle(&mut w, &damage("ilyra_q"));
        FxDispatcher::handle(&mut w, &damage("autre"));
        assert_eq!(take_calls(), ["level 3 6", "ability hit 9"]);
    }
}
