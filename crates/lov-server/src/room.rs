//! Salle de jeu : lobby -> draft -> chargement -> partie -> fin (-> lobby). Port de
//! `packages/server/src/room.ts`.
//!
//! Draft : ordre B1 – R1 R2 – B2 B3 – R3 R4 – B4 (1-2-2-2-1), `DRAFT_TURN_SECONDS` par tour, picks
//! simultanés dans un tour, survol visible des alliés seulement, champion unique, choix automatique à
//! l'expiration (le survol s'il est libre, sinon au hasard). Puis `FINALIZE_SECONDS` pour les sorts
//! d'invocateur, puis `LOADING_SECONDS` de chargement.
//!
//! L'horloge est injectable ([`Clock`]) et les boucles désactivables (`manual`) pour les tests : appeler
//! alors [`RoomCore::update`] (timers du draft et du chargement) et [`RoomCore::tick_game`] (un tick).
//!
//! Différences avec le TypeScript :
//! - la logique vit dans [`RoomCore`], partagé (`Arc<Mutex>`) entre [`GameRoom`] (vu par le serveur
//!   réseau) et les deux tâches tokio qui remplacent `setInterval` (timers) et `setTimeout` (boucle de
//!   simulation) ;
//! - `random` devient un générateur semé ([`Rng`]) ; la graine du monde vient de l'horloge ;
//! - `data.goldEarned`, lu par l'écran de fin en TS, n'était posé par aucun système : les stats de fin
//!   donnent l'or du champion, comme le repli du TS.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lov_shared::constants::{
    DRAFT_TURN_SECONDS, LOADING_SECONDS, SNAPSHOT_RATE, TEAM_SIZE, TICK_DT, TICK_RATE,
};
use lov_shared::data::champions::{CHAMPIONS, get_champion};
use lov_shared::map::FOUNTAINS;
use lov_shared::protocol::{
    ClientMsg, DraftPick, DraftState, GameOver, GameStart, GameStartPlayer, LobbyPlayer,
    LobbyState, PlayerStats, RoomPhase, ServerMsg, encode,
};
use lov_shared::types::{PlayerId, Role, SummonerId, Team, Vec2};
use lov_sim::systems::registry::create_systems;
use lov_sim::{CreateChampion, QueuedCommand, Rng, System, World, create_champion};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::perf::LoopPerf;
use crate::snapshot::{ClientView, ScoreTracker, SnapshotFrame, js_round};

/// Durée de la finalisation (sorts d'invocateur) après le dernier pick (s).
pub const FINALIZE_SECONDS: f64 = 15.0;
/// Délai avant suppression d'une salle dont tous les joueurs sont déconnectés en cours de partie.
pub const EMPTY_ROOM_GRACE: Duration = Duration::from_secs(3 * 60);
/// Période des timers hors simulation (draft, chargement).
const TIMER_PERIOD: Duration = Duration::from_millis(250);

// ------------------------------------------------------------------------------------------------
// Interface avec le serveur réseau
// ------------------------------------------------------------------------------------------------

/// Un lien d'envoi vers un client : `send` met un message JSON déjà encodé en file vers la tâche qui
/// possède la socket. Cloner une `Conn` donne un autre handle sur la même connexion.
#[derive(Debug, Clone)]
pub struct Conn {
    id: u64,
    tx: UnboundedSender<String>,
}

impl Conn {
    /// `id` identifie la connexion (comparaison `p.conn === conn` du TS).
    pub fn new(id: u64, tx: UnboundedSender<String>) -> Self {
        Self { id, tx }
    }

    /// Envoie un message JSON. Sans effet si la connexion est déjà fermée.
    pub fn send(&self, raw: String) {
        // Une erreur signifie que la socket est fermée : le TS ignore ce cas aussi (`readyState`).
        let _ = self.tx.send(raw);
    }

    pub fn same_as(&self, other: &Conn) -> bool {
        self.id == other.id
    }
}

/// Ce que le serveur réseau attend d'une salle.
pub trait Room: Send {
    /// Fait entrer (ou rentrer, en reconnexion) un joueur. `Err(message)` est renvoyé tel quel au client.
    fn join(&mut self, player_id: &str, name: &str, conn: Conn) -> Result<(), String>;
    fn leave(&mut self, player_id: &str);
    /// Tout message de jeu ou de lobby que le serveur ne traite pas lui-même.
    fn handle(&mut self, player_id: &str, msg: ClientMsg);
    /// Plus aucun joueur connecté.
    fn is_empty(&self) -> bool;
    /// La salle est encore au lobby.
    fn is_lobby(&self) -> bool;
    /// Depuis quand la salle est vide.
    fn empty_since(&self) -> Option<Instant>;
    /// Nombre de joueurs connectés (pour `/health`).
    fn connected_players(&self) -> usize;
    /// Vrai si `conn` est la connexion actuelle de ce joueur.
    fn is_connection_of(&self, player_id: &str, conn: &Conn) -> bool;
    /// Arrête les boucles de la salle (suppression ou arrêt du serveur).
    fn stop(&mut self);
}

