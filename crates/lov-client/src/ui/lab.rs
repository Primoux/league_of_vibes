//! Atelier de l'interface : `LOV_UILAB=<écran>` affiche un écran avec des données fictives, sans serveur
//! (`?uilab` du client web). Écrans : `accueil`, `salon`, `draft`, `chargement`, `jeu` (HUD),
//! `boutique` (HUD, boutique ouverte), `fin` ;
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 2 s, pour vérifier le rendu sans regarder la
//! fenêtre.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::collections::BTreeMap;

use lov_shared::protocol::{
    DraftPick, DraftState, GameOver, GameStart, GameStartPlayer, LobbyPlayer, LobbyState,
    PlayerStats, RoomPhase, Score, TeamScore,
};
use lov_shared::types::{
    AbilityMap, BuffSnapshot, ChampSnapshot, Cooldowns, EntityKind, EntitySnapshot, Role,
    SelfState, Stats, SummonerId, SummonerMap, Team, TeamOrNeutral, WardCharges,
};

use crate::input::PlayerInput;
use crate::net::GameState;

/// Écran demandé par `LOV_UILAB`, s'il est défini.
pub fn requested() -> Option<String> {
    std::env::var("LOV_UILAB").ok()
}

fn player(id: &str, name: &str, team: Team, ready: bool, role: Option<Role>) -> LobbyPlayer {
    LobbyPlayer {
        id: id.into(),
        name: name.into(),
        team,
        ready,
        is_host: id == "p1",
        connected: id != "p6",
        role,
    }
}

/// Salon fictif : 7 joueurs, l'hôte est le joueur local.
pub fn demo_lobby() -> LobbyState {
    LobbyState {
        code: "VIBE5".into(),
        phase: RoomPhase::Lobby,
        players: vec![
            player("p1", "Aymeric", Team::Blue, true, Some(Role::Mid)),
            player("p2", "Lyra", Team::Blue, true, Some(Role::Jungle)),
            player("p3", "Bastien", Team::Blue, false, None),
            player("p4", "Nora", Team::Red, true, Some(Role::Top)),
            player("p5", "Yanis", Team::Red, false, Some(Role::Bot)),
            player("p6", "Kenji", Team::Red, false, None),
            player("p7", "Mila", Team::Red, true, Some(Role::Mid)),
        ],
        dev_mode: true,
    }
}

/// Champions verrouillés dans le draft fictif, par joueur.
const PICKS: [(&str, Option<&str>); 7] = [
    ("p1", Some("ilyra")),
    ("p4", Some("grondak")),
    ("p5", Some("vrenna")),
    ("p2", None),
    ("p3", None),
    ("p6", None),
    ("p7", None),
];

/// Draft fictif : tour 4 sur 7, le joueur local a verrouillé Ilyra, un allié survole Kaelen.
pub fn demo_draft(server_now: f64) -> DraftState {
    let lobby = demo_lobby();
    let picks = PICKS
        .iter()
        .map(|(id, champ)| DraftPick {
            player_id: (*id).into(),
            team: lobby
                .players
                .iter()
                .find(|p| p.id == *id)
                .map_or(Team::Blue, |p| p.team),
            champion_id: champ.map(Into::into),
            hover_champion_id: (*id == "p2").then(|| "kaelen".into()),
            locked: champ.is_some(),
            summoners: [
                SummonerId::Flash,
                if *id == "p1" {
                    SummonerId::Heal
                } else {
                    SummonerId::Ignite
                },
            ],
        })
        .collect();
    DraftState {
        turns: [
            ["p1"].as_slice(),
            &["p4", "p5"],
            &["p2", "p3"],
            &["p6", "p7"],
        ]
        .iter()
        .map(|t| t.iter().map(|s| (*s).to_string()).collect())
        .collect(),
        current_turn: 2,
        turn_ends_at: server_now + 22_000.0,
        picks,
        finalize_ends_at: None,
    }
}

/// Fin de partie fictive : victoire bleue en 27:42.
pub fn demo_result() -> GameOver {
    let row = |id: &str, champ: &str, team: Team, k: u32, d: u32, a: u32, cs: u32, dmg: f64| {
        PlayerStats {
            player_id: id.into(),
            champion_id: champ.into(),
            team,
            kills: k,
            deaths: d,
            assists: a,
            cs,
            gold: f64::from(cs) * 21.0 + f64::from(k) * 300.0,
            damage_to_champions: dmg,
            items: vec![Some("long_sword".into()), Some("boots".into()), None],
        }
    };
    GameOver {
        winner: Team::Blue,
        duration: 1662.0,
        stats: vec![
            row("p1", "ilyra", Team::Blue, 9, 2, 11, 182, 21450.0),
            row("p2", "kaelen", Team::Blue, 6, 4, 8, 121, 14200.0),
            row("p3", "solenne", Team::Blue, 1, 3, 17, 34, 6100.0),
            row("p4", "grondak", Team::Red, 3, 6, 5, 140, 9800.0),
            row("p5", "vrenna", Team::Red, 4, 5, 3, 166, 12900.0),
            row("p7", "zephyr", Team::Red, 2, 5, 4, 150, 11000.0),
        ],
    }
}

