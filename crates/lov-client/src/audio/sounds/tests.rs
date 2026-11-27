//! Tests de la table événement → son, dont le portage des cas de `kill-audio.test.ts` qui portent sur
//! `sounds.ts` (le cas des niveaux de jingle est porté avec la musique, dans `music/stings.rs`).

use lov_shared::data::champions::CHAMPIONS;
use lov_shared::types::{MonsterType, StructureKind, SummonerId, Team, Vec2 as GameVec2};

use super::kills::{KillAnnouncer, Sting};
use super::lab::{ALLY, EventLab, FOE, FOE2, LAB, ME, MINION, fake_state};
use super::*;
use crate::audio::music::BusCommand;
use crate::audio::music::stings::StingSide;
use crate::audio::{AudioManager, Command};

const HERE: GameVec2 = GameVec2 {
    x: 6000.0,
    y: 6000.0,
};

/// Joue `e` sur un gestionnaire neuf (polyphonie libre) et renvoie ce que le fil audio recevrait.
fn run(e: &GameEvent) -> Vec<Command> {
    let (mut am, rx) = AudioManager::for_tests();
    let mut kills = KillAnnouncer::default();
    play_event(&mut am, &mut kills, &fake_state(), e);
    rx.try_iter().collect()
}

/// Les effets envoyés : canal, gain et nombre de voix.
fn effects(cmds: &[Command]) -> Vec<(Channel, f32, usize)> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Play(sfx) => Some((sfx.channel, sfx.gain, sfx.voices.len())),
            _ => None,
        })
        .collect()
}

