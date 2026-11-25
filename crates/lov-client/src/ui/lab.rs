//! Atelier de l'interface : `LOV_UILAB=<écran>` affiche un écran avec des données fictives, sans serveur
//! (`?uilab` du client web). Écrans : `accueil` ;
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 2 s, pour vérifier le rendu sans regarder la
//! fenêtre.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::net::GameState;

/// Écran demandé par `LOV_UILAB`, s'il est défini.
pub fn requested() -> Option<String> {
    std::env::var("LOV_UILAB").ok()
}

fn fill(mut state: ResMut<GameState>) {
    state.player_id = Some("p1".into());
    state.connected = true;
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
