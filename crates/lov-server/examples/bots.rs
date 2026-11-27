//! Bots de charge WebSocket — UNIQUEMENT pour les tests et la CI, jamais en jeu.
//!
//! `cargo run --release -p lov-server --example bots -- [--url ws://localhost:3000/ws] [--bots 8] [--seconds 30]`
//!
//! Portage de `scripts/bots.ts` : le premier bot crée une salle (mode dev), les autres la rejoignent,
//! l'hôte lance, chacun verrouille un champion libre à son tour, puis tous envoient des ordres aléatoires
//! (move / attackMove / attack / cast). Affiche le débit reçu par client, la cadence des snapshots et les
//! erreurs ; échoue si un client reçoit moins de 15 snapshots par seconde.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg};
use lov_shared::types::{AbilitySlot, EntityId, Vec2};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Default)]
struct Bot {
    id: Option<String>,
    bytes: usize,
    snaps: u32,
    errors: Vec<String>,
    in_game: bool,
    entity: Option<EntityId>,
    visible: Vec<EntityId>,
}

/// Générateur pseudo-aléatoire (xorshift) : les bots n'ont pas besoin de mieux.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next() * (hi - lo)
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f64) as usize).min(n.saturating_sub(1))
    }
}

fn arg(name: &str, def: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| *a == format!("--{name}"))
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| def.to_owned())
}

type Shared = Arc<Mutex<Vec<Bot>>>;
type Room = Arc<Mutex<Option<String>>>;

fn with<T>(bots: &Shared, i: usize, f: impl FnOnce(&mut Bot) -> T) -> Option<T> {
    bots.lock().ok().and_then(|mut b| b.get_mut(i).map(f))
}

