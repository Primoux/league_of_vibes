//! Atelier de l'interface : `LOV_UILAB=<écran>` affiche un écran avec des données fictives, sans serveur
//! (`?uilab` du client web). Écrans : `accueil`, `salon`, `draft`, `chargement`, `fin` ;
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 2 s, pour vérifier le rendu sans regarder la
//! fenêtre.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lov_shared::protocol::{
    DraftPick, DraftState, GameOver, LobbyPlayer, LobbyState, PlayerStats, RoomPhase,
};
use lov_shared::types::{Role, SummonerId, Team};

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

fn fill(mut state: ResMut<GameState>) {
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