/// Fabrique de salles (`new Room(code, devMode, { skipDraft })`).
pub trait RoomFactory: Send + Sync {
    fn create(&self, code: &str, dev_mode: bool, skip_draft: bool)
    -> Result<Box<dyn Room>, String>;
}

// ------------------------------------------------------------------------------------------------
// Options
// ------------------------------------------------------------------------------------------------

/// Horloge de la salle, en millisecondes.
pub trait Clock: Send + Sync {
    fn now(&self) -> f64;
}

/// Heure système (ms depuis l'époque Unix), comme `Date.now()`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}

/// Durées (s) du draft, de la finalisation et du chargement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DraftTimings {
    pub turn: f64,
    pub finalize: f64,
    pub loading: f64,
}

pub const DEFAULT_TIMINGS: DraftTimings = DraftTimings {
    turn: DRAFT_TURN_SECONDS,
    finalize: FINALIZE_SECONDS,
    loading: LOADING_SECONDS,
};

/// Durées raccourcies du mode dev.
pub const DEV_TIMINGS: DraftTimings = DraftTimings {
    turn: 10.0,
    finalize: 3.0,
    loading: 2.0,
};

/// Fabrique des systèmes de simulation d'une partie.
pub type SystemsFactory = Arc<dyn Fn() -> Vec<System> + Send + Sync>;

#[derive(Clone)]
pub struct RoomOptions {
    pub clock: Arc<dyn Clock>,
    /// Pas de tâches de fond : l'appelant pilote `update()` et `tick_game()` (tests).
    pub manual: bool,
    /// Systèmes de simulation (défaut : `create_systems`).
    pub systems: Option<SystemsFactory>,
    /// Graine des choix automatiques du draft et du monde (défaut : l'horloge).
    pub seed: Option<u64>,
    /// Durées (défaut : officielles, raccourcies en mode dev).
    pub timings: Option<DraftTimings>,
    /// Lancer directement la partie (picks automatiques) sans draft.
    pub skip_draft: bool,
    /// Mesures de la boucle (`LOV_PERF=1`).
    pub perf: bool,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            clock: Arc::new(SystemClock),
            manual: false,
            systems: None,
            seed: None,
            timings: None,
            skip_draft: false,
            perf: std::env::var("LOV_PERF").is_ok_and(|v| v == "1"),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// État de la salle
// ------------------------------------------------------------------------------------------------

pub struct RoomPlayer {
    pub lobby: LobbyPlayer,
    pub conn: Option<Conn>,
    pub view: ClientView,
}

/// Draft en cours.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub turns: Vec<Vec<PlayerId>>,
    pub current_turn: usize,
    pub turn_ends_at: f64,
    pub finalize_ends_at: Option<f64>,
    /// Picks dans l'ordre d'arrivée des joueurs.
    pub picks: Vec<DraftPick>,
}

impl Draft {
    pub fn pick(&self, id: &str) -> Option<&DraftPick> {
        self.picks.iter().find(|p| p.player_id == id)
    }

    fn pick_mut(&mut self, id: &str) -> Option<&mut DraftPick> {
        self.picks.iter_mut().find(|p| p.player_id == id)
    }

    fn is_taken(&self, champion_id: &str, except: Option<&str>) -> bool {
        self.picks.iter().any(|p| {
            Some(p.player_id.as_str()) != except
                && p.locked
                && p.champion_id.as_deref() == Some(champion_id)
        })
    }
}

/// Partie en cours.
pub struct Game {
    pub world: World,
    pub systems: Vec<System>,
    score: ScoreTracker,
    start: GameStart,
    ticks_since_snapshot: u32,
    /// Instant (horloge de la salle, ms) du tick 0 de la boucle ; `None` en mode manuel.
    loop_epoch: Option<f64>,
}

/// Logique d'une salle, sans E/S autre que l'envoi aux connexions.
pub struct RoomCore {
    pub code: String,
    pub dev_mode: bool,
    pub phase: RoomPhase,
    /// Joueurs dans l'ordre d'arrivée.
    pub players: Vec<RoomPlayer>,
    pub draft: Option<Draft>,
    pub game: Option<Game>,
    pub loading_ends_at: f64,
    pub result: Option<GameOver>,
    /// Instant où la salle est devenue vide.
    pub empty_since: Option<Instant>,
    opts: RoomOptions,
    timings: DraftTimings,
    rng: Rng,
    ticks_per_snapshot: u32,
    perf: Option<LoopPerf>,
    /// Lancement des tâches de fond (absent en mode manuel).
    tasks: Option<Tasks>,
}

/// Snapshot tous les N ticks : `TICK_RATE / N` doit tomber juste pour des envois réguliers.
fn ticks_per_snapshot() -> u32 {
    (f64::from(TICK_RATE) / f64::from(SNAPSHOT_RATE))
        .round()
        .max(1.0) as u32
}