/// Commandes adressées à un bus de musique (création ou phrase) : les jingles.
fn bus_commands(cmds: &[Command]) -> usize {
    cmds.iter()
        .filter(|c| matches!(c, Command::AddBus(..) | Command::Bus(..)))
        .count()
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

fn death(target: u32, killer: Option<u32>) -> GameEvent {
    GameEvent::Death {
        target,
        killer,
        assists: None,
        bounty: None,
    }
}

fn damage(damage_type: DamageType, crit: Option<bool>) -> GameEvent {
    GameEvent::Damage {
        source: ME,
        target: FOE,
        amount: 1.0,
        damage_type,
        crit,
        ability: None,
    }
}

#[test]
fn chaque_type_d_evenement_sonore_programme_des_voix_sur_le_bon_canal() {
    use Channel::{Sfx, Ui, Voice};
    let cases: Vec<(&str, GameEvent, Channel)> = vec![
        (
            "autoAttack mêlée",
            GameEvent::AutoAttack {
                source: ME,
                target: FOE,
                ranged: false,
                speed: None,
                crit: None,
            },
            Sfx,
        ),
        (
            "autoAttack distance",
            GameEvent::AutoAttack {
                source: ME,
                target: FOE,
                ranged: true,
                speed: None,
                crit: None,
            },
            Sfx,
        ),
        ("damage physique", damage(DamageType::Physical, None), Sfx),
        ("damage magique", damage(DamageType::Magic, None), Sfx),
        ("damage brut", damage(DamageType::True, Some(true)), Sfx),
        (
            "heal",
            GameEvent::Heal {
                source: ME,
                target: ME,
                amount: 1.0,
            },
            Sfx,
        ),
        (
            "shield",
            GameEvent::Shield {
                source: ME,
                target: ME,
                amount: 1.0,
            },
            Sfx,
        ),
        ("death", death(MINION, Some(ME)), Sfx),
        (
            "respawn",
            GameEvent::Respawn {
                target: ME,
                pos: HERE,
            },
            Sfx,
        ),
        (
            "levelUp",
            GameEvent::LevelUp {
                target: FOE,
                level: 2,
                fx_key: None,
            },
            Sfx,
        ),
        (
            "goldGain",
            GameEvent::GoldGain {
                target: ME,
                amount: 1.0,
                pos: None,
            },
            Ui,
        ),
        (
            "itemBought",
            GameEvent::ItemBought {
                target: ME,
                item_id: "x".into(),
            },
            Ui,
        ),
        (
            "itemSold",
            GameEvent::ItemSold {
                target: ME,
                item_id: "x".into(),
            },
            Ui,
        ),
        (
            "itemUsed potion",
            GameEvent::ItemUsed {
                target: FOE,
                item_id: "health_potion".into(),
            },
            Sfx,
        ),
        (
            "itemUsed autre",
            GameEvent::ItemUsed {
                target: FOE,
                item_id: "x".into(),
            },
            Sfx,
        ),
        ("recallStart", GameEvent::RecallStart { target: FOE }, Sfx),
        (
            "recallEnd complet",
            GameEvent::RecallEnd {
                target: FOE,
                completed: true,
            },
            Sfx,
        ),
        (
            "recallEnd interrompu",
            GameEvent::RecallEnd {
                target: FOE,
                completed: false,
            },
            Sfx,
        ),
        ("abilityCast", cast("nyx", AbilitySlot::E), Sfx),
        ("abilityCast inconnu", cast("personne", AbilitySlot::Q), Sfx),
        (
            "summonerCast",
            GameEvent::SummonerCast {
                source: 999,
                spell: SummonerId::Flash,
                from: HERE,
                to: None,
                target_id: None,
                fx_key: None,
            },
            Sfx,
        ),
        (
            "projectileHit",
            GameEvent::ProjectileHit {
                id: 1,
                target: None,
                pos: HERE,
                fx_key: String::new(),
            },
            Sfx,
        ),
        (
            "dash",
            GameEvent::Dash {
                source: ME,
                from: HERE,
                to: HERE,
                duration: 2.0,
                fx_key: None,
            },
            Sfx,
        ),
        (
            "areaEffect",
            GameEvent::AreaEffect {
                source: ME,
                fx_key: "nyx_w".into(),
                pos: HERE,
                radius: 1.0,
                duration: 3.0,
            },
            Sfx,
        ),
        (
            "buffApplied",
            GameEvent::BuffApplied {
                target: ME,
                buff_id: String::new(),
                duration: 1.0,
                source: None,
                fx_key: None,
                stacks: None,
            },
            Sfx,
        ),
        (
            "buffRemoved",
            GameEvent::BuffRemoved {
                target: ME,
                buff_id: String::new(),
                fx_key: None,
            },
            Sfx,
        ),
        (
            "structureDestroyed",
            GameEvent::StructureDestroyed {
                target: 70,
                kind: StructureKind::Tower,
                team: Team::Red,
                plate: None,
            },
            Sfx,
        ),
        (
            "objectiveTaken",
            GameEvent::ObjectiveTaken {
                monster: MonsterType::Dragon,
                team: Team::Red,
                killer: None,
                element: None,
            },
            Sfx,
        ),
        (
            "announcement",
            GameEvent::Announcement {
                key: "colosse_invoque".into(),
                team: None,
                params: None,
            },
            Voice,
        ),
        (
            "ping",
            GameEvent::Ping {
                player_id: "p".into(),
                pos: HERE,
                kind: String::new(),
            },
            Ui,
        ),
    ];
    for (name, e, channel) in cases {
        let fx = effects(&run(&e));
        assert_eq!(fx.len(), 1, "{name} : un effet");
        assert_eq!(fx[0].0, channel, "{name} : canal");
        assert!(fx[0].2 > 0, "{name} : des voix");
    }
}

#[test]
fn les_evenements_d_autrui_ou_sans_son_restent_muets() {
    let silent = [
        GameEvent::GoldGain {
            target: FOE,
            amount: 1.0,
            pos: None,
        },
        GameEvent::ItemBought {
            target: ALLY,
            item_id: "x".into(),
        },
        GameEvent::ItemSold {
            target: FOE,
            item_id: "x".into(),
        },
        // l'ult de Mélenchon gère son impact elle-même
        GameEvent::AreaEffect {
            source: ME,
            fx_key: "melenchon_r".into(),
            pos: HERE,
            radius: 1.0,
            duration: 1.0,
        },
        GameEvent::Chat {
            player_id: "p".into(),
            text: "gg".into(),
            team_only: false,
        },
        GameEvent::ObjectiveSpawn {
            monster: MonsterType::Dragon,
            element: None,
            pos: HERE,
        },
    ];
    for e in &silent {
        assert!(run(e).is_empty(), "{e:?}");
    }
}

#[test]
fn la_montee_de_niveau_du_joueur_est_plus_forte_que_celle_des_autres() {
    let gain = |target| {
        effects(&run(&GameEvent::LevelUp {
            target,
            level: 3,
            fx_key: None,
        }))[0]
            .1
    };
    assert!((gain(ME) - 1.0).abs() < 1e-6);
    assert!((gain(FOE) - 0.7).abs() < 1e-6);
}

#[test]
fn chaque_sort_de_chaque_champion_joue_sa_recette_procedurale() {
    for c in CHAMPIONS.iter() {
        for slot in [
            AbilitySlot::Q,
            AbilitySlot::W,
            AbilitySlot::E,
            AbilitySlot::R,
        ] {
            let fx = effects(&run(&cast(&c.id, slot)));
            // onde + souffle : la recette procédurale (le son générique n'a qu'une voix)
            assert_eq!(fx.len(), 1, "{} {slot:?}", c.id);
            assert_eq!(fx[0].2, 2, "{} {slot:?}", c.id);
        }
    }
}

/// Fidèle au web : la clé `grondak_R` ne trouve pas la recette dédiée `grondak_r`, l'ult joue la recette
/// procédurale. Si on corrige un jour la recherche, ce test doit changer avec elle.
#[test]
fn comme_le_web_l_ult_joue_la_recette_procedurale_et_non_la_dediee() {
    let fx = effects(&run(&cast("grondak", AbilitySlot::R)));
    assert_eq!(fx[0].2, 2);
    let (mut am, rx) = AudioManager::for_tests();
    let dedicated = recipes::custom(|k| k == "grondak_r");
    assert!(dedicated.is_some());
    if let Some(r) = dedicated {
        r.play(&mut am, None, Some(1.0));
    }
    assert_eq!(effects(&rx.try_iter().collect::<Vec<_>>())[0].2, 8);
}

#[test]
fn chaque_recette_dediee_et_chaque_sort_d_invocateur_programme_des_voix() {
    for &(key, f) in recipes::CUSTOM {
        let (mut am, rx) = AudioManager::for_tests();
        f(&mut am, None, 1.0);
        let fx = effects(&rx.try_iter().collect::<Vec<_>>());
        assert!(fx.len() == 1 && fx[0].2 >= 3, "{key}");
        assert!(recipes::is_custom_fx_key(key));
        assert!(
            CHAMPIONS.iter().any(|c| c.abilities.r.fx_key == key),
            "{key} existe"
        );
    }
    for spell in [
        SummonerId::Flash,
        SummonerId::Ignite,
        SummonerId::Heal,
        SummonerId::Barrier,
        SummonerId::Exhaust,
        SummonerId::Teleport,
        SummonerId::Smite,
        SummonerId::Ghost,
        SummonerId::Cleanse,
    ] {
        let e = GameEvent::SummonerCast {
            source: ME,
            spell,
            from: HERE,
            to: None,
            target_id: None,
            fx_key: None,
        };
        let fx = effects(&run(&e));
        assert!(fx.len() == 1 && fx[0].2 >= 1, "{spell:?}");
    }
}

/// Portage de « événements death -> annonce sonore : ignore les sbires, joue un jingle pour un kill
/// entre champions ».
#[test]
fn ignore_les_sbires_joue_un_jingle_pour_un_kill_entre_champions() {
    let (mut am, rx) = AudioManager::for_tests();
    let mut state = fake_state();
    state.buffer.entities.insert(
        51,
        lab::entity(
            51,
            lov_shared::types::EntityKind::Minion,
            lov_shared::types::TeamOrNeutral::Red,
        ),
    );
    let mut kills = KillAnnouncer::default();
    kills.reset();
    play_event(&mut am, &mut kills, &state, &death(MINION, Some(ME)));
    play_event(&mut am, &mut kills, &state, &death(51, Some(ME)));
    let cmds: Vec<Command> = rx.try_iter().collect();
    assert_eq!(bus_commands(&cmds), 0);
    play_event(&mut am, &mut kills, &state, &death(FOE, Some(ME)));
    let cmds: Vec<Command> = rx.try_iter().collect();
    assert!(bus_commands(&cmds) > 0);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::AddBus(Channel::Voice, _))),
        "bus sur le canal Voix"
    );
    assert!(
        cmds.iter().any(
            |c| matches!(c, Command::Bus(_, BusCommand::SetMaster(v)) if (*v - 1.0).abs() < 1e-6)
        ),
        "gain global 1"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::Bus(_, BusCommand::Play(s)) if !s.is_empty())),
        "jingle programmé"
    );
    // le bus est réutilisé au kill suivant
    play_event(&mut am, &mut kills, &state, &death(FOE2, Some(ME)));
    let cmds: Vec<Command> = rx.try_iter().collect();
    assert!(!cmds.iter().any(|c| matches!(c, Command::AddBus(..))));
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::Bus(_, BusCommand::Play(_))))
    );
}

