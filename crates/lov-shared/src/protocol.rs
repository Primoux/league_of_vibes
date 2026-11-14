//! Protocole WebSocket client <-> serveur, JSON (port de `protocol.ts`).
//!
//! Les messages sont des unions étiquetées par le champ `t`.

use crate::types::{
    AbilitySlot, BuffSnapshot, ChampSnapshot, EntityId, EntityKind, EntitySnapshot, GameEvent,
    PlayerId, Role, SelfState, SummonerId, SummonerSlot, Team, TeamOrNeutral, Vec2,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;

/// v2 : `Snapshot.patches` (mises à jour partielles des entités dans les snapshots delta).
pub const PROTOCOL_VERSION: u32 = 2;
pub const WS_PATH: &str = "/ws";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoomPhase {
    Lobby,
    Draft,
    Loading,
    Game,
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyPlayer {
    pub id: PlayerId,
    pub name: String,
    pub team: Team,
    pub ready: bool,
    pub is_host: bool,
    pub connected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyState {
    pub code: String,
    pub phase: RoomPhase,
    pub players: Vec<LobbyPlayer>,
    /// Autorise une partie à moins de 8 joueurs (tests).
    pub dev_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftPick {
    pub player_id: PlayerId,
    pub team: Team,
    /// `null` = pas encore choisi.
    pub champion_id: Option<String>,
    pub hover_champion_id: Option<String>,
    pub locked: bool,
    pub summoners: [SummonerId; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftState {
    /// Ordre des tours de pick : chaque tour liste les joueurs qui choisissent simultanément.
    pub turns: Vec<Vec<PlayerId>>,
    pub current_turn: u32,
    /// Timestamp serveur (ms).
    pub turn_ends_at: f64,
    pub picks: Vec<DraftPick>,
    /// Fin de la phase de finalisation (sorts d'invocateur, swaps) après le dernier pick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finalize_ends_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStartPlayer {
    pub id: PlayerId,
    pub name: String,
    pub team: Team,
    pub champion_id: String,
    pub summoners: [SummonerId; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStart {
    pub you: PlayerId,
    pub team: Team,
    pub entity_id: EntityId,
    pub players: Vec<GameStartPlayer>,
    pub server_time: f64,
}

/// Score d'une équipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamScore {
    pub kills: u32,
    pub towers: u32,
    pub dragons: Vec<String>,
    pub grubs: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub blue: TeamScore,
    pub red: TeamScore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tick: u64,
    /// Secondes de jeu depuis le début.
    pub time: f64,
    /// ms epoch, pour l'interpolation.
    pub server_time: f64,
    pub entities: Vec<EntitySnapshot>,
    /// Si défini, le snapshot est un DELTA (entités absentes conservées ; `removed` liste celles
    /// qui disparaissent). Sinon il est COMPLET (toute entité absente est supprimée).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<EntityId>>,
    /// Snapshots delta uniquement : mises à jour partielles d'entités déjà connues du client.
    /// Appliquer avec [`apply_entity_patch`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patches: Option<Vec<EntityPatch>>,
    pub events: Vec<GameEvent>,
    #[serde(rename = "self", default, skip_serializing_if = "Option::is_none")]
    pub self_state: Option<SelfState>,
    /// Timers d'objectifs connus de l'équipe (secondes de jeu du prochain spawn).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_timers: Option<BTreeMap<String, f64>>,
    pub score: Score,
}

impl Snapshot {
    /// Un snapshot est un delta quand `removed` est défini.
    pub fn is_delta(&self) -> bool {
        self.removed.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub player_id: PlayerId,
    pub champion_id: String,
    pub team: Team,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub gold: f64,
    pub damage_to_champions: f64,
    pub items: Vec<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameOver {
    pub winner: Team,
    pub duration: f64,
    pub stats: Vec<PlayerStats>,
}

// ---------------------------------------------------------------- Client -> Serveur

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMsg {
    Hello {
        name: String,
        version: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reconnect_token: Option<String>,
    },
    CreateRoom {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dev_mode: Option<bool>,
    },
    JoinRoom {
        code: String,
    },
    LeaveRoom,
    SetTeam {
        team: Team,
    },
    SetRole {
        role: Role,
    },
    SetReady {
        ready: bool,
    },
    /// Hôte uniquement.
    StartGame,
    DraftHover {
        champion_id: String,
    },
    DraftLock {
        champion_id: String,
    },
    DraftSummoners {
        summoners: [SummonerId; 2],
    },
    // --- en jeu
    Move {
        pos: Vec2,
    },
    AttackMove {
        pos: Vec2,
    },
    Attack {
        target_id: EntityId,
    },
    Stop,
    Cast {
        slot: AbilitySlot,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_pos: Option<Vec2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<EntityId>,
    },
    Summoner {
        slot: SummonerSlot,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_pos: Option<Vec2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_id: Option<EntityId>,
    },
    LevelAbility {
        slot: AbilitySlot,
    },
    Buy {
        item_id: String,
    },
    Sell {
        slot: i32,
    },
    UndoShop,
    /// Actif d'objet ; slot -1 = trinket.
    UseItem {
        slot: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_pos: Option<Vec2>,
    },
    Recall,
    PlaceWard {
        pos: Vec2,
    },
    Ping {
        pos: Vec2,
        kind: String,
    },
    Chat {
        text: String,
        team_only: bool,
    },
    // --- divers
    /// Mesure de latence : le serveur répond `pong` avec le même `clientTime`.
    NetPing {
        client_time: f64,
    },
    /// Après la fin de partie : retour au lobby de la même salle.
    ReturnToLobby,
}

// ---------------------------------------------------------------- Serveur -> Client

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMsg {
    Welcome {
        player_id: PlayerId,
        reconnect_token: String,
        version: u32,
    },
    Error {
        message: String,
    },
    Lobby {
        state: LobbyState,
    },
    Draft {
        state: DraftState,
    },
    Loading {
        ends_at: f64,
    },
    GameStart {
        start: GameStart,
    },
    Snapshot {
        snap: Box<Snapshot>,
    },
    GameOver {
        result: GameOver,
    },
    Pong {
        server_time: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_time: Option<f64>,
    },
}

// ---------------------------------------------------------------- Patches d'entités

/// Les champs de `EntitySnapshot` (`keyof EntitySnapshot`), pour la liste `u` d'un patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityField {
    Id,
    Kind,
    Team,
    X,
    Y,
    Facing,
    Hp,
    MaxHp,
    Mana,
    MaxMana,
    Shield,
    Dead,
    Subtype,
    OwnerId,
    Level,
    Anim,
    Buffs,
    Champ,
}

/// Mise à jour partielle d'une entité (voir `Snapshot.patches`) : `Partial<Omit<EntitySnapshot,
/// 'id'>> & { id, u? }`.
///
/// Un champ `None` n'est pas modifié ; un champ `Some` remplace la valeur. Pour SUPPRIMER un champ
/// optionnel, son nom figure dans `u`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityPatch {
    pub id: EntityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntityKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamOrNeutral>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mana: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mana: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffs: Option<Vec<BuffSnapshot>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub champ: Option<ChampSnapshot>,
    /// Champs supprimés.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<Vec<EntityField>>,
}

impl EntityPatch {
    /// Patch vide (ne change rien) pour l'entité `id`.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            kind: None,
            team: None,
            x: None,
            y: None,
            facing: None,
            hp: None,
            max_hp: None,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: None,
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
            u: None,
        }
    }
}

/// Applique un patch : retourne une NOUVELLE entité (`prev` n'est pas modifiée).
///
/// D'abord les champs présents sont écrasés, puis ceux listés dans `u` sont supprimés. Seuls les
/// champs optionnels peuvent être supprimés : un nom de champ obligatoire dans `u` est ignoré (le
/// TypeScript produirait une entité invalide, le Rust garde une entité valide).
pub fn apply_entity_patch(prev: &EntitySnapshot, patch: &EntityPatch) -> EntitySnapshot {
    let mut next = prev.clone();
    if let Some(v) = patch.kind {
        next.kind = v;
    }
    if let Some(v) = patch.team {
        next.team = v;
    }
    if let Some(v) = patch.x {
        next.x = v;
    }
    if let Some(v) = patch.y {
        next.y = v;
    }
    if let Some(v) = patch.facing {
        next.facing = v;
    }
    if let Some(v) = patch.hp {
        next.hp = v;
    }
    if let Some(v) = patch.max_hp {
        next.max_hp = v;
    }
    if patch.mana.is_some() {
        next.mana = patch.mana;
    }
    if patch.max_mana.is_some() {
        next.max_mana = patch.max_mana;
    }
    if patch.shield.is_some() {
        next.shield = patch.shield;
    }
    if patch.dead.is_some() {
        next.dead = patch.dead;
    }
    if patch.subtype.is_some() {
        next.subtype.clone_from(&patch.subtype);
    }
    if patch.owner_id.is_some() {
        next.owner_id.clone_from(&patch.owner_id);
    }
    if patch.level.is_some() {
        next.level = patch.level;
    }
    if patch.anim.is_some() {
        next.anim.clone_from(&patch.anim);
    }
    if patch.buffs.is_some() {
        next.buffs.clone_from(&patch.buffs);
    }
    if patch.champ.is_some() {
        next.champ.clone_from(&patch.champ);
    }
    if let Some(unset) = &patch.u {
        for field in unset {
            match field {
                EntityField::Mana => next.mana = None,
                EntityField::MaxMana => next.max_mana = None,
                EntityField::Shield => next.shield = None,
                EntityField::Dead => next.dead = None,
                EntityField::Subtype => next.subtype = None,
                EntityField::OwnerId => next.owner_id = None,
                EntityField::Level => next.level = None,
                EntityField::Anim => next.anim = None,
                EntityField::Buffs => next.buffs = None,
                EntityField::Champ => next.champ = None,
                // champs obligatoires : non supprimables
                EntityField::Id
                | EntityField::Kind
                | EntityField::Team
                | EntityField::X
                | EntityField::Y
                | EntityField::Facing
                | EntityField::Hp
                | EntityField::MaxHp => {}
            }
        }
    }
    next
}

pub fn encode<T: Serialize>(msg: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

pub fn decode<T: DeserializeOwned>(raw: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> EntitySnapshot {
        EntitySnapshot {
            id: 5,
            kind: EntityKind::Champion,
            team: TeamOrNeutral::Blue,
            x: 100.0,
            y: 200.0,
            facing: 0.0,
            hp: 500.0,
            max_hp: 600.0,
            mana: Some(300.0),
            max_mana: Some(300.0),
            shield: Some(20.0),
            dead: None,
            subtype: Some("grondak".into()),
            owner_id: None,
            level: Some(3),
            anim: Some("run".into()),
            buffs: None,
            champ: None,
        }
    }

    #[test]
    fn patch_remplace_et_supprime() {
        let prev = entity();
        let mut patch = EntityPatch::new(5);
        patch.x = Some(150.0);
        patch.anim = Some("idle".into());
        patch.u = Some(vec![EntityField::Shield, EntityField::Mana]);
        let next = apply_entity_patch(&prev, &patch);
        assert_eq!(next.x, 150.0);
        assert_eq!(next.y, 200.0);
        assert_eq!(next.anim.as_deref(), Some("idle"));
        assert_eq!(next.shield, None);
        assert_eq!(next.mana, None);
        assert_eq!(next.max_mana, Some(300.0));
        assert_eq!(prev, entity(), "prev ne doit pas être modifiée");
    }

    #[test]
    fn patch_json_comme_le_ts() {
        let patch: EntityPatch =
            decode(r#"{"id":5,"hp":10.5,"u":["shield"]}"#).expect("patch valide");
        let next = apply_entity_patch(&entity(), &patch);
        assert_eq!(next.hp, 10.5);
        assert_eq!(next.shield, None);
        assert_eq!(
            encode(&patch).unwrap(),
            r#"{"id":5,"hp":10.5,"u":["shield"]}"#
        );
    }

    #[test]
    fn messages_etiquetes_par_t() {
        assert_eq!(encode(&ClientMsg::Stop).unwrap(), r#"{"t":"stop"}"#);
        assert_eq!(
            encode(&ClientMsg::NetPing { client_time: 1.5 }).unwrap(),
            r#"{"t":"netPing","clientTime":1.5}"#
        );
        let m: ServerMsg = decode(r#"{"t":"pong","serverTime":10}"#).unwrap();
        assert_eq!(
            m,
            ServerMsg::Pong {
                server_time: 10.0,
                client_time: None
            }
        );
        let m: ClientMsg = decode(r#"{"t":"createRoom"}"#).unwrap();
        assert_eq!(m, ClientMsg::CreateRoom { dev_mode: None });
    }

    #[test]
    fn version() {
        assert_eq!(PROTOCOL_VERSION, 2);
        assert_eq!(WS_PATH, "/ws");
    }
}