fn send_to(conn: Option<&Conn>, msg: &ServerMsg) {
    if let (Some(conn), Ok(raw)) = (conn, encode(msg)) {
        conn.send(raw);
    }
}

fn default_summoners(role: Option<Role>) -> [SummonerId; 2] {
    if role == Some(Role::Jungle) {
        [SummonerId::Flash, SummonerId::Smite]
    } else {
        [SummonerId::Flash, SummonerId::Ignite]
    }
}

impl RoomCore {
    pub fn new(code: &str, dev_mode: bool, opts: RoomOptions) -> Self {
        let base = if dev_mode {
            DEV_TIMINGS
        } else {
            DEFAULT_TIMINGS
        };
        let timings = opts.timings.unwrap_or(base);
        let seed = opts.seed.unwrap_or_else(|| opts.clock.now() as u64);
        Self {
            code: code.to_owned(),
            dev_mode,
            phase: RoomPhase::Lobby,
            players: Vec::new(),
            draft: None,
            game: None,
            loading_ends_at: 0.0,
            result: None,
            empty_since: None,
            timings,
            rng: Rng::new(seed),
            ticks_per_snapshot: ticks_per_snapshot(),
            perf: None,
            tasks: None,
            opts,
        }
    }

    fn now(&self) -> f64 {
        self.opts.clock.now()
    }

    pub fn player(&self, id: &str) -> Option<&RoomPlayer> {
        self.players.iter().find(|p| p.lobby.id == id)
    }

    fn player_mut(&mut self, id: &str) -> Option<&mut RoomPlayer> {
        self.players.iter_mut().find(|p| p.lobby.id == id)
    }

    fn count(&self, team: Team) -> usize {
        self.players.iter().filter(|p| p.lobby.team == team).count()
    }

    pub fn is_empty(&self) -> bool {
        self.players.iter().all(|p| !p.lobby.connected)
    }

    // ------------------------------------------------------------------ connexion

    /// Rejoint (ou reprend sa place après une déconnexion).
    pub fn join(&mut self, id: &str, name: &str, conn: Conn) -> Result<(), String> {
        if let Some(p) = self.player_mut(id) {
            p.conn = Some(conn);
            p.lobby.connected = true;
            self.empty_since = None;
            self.resync(id);
            return Ok(());
        }
        if self.phase != RoomPhase::Lobby {
            return Err("La partie a déjà commencé".into());
        }
        if self.players.len() >= (TEAM_SIZE * 2) as usize {
            return Err("Salle pleine".into());
        }
        let blue = self.count(Team::Blue);
        let team = if blue < TEAM_SIZE as usize && blue <= self.players.len() - blue {
            Team::Blue
        } else {
            Team::Red
        };
        let is_host = self.players.is_empty();
        self.players.push(RoomPlayer {
            lobby: LobbyPlayer {
                id: id.to_owned(),
                name: name.to_owned(),
                team,
                ready: false,
                is_host,
                connected: true,
                role: None,
            },
            conn: Some(conn),
            view: ClientView::new(),
        });
        self.empty_since = None;
        self.broadcast_lobby();
        Ok(())
    }

    /// Renvoie l'état courant à un joueur qui revient.
    fn resync(&mut self, id: &str) {
        self.broadcast_lobby();
        let Some(team) = self.player(id).map(|p| p.lobby.team) else {
            return;
        };
        let conn = self.player(id).and_then(|p| p.conn.clone());
        let conn = conn.as_ref();
        match self.phase {
            RoomPhase::Draft => {
                if let Some(state) = self.draft_state_for(team) {
                    send_to(conn, &ServerMsg::Draft { state });
                }
            }
            RoomPhase::Loading => {
                if let Some(state) = self.draft_state_for(team) {
                    send_to(conn, &ServerMsg::Draft { state });
                }
                send_to(
                    conn,
                    &ServerMsg::Loading {
                        ends_at: self.loading_ends_at,
                    },
                );
            }
            RoomPhase::Game if self.game.is_some() => {
                if let Some(p) = self.player_mut(id) {
                    p.view.reset(); // prochain snapshot complet
                }
                self.send_game_start(id);
            }
            RoomPhase::End => {
                if let Some(result) = &self.result {
                    send_to(
                        conn,
                        &ServerMsg::GameOver {
                            result: result.clone(),
                        },
                    );
                }
            }
            _ => {}
        }
    }

    pub fn leave(&mut self, id: &str) {
        let Some(i) = self.players.iter().position(|p| p.lobby.id == id) else {
            return;
        };
        if self.phase == RoomPhase::Lobby {
            let was_host = self.players.remove(i).lobby.is_host;
            if was_host && let Some(next) = self.players.first_mut() {
                next.lobby.is_host = true;
            }
        } else {
            let p = &mut self.players[i];
            p.lobby.connected = false;
            p.conn = None;
        }
        if self.is_empty() && self.empty_since.is_none() {
            self.empty_since = Some(Instant::now());
        }
        self.broadcast_lobby();
    }

