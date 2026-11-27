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
use crate::input::PlayerInput;
use crate::net::{Connection, GameState, ServerEvent};
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::theme::{BLUE, GOLD_HI, MUTED, RED, TEXT, hexa};
use crate::ui::{BlocksWorld, Screen, UiFonts};

const VISIBLE_SECS: f32 = 10.0;
const KEEP: usize = 40;
const MAX_LEN: usize = 200;
/// Lignes affichées, saisie fermée (récentes seulement) et ouverte (historique).
const SHOWN_CLOSED: usize = 7;
const SHOWN_OPEN: usize = 30;

/// Une ligne reçue.
#[derive(Clone, Debug)]
struct Line {
    /// Numéro croissant : identifie la ligne même quand l'historique est plein.
    key: u64,
    at: f32,
    player: String,
    text: String,
    team_only: bool,
}

#[derive(Resource, Default)]
pub struct ChatUi {
    lines: Vec<Line>,
    next_key: u64,
    pub open: bool,
    all: bool,
    /// Ce qui est affiché (première et dernière ligne, saisie ouverte, palier d'estompage) : redessin
    /// si ça change.
    shown: Option<(Option<u64>, Option<u64>, bool, u32)>,
}

impl ChatUi {
    /// Lignes à afficher : les 30 dernières saisie ouverte, sinon les 7 dernières de moins de 10 s.
    fn visible(&self, now: f32) -> &[Line] {
        if self.open {
            return &self.lines[self.lines.len().saturating_sub(SHOWN_OPEN)..];
        }
        // les lignes arrivent dans l'ordre : les récentes sont à la fin
        let fresh = self
            .lines
            .iter()
            .rposition(|l| now - l.at >= VISIBLE_SECS)
            .map_or(0, |i| i + 1);
        let start = fresh.max(self.lines.len().saturating_sub(SHOWN_CLOSED));
        &self.lines[start..]
    }

    fn push(&mut self, at: f32, player: &str, text: &str, team_only: bool) {
        self.lines.push(Line {
            key: self.next_key,
            at,
            player: player.to_owned(),
            text: text.to_owned(),
            team_only,
        });
        self.next_key += 1;
        let n = self.lines.len();
        if n > KEEP {
            self.lines.drain(..n - KEEP);
        }
    }
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
            chat.push(time.elapsed_secs(), player_id, text, *team_only);
        }
    }
}

/// Entrée ouvre ou envoie, Échap ferme.
#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<PlayerInput>,
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
    } else if keys.just_pressed(KeyCode::Enter)
        // boutique ouverte, Entrée achète la sélection : elle n'ouvre pas le chat
        && !input.shop_open
        && !focus.get().is_some_and(|f| fields.contains(f))
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

