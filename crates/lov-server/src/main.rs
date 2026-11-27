//! Point d'entrée serveur (port de `packages/server/src/index.ts`) : HTTP (sert le build client) +
//! WebSocket sur `/ws`. Logique dans `server.rs`.
//!
//! Variables d'environnement : `PORT` (3000), `DEV_LOBBY=1`, `SKIP_DRAFT=1`, et `CLIENT_DIST` (dossier de
//! fichiers statiques à servir en HTTP, facultatif : le jeu est un client natif).

use lov_server::room::GameRooms;
use lov_server::server::{GameServerOptions, create_game_server};
use lov_shared::protocol::WS_PATH;
use std::{path::PathBuf, process::ExitCode, sync::Arc};

#[tokio::main]
async fn main() -> ExitCode {
    let port = match std::env::var("PORT") {
        Err(_) => 3000,
        Ok(p) => match p.trim().parse::<u16>() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("[lov] PORT invalide : {p:?}");
                return ExitCode::FAILURE;
            }
        },
    };
    let dev_lobby = std::env::var("DEV_LOBBY").is_ok_and(|v| v == "1");
    let skip_draft = std::env::var("SKIP_DRAFT").is_ok_and(|v| v == "1");
    // Fichiers statiques servis en HTTP, s'il y en a (le jeu est un client natif : par défaut, aucun).
    let client_dist = std::env::var_os("CLIENT_DIST").map(PathBuf::from);

    let opts = GameServerOptions {
        port,
        dev_lobby,
        skip_draft,
        client_dist,
        ..GameServerOptions::default()
    };
    let server = match create_game_server(opts, Arc::new(GameRooms::default())).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[lov] impossible d'écouter sur le port {port} : {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "[lov] serveur sur http://localhost:{} (ws {WS_PATH}){}{}",
        server.port(),
        if dev_lobby { " [DEV_LOBBY]" } else { "" },
        if skip_draft { " [SKIP_DRAFT]" } else { "" },
    );

    shutdown_signal().await;
    server.close().await;
    ExitCode::SUCCESS
}

/// SIGINT ou SIGTERM.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