    // ------------------------------------------------------------------ messages

    pub fn handle(&mut self, id: &str, msg: ClientMsg) {
        let Some(p) = self.player(id) else { return };
        let (team, is_host, conn) = (p.lobby.team, p.lobby.is_host, p.conn.clone());
        let lobby = self.phase == RoomPhase::Lobby;
        match msg {
            ClientMsg::SetTeam { team: wanted } => {
                if !lobby {
                    return;
                }
                let count = self.count(wanted);
                if let Some(p) = self.player_mut(id)
                    && team != wanted
                    && count < TEAM_SIZE as usize
                {
                    p.lobby.team = wanted;
                    p.lobby.ready = false;
                }
                self.broadcast_lobby();
            }
            ClientMsg::SetRole { role } => {
                if !lobby {
                    return;
                }
                if let Some(p) = self.player_mut(id) {
                    p.lobby.role = Some(role);
                }
                self.broadcast_lobby();
            }
            ClientMsg::SetReady { ready } => {
                if !lobby {
                    return;
                }
                if let Some(p) = self.player_mut(id) {
                    p.lobby.ready = ready;
                }
                self.broadcast_lobby();
            }
            ClientMsg::StartGame => {
                if !lobby {
                    return;
                }
                if is_host && self.can_start() {
                    if self.opts.skip_draft {
                        self.start_game();
                    } else {
                        self.start_draft();
                    }
                } else {
                    send_to(
                        conn.as_ref(),
                        &ServerMsg::Error {
                            message: "Impossible de lancer : 8 joueurs prêts requis (ou mode dev)."
                                .into(),
                        },
                    );
                }
            }
            ClientMsg::DraftHover { champion_id } => self.draft_hover(id, &champion_id),
            ClientMsg::DraftLock { champion_id } => {
                self.draft_lock(id, &champion_id, conn.as_ref())
            }
            ClientMsg::DraftSummoners { summoners } => self.draft_summoners(id, summoners),
            ClientMsg::ReturnToLobby => {
                if self.phase == RoomPhase::End {
                    self.back_to_lobby();
                }
            }
            other => {
                if self.phase == RoomPhase::Game
                    && let Some(game) = &mut self.game
                {
                    game.world.commands.push(QueuedCommand {
                        player_id: id.to_owned(),
                        msg: other,
                    });
                }
            }
        }
    }

    pub fn can_start(&self) -> bool {
        if self.dev_mode {
            return !self.players.is_empty();
        }
        self.players.len() == (TEAM_SIZE * 2) as usize && self.players.iter().all(|p| p.lobby.ready)
    }

    // ------------------------------------------------------------------ draft

    pub fn start_draft(&mut self) {
        let ids = |team: Team| -> Vec<PlayerId> {
            self.players
                .iter()
                .filter(|p| p.lobby.team == team)
                .map(|p| p.lobby.id.clone())
                .collect()
        };
        let (blue, red) = (ids(Team::Blue), ids(Team::Red));
        let order: [(&[PlayerId], std::ops::Range<usize>); 5] = [
            (&blue, 0..1),
            (&red, 0..2),
            (&blue, 1..3),
            (&red, 2..4),
            (&blue, 3..4),
        ];
        let turns: Vec<Vec<PlayerId>> = order
            .into_iter()
            .map(|(team, range)| {
                range
                    .filter_map(|i| team.get(i).cloned())
                    .collect::<Vec<_>>()
            })
            .filter(|t| !t.is_empty())
            .collect();
        let picks = self
            .players
            .iter()
            .map(|p| DraftPick {
                player_id: p.lobby.id.clone(),
                team: p.lobby.team,
                champion_id: None,
                hover_champion_id: None,
                locked: false,
                summoners: default_summoners(p.lobby.role),
            })
            .collect();
        self.draft = Some(Draft {
            turns,
            current_turn: 0,
            turn_ends_at: self.now() + self.timings.turn * 1000.0,
            finalize_ends_at: None,
            picks,
        });
        self.phase = RoomPhase::Draft;
        self.broadcast_lobby();
        self.broadcast_draft();
        self.start_timer();
    }

    fn draft_hover(&mut self, id: &str, champion_id: &str) {
        if self.phase != RoomPhase::Draft || get_champion(champion_id).is_none() {
            return;
        }
        let Some(pick) = self.draft.as_mut().and_then(|d| d.pick_mut(id)) else {
            return;
        };
        if pick.locked {
            return;
        }
        pick.hover_champion_id = Some(champion_id.to_owned());
        self.broadcast_draft();
    }