/// Liste des messages, redessinée quand les lignes affichées ou leur estompage changent.
fn redraw(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut chat: ResMut<ChatUi>,
    list: Query<Entity, With<ChatList>>,
) {
    let now = time.elapsed_secs();
    let visible = chat.visible(now);
    // l'opacité baisse par paliers d'une demi-seconde dans les deux dernières secondes
    let fade = visible.first().filter(|_| !chat.open).map_or(0, |l| {
        ((now - l.at - (VISIBLE_SECS - 2.0)).max(0.0) * 2.0) as u32
    });
    let key = Some((
        visible.first().map(|l| l.key),
        visible.last().map(|l| l.key),
        chat.open,
        fade,
    ));
    if chat.shown == key {
        return;
    }
    let mine = state.game.as_ref().map_or(Team::Blue, |g| g.team);
    let f = &*fonts;
    for e in &list {
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|col| {
            for l in visible {
                let p = state
                    .game
                    .as_ref()
                    .and_then(|g| g.players.iter().find(|p| p.id == l.player));
                let color = match p.map(|p| p.team) {
                    Some(t) if t == mine => BLUE,
                    Some(_) => RED,
                    None => MUTED,
                };
                let alpha = if chat.open {
                    1.0
                } else {
                    (VISIBLE_SECS - (now - l.at)).clamp(0.0, 2.0) / 2.0
                };
                col.spawn((
                    Node {
                        column_gap: px(5),
                        padding: UiRect::axes(px(6), px(2)),
                        border_radius: BorderRadius::all(px(4)),
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(hexa(0x05060c, 0.55 * alpha)),
                ))
                .with_children(|row| {
                    if let Some(p) = p {
                        row.spawn(Node {
                            width: px(18),
                            height: px(18),
                            ..default()
                        })
                        .with_child(
                            ChampionPortrait::new(p.champion_id.clone(), PortraitKind::Round)
                                .sized(18.0),
                        );
                    }
                    row.spawn(label(
                        f,
                        if l.team_only { "[Équipe]" } else { "[Tous]" },
                        11.0,
                        700,
                        if l.team_only { BLUE } else { GOLD_HI }.with_alpha(alpha),
                    ));
                    let name = p.map_or("Invocateur".to_string(), |p| p.name.clone());
                    row.spawn(label(f, name, 12.0, 800, color.with_alpha(alpha)));
                    if let Some(c) = p.and_then(|p| get_champion(&p.champion_id)) {
                        row.spawn(label(
                            f,
                            format!("({})", c.name),
                            10.0,
                            500,
                            MUTED.with_alpha(alpha),
                        ));
                    }
                    row.spawn(label(f, l.text.clone(), 12.0, 500, TEXT.with_alpha(alpha)));
                });
            }
        });
    }
    chat.shown = key;
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
                    // après les raccourcis : Échap qui ferme la saisie n'ouvre pas aussi les réglages
                    .after(crate::input::key_orders)
                    .run_if(in_state(Screen::Game)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lignes_recentes_plafonnees_et_historique_a_l_ouverture() {
        let mut chat = ChatUi::default();
        for i in 0..45 {
            chat.push(i as f32, "p", &format!("m{i}"), true);
        }
        assert_eq!(chat.lines.len(), KEEP);
        // à 50 s : seules les lignes de moins de 10 s (41 à 44) restent, et au plus 7
        let v = chat.visible(50.0);
        assert_eq!(
            v.iter().map(|l| l.key).collect::<Vec<_>>(),
            vec![41, 42, 43, 44]
        );
        let v = chat.visible(44.5);
        assert_eq!(v.len(), SHOWN_CLOSED);
        assert_eq!(v.last().map(|l| l.key), Some(44));
        chat.open = true;
        let v = chat.visible(500.0);
        assert_eq!(v.len(), SHOWN_OPEN);
        assert_eq!(v.first().map(|l| l.key), Some(15));
    }

    /// Un message reçu s'affiche même quand l'historique est plein et la saisie ouverte.
    #[test]
    fn un_message_recu_s_affiche_historique_plein() {
        let mut app = App::new();
        app.add_message::<ServerEvent>();
        app.insert_resource(Time::<()>::default());
        app.insert_resource(GameState::default());
        app.insert_resource(UiFonts {
            display: Handle::default(),
            text: Handle::default(),
        });
        let mut chat = ChatUi {
            open: true,
            ..ChatUi::default()
        };
        for i in 0..KEEP {
            chat.push(0.0, "p", &format!("m{i}"), true);
        }
        app.insert_resource(chat);
        app.world_mut().spawn((Node::default(), ChatList));
        app.add_systems(Update, (receive, redraw).chain());
        app.update();
        let before = app.world().resource::<ChatUi>().shown;
        app.world_mut().write_message(ServerEvent(GameEvent::Chat {
            player_id: "q".into(),
            text: "salut".into(),
            team_only: false,
        }));
        app.update();
        let chat = app.world().resource::<ChatUi>();
        assert_eq!(chat.lines.len(), KEEP);
        assert_eq!(chat.lines.last().map(|l| l.text.as_str()), Some("salut"));
        assert_ne!(chat.shown, before, "la nouvelle ligne est redessinée");
    }
}