/// Réaction d'un bot à un message du serveur ; renvoie les messages à envoyer (à lui ou aux autres).
fn on_msg(i: usize, m: ServerMsg, bots: &Shared, room: &Room) -> Vec<(usize, ClientMsg)> {
    let mut out = Vec::new();
    match m {
        ServerMsg::Welcome { player_id, .. } => {
            with(bots, i, |b| b.id = Some(player_id));
        }
        ServerMsg::Error { message } => {
            with(bots, i, |b| b.errors.push(message));
        }
        ServerMsg::Lobby { state } => {
            if i == 0
                && let Ok(mut r) = room.lock()
                && r.is_none()
            {
                *r = Some(state.code.clone());
                let n = bots.lock().map_or(0, |b| b.len());
                for o in 1..n {
                    out.push((
                        o,
                        ClientMsg::JoinRoom {
                            code: state.code.clone(),
                        },
                    ));
                }
            }
        }
        ServerMsg::Draft { state } => {
            let me = with(bots, i, |b| b.id.clone()).flatten();
            let turn = state.turns.get(state.current_turn as usize);
            let mine = state
                .picks
                .iter()
                .find(|p| Some(&p.player_id) == me.as_ref());
            if let (Some(me), Some(turn), Some(pick)) = (me, turn, mine)
                && turn.contains(&me)
                && !pick.locked
            {
                let free: Vec<&str> = CHAMPIONS
                    .iter()
                    .map(|c| c.id.as_str())
                    .filter(|id| {
                        !state
                            .picks
                            .iter()
                            .any(|p| p.champion_id.as_deref() == Some(id))
                    })
                    .collect();
                if !free.is_empty() {
                    out.push((
                        i,
                        ClientMsg::DraftLock {
                            // un choix par bot : deux bots du même tour ne visent pas le même champion
                            champion_id: free[i % free.len()].to_owned(),
                        },
                    ));
                }
            }
        }
        ServerMsg::GameStart { start } => {
            with(bots, i, |b| {
                b.in_game = true;
                b.entity = Some(start.entity_id);
            });
        }
        ServerMsg::Snapshot { snap } => {
            with(bots, i, |b| {
                b.snaps += 1;
                if !snap.entities.is_empty() {
                    let me = b.entity;
                    b.visible = snap
                        .entities
                        .iter()
                        .map(|e| e.id)
                        .filter(|id| Some(*id) != me)
                        .collect();
                }
            });
        }
        _ => {}
    }
    out
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let url = arg("url", "ws://localhost:3000/ws");
    let n: usize = arg("bots", "8").parse().unwrap_or(8);
    let seconds: u64 = arg("seconds", "30").parse().unwrap_or(30);
    let bots: Shared = Arc::new(Mutex::new((0..n).map(|_| Bot::default()).collect()));
    let room: Room = Arc::new(Mutex::new(None));
    // un canal d'envoi par bot ; les réactions d'un bot peuvent écrire aux autres
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    for _ in 0..n {
        let (tx, rx) = mpsc::unbounded_channel::<ClientMsg>();
        senders.push(tx);
        receivers.push(rx);
    }
    let senders = Arc::new(senders);
    for (i, mut rx) in receivers.into_iter().enumerate() {
        let (ws, _) = match connect_async(url.as_str()).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[bots] connexion à {url} impossible : {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
        let (mut write, mut read) = ws.split();
        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let Ok(text) = serde_json::to_string(&m) else {
                    continue;
                };
                if write.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        });
        let (bots, room, out_tx) = (bots.clone(), room.clone(), senders.clone());
        tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                let Message::Text(raw) = msg else { continue };
                with(&bots, i, |b| b.bytes += raw.len());
                let Ok(m) = serde_json::from_str::<ServerMsg>(&raw) else {
                    continue;
                };
                for (to, out) in on_msg(i, m, &bots, &room) {
                    let _ = out_tx[to].send(out);
                }
            }
        });
        let _ = senders[i].send(ClientMsg::Hello {
            name: format!("Bot{i}"),
            version: PROTOCOL_VERSION,
            reconnect_token: None,
        });
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let _ = senders[0].send(ClientMsg::CreateRoom {
        dev_mode: Some(true),
    });
    let all = |f: &dyn Fn(&Bot) -> bool| bots.lock().is_ok_and(|b| b.iter().all(f));
    for _ in 0..50 {
        if room.lock().is_ok_and(|r| r.is_some()) && all(&|b| b.id.is_some()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = senders[0].send(ClientMsg::StartGame);
    let code = room.lock().ok().and_then(|r| r.clone()).unwrap_or_default();
    println!("[bots] salle {code}, {n} bots ; draft + finalisation + chargement (~20 s)...");
    for _ in 0..600 {
        if all(&|b| b.in_game) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !all(&|b| b.in_game) {
        eprintln!("[bots] ÉCHEC : la partie ne démarre pas");
        return std::process::ExitCode::FAILURE;
    }
    if let Ok(mut b) = bots.lock() {
        for bot in b.iter_mut() {
            bot.bytes = 0;
            bot.snaps = 0;
        }
    }
    let t0 = Instant::now();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    while t0.elapsed() < Duration::from_secs(seconds) {
        for i in 0..n {
            let visible = with(&bots, i, |b| b.visible.clone()).unwrap_or_default();
            let r = rng.next();
            let pos = |rng: &mut Rng, lo: f64, hi: f64| Vec2 {
                x: rng.range(lo, hi),
                y: rng.range(lo, hi),
            };
            let m = if r < 0.5 {
                ClientMsg::Move {
                    pos: pos(&mut rng, 500.0, 11500.0),
                }
            } else if r < 0.7 {
                ClientMsg::AttackMove {
                    pos: pos(&mut rng, 4000.0, 8000.0),
                }
            } else if r < 0.85 && !visible.is_empty() {
                ClientMsg::Attack {
                    target_id: visible[rng.below(visible.len())],
                }
            } else {
                let slots = [
                    AbilitySlot::Q,
                    AbilitySlot::W,
                    AbilitySlot::E,
                    AbilitySlot::R,
                ];
                ClientMsg::Cast {
                    slot: slots[rng.below(4)],
                    target_pos: Some(pos(&mut rng, 0.0, 12000.0)),
                    target_id: None,
                }
            };
            let _ = senders[i].send(m);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let dt = t0.elapsed().as_secs_f64();
    let mut failed = false;
    if let Ok(b) = bots.lock() {
        for (i, bot) in b.iter().enumerate() {
            let rate = f64::from(bot.snaps) / dt;
            let errors = if bot.errors.is_empty() {
                "aucune".to_owned()
            } else {
                bot.errors.join(" | ")
            };
            println!(
                "[bots] Bot{i}: {rate:.1} snapshots/s, {:.1} Ko/s (JSON décompressé), erreurs: {errors}",
                bot.bytes as f64 / 1024.0 / dt
            );
            failed |= rate < 15.0;
        }
    }
    if failed {
        eprintln!("[bots] ÉCHEC : cadence de snapshots trop basse");
        return std::process::ExitCode::FAILURE;
    }
    println!("[bots] OK");
    std::process::ExitCode::SUCCESS
}
