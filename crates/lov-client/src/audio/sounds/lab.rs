//! Séquence d'événements factices pour la démo `LOV_AUDIOLAB` : fait entendre la table événement → son
//! sans serveur, avec une petite partie simulée (le joueur, un allié, deux ennemis, un sbire).
//!
//! Les événements passent directement par [`play_event`], pas par des `ServerEvent` : ils ne
//! déclenchent ni FX ni rendu.

use std::collections::HashMap;

use lov_shared::protocol::GameStart;
use lov_shared::types::{
    AbilitySlot, DamageType, EntityId, EntityKind, EntitySnapshot, GameEvent, MonsterType,
    StructureKind, SummonerId, Team, TeamOrNeutral, Vec2 as GameVec2,
};

use super::kills::KillAnnouncer;
use super::{play_event, recipes};
use crate::audio::AudioManager;
use crate::net::GameState;

/// Le joueur local (bleu).
pub const ME: EntityId = 1;
/// Un allié (bleu).
pub const ALLY: EntityId = 2;
/// Deux ennemis (rouges).
pub const FOE: EntityId = 10;
pub const FOE2: EntityId = 11;
/// Un sbire rouge.
pub const MINION: EntityId = 50;

/// Une entité minimale pour les annonces et la spatialisation.
pub fn entity(id: EntityId, kind: EntityKind, team: TeamOrNeutral) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind,
        team,
        x: 6000.0,
        y: 6000.0,
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
    }
}

/// Partie factice : le joueur [`ME`] dans l'équipe bleue.
pub fn fake_state() -> GameState {
    let mut state = GameState::default();
    state.game = Some(GameStart {
        you: "moi".into(),
        team: Team::Blue,
        entity_id: ME,
        players: Vec::new(),
        server_time: 1.0,
    });
    state.my_entity = Some(ME);
    let ents: HashMap<EntityId, EntitySnapshot> = [
        entity(ME, EntityKind::Champion, TeamOrNeutral::Blue),
        entity(ALLY, EntityKind::Champion, TeamOrNeutral::Blue),
        entity(FOE, EntityKind::Champion, TeamOrNeutral::Red),
        entity(FOE2, EntityKind::Champion, TeamOrNeutral::Red),
        entity(MINION, EntityKind::Minion, TeamOrNeutral::Red),
    ]
    .into_iter()
    .map(|e| (e.id, e))
    .collect();
    state.buffer.entities = ents;
    state
}

/// État de la démo : la partie factice et ses annonces.
pub struct EventLab {
    pub state: GameState,
    pub kills: KillAnnouncer,
}

impl Default for EventLab {
    fn default() -> Self {
        Self {
            state: fake_state(),
            kills: KillAnnouncer::default(),
        }
    }
}

impl EventLab {
    /// Place toutes les entités sur l'écouteur (sons à pleine présence) et joue un événement.
    fn play(&mut self, a: &mut AudioManager, e: &GameEvent) {
        let l = a.listener;
        for ent in self.state.buffer.entities.values_mut() {
            ent.x = l.x;
            ent.y = l.y;
        }
        play_event(a, &mut self.kills, &self.state, e);
    }
}

fn p(x: f64, y: f64) -> GameVec2 {
    GameVec2 { x, y }
}

fn cast(champion: &str, slot: AbilitySlot) -> GameEvent {
    GameEvent::AbilityCast {
        source: FOE,
        champion_id: champion.into(),
        slot,
        ability_id: String::new(),
        target_pos: None,
        target_id: None,
        fx_key: None,
    }
}

fn death(target: EntityId, killer: EntityId) -> GameEvent {
    GameEvent::Death {
        target,
        killer: Some(killer),
        assists: None,
        bounty: None,
    }
}

fn damage(damage_type: DamageType, crit: bool) -> GameEvent {
    GameEvent::Damage {
        source: ME,
        target: FOE,
        amount: 50.0,
        damage_type,
        crit: Some(crit),
        ability: None,
    }
}