    fn draft_lock(&mut self, id: &str, champion_id: &str, conn: Option<&Conn>) {
        let now = self.now();
        let timings = self.timings;
        if self.phase != RoomPhase::Draft {
            return;
        }
        let Some(d) = self.draft.as_mut() else { return };
        if d.pick(id).is_none_or(|p| p.locked) || d.finalize_ends_at.is_some() {
            return;
        }
        if !d
            .turns
            .get(d.current_turn)
            .is_some_and(|t| t.iter().any(|p| p == id))
        {
            send_to(
                conn,
                &ServerMsg::Error {
                    message: "Ce n'est pas ton tour.".into(),
                },
            );
            return;
        }
        if get_champion(champion_id).is_none() {
            return;
        }
        if d.is_taken(champion_id, None) {
            send_to(
                conn,
                &ServerMsg::Error {
                    message: "Champion déjà choisi.".into(),
                },
            );
            return;
        }
        if let Some(pick) = d.pick_mut(id) {
            pick.champion_id = Some(champion_id.to_owned());
            pick.hover_champion_id = Some(champion_id.to_owned());
            pick.locked = true;
        }
        advance_if_turn_done(d, now, timings);
        self.broadcast_draft();
    }

    fn draft_summoners(&mut self, id: &str, summoners: [SummonerId; 2]) {
        if self.phase != RoomPhase::Draft || summoners[0] == summoners[1] {
            return;
        }
        let Some(pick) = self.draft.as_mut().and_then(|d| d.pick_mut(id)) else {
            return;
        };
        pick.summoners = summoners;
        self.broadcast_draft();
    }

    /// Expiration du tour : lock du survol s'il est libre, sinon un champion libre au hasard.
    fn auto_pick(&mut self) {
        let now = self.now();
        let timings = self.timings;
        let Some(d) = self.draft.as_mut() else { return };
        let turn = d.turns.get(d.current_turn).cloned().unwrap_or_default();
        for id in turn {
            let Some(pick) = d.pick(&id) else { continue };
            if pick.locked {
                continue;
            }
            let hover = pick
                .hover_champion_id
                .clone()
                .filter(|h| !d.is_taken(h, None));
            let champ = hover.or_else(|| {
                let free: Vec<&String> = CHAMPIONS
                    .iter()
                    .map(|c| &c.id)
                    .filter(|c| !d.is_taken(c, None))
                    .collect();
                let i = self.rng.below(free.len());
                free.get(i).map(|c| (*c).clone())
            });
            let Some(champ) = champ.or_else(|| CHAMPIONS.first().map(|c| c.id.clone())) else {
                continue;
            };
            if let Some(pick) = d.pick_mut(&id) {
                pick.champion_id = Some(champ.clone());
                pick.hover_champion_id = Some(champ);
                pick.locked = true;
            }
        }
        advance_if_turn_done(d, now, timings);
    }

    /// Vue du draft pour une équipe : survols et sorts d'invocateur des ennemis masqués.
    pub fn draft_state_for(&self, team: Team) -> Option<DraftState> {
        let d = self.draft.as_ref()?;
        let drafting = self.phase == RoomPhase::Draft;
        Some(DraftState {
            turns: d.turns.clone(),
            current_turn: d.current_turn as u32,
            turn_ends_at: d.turn_ends_at,
            finalize_ends_at: d.finalize_ends_at,
            picks: d
                .picks
                .iter()
                .map(|p| {
                    if p.team == team || !drafting {
                        p.clone()
                    } else {
                        DraftPick {
                            hover_champion_id: None,
                            summoners: [SummonerId::Flash, SummonerId::Ignite],
                            ..p.clone()
                        }
                    }
                })
                .collect(),
        })
    }

    fn broadcast_draft(&self) {
        let encode_for = |team| {
            self.draft_state_for(team)
                .and_then(|state| encode(&ServerMsg::Draft { state }).ok())
        };
        let (Some(blue), Some(red)) = (encode_for(Team::Blue), encode_for(Team::Red)) else {
            return;
        };
        for p in &self.players {
            if let Some(c) = &p.conn {
                c.send(if p.lobby.team == Team::Blue {
                    blue.clone()
                } else {
                    red.clone()
                });
            }
        }
    }

    // ------------------------------------------------------------------ timers

    /// Avance les timers hors simulation. Appelé toutes les 250 ms (ou à la main en test).
    pub fn update(&mut self) {
        let now = self.now();
        match self.phase {
            RoomPhase::Draft => {
                let Some(d) = &self.draft else { return };
                match d.finalize_ends_at {
                    Some(end) if now >= end => self.start_loading(),
                    Some(_) => {}
                    None if now >= d.turn_ends_at => {
                        self.auto_pick();
                        self.broadcast_draft();
                    }
                    None => {}
                }
            }
            RoomPhase::Loading if now >= self.loading_ends_at => self.start_game(),
            _ => {}
        }
    }