fn champion(id: u32, champ: &str, team: TeamOrNeutral, x: f64, y: f64, hp: f64) -> EntitySnapshot {
    EntitySnapshot {
        id,
        kind: EntityKind::Champion,
        team,
        x,
        y,
        facing: 0.0,
        hp,
        max_hp: 1450.0,
        mana: Some(420.0),
        max_mana: Some(780.0),
        shield: None,
        dead: None,
        subtype: Some(champ.into()),
        owner_id: Some(format!("p{id}")),
        level: Some(9),
        anim: None,
        buffs: None,
        champ: Some(ChampSnapshot {
            player_id: format!("p{id}"),
            name: champ.into(),
            kills: 3,
            deaths: 1,
            assists: 5,
            cs: 87,
            items: vec![
                Some("long_sword".into()),
                Some("boots".into()),
                None,
                None,
                None,
                None,
                Some("stealth_ward".into()),
            ],
            respawn_in: None,
        }),
    }
}

/// Partie fictive pour le HUD : Ilyra niveau 9 en milieu de voie, un allié, des ennemis.
pub fn demo_game(state: &mut GameState) {
    let mut me = champion(1, "ilyra", TeamOrNeutral::Blue, 3400.0, 3300.0, 610.0);
    me.shield = Some(180.0);
    me.buffs = Some(vec![
        BuffSnapshot {
            id: "blue_buff".into(),
            remaining: 92.0,
            stacks: None,
        },
        BuffSnapshot {
            id: "ilyra_q_slow".into(),
            remaining: 1.4,
            stacks: None,
        },
    ]);
    let ally = champion(2, "kaelen", TeamOrNeutral::Blue, 3000.0, 3600.0, 900.0);
    let enemy = champion(10, "grondak", TeamOrNeutral::Red, 3900.0, 3500.0, 1100.0);
    for e in [me, ally, enemy] {
        state.buffer.entities.insert(e.id, e);
    }
    state.my_entity = Some(1);
    state.time = 754.0;
    state.game = Some(GameStart {
        you: "p1".into(),
        team: Team::Blue,
        entity_id: 1,
        players: vec![GameStartPlayer {
            id: "p1".into(),
            name: "Aymeric".into(),
            team: Team::Blue,
            champion_id: "ilyra".into(),
            summoners: [SummonerId::Flash, SummonerId::Heal],
        }],
        server_time: 0.0,
    });
    let side = |kills, towers, dragons: &[&str]| TeamScore {
        kills,
        towers,
        dragons: dragons.iter().map(|d| (*d).to_string()).collect(),
        grubs: 2,
    };
    state.score = Some(Score {
        blue: side(14, 3, &["infernal", "ocean"]),
        red: side(9, 1, &["mountain"]),
    });
    state.objective_timers = Some(BTreeMap::from([
        ("dragon".to_string(), 812.0),
        ("herald".to_string(), 1020.0),
    ]));
    state.self_state = Some(SelfState {
        entity_id: 1,
        gold: 1375.0,
        xp: 420.0,
        xp_to_next: 900.0,
        level: 9,
        skill_points: 1,
        ability_levels: AbilityMap {
            q: 5,
            w: 2,
            e: 1,
            r: 1,
        },
        cooldowns: Cooldowns {
            q: 2.4,
            w: 0.0,
            e: 9.0,
            r: 0.0,
            d: 140.0,
            f: 0.0,
        },
        summoners: SummonerMap {
            d: SummonerId::Flash,
            f: SummonerId::Heal,
        },
        stats: Stats {
            max_hp: 1450.0,
            hp_regen: 9.0,
            max_mana: 780.0,
            mana_regen: 11.0,
            ad: 62.0,
            ap: 145.0,
            armor: 48.0,
            mr: 41.0,
            attack_speed: 0.72,
            attack_range: 550.0,
            move_speed: 380.0,
            crit_chance: 0.0,
            ability_haste: 20.0,
            lifesteal: 0.0,
            armor_pen: 0.0,
            magic_pen: 10.0,
        },
        recalling: None,
        can_shop: true,
        ward_charges: Some(WardCharges {
            charges: 2,
            cooldown_remaining: 0.0,
        }),
        item_charges: None,
        item_cooldowns: None,
    });
}

fn fill(mut state: ResMut<GameState>, mut input: ResMut<PlayerInput>) {
    let screen = requested().unwrap_or_default();
    state.player_id = Some("p1".into());
    state.connected = true;
    let now = state.clock.server_now(crate::net::now_ms());
    let mut lobby = demo_lobby();
    match screen.as_str() {
        "salon" => {}
        "draft" => {
            lobby.phase = RoomPhase::Draft;
            state.draft = Some(demo_draft(now));
        }
        "chargement" => {
            lobby.phase = RoomPhase::Loading;
            let mut d = demo_draft(now);
            for (p, c) in d.picks.iter_mut().zip([
                "ilyra", "grondak", "vrenna", "kaelen", "solenne", "nyx", "zephyr",
            ]) {
                p.champion_id = Some(c.into());
                p.locked = true;
            }
            state.draft = Some(d);
            state.loading_ends_at = Some(now + 3500.0);
        }
        "fin" => state.result = Some(demo_result()),
        "jeu" | "boutique" => {
            demo_game(&mut state);
            input.shop_open = screen == "boutique";
            return;
        }
        _ => return,
    }
    state.lobby = Some(lobby);
}

fn shoot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>) {
    if *done || time.elapsed_secs() < 2.0 {
        return;
    }
    *done = true;
    if let Ok(path) = std::env::var("LOV_SCREENSHOT") {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

pub struct UiLabPlugin;

impl Plugin for UiLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, fill).add_systems(Update, shoot);
    }
}