#[test]
fn le_joueur_un_allie_et_un_ennemi_declenchent_le_bon_cote_de_jingle() {
    let state = fake_state();
    let mut k = KillAnnouncer::default();
    assert_eq!(
        k.on_death(&state, FOE, Some(ME), 0.0),
        Some(Sting::FirstBlood(StingSide::Own))
    );
    assert_eq!(
        k.on_death(&state, FOE2, Some(ALLY), 1.0),
        Some(Sting::Kill(0, StingSide::Ally))
    );
    assert_eq!(
        k.on_death(&state, ALLY, Some(FOE), 2.0),
        Some(Sting::Kill(0, StingSide::Enemy))
    );
    // double kill du joueur : niveau 2
    assert_eq!(
        k.on_death(&state, FOE2, Some(ME), 3.0),
        Some(Sting::Kill(2, StingSide::Own))
    );

    // premier sang ennemi
    let mut k = KillAnnouncer::default();
    assert_eq!(
        k.on_death(&state, ME, Some(FOE), 0.0),
        Some(Sting::FirstBlood(StingSide::Enemy))
    );
    // l'entité du joueur vient de `self` quand il est connu
    let mut k = KillAnnouncer::default();
    let mut s = fake_state();
    if let Some(g) = s.game.as_mut() {
        g.entity_id = 999;
    }
    assert_eq!(
        k.on_death(&s, FOE, Some(ME), 0.0),
        Some(Sting::FirstBlood(StingSide::Ally))
    );
}

