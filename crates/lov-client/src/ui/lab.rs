//! Atelier de l'interface : `LOV_UILAB=<écran>` affiche un écran avec des données fictives, sans serveur
//! (`?uilab` du client web). Écrans : `accueil`, `salon` ;
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 2 s, pour vérifier le rendu sans regarder la
//! fenêtre.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use lov_shared::protocol::{LobbyPlayer, LobbyState, RoomPhase};
use lov_shared::types::{Role, Team};

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

fn fill(mut state: ResMut<GameState>) {
    state.player_id = Some("p1".into());
    state.connected = true;
    if requested().as_deref() == Some("salon") {
        state.lobby = Some(demo_lobby());
    }
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