    fn start_timer(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            tasks.start_timer();
        }
    }

    fn start_loading(&mut self) {
        self.phase = RoomPhase::Loading;
        self.loading_ends_at = self.now() + self.timings.loading * 1000.0;
        self.broadcast_lobby();
        self.broadcast_draft();
        self.broadcast(&ServerMsg::Loading {
            ends_at: self.loading_ends_at,
        });
    }

    // ------------------------------------------------------------------ partie

    /// Crée le monde et démarre la simulation à partir des picks (ou d'attributions par défaut).
    pub fn start_game(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            tasks.stop_timer();
        }
        let mut systems = match &self.opts.systems {
            Some(f) => f(),
            None => create_systems(),
        };
        let seed = self.rng.next_u64();
        let mut world = World::new(seed);
        let mut used: Vec<String> = Vec::new();
        let mut slot = [0u32; 2];
        for p in &self.players {
            let pick = self.draft.as_ref().and_then(|d| d.pick(&p.lobby.id));
            let mut champion_id = pick.and_then(|k| k.champion_id.clone());
            if champion_id.as_ref().is_none_or(|c| used.contains(c)) {
                champion_id = CHAMPIONS
                    .iter()
                    .find(|c| !used.contains(&c.id))
                    .map(|c| c.id.clone());
            }
            let Some(champion_id) = champion_id else {
                continue;
            };
            let Some(def) = get_champion(&champion_id) else {
                continue;
            };
            used.push(champion_id.clone());
            let team = p.lobby.team;
            let i = &mut slot[usize::from(team == Team::Red)];
            let n = f64::from(*i);
            *i += 1;
            let (f, sign) = match team {
                Team::Blue => (FOUNTAINS.blue, 1.0),
                Team::Red => (FOUNTAINS.red, -1.0),
            };
            let summoners = pick.map_or([SummonerId::Flash, SummonerId::Ignite], |k| k.summoners);
            create_champion(
                &mut world,
                CreateChampion {
                    player_id: p.lobby.id.clone(),
                    name: p.lobby.name.clone(),
                    team,
                    champion_id,
                    summoners,
                    pos: Vec2 {
                        x: f.x + sign * (150.0 + (n % 2.0) * 150.0),
                        y: f.y + sign * (150.0 + (n / 2.0).floor() * 150.0),
                    },
                },
                &def.base_stats,
            );
        }
        let score = ScoreTracker::new(&mut world);
        world.init(&mut systems);
        let players = self
            .players
            .iter()
            .filter_map(|o| {
                let c = world.champion_of(&o.lobby.id)?.champ.as_ref()?;
                Some(GameStartPlayer {
                    id: o.lobby.id.clone(),
                    name: o.lobby.name.clone(),
                    team: o.lobby.team,
                    champion_id: c.champion_id.clone(),
                    summoners: [c.summoners.d, c.summoners.f],
                })
            })
            .collect();
        let start = GameStart {
            you: String::new(),
            team: Team::Blue,
            entity_id: 0,
            players,
            server_time: self.now(),
        };
        self.game = Some(Game {
            world,
            systems,
            score,
            start,
            ticks_since_snapshot: 0,
            loop_epoch: None,
        });
        self.phase = RoomPhase::Game;
        self.broadcast_lobby();
        let ids: Vec<PlayerId> = self.players.iter().map(|p| p.lobby.id.clone()).collect();
        for id in ids {
            if let Some(p) = self.player_mut(&id) {
                p.view.reset();
            }
            self.send_game_start(&id);
        }
        self.perf = self.opts.perf.then(|| LoopPerf::new(&self.code));
        if let Some(tasks) = &mut self.tasks {
            tasks.start_loop();
        }
    }

    fn send_game_start(&self, id: &str) {
        let Some(game) = &self.game else { return };
        let Some(p) = self.player(id) else { return };
        let Some(entity_id) = game.world.champion_id_of(id) else {
            return;
        };
        send_to(
            p.conn.as_ref(),
            &ServerMsg::GameStart {
                start: GameStart {
                    you: id.to_owned(),
                    team: p.lobby.team,
                    entity_id,
                    server_time: self.now(),
                    ..game.start.clone()
                },
            },
        );
    }

    /// Un tick de simulation ; snapshots tous les `ticks_per_snapshot` ticks (cadence régulière).
    pub fn tick_game(&mut self) {
        if self.phase != RoomPhase::Game {
            return;
        }
        let Some(game) = &mut self.game else { return };
        let t0 = Instant::now();
        game.world.step_dt(&mut game.systems, TICK_DT);
        if let Some(perf) = &mut self.perf {
            perf.tick(t0.elapsed());
        }
        game.ticks_since_snapshot += 1;
        let winner = game.world.winner;
        if game.ticks_since_snapshot >= self.ticks_per_snapshot || winner.is_some() {
            game.ticks_since_snapshot = 0;
            self.send_snapshots();
        }
        if let Some(winner) = winner {
            self.end_game(winner);
        }
    }

    pub fn send_snapshots(&mut self) {
        let now = self.now();
        let Some(game) = &mut self.game else { return };
        let t0 = Instant::now();
        // Horodatage = instant prévu du tick : positions et temps cohérents malgré la gigue des timers.
        let server_time = game.loop_epoch.map_or(now, |epoch| {
            js_round(epoch + game.world.tick as f64 * 1000.0 / f64::from(TICK_RATE))
        });
        let events = game.world.events.drain();
        let score = game.score.score(&game.world);
        let frame = SnapshotFrame::new(&game.world, events, score, server_time);
        let (mut bytes, mut clients) = (0usize, 0usize);
        for p in &mut self.players {
            let Some(conn) = &p.conn else { continue };
            let snap = p.view.build(&frame, &p.lobby.id, p.lobby.team);
            if let Ok(raw) = encode(&ServerMsg::Snapshot {
                snap: Box::new(snap),
            }) {
                bytes += raw.len();
                clients += 1;
                conn.send(raw);
            }
        }
        if let Some(perf) = &mut self.perf {
            perf.snapshot(t0.elapsed(), bytes, clients);
        }
    }

    pub fn end_game(&mut self, winner: Team) {
        let Some(game) = &self.game else { return };
        let world = &game.world;
        let stats = self
            .players
            .iter()
            .map(|p| {
                let c = world
                    .champion_of(&p.lobby.id)
                    .and_then(|e| e.champ.as_ref());
                PlayerStats {
                    player_id: p.lobby.id.clone(),
                    champion_id: c.map(|c| c.champion_id.clone()).unwrap_or_default(),
                    team: p.lobby.team,
                    kills: c.map_or(0, |c| c.kills),
                    deaths: c.map_or(0, |c| c.deaths),
                    assists: c.map_or(0, |c| c.assists),
                    cs: c.map_or(0, |c| c.cs),
                    gold: js_round(c.map_or(0.0, |c| c.gold)),
                    damage_to_champions: js_round(c.map_or(0.0, |c| c.damage_to_champions)),
                    items: c.map(|c| c.items.clone()).unwrap_or_default(),
                }
            })
            .collect();
        let result = GameOver {
            winner,
            duration: js_round(world.time),
            stats,
        };
        self.stop();
        self.phase = RoomPhase::End;
        self.broadcast_lobby();
        self.broadcast(&ServerMsg::GameOver {
            result: result.clone(),
        });
        self.result = Some(result);
    }

    fn back_to_lobby(&mut self) {
        self.stop();
        self.phase = RoomPhase::Lobby;
        self.game = None;
        self.draft = None;
        self.result = None;
        self.players.retain(|p| p.lobby.connected);
        for p in &mut self.players {
            p.lobby.ready = false;
        }
        if !self.players.iter().any(|p| p.lobby.is_host)
            && let Some(first) = self.players.first_mut()
        {
            first.lobby.is_host = true;
        }
        self.broadcast_lobby();
    }

    pub fn stop(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            tasks.stop_all();
        }
        self.perf = None;
        if self.phase == RoomPhase::Game {
            self.phase = RoomPhase::End;
        }
    }

    // ------------------------------------------------------------------ envoi

    pub fn lobby_state(&self) -> LobbyState {
        LobbyState {
            code: self.code.clone(),
            phase: self.phase,
            dev_mode: self.dev_mode,
            players: self.players.iter().map(|p| p.lobby.clone()).collect(),
        }
    }

    pub fn broadcast_lobby(&self) {
        self.broadcast(&ServerMsg::Lobby {
            state: self.lobby_state(),
        });
    }

    pub fn broadcast(&self, msg: &ServerMsg) {
        let Ok(raw) = encode(msg) else { return };
        for p in &self.players {
            if let Some(c) = &p.conn {
                c.send(raw.clone());
            }
        }
    }
}