#[test]
fn sans_tueur_champion_ennemi_pas_de_jingle_et_la_serie_s_arrete() {
    let state = fake_state();
    let mut k = KillAnnouncer::default();
    assert_eq!(k.on_death(&state, MINION, Some(ME), 0.0), None, "sbire");
    assert_eq!(
        k.on_death(&state, FOE, Some(ME), 1.0),
        Some(Sting::FirstBlood(StingSide::Own))
    );
    assert_eq!(
        k.on_death(&state, FOE2, Some(ME), 30.0),
        Some(Sting::Kill(0, StingSide::Own))
    );
    // exécuté par une tour : pas d'annonce, la série du joueur repart de zéro
    assert_eq!(k.on_death(&state, ME, Some(70), 31.0), None);
    assert_eq!(k.on_death(&state, ME, None, 32.0), None);
    assert_eq!(
        k.on_death(&state, ALLY, Some(ME), 33.0),
        None,
        "même équipe"
    );
    assert_eq!(
        k.on_death(&state, FOE, Some(ME), 60.0),
        Some(Sting::Kill(0, StingSide::Own))
    );
}

#[test]
fn les_annonces_sont_remises_a_zero_a_chaque_nouvelle_partie() {
    let mut state = fake_state();
    let mut k = KillAnnouncer::default();
    assert!(k.track_game(&state), "première partie");
    assert!(!k.track_game(&state), "même partie");
    assert_eq!(
        k.on_death(&state, FOE, Some(ME), 0.0),
        Some(Sting::FirstBlood(StingSide::Own))
    );
    assert_eq!(
        k.on_death(&state, FOE2, Some(ME), 1.0),
        Some(Sting::Kill(2, StingSide::Own))
    );
    assert!(!k.track_game(&state));

    // nouveau GameStart : premier sang et séries repartent de zéro
    if let Some(g) = state.game.as_mut() {
        g.server_time = 2.0;
    }
    assert!(k.track_game(&state), "nouvelle partie");
    assert_eq!(
        k.on_death(&state, FOE, Some(ME), 2.0),
        Some(Sting::FirstBlood(StingSide::Own))
    );

    // retour au menu (ni partie ni résultat), puis la même partie revient : remise à zéro
    let game = state.game.take();
    assert!(!k.track_game(&state));
    state.game = game;
    assert!(k.track_game(&state));
    assert_eq!(
        k.on_death(&state, FOE2, Some(ME), 3.0),
        Some(Sting::FirstBlood(StingSide::Own))
    );
}

