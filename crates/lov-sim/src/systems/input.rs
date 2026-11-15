//! Entrées : convertit les commandes de déplacement/attaque en ordres sur l'entité.
//! Port de `packages/server/src/systems/input.ts`.
//!
//! Les commandes `cast`, `summoner`, `levelAbility`, `buy`, `sell`, `recall`, `placeWard` sont lues
//! directement dans `world.commands` par les systèmes concernés (abilities, summoners, leveling,
//! economy) : ce système les ignore, comme en TS. `world.commands` n'est jamais vidé ici (`World::step`
//! le fait en fin de tick).
//!
//! Différences avec le TypeScript, imposées par l'emprunt Rust :
//! - la liste des commandes est sortie du monde le temps du parcours (le traitement a besoin de
//!   `&mut World`) puis remise ; une commande ajoutée pendant le parcours (par un écouteur) est conservée
//!   après les commandes existantes, mais n'est pas traitée ce tick ;
//! - `chat` : `trim()` et `slice(0, 200)` JS comptent en unités UTF-16 ; la coupe ici ne sépare jamais un
//!   caractère (le TS pourrait laisser un demi-substitut).

use std::collections::HashMap;

use lov_shared::constants::MAP_SIZE;
use lov_shared::math::clamp;
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{GameEvent, PlayerId, Vec2};

use crate::systems::combat::{attack_state, cancel_attack, order_attack_move};
use crate::world::{QueuedCommand, System, World};

/// Intervalle minimal entre deux pings d'un même joueur (s).
pub const PING_COOLDOWN: f64 = 0.4;
/// Longueur maximale d'un message de chat (unités UTF-16, comme `slice(0, 200)`).
const CHAT_MAX_UNITS: usize = 200;

fn clamp_pos(p: Vec2) -> Vec2 {
    Vec2 {
        x: clamp(p.x, 0.0, MAP_SIZE),
        y: clamp(p.y, 0.0, MAP_SIZE),
    }
}

/// Espaces au sens de `String.prototype.trim` (WhiteSpace + LineTerminator d'ECMAScript).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// `text.trim().slice(0, 200)`.
fn clean_chat(text: &str) -> String {
    let t = text.trim_matches(is_js_space);
    let mut units = 0;
    let mut end = t.len();
    for (i, c) in t.char_indices() {
        units += c.len_utf16();
        if units > CHAT_MAX_UNITS {
            end = i;
            break;
        }
    }
    t[..end].to_owned()
}

/// Le système d'entrées (à placer en premier).
pub fn input_system() -> System {
    // Dernier ping de chaque joueur (le `WeakMap<World, Map>` du TS).
    let mut last_ping: HashMap<PlayerId, f64> = HashMap::new();
    System::new("input", move |world, _dt| update(world, &mut last_ping))
}

fn update(world: &mut World, last_ping: &mut HashMap<PlayerId, f64>) {
    if world.commands.is_empty() {
        return;
    }
    let commands = std::mem::take(&mut world.commands);
    for QueuedCommand { player_id, msg } in &commands {
        let Some(id) = world.champion_id_of(player_id) else {
            continue;
        };
        match msg {
            ClientMsg::Ping { pos, kind } => {
                let last = last_ping
                    .get(player_id)
                    .copied()
                    .unwrap_or(f64::NEG_INFINITY);
                if world.time - last < PING_COOLDOWN {
                    continue;
                }
                match last_ping.get_mut(player_id) {
                    Some(t) => *t = world.time,
                    None => {
                        last_ping.insert(player_id.clone(), world.time);
                    }
                }
                world.emit(GameEvent::Ping {
                    player_id: player_id.clone(),
                    pos: clamp_pos(*pos),
                    kind: kind.clone(),
                });
                continue;
            }
            ClientMsg::Chat { text, team_only } => {
                let text = clean_chat(text);
                if !text.is_empty() {
                    world.emit(GameEvent::Chat {
                        player_id: player_id.clone(),
                        text,
                        team_only: *team_only,
                    });
                }
                continue;
            }
            _ => {}
        }
        let Some(ent) = world.get_mut(id) else {
            continue;
        };
        if ent.dead {
            continue;
        }
        match msg {
            ClientMsg::Move { pos } => {
                cancel_attack(ent);
                ent.move_target = Some(clamp_pos(*pos));
                ent.path = None;
                ent.attack_target = None;
                let st = attack_state(ent);
                st.attack_move = None;
                st.chasing = false;
            }
            ClientMsg::AttackMove { pos } => {
                cancel_attack(ent);
                order_attack_move(ent, clamp_pos(*pos));
            }
            ClientMsg::Attack { target_id } => {
                let team = ent.team;
                let Some(t) = world.get(*target_id) else {
                    continue;
                };
                if t.team == team || t.dead {
                    continue;
                }
                let tid = t.id;
                let Some(ent) = world.get_mut(id) else {
                    continue;
                };
                if ent.attack_target != Some(tid) {
                    cancel_attack(ent);
                }
                ent.attack_target = Some(tid);
                attack_state(ent).attack_move = None;
            }
            ClientMsg::Stop => {
                cancel_attack(ent);
                ent.move_target = None;
                ent.path = None;
                ent.attack_target = None;
                let st = attack_state(ent);
                st.attack_move = None;
                st.chasing = false;
            }
            _ => {}
        }
    }
    // Commandes ajoutées pendant le parcours : après les existantes.
    let added = std::mem::replace(&mut world.commands, commands);
    world.commands.extend(added);
}
