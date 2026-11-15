//! Validation des messages client et limitation de débit (port de `net/validate.ts`).
//!
//! Tout message entrant passe par [`parse_client_msg`] : forme vérifiée, nombres finis, chaînes bornées.
//! Les longueurs de chaîne sont comptées en unités UTF-16 comme `String.length` en JavaScript.

use lov_shared::constants::MAP_SIZE;
use lov_shared::protocol::{ClientMsg, decode};
use lov_shared::types::SummonerId;
use serde_json::{Map, Value};

type Obj = Map<String, Value>;

/// Longueur en unités UTF-16 (`String.length` du JS).
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Espaces retirés par `String.prototype.trim` (diffère de `char::is_whitespace` : U+FEFF oui, U+0085 non).
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `String.prototype.trim` du JS.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `s.slice(0, max)` en unités UTF-16 ; une paire de substitution coupée en deux est abandonnée.
pub fn js_slice(s: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        units += c.len_utf16();
        if units > max {
            return &s[..i];
        }
    }
    s
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

fn str_max(v: &Value, max: usize) -> bool {
    v.as_str().is_some_and(|s| js_len(s) <= max)
}

/// `Number.isInteger` : entier de valeur (5.0 compte), pas seulement de type.
fn integer(v: &Value) -> Option<f64> {
    num(v).filter(|n| n.fract() == 0.0)
}

fn id(v: &Value) -> bool {
    integer(v).is_some_and(|n| n > 0.0)
}

fn pos(v: &Value) -> bool {
    let Some(o) = v.as_object() else {
        return false;
    };
    let in_map = |n: f64| (-1000.0..=MAP_SIZE + 1000.0).contains(&n);
    matches!((o.get("x").and_then(num), o.get("y").and_then(num)), (Some(x), Some(y)) if in_map(x) && in_map(y))
}

/// Champ optionnel : absent = valide (`undefined` du TS) ; présent, `null` compris, il doit passer `ok`.
fn opt(o: &Obj, key: &str, ok: fn(&Value) -> bool) -> bool {
    o.get(key).is_none_or(ok)
}

fn one_of(o: &Obj, key: &str, set: &[&str]) -> bool {
    o.get(key)
        .and_then(Value::as_str)
        .is_some_and(|s| set.contains(&s))
}

fn int_in(o: &Obj, key: &str, lo: f64, hi: f64) -> bool {
    o.get(key)
        .and_then(integer)
        .is_some_and(|n| n >= lo && n <= hi)
}

/// `/^[a-z_]{1,16}$/`
fn ping_kind(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        (1..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
    })
}

const SLOTS: [&str; 4] = ["Q", "W", "E", "R"];

/// Parse et valide un message brut. Retourne `None` si invalide.
pub fn parse_client_msg(raw: &str) -> Option<ClientMsg> {
    if js_len(raw) > 4096 {
        return None;
    }
    let m: Value = serde_json::from_str(raw).ok()?;
    let o = m.as_object()?;
    o.get("t")?.as_str()?;
    if !validate(o) {
        return None;
    }
    // Forme vérifiée : le décodage typé fixe la représentation (et refuse ce que le type ne peut porter).
    decode::<ClientMsg>(raw).ok()
}

fn validate(m: &Obj) -> bool {
    let Some(t) = m.get("t").and_then(Value::as_str) else {
        return false;
    };
    let get = |k: &str| m.get(k);
    match t {
        "hello" => {
            get("name").is_some_and(|v| str_max(v, 64))
                && get("version").and_then(num).is_some()
                && opt(m, "reconnectToken", |v| str_max(v, 128))
        }
        "createRoom" => opt(m, "devMode", Value::is_boolean),
        "joinRoom" => get("code").is_some_and(|v| str_max(v, 16)),
        "leaveRoom" | "startGame" | "stop" | "undoShop" | "recall" | "returnToLobby" => true,
        "setTeam" => one_of(m, "team", &["blue", "red"]),
        "setRole" => one_of(m, "role", &["top", "jungle", "mid", "bot"]),
        "setReady" => get("ready").is_some_and(Value::is_boolean),
        "draftHover" | "draftLock" => get("championId").is_some_and(|v| str_max(v, 32)),
        "draftSummoners" => get("summoners").and_then(Value::as_array).is_some_and(|a| {
            a.len() == 2
                && a.iter()
                    .all(|s| serde_json::from_value::<SummonerId>(s.clone()).is_ok())
        }),
        "move" | "attackMove" | "placeWard" => get("pos").is_some_and(pos),
        "attack" => get("targetId").is_some_and(id),
        "cast" => one_of(m, "slot", &SLOTS) && opt(m, "targetPos", pos) && opt(m, "targetId", id),
        "summoner" => {
            one_of(m, "slot", &["D", "F"]) && opt(m, "targetPos", pos) && opt(m, "targetId", id)
        }
        "levelAbility" => one_of(m, "slot", &SLOTS),
        "buy" => get("itemId").is_some_and(|v| str_max(v, 48)),
        // slot -1 = trinket, 0..5 = inventaire (protocol.ts)
        "useItem" => int_in(m, "slot", -1.0, 5.0) && opt(m, "targetPos", pos),
        "sell" => int_in(m, "slot", 0.0, 7.0),
        "ping" => get("pos").is_some_and(pos) && get("kind").is_some_and(ping_kind),
        "chat" => {
            get("text")
                .and_then(Value::as_str)
                .is_some_and(|s| js_len(s) <= 200 && !js_trim(s).is_empty())
                && get("teamOnly").is_some_and(Value::is_boolean)
        }
        "netPing" => get("clientTime").and_then(num).is_some(),
        _ => false,
    }
}