fn advance_if_turn_done(d: &mut Draft, now: f64, timings: DraftTimings) {
    let done = d
        .turns
        .get(d.current_turn)
        .is_none_or(|turn| turn.iter().all(|id| d.pick(id).is_some_and(|p| p.locked)));
    if !done {
        return;
    }
    d.current_turn += 1;
    if d.current_turn >= d.turns.len() {
        let end = now + timings.finalize * 1000.0;
        d.finalize_ends_at = Some(end);
        d.turn_ends_at = end;
    } else {
        d.turn_ends_at = now + timings.turn * 1000.0;
    }
}

// ------------------------------------------------------------------------------------------------
// Tâches de fond
// ------------------------------------------------------------------------------------------------

fn lock(m: &Mutex<RoomCore>) -> MutexGuard<'_, RoomCore> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Les deux boucles d'une salle : timers (draft, chargement) et simulation.
struct Tasks {
    core: Weak<Mutex<RoomCore>>,
    timer: Option<JoinHandle<()>>,
    game: Option<JoinHandle<()>>,
}

impl Tasks {
    fn start_timer(&mut self) {
        if self.timer.is_some() {
            return;
        }
        let core = self.core.clone();
        self.timer = Some(tokio::spawn(async move {
            let mut every = tokio::time::interval(TIMER_PERIOD);
            loop {
                every.tick().await;
                let Some(core) = core.upgrade() else { return };
                let mut c = lock(&core);
                c.update();
                if !matches!(c.phase, RoomPhase::Draft | RoomPhase::Loading) {
                    return;
                }
            }
        }));
    }

