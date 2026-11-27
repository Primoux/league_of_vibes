//! Chat en partie : Entrée = équipe, Maj + Entrée = tous ; Échap ferme la saisie. Les messages
//! s'estompent après 10 s (l'historique complet réapparaît quand la saisie est ouverte).
//!
//! Portage de `packages/client/src/ui/hud/Chat.tsx` et de la tenue des messages de `ui/game/world.ts`
//! (40 lignes gardées, déjà filtrées par équipe côté serveur).

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::TextInput;
use lov_shared::data::champions::get_champion;
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{GameEvent, Team};

use super::{HUD_Z, label};
use crate::net::{Connection, GameState, ServerEvent};
use crate::ui::theme::{BLUE, GOLD_HI, MUTED, RED, TEXT, hexa};
use crate::ui::{BlocksWorld, Screen, UiFonts};

const VISIBLE_SECS: f32 = 10.0;
const KEEP: usize = 40;
const MAX_LEN: usize = 200;

/// Une ligne reçue.
#[derive(Clone, Debug)]
struct Line {
    at: f32,
    player: String,
    text: String,
    team_only: bool,
}

#[derive(Resource, Default)]
pub struct ChatUi {
    lines: Vec<Line>,
    pub open: bool,
    all: bool,
    /// Lignes affichées et état de la saisie : redessin si ça change.
    shown: (usize, bool, u32),
}

#[derive(Component)]
struct ChatList;

#[derive(Component)]
struct ChatField;

#[derive(Component)]
struct ChatBox;

#[derive(Component)]
struct ChatMode;

fn spawn(mut commands: Commands, fonts: Res<UiFonts>) {
    let f = &*fonts;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                bottom: px(190),
                width: px(380),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            GlobalZIndex(HUD_Z + 1),
            Pickable::IGNORE,
            DespawnOnExit(Screen::Game),
        ))
        .with_children(|col| {
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                },
                ChatList,
                Pickable::IGNORE,
            ));
            col.spawn((
                Node {
                    height: px(30),
                    padding: UiRect::horizontal(px(8)),
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.5)),
                BackgroundColor(hexa(0x05060c, 0.85)),
                Visibility::Hidden,
                ChatBox,
                BlocksWorld,
            ))
            .with_children(|b| {
                b.spawn((label(f, "[Équipe]", 12.0, 800, BLUE), ChatMode));
                let mut text = EditableText::new("");
                text.max_characters = Some(MAX_LEN);
                b.spawn((
                    ChatField,
                    TextInput,
                    text,
                    f.text(13.0, 500),
                    TextColor(TEXT),
                    Node {
                        flex_grow: 1.0,
                        height: px(18),
                        ..default()
                    },
                ));
            });
        });
}

fn receive(mut events: MessageReader<ServerEvent>, time: Res<Time>, mut chat: ResMut<ChatUi>) {
    for ServerEvent(e) in events.read() {
        if let GameEvent::Chat {
            player_id,
            text,
            team_only,
        } = e
        {
            chat.lines.push(Line {
                at: time.elapsed_secs(),
                player: player_id.clone(),
                text: text.clone(),
                team_only: *team_only,
            });
            let n = chat.lines.len();
            if n > KEEP {
                chat.lines.drain(..n - KEEP);
            }
        }
    }
}