/// Seau à jetons : `rate` messages/s en moyenne, rafales jusqu'à `burst`.
/// `take()` retourne `false` si le message doit être ignoré ; `strikes` compte les dépassements.
pub struct RateLimiter {
    rate: f64,
    burst: f64,
    now: Box<dyn FnMut() -> f64 + Send>,
    tokens: f64,
    last: f64,
    pub strikes: u32,
}

impl RateLimiter {
    /// Horloge réelle (ms depuis la création du limiteur).
    pub fn new(rate: f64, burst: f64) -> Self {
        let start = std::time::Instant::now();
        Self::with_clock(rate, burst, move || start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Horloge injectée, en millisecondes.
    pub fn with_clock(
        rate: f64,
        burst: f64,
        mut now: impl FnMut() -> f64 + Send + 'static,
    ) -> Self {
        let last = now();
        Self {
            rate,
            burst,
            now: Box::new(now),
            tokens: burst,
            last,
            strikes: 0,
        }
    }

    pub fn take(&mut self) -> bool {
        self.take_cost(1.0)
    }

    pub fn take_cost(&mut self, cost: f64) -> bool {
        let t = (self.now)();
        self.tokens = self
            .burst
            .min(self.tokens + ((t - self.last) / 1000.0) * self.rate);
        self.last = t;
        if self.tokens < cost {
            self.strikes = self.strikes.saturating_add(1);
            return false;
        }
        self.tokens -= cost;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::Vec2;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    fn parse(v: Value) -> Option<ClientMsg> {
        parse_client_msg(&v.to_string())
    }

    // net.test.ts › « validation des messages » › « accepte les messages valides et rejette les malformés »
    #[test]
    fn accepte_les_messages_valides_et_rejette_les_malformes() {
        assert_eq!(
            parse(json!({ "t": "move", "pos": { "x": 10, "y": 20 } })),
            Some(ClientMsg::Move {
                pos: Vec2 { x: 10.0, y: 20.0 }
            })
        );
        assert_eq!(
            parse_client_msg(r#"{"t":"move","pos":{"x":"a","y":1}}"#),
            None
        );
        assert_eq!(
            parse_client_msg(r#"{"t":"move","pos":{"x":1e309,"y":1}}"#),
            None
        );
        assert_eq!(parse_client_msg(r#"{"t":"attack","targetId":-1}"#), None);
        assert_eq!(parse_client_msg(r#"{"t":"inconnu"}"#), None);
        assert_eq!(parse_client_msg("pas du json"), None);
        assert_eq!(
            parse(json!({ "t": "draftSummoners", "summoners": ["flash", "nuke"] })),
            None
        );
        assert_eq!(
            parse(json!({ "t": "chat", "text": "x".repeat(500), "teamOnly": true })),
            None
        );
        assert!(
            parse(json!({ "t": "cast", "slot": "Q", "targetPos": { "x": 1, "y": 2 } })).is_some()
        );
    }

    // net.test.ts › « validation des messages » › « rate-limit à jetons »
    #[test]
    fn rate_limit_a_jetons() {
        let t = Arc::new(Mutex::new(0.0_f64));
        let clock = Arc::clone(&t);
        let mut rl = RateLimiter::with_clock(10.0, 5.0, move || *clock.lock().unwrap());
        let mut ok = 0;
        for _ in 0..20 {
            if rl.take() {
                ok += 1;
            }
        }
        assert_eq!(ok, 5);
        *t.lock().unwrap() += 1000.0;
        for _ in 0..20 {
            if rl.take() {
                ok += 1;
            }
        }
        assert_eq!(ok, 10);
        assert_eq!(rl.strikes, 30);
    }

    // validate-useitem.test.ts › « accepte les slots −1..5 avec ou sans targetPos »
    #[test]
    fn useitem_accepte_les_slots_moins1_a_5_avec_ou_sans_target_pos() {
        for slot in [-1, 0, 3, 5] {
            assert!(
                parse(json!({ "t": "useItem", "slot": slot })).is_some(),
                "slot {slot}"
            );
        }
        assert!(
            parse(json!({ "t": "useItem", "slot": 2, "targetPos": { "x": 100, "y": 200 } }))
                .is_some()
        );
    }

    // validate-useitem.test.ts › « rejette les slots hors bornes et les positions invalides »
    #[test]
    fn useitem_rejette_les_slots_hors_bornes_et_les_positions_invalides() {
        for slot in [json!(-2), json!(6), json!(1.5), json!("1"), Value::Null] {
            assert_eq!(
                parse(json!({ "t": "useItem", "slot": slot })),
                None,
                "slot {slot}"
            );
        }
        assert_eq!(
            parse(json!({ "t": "useItem", "slot": 1, "targetPos": { "x": "a", "y": 0 } })),
            None
        );
    }

    // Règles de validate.ts non couvertes par les tests TS d'origine (ajoutées ici).
    #[test]
    fn regles_supplementaires() {
        // trop long (> 4096 unités UTF-16)
        let long = json!({ "t": "chat", "text": "a", "teamOnly": false, "x": "y".repeat(5000) });
        assert_eq!(parse(long), None);
        // pos : carte ± 1000
        assert!(parse(json!({ "t": "move", "pos": { "x": -1000, "y": 13000 } })).is_some());
        assert_eq!(
            parse(json!({ "t": "move", "pos": { "x": -1001, "y": 0 } })),
            None
        );
        // tableau, null, t non chaîne
        assert_eq!(parse_client_msg("[]"), None);
        assert_eq!(parse_client_msg("null"), None);
        assert_eq!(parse_client_msg(r#"{"t":3}"#), None);
        // optionnels : `null` n'est pas `undefined`
        let null_token = json!({ "t": "hello", "name": "A", "version": 2, "reconnectToken": null });
        assert_eq!(parse(null_token), None);
        assert!(parse(json!({ "t": "hello", "name": "A", "version": 2 })).is_some());
        assert_eq!(parse(json!({ "t": "createRoom", "devMode": 1 })), None);
        // chat vide / espaces
        let blank = json!({ "t": "chat", "text": " \u{feff} ", "teamOnly": false });
        assert_eq!(parse(blank), None);
        // ping : forme du `kind`
        let ping = |kind: &str| json!({ "t": "ping", "pos": { "x": 1, "y": 1 }, "kind": kind });
        assert!(parse(ping("danger_zone")).is_some());
        assert_eq!(parse(ping("Danger")), None);
        assert_eq!(parse(ping("")), None);
        // sell : 0..7
        assert!(parse(json!({ "t": "sell", "slot": 7 })).is_some());
        assert_eq!(parse(json!({ "t": "sell", "slot": 8 })), None);
        assert_eq!(parse(json!({ "t": "sell", "slot": -1 })), None);
        // enums
        assert_eq!(parse(json!({ "t": "setTeam", "team": "green" })), None);
        assert!(parse(json!({ "t": "setRole", "role": "jungle" })).is_some());
        assert_eq!(parse(json!({ "t": "cast", "slot": "q" })), None);
        assert_eq!(parse(json!({ "t": "summoner", "slot": "Q" })), None);
        assert!(parse(json!({ "t": "draftSummoners", "summoners": ["flash", "smite"] })).is_some());
        assert_eq!(
            parse(json!({ "t": "draftSummoners", "summoners": ["flash"] })),
            None
        );
        // netPing
        assert!(parse(json!({ "t": "netPing", "clientTime": 42 })).is_some());
        assert_eq!(parse(json!({ "t": "netPing", "clientTime": "42" })), None);
    }

    #[test]
    fn utilitaires_js() {
        assert_eq!(js_trim("\u{feff} a b \u{00a0}"), "a b");
        assert_eq!(js_slice("abcdefghijklmnopqrstu", 16), "abcdefghijklmnop");
        assert_eq!(js_slice("é😀", 2), "é"); // 😀 = 2 unités UTF-16, dépasse
        assert_eq!(js_len("😀"), 2);
    }
}