    fn stop_timer(&mut self) {
        // Le timer peut appeler `start_game` lui-même : ne pas l'avorter, il s'arrête au changement
        // de phase.
        self.timer = None;
    }

    /// Boucle à échéances fixes : le tick n est dû à `epoch + n * pas` (pas de dérive). En retard, on
    /// rattrape au plus 4 ticks puis on recale l'horloge (pas de spirale).
    fn start_loop(&mut self) {
        let core = self.core.clone();
        if let Some(old) = self.game.take() {
            old.abort();
        }
        self.game = Some(tokio::spawn(async move {
            let step = 1000.0 / f64::from(TICK_RATE);
            let (clock, mut epoch) = {
                let Some(core) = core.upgrade() else { return };
                let mut c = lock(&core);
                let clock = Arc::clone(&c.opts.clock);
                let epoch = clock.now();
                if let Some(g) = &mut c.game {
                    g.loop_epoch = Some(epoch);
                }
                (clock, epoch)
            };
            let mut n = 0u64;
            loop {
                let due = epoch + (n + 1) as f64 * step;
                let wait = (due - clock.now()).max(0.0);
                tokio::time::sleep(Duration::from_secs_f64(wait / 1000.0)).await;
                let Some(core) = core.upgrade() else { return };
                let mut c = lock(&core);
                let mut caught = 0;
                while clock.now() >= epoch + (n + 1) as f64 * step
                    && caught < 4
                    && c.phase == RoomPhase::Game
                {
                    n += 1;
                    caught += 1;
                    c.tick_game();
                }
                if c.phase != RoomPhase::Game {
                    return;
                }
                if clock.now() >= epoch + (n + 1) as f64 * step {
                    // Trop en retard : recalage.
                    epoch = clock.now() - n as f64 * step;
                    if let Some(g) = &mut c.game {
                        g.loop_epoch = Some(epoch);
                    }
                }
            }
        }));
    }

    fn stop_all(&mut self) {
        if let Some(t) = self.game.take() {
            t.abort();
        }
        if let Some(t) = self.timer.take() {
            t.abort();
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Salle vue par le serveur réseau
// ------------------------------------------------------------------------------------------------

/// Une salle : la logique partagée avec ses tâches de fond.
pub struct GameRoom {
    core: Arc<Mutex<RoomCore>>,
}

impl GameRoom {
    pub fn new(code: &str, dev_mode: bool, opts: RoomOptions) -> Self {
        let manual = opts.manual;
        let core = Arc::new(Mutex::new(RoomCore::new(code, dev_mode, opts)));
        if !manual {
            lock(&core).tasks = Some(Tasks {
                core: Arc::downgrade(&core),
                timer: None,
                game: None,
            });
        }
        Self { core }
    }

    /// Accès direct à la logique (tests, outils).
    pub fn core(&self) -> MutexGuard<'_, RoomCore> {
        lock(&self.core)
    }
}

impl Room for GameRoom {
    fn join(&mut self, player_id: &str, name: &str, conn: Conn) -> Result<(), String> {
        self.core().join(player_id, name, conn)
    }

    fn leave(&mut self, player_id: &str) {
        self.core().leave(player_id);
    }

    fn handle(&mut self, player_id: &str, msg: ClientMsg) {
        self.core().handle(player_id, msg);
    }

    fn is_empty(&self) -> bool {
        self.core().is_empty()
    }

    fn is_lobby(&self) -> bool {
        self.core().phase == RoomPhase::Lobby
    }

    fn empty_since(&self) -> Option<Instant> {
        self.core().empty_since
    }

    fn connected_players(&self) -> usize {
        self.core()
            .players
            .iter()
            .filter(|p| p.lobby.connected)
            .count()
    }

    fn is_connection_of(&self, player_id: &str, conn: &Conn) -> bool {
        self.core()
            .player(player_id)
            .and_then(|p| p.conn.as_ref())
            .is_some_and(|c| c.same_as(conn))
    }

    fn stop(&mut self) {
        self.core().stop();
    }
}

impl Drop for GameRoom {
    fn drop(&mut self) {
        self.core().stop();
    }
}

/// Fabrique des vraies salles (partie complète en Rust).
#[derive(Default, Clone)]
pub struct GameRooms {
    /// Options appliquées à chaque salle (horloge, systèmes…).
    pub options: Option<RoomOptions>,
}

impl RoomFactory for GameRooms {
    fn create(
        &self,
        code: &str,
        dev_mode: bool,
        skip_draft: bool,
    ) -> Result<Box<dyn Room>, String> {
        let opts = RoomOptions {
            skip_draft,
            ..self.options.clone().unwrap_or_default()
        };
        Ok(Box::new(GameRoom::new(code, dev_mode, opts)))
    }
}
