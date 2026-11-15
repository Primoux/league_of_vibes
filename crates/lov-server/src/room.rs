//! Interface entre le serveur réseau et la couche « salles ».
//!
//! `room.ts` n'est pas porté ici (autre tâche) : ce module ne définit que ce que `net/server.ts` appelle
//! sur une `Room`. Le jour où les salles existent en Rust, elles implémentent [`Room`] et [`RoomFactory`],
//! et `main.rs` remplace [`UnportedRooms`].

use lov_shared::protocol::ClientMsg;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

/// Délai avant suppression d'une salle dont tous les joueurs sont déconnectés en cours de partie
/// (`EMPTY_ROOM_GRACE_MS` de `room.ts`).
pub const EMPTY_ROOM_GRACE: Duration = Duration::from_secs(3 * 60);

/// Un lien d'envoi vers un client (`Conn` de `room.ts`) : `send` met un message JSON déjà encodé en file
/// vers la tâche qui possède la socket. Cloner une `Conn` donne un autre handle sur la même connexion.
#[derive(Debug, Clone)]
pub struct Conn {
    id: u64,
    tx: UnboundedSender<String>,
}

impl Conn {
    pub(crate) fn new(id: u64, tx: UnboundedSender<String>) -> Self {
        Self { id, tx }
    }

    /// Envoie un message JSON. Sans effet si la connexion est déjà fermée.
    pub fn send(&self, raw: String) {
        // Une erreur signifie que la socket est fermée : le TS ignore ce cas aussi (`readyState`).
        let _ = self.tx.send(raw);
    }

    /// Vrai si les deux handles désignent la même connexion (comparaison `p.conn === conn` du TS).
    pub fn same_as(&self, other: &Conn) -> bool {
        self.id == other.id
    }
}

/// Ce que le serveur réseau attend d'une salle.
pub trait Room: Send {
    /// Fait entrer (ou rentrer, en reconnexion) un joueur. `Err(message)` est renvoyé tel quel au client.
    fn join(&mut self, player_id: &str, name: &str, conn: Conn) -> Result<(), String>;
    fn leave(&mut self, player_id: &str);
    /// Tout message de jeu / lobby que le serveur ne traite pas lui-même.
    fn handle(&mut self, player_id: &str, msg: ClientMsg);
    /// Plus aucun joueur connecté (`room.empty`).
    fn is_empty(&self) -> bool;
    /// La salle est encore au lobby (`room.phase === 'lobby'`).
    fn is_lobby(&self) -> bool;
    /// Depuis quand la salle est vide (`room.emptySince`).
    fn empty_since(&self) -> Option<Instant>;
    /// Nombre de joueurs connectés (pour `/health`).
    fn connected_players(&self) -> usize;
    /// Vrai si `conn` est la connexion actuelle de ce joueur (`players.get(id).conn === conn`).
    fn is_connection_of(&self, player_id: &str, conn: &Conn) -> bool;
    /// Arrête les boucles de la salle (suppression ou arrêt du serveur).
    fn stop(&mut self);
}

/// Fabrique de salles (`new Room(code, devMode, { skipDraft })`).
pub trait RoomFactory: Send + Sync {
    fn create(&self, code: &str, dev_mode: bool, skip_draft: bool)
    -> Result<Box<dyn Room>, String>;
}

/// PLACEHOLDER : les salles ne sont pas encore portées en Rust. Toute création de salle est refusée par
/// une erreur claire ; le reste du serveur (handshake, ping, `/health`) fonctionne normalement.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnportedRooms;

impl RoomFactory for UnportedRooms {
    fn create(&self, _: &str, _: bool, _: bool) -> Result<Box<dyn Room>, String> {
        Err(
            "Les salles ne sont pas encore portées en Rust : utilise le serveur Node (npm run dev:server)."
                .into(),
        )
    }
}