/// Une entrée : nom, durée avant la suivante (s), et ce qu'elle joue.
pub type LabEntry = (&'static str, f32, fn(&mut AudioManager, &mut EventLab));

/// La séquence, rejouée dans l'ordre ; la partie factice repart de zéro à la première entrée.
pub const LAB: &[LabEntry] = &[
    (
        "événements : nouvelle partie, auto-attaques mêlée puis distance",
        1.0,
        |a, l| {
            l.kills.reset();
            l.play(
                a,
                &GameEvent::AutoAttack {
                    source: ME,
                    target: FOE,
                    ranged: false,
                    speed: None,
                    crit: None,
                },
            );
            l.play(
                a,
                &GameEvent::AutoAttack {
                    source: FOE,
                    target: ME,
                    ranged: true,
                    speed: None,
                    crit: None,
                },
            );
        },
    ),
    (
        "événements : dégâts physiques, magiques, bruts, critique",
        1.0,
        |a, l| {
            l.play(a, &damage(DamageType::Physical, false));
            l.play(a, &damage(DamageType::Magic, false));
            l.play(a, &damage(DamageType::True, false));
            l.play(a, &damage(DamageType::Physical, true));
        },
    ),
    ("événements : soin et bouclier", 1.0, |a, l| {
        l.play(
            a,
            &GameEvent::Heal {
                source: ME,
                target: ME,
                amount: 10.0,
            },
        );
        l.play(
            a,
            &GameEvent::Shield {
                source: ME,
                target: ME,
                amount: 10.0,
            },
        );
    }),
    (
        "événements : sorts Q, W, E et R de Kaelen",
        1.6,
        |a, l| {
            for slot in [
                AbilitySlot::Q,
                AbilitySlot::W,
                AbilitySlot::E,
                AbilitySlot::R,
            ] {
                l.play(a, &cast("kaelen", slot));
            }
        },
    ),
    (
        "événements : ult de Grondak (recette procédurale, comme le web)",
        1.2,
        |a, l| {
            l.play(a, &cast("grondak", AbilitySlot::R));
        },
    ),
    (
        "événements : sort inconnu (son générique)",
        1.0,
        |a, l| l.play(a, &cast("inconnu", AbilitySlot::Q)),
    ),
    ("événements : sorts d'invocateur", 2.5, |a, l| {
        for spell in [SummonerId::Flash, SummonerId::Ignite, SummonerId::Heal] {
            l.play(
                a,
                &GameEvent::SummonerCast {
                    source: ME,
                    spell,
                    from: p(0.0, 0.0),
                    to: None,
                    target_id: None,
                    fx_key: None,
                },
            );
        }
    }),
    ("événements : projectile, ruée, zone", 1.2, |a, l| {
        let here = a.listener;
        l.play(
            a,
            &GameEvent::ProjectileHit {
                id: 99,
                target: None,
                pos: here,
                fx_key: String::new(),
            },
        );
        l.play(
            a,
            &GameEvent::Dash {
                source: ME,
                from: here,
                to: here,
                duration: 0.3,
                fx_key: None,
            },
        );
        l.play(
            a,
            &GameEvent::AreaEffect {
                source: ME,
                fx_key: "zone".into(),
                pos: here,
                radius: 100.0,
                duration: 1.0,
            },
        );
    }),
    (
        "événements : altération posée puis retirée",
        1.0,
        |a, l| {
            l.play(
                a,
                &GameEvent::BuffApplied {
                    target: ME,
                    buff_id: String::new(),
                    duration: 1.0,
                    source: None,
                    fx_key: None,
                    stacks: None,
                },
            );
            l.play(
                a,
                &GameEvent::BuffRemoved {
                    target: ME,
                    buff_id: String::new(),
                    fx_key: None,
                },
            );
        },
    ),
    (
        "événements : niveau, or, achat, vente, potion",
        1.4,
        |a, l| {
            l.play(
                a,
                &GameEvent::LevelUp {
                    target: ME,
                    level: 2,
                    fx_key: None,
                },
            );
            l.play(
                a,
                &GameEvent::GoldGain {
                    target: ME,
                    amount: 20.0,
                    pos: None,
                },
            );
            l.play(
                a,
                &GameEvent::ItemBought {
                    target: ME,
                    item_id: String::new(),
                },
            );
            l.play(
                a,
                &GameEvent::ItemSold {
                    target: ME,
                    item_id: String::new(),
                },
            );
            l.play(
                a,
                &GameEvent::ItemUsed {
                    target: ME,
                    item_id: "health_potion".into(),
                },
            );
        },
    ),
    (
        "événements : rappel interrompu, puis rappel complet",
        2.6,
        |a, l| {
            l.play(
                a,
                &GameEvent::RecallEnd {
                    target: ME,
                    completed: false,
                },
            );
            l.play(a, &GameEvent::RecallStart { target: ME });
        },
    ),
    (
        "événements : rappel terminé, réapparition",
        1.2,
        |a, l| {
            l.play(
                a,
                &GameEvent::RecallEnd {
                    target: ME,
                    completed: true,
                },
            );
            l.play(
                a,
                &GameEvent::Respawn {
                    target: ME,
                    pos: p(0.0, 0.0),
                },
            );
        },
    ),
    (
        "événements : mort d'un sbire (pas de jingle)",
        1.0,
        |a, l| l.play(a, &death(MINION, ME)),
    ),
    ("événements : premier sang (soi)", 3.6, |a, l| {
        l.play(a, &death(FOE, ME))
    }),
    ("événements : double kill (soi)", 2.4, |a, l| {
        l.play(a, &death(FOE2, ME))
    }),
    (
        "événements : kill d'un ennemi sur l'allié",
        2.0,
        |a, l| l.play(a, &death(ALLY, FOE)),
    ),
    ("événements : kill de l'allié", 2.0, |a, l| {
        l.play(a, &death(FOE, ALLY))
    }),
    ("événements : tour détruite", 1.8, |a, l| {
        l.play(
            a,
            &GameEvent::StructureDestroyed {
                target: 70,
                kind: StructureKind::Tower,
                team: Team::Red,
                plate: None,
            },
        );
    }),
    (
        "événements : objectif pris, annonce, ping",
        1.8,
        |a, l| {
            let here = a.listener;
            l.play(
                a,
                &GameEvent::ObjectiveTaken {
                    monster: MonsterType::Dragon,
                    team: Team::Blue,
                    killer: None,
                    element: None,
                },
            );
            l.play(
                a,
                &GameEvent::Announcement {
                    key: "colosse_invoque".into(),
                    team: None,
                    params: None,
                },
            );
            l.play(
                a,
                &GameEvent::Ping {
                    player_id: "moi".into(),
                    pos: here,
                    kind: String::new(),
                },
            );
        },
    ),
    (
        "recettes dédiées des ults (jamais jouées par le web au lancement)",
        1.0,
        |_, _| {},
    ),
    ("ult : Mélenchon", 2.0, |a, _| custom(a, 0)),
    ("ult : Grondak", 2.0, |a, _| custom(a, 1)),
    ("ult : Tharox", 1.8, |a, _| custom(a, 2)),
    ("ult : Kaelen", 1.2, |a, _| custom(a, 3)),
    ("ult : Vrenna", 1.2, |a, _| custom(a, 4)),
    ("ult : Ilyra", 4.3, |a, _| custom(a, 5)),
    ("ult : Zephyr", 1.6, |a, _| custom(a, 6)),
    ("ult : Sable", 2.0, |a, _| custom(a, 7)),
    ("ult : Corvin", 3.2, |a, _| custom(a, 8)),
    ("ult : Nyx", 4.2, |a, _| custom(a, 9)),
    ("ult : Solenne", 1.6, |a, _| custom(a, 10)),
];

fn custom(a: &mut AudioManager, i: usize) {
    if let Some(&(_, f)) = recipes::CUSTOM.get(i) {
        f(a, None, 1.0);
    }
}