#[test]
fn le_systeme_lit_les_server_event_et_remet_a_zero_la_nouvelle_partie() {
    let (am, rx) = AudioManager::for_tests();
    let mut app = App::new();
    app.insert_resource(am)
        .insert_resource(fake_state())
        .add_plugins(plugin);
    // une série d'une partie précédente, à effacer au lancement
    let mut stale = KillAnnouncer::default();
    let _ = stale.on_death(&fake_state(), FOE, Some(ME), 0.0);
    app.insert_resource(stale);
    app.world_mut()
        .write_message(ServerEvent(death(FOE2, Some(ALLY))));
    app.update();
    let cmds: Vec<Command> = rx.try_iter().collect();
    assert_eq!(effects(&cmds).len(), 1, "son de mort");
    assert!(bus_commands(&cmds) > 0, "jingle");
    // sans remise à zéro, ce serait le double kill du joueur (niveau 2)
    let mut kills = app.world_mut().resource_mut::<KillAnnouncer>();
    assert_eq!(
        kills.on_death(&fake_state(), FOE, Some(ME), 5.0),
        Some(Sting::Kill(0, StingSide::Own))
    );
}

#[test]
fn chaque_entree_de_la_demo_fait_entendre_quelque_chose() {
    let (mut am, rx) = AudioManager::for_tests();
    let mut lab = EventLab::default();
    for (i, &(name, period, play)) in LAB.iter().enumerate() {
        // une entrée tous les 3 s : la polyphonie est libre à chaque fois
        am.now = i as f64 * 3.0;
        play(&mut am, &mut lab);
        let cmds: Vec<Command> = rx.try_iter().collect();
        assert!(period >= 1.0, "{name}");
        if name.starts_with("recettes") {
            continue; // titre de section
        }
        assert!(!cmds.is_empty(), "{name}");
        if name.contains("sbire") {
            assert_eq!(bus_commands(&cmds), 0, "{name}");
        }
        if name.contains("kill") || name.contains("sang") {
            assert!(bus_commands(&cmds) > 0, "{name}");
        }
    }
}