/// Entrée ouvre ou envoie, Échap ferme.
#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut chat: ResMut<ChatUi>,
    mut focus: ResMut<InputFocus>,
    conn: Res<Connection>,
    fields: Query<(), With<EditableText>>,
    mut field: Query<(Entity, &mut EditableText), With<ChatField>>,
    mut boxes: Query<&mut Visibility, With<ChatBox>>,
    mut mode: Query<(&mut Text, &mut TextColor), With<ChatMode>>,
) {
    let Some((e, mut text)) = field.iter_mut().next() else {
        return;
    };
    let mine = focus.get() == Some(e);
    let close = |chat: &mut ChatUi, focus: &mut InputFocus, text: &mut EditableText| {
        chat.open = false;
        text.clear();
        focus.clear();
    };
    if chat.open {
        if keys.just_pressed(KeyCode::Escape) {
            close(&mut chat, &mut focus, &mut text);
        } else if keys.just_pressed(KeyCode::Enter) {
            let msg: String = text
                .value()
                .to_string()
                .trim()
                .chars()
                .take(MAX_LEN)
                .collect();
            if !msg.is_empty() {
                conn.send(ClientMsg::Chat {
                    text: msg,
                    team_only: !chat.all,
                });
            }
            close(&mut chat, &mut focus, &mut text);
        } else if !mine && focus.get().is_some() {
            // le focus est parti ailleurs (clic) : la saisie se ferme
            close(&mut chat, &mut focus, &mut text);
        }
    } else if keys.just_pressed(KeyCode::Enter) && !focus.get().is_some_and(|f| fields.contains(f))
    {
        chat.open = true;
        chat.all = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        focus.set(e, FocusCause::Navigated);
        for (mut t, mut c) in &mut mode {
            t.0 = if chat.all { "[Tous]" } else { "[Équipe]" }.into();
            c.0 = if chat.all { GOLD_HI } else { BLUE };
        }
    }
    for mut v in &mut boxes {
        v.set_if_neq(if chat.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Liste des messages (récents seulement, sauf saisie ouverte), redessinée quand elle change.
fn redraw(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut chat: ResMut<ChatUi>,
    list: Query<Entity, With<ChatList>>,
) {
    let now = time.elapsed_secs();
    let visible: Vec<&Line> = chat
        .lines
        .iter()
        .filter(|l| chat.open || now - l.at < VISIBLE_SECS)
        .collect();
    // l'opacité baisse par paliers d'une seconde dans les deux dernières secondes
    let fade = visible.first().map_or(0, |l| {
        ((now - l.at - (VISIBLE_SECS - 2.0)).max(0.0) * 2.0) as u32
    });
    let key = (visible.len(), chat.open, fade);
    if chat.shown == key {
        return;
    }
    let mine = state.game.as_ref().map_or(Team::Blue, |g| g.team);
    let lines: Vec<(String, String, Color, f32)> = visible
        .iter()
        .map(|l| {
            let p = state
                .game
                .as_ref()
                .and_then(|g| g.players.iter().find(|p| p.id == l.player));
            let name = p.map_or(l.player.clone(), |p| {
                let champ = get_champion(&p.champion_id)
                    .map_or(String::new(), |c| format!(" ({})", c.name));
                format!("{}{champ}", p.name)
            });
            let color = match p.map(|p| p.team) {
                Some(t) if t == mine => BLUE,
                Some(_) => RED,
                None => MUTED,
            };
            let prefix = if l.team_only { "" } else { "[Tous] " };
            let age = now - l.at;
            let alpha = if chat.open {
                1.0
            } else {
                (VISIBLE_SECS - age).clamp(0.0, 2.0) / 2.0
            };
            (format!("{prefix}{name} :"), l.text.clone(), color, alpha)
        })
        .collect();
    chat.shown = key;
    let f = &*fonts;
    for e in &list {
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|col| {
            for (name, text, color, alpha) in &lines {
                col.spawn((
                    Node {
                        column_gap: px(5),
                        padding: UiRect::axes(px(6), px(2)),
                        border_radius: BorderRadius::all(px(4)),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                    BackgroundColor(hexa(0x05060c, 0.55 * alpha)),
                ))
                .with_children(|row| {
                    row.spawn(label(f, name.clone(), 12.0, 800, color.with_alpha(*alpha)));
                    row.spawn(label(f, text.clone(), 12.0, 500, TEXT.with_alpha(*alpha)));
                });
            }
        });
    }
}

fn reset(mut chat: ResMut<ChatUi>) {
    *chat = ChatUi::default();
}

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatUi>()
            .add_systems(OnEnter(Screen::Game), (reset, spawn))
            .add_systems(
                Update,
                (receive, keys, redraw)
                    .chain()
                    .run_if(in_state(Screen::Game)),
            );
    }
}
