//! Accueil : portage de `packages/client/src/ui/screens/Home.tsx`.
//!
//! À gauche : logo, carte de jeu (onglets Créer / Rejoindre, nom d'invocateur, code de salle à 5
//! caractères, mode entraînement). À droite : vitrine du champion mis en avant (change toutes les 7 s,
//! en pause au survol) avec la scène 3D du champion (`stage`), et galerie.
//!
//! Écarts : le nom n'est pas encore mémorisé d'une session à l'autre (`localStorage` du web) ; il
//! reprend `LOV_NAME` s'il est défini. Le pied de page compte les champions au lieu du « 11 » figé du web.

use std::f32::consts::FRAC_PI_2;

use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{EditableText, EditableTextFilter, TextEdit};
use bevy::ui_widgets::{Activate, TextInput};
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::protocol::ClientMsg;

use super::rift::RiftBackground;
use super::screen_root;
use super::stage::{ChampionStage, StageView};
use crate::net::{Connection, GameState, now_ms};
use crate::ui::icons::{Artworks, ChampionPortrait, PortraitKind};
use crate::ui::logic::format::{class_label, role_label};
use crate::ui::theme::{GOLD, GOLD_HI, GOLD_LO, MUTED, PANEL, TEXT, css, hexa};
use crate::ui::{ButtonLook, Screen, UiFonts, button, set_enabled};

/// Durée d'affichage d'un champion de la vitrine (s).
const FEATURE_SECONDS: f32 = 7.0;
/// Les boutons de jeu restent désactivés le temps que le serveur réponde (ms).
const BUSY_MS: f64 = 2500.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Create,
    Join,
}

/// État local de l'écran (les `useState` du web).
#[derive(Resource)]
pub struct HomeUi {
    pub mode: Mode,
    pub dev: bool,
    pub featured: usize,
    /// Incrémenté quand le joueur choisit un champion : il joue son ultime.
    pub pulse: u32,
    since: f32,
    busy_until: f64,
    stage: Option<Entity>,
}

impl Default for HomeUi {
    fn default() -> Self {
        let n = CHAMPIONS.len().max(1);
        Self {
            mode: Mode::Create,
            dev: std::env::var_os("LOV_DEV").is_some(),
            featured: (now_ms() as usize / 7) % n,
            pulse: 0,
            since: 0.0,
            busy_until: 0.0,
            stage: None,
        }
    }
}

/// Code de salle normalisé : majuscules, lettres et chiffres, 5 caractères au plus.
pub fn normalize_code(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .take(5)
        .collect()
}

/// Nom envoyé au serveur : saisi, ou « Invocateur ».
pub fn player_name(raw: &str) -> String {
    let n = raw.trim();
    if n.is_empty() {
        "Invocateur".into()
    } else {
        n.into()
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Act {
    Tab(Mode),
    Create,
    Join,
    Dev,
    Prev,
    Next,
    Pick(usize),
}

#[derive(Component)]
struct NameInput;
#[derive(Component)]
struct CodeInput;
#[derive(Component)]
struct Avatar;
/// Texte indicatif du champ de nom, affiché quand il est vide (`placeholder`).
#[derive(Component)]
struct NameHint;
#[derive(Component)]
struct ModePanel(Mode);
#[derive(Component)]
struct DevBox;
#[derive(Component)]
struct JoinHint;
#[derive(Component)]
struct ErrorText;
#[derive(Component)]
struct ConnText;
#[derive(Component)]
struct Showcase;
#[derive(Component, Clone, Copy)]
enum Plate {
    Class,
    Name,
    Title,
    Roles,
}
#[derive(Component)]
struct Glow;
#[derive(Component)]
struct PlateBox;
#[derive(Component)]
struct StageArt;
#[derive(Component)]
struct Chip(usize);

fn on_enter(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut ui: ResMut<HomeUi>,
    mut images: ResMut<Assets<Image>>,
) {
    ui.since = 0.0;
    let stage = ChampionStage::spawn(&mut commands, &mut images, Screen::Home, 0.3);
    ui.stage = Some(stage);
    let f = &*fonts;
    let gold_button = ButtonLook::new(GOLD, GOLD_HI);
    let tab_look = ButtonLook::new(hexa(0xffffff, 0.03), hexa(0xffffff, 0.08));
    let name = std::env::var("LOV_NAME").unwrap_or_default();
    commands
        .spawn(
            screen_root(
                Screen::Home,
                Node {
                flex_direction: FlexDirection::Column,
                ..default()
            }),
        )
        .with_children(|root| {
            root.spawn(RiftBackground::new(css(&CHAMPIONS[ui.featured % CHAMPIONS.len()].palette.primary), 1.0).bundle());
            root.spawn(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|row| {
                // ---------------------------------------------------------------- gauche
                row.spawn(Node {
                    width: percent(36),
                    min_width: px(380),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    row_gap: vh(4),
                    padding: UiRect::new(vw(5), px(0), vh(5), vh(4)),
                    ..default()
                })
                .with_children(|left| {
                    left.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|logo| {
                        logo.spawn((Text::new("LEAGUE OF"), f.display(20.0, 700), TextColor(MUTED)));
                        logo.spawn((Text::new("VIBES"), f.display(118.0, 900), TextColor(GOLD_HI)));
                        logo.spawn((
                            Text::new("◆  La Faille des Vibes · 4 contre 4  ◆"),
                            f.text(15.0, 600),
                            TextColor(GOLD),
                        ));
                    });
                    // carte de jeu
                    left.spawn((
                        Node {
                            width: px(440),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(16),
                            padding: UiRect::new(px(24), px(24), px(22), px(18)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(14)),
                            ..default()
                        },
                        BackgroundColor(PANEL),
                        BorderColor::all(hexa(0xf6dc94, 0.22)),
                    ))
                    .with_children(|card| {
                        card.spawn(Node {
                            column_gap: px(4),
                            ..default()
                        })
                        .with_children(|tabs| {
                            for (mode, label) in [(Mode::Create, "Créer une partie"), (Mode::Join, "Rejoindre")] {
                                tabs.spawn((
                                    button(
                                        tab_look,
                                        Node {
                                            flex_grow: 1.0,
                                            justify_content: JustifyContent::Center,
                                            padding: UiRect::axes(px(8), px(10)),
                                            border: UiRect::bottom(px(2)),
                                            ..default()
                                        },
                                    ),
                                    BorderColor::all(Color::NONE),
                                    Act::Tab(mode),
                                ))
                                .with_child((Text::new(label), f.text(15.0, 700), TextColor(TEXT)));
                            }
                        });
                        // nom d'invocateur
                        card.spawn((
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: px(12),
                                padding: UiRect::all(px(8)),
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(10)),
                                ..default()
                            },
                            BackgroundColor(hexa(0x000000, 0.35)),
                            BorderColor::all(hexa(0xd9b25f, 0.3)),
                        ))
                        .with_children(|field| {
                            field
                                .spawn((
                                    Node {
                                        width: px(42),
                                        height: px(42),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(px(2)),
                                        border_radius: BorderRadius::MAX,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb_u8(0x1e, 0x22, 0x3f)),
                                    BorderColor::all(GOLD_LO),
                                ))
                                .with_child((Avatar, Text::new("?"), f.display(22.0, 800), TextColor(GOLD_HI)));
                            let mut text = EditableText::new(&name);
                            text.max_characters = Some(16);
                            field.spawn((
                                NameHint,
                                Pickable::IGNORE,
                                Text::new("Ton nom d’invocateur"),
                                f.text(18.0, 500),
                                TextColor(hexa(0xece7da, 0.35)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(62),
                                    ..default()
                                },
                            ));
                            field.spawn((
                                NameInput,
                                TextInput,
                                text,
                                f.text(18.0, 500),
                                TextColor(TEXT),
                                Node {
                                    flex_grow: 1.0,
                                    height: px(28),
                                    ..default()
                                },
                            ));
                        });
                        // créer
                        card.spawn((
                            ModePanel(Mode::Create),
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(12),
                                ..default()
                            },
                        ))
                        .with_children(|panel| {
                            cta(panel, f, gold_button, "JOUER", "Créer une salle et inviter tes potes", Act::Create, None);
                            panel
                                .spawn((
                                    button(ButtonLook::new(Color::NONE, hexa(0xffffff, 0.04)), Node {
                                        padding: UiRect::all(px(4)),
                                        ..default()
                                    }),
                                    Act::Dev,
                                ))
                                .with_child((DevBox, Text::new(""), f.text(14.0, 500), TextColor(MUTED)));
                        });
                        // rejoindre
                        card.spawn((
                            ModePanel(Mode::Join),
                            Node {
                                display: Display::None,
                                flex_direction: FlexDirection::Column,
                                row_gap: px(12),
                                ..default()
                            },
                        ))
                        .with_children(|panel| {
                            let mut code = EditableText::new("");
                            code.max_characters = Some(5);
                            panel.spawn((
                                CodeInput,
                                TextInput,
                                code,
                                EditableTextFilter::new(|c| c.is_ascii_alphanumeric()),
                                f.display(34.0, 700),
                                TextColor(GOLD_HI),
                                Node {
                                    height: px(52),
                                    padding: UiRect::axes(px(16), px(6)),
                                    border: UiRect::all(px(1)),
                                    border_radius: BorderRadius::all(px(10)),
                                    ..default()
                                },
                                BackgroundColor(hexa(0x000000, 0.35)),
                                BorderColor::all(hexa(0xd9b25f, 0.3)),
                            ));
                            cta(panel, f, gold_button, "REJOINDRE", "", Act::Join, Some(JoinHint));
                        });
                        card.spawn((ErrorText, Text::new(""), f.text(14.0, 600), TextColor(crate::ui::theme::DANGER)));
                        card.spawn((ConnText, Text::new(""), f.text(12.0, 600), TextColor(MUTED)));
                    });
                });
                // ---------------------------------------------------------------- droite : vitrine
                row.spawn((
                    Showcase,
                    Hovered::default(),
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: px(24),
                        padding: UiRect::new(vw(2), vw(4), vh(3), px(0)),
                        ..default()
                    },
                ))
                .with_children(|right| {
                    right
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(28),
                            ..default()
                        })
                        .with_children(|show| {
                            arrow(show, f, "‹", Act::Prev);
                            show.spawn((
                                Glow,
                                Node {
                                    width: px(640),
                                    height: px(600),
                                    ..default()
                                },
                                BackgroundGradient::default(),
                            ))
                            .with_children(|glow| {
                                // illustration peinte en fond d'ambiance derrière la scène
                                glow.spawn((
                                    StageArt,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: percent(-8),
                                        height: percent(100),
                                        aspect_ratio: Some(1.0),
                                        ..default()
                                    },
                                    ImageNode::default().with_color(hexa(0xffffff, 0.28)),
                                    Pickable::IGNORE,
                                ));
                                glow.spawn((
                                    StageView(stage),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        width: percent(100),
                                        height: percent(100),
                                        ..default()
                                    },
                                ));
                                // plaque du champion, en bas à droite de la scène
                                glow.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: px(0),
                                        bottom: px(30),
                                        min_width: px(240),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(4),
                                        padding: UiRect::new(px(20), px(22), px(16), px(16)),
                                        border: UiRect::left(px(3)),
                                        border_radius: BorderRadius::right(px(10)),
                                        ..default()
                                    },
                                    BackgroundGradient::from(LinearGradient {
                                        angle: FRAC_PI_2,
                                        stops: vec![
                                            ColorStop::new(hexa(0x05060e, 0.85), percent(0)),
                                            ColorStop::new(hexa(0x05060e, 0.3), percent(100)),
                                        ],
                                        ..default()
                                    }),
                                    BorderColor::all(GOLD),
                                    PlateBox,
                                    Pickable::IGNORE,
                                ))
                                .with_children(|plate| {
                                    plate.spawn((Plate::Class, Text::new(""), f.text(12.0, 700), TextColor(GOLD)));
                                    plate.spawn((Plate::Name, Text::new(""), f.display(48.0, 900), TextColor(TEXT)));
                                    plate.spawn((Plate::Title, Text::new(""), f.text(16.0, 500), TextColor(MUTED)));
                                    plate.spawn((Plate::Roles, Text::new(""), f.text(13.0, 600), TextColor(GOLD_HI)));
                                });
                            });
                            arrow(show, f, "›", Act::Next);
                        });
                    right
                        .spawn(Node {
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            column_gap: px(10),
                            row_gap: px(10),
                            max_width: px(760),
                            ..default()
                        })
                        .with_children(|roster| {
                            for (i, c) in CHAMPIONS.iter().enumerate() {
                                let color = css(&c.palette.primary);
                                roster
                                    .spawn((
                                        button(ButtonLook::new(color.with_alpha(0.55), color), Node {
                                            width: px(48),
                                            height: px(48),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(px(2)),
                                            border_radius: BorderRadius::MAX,
                                            ..default()
                                        }),
                                        BorderColor::all(hexa(0xffffff, 0.15)),
                                        Chip(i),
                                        Act::Pick(i),
                                    ))
                                    .with_child(ChampionPortrait::new(&c.id, PortraitKind::Round).sized(44.0));
                            }
                        });
                });
            });
            // ---------------------------------------------------------------- pied de page
            root.spawn((
                Node {
                    justify_content: JustifyContent::Center,
                    column_gap: px(18),
                    padding: UiRect::all(px(12)),
                    border: UiRect::top(px(1)),
                    ..default()
                },
                BorderColor::all(hexa(0xffffff, 0.05)),
            ))
            .with_child((
                Text::new(format!(
                    "{} CHAMPIONS  ◆  3 VOIES  ◆  DRAKES · COLOSSE · LÉVIATHAN  ◆  FAIT AUX VIBES, CE SOIR",
                    CHAMPIONS.len()
                )),
                f.text(12.0, 600),
                TextColor(hexa(0xece7da, 0.45)),
            ));
        });
}

/// Grand bouton doré (`.lov-cta`) : libellé en capitales, sous-titre.
fn cta(
    parent: &mut ChildSpawnerCommands,
    f: &UiFonts,
    look: ButtonLook,
    label: &str,
    sub: &str,
    act: Act,
    hint: Option<JoinHint>,
) {
    parent
        .spawn((
            button(
                look,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(2),
                    padding: UiRect::new(px(20), px(20), px(16), px(13)),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
            ),
            act,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                f.display(26.0, 900),
                TextColor(Color::srgb_u8(0x1a, 0x10, 0x02)),
            ));
            let mut sub = b.spawn((
                Text::new(sub),
                f.text(12.0, 700),
                TextColor(hexa(0x281802, 0.8)),
            ));
            if let Some(h) = hint {
                sub.insert(h);
            }
        });
}

fn arrow(parent: &mut ChildSpawnerCommands, f: &UiFonts, label: &str, act: Act) {
    parent
        .spawn((
            button(
                ButtonLook::new(hexa(0xffffff, 0.04), hexa(0xffffff, 0.12)),
                Node {
                    width: px(52),
                    height: px(52),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
            ),
            BorderColor::all(hexa(0xf6dc94, 0.3)),
            act,
        ))
        .with_child((Text::new(label), f.text(30.0, 400), TextColor(GOLD_HI)));
}

/// Lance la création ou la jonction : connexion sous le nom saisi, puis le message.
fn go(conn: &Connection, state: &mut GameState, ui: &mut HomeUi, name: &str, msg: ClientMsg) {
    state.error = None;
    conn.connect(&player_name(name));
    conn.send(msg);
    ui.busy_until = now_ms() + BUSY_MS;
}

/// Champs de saisie de l'accueil : texte, est-ce le nom, est-ce le code.
type Fields<'w, 's> = Query<'w, 's, (&'static EditableText, Has<NameInput>, Has<CodeInput>)>;

/// Texte du champ de nom (`name`) ou de code.
fn field(fields: &Fields, name: bool) -> String {
    fields
        .iter()
        .find(|(_, n, c)| if name { *n } else { *c })
        .map(|(t, _, _)| t.value().to_string())
        .unwrap_or_default()
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    fields: Fields,
    conn: Res<Connection>,
    mut state: ResMut<GameState>,
    mut ui: ResMut<HomeUi>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    let n = CHAMPIONS.len();
    let name_text = field(&fields, true);
    match *act {
        Act::Tab(mode) => ui.mode = mode,
        Act::Dev => ui.dev = !ui.dev,
        Act::Create => {
            let dev = ui.dev;
            go(
                &conn,
                &mut state,
                &mut ui,
                &name_text,
                ClientMsg::CreateRoom {
                    dev_mode: Some(dev),
                },
            );
        }
        Act::Join => {
            let c = normalize_code(&field(&fields, false));
            if c.len() == 5 {
                go(
                    &conn,
                    &mut state,
                    &mut ui,
                    &name_text,
                    ClientMsg::JoinRoom { code: c },
                );
            }
        }
        Act::Prev | Act::Next | Act::Pick(_) => {
            ui.featured = match *act {
                Act::Prev => (ui.featured + n - 1) % n,
                Act::Next => (ui.featured + 1) % n,
                Act::Pick(i) => i,
                _ => ui.featured,
            };
            ui.since = 0.0;
            ui.pulse += 1;
        }
    }
}

/// Champion de la vitrine et éclat d'ultime sur la scène ; plaque et fond à sa couleur.
fn drive_stage(
    ui: Res<HomeUi>,
    mut stages: Query<&mut ChampionStage>,
    mut plates: Query<&mut BorderColor, With<PlateBox>>,
    mut backgrounds: Query<&mut RiftBackground>,
    mut arts: Query<&mut ImageNode, With<StageArt>>,
    artworks: Res<Artworks>,
) {
    let Some(champ) = CHAMPIONS.get(ui.featured % CHAMPIONS.len().max(1)) else {
        return;
    };
    if let Some(mut stage) = ui.stage.and_then(|e| stages.get_mut(e).ok()) {
        if stage.champion.as_deref() != Some(champ.id.as_str()) {
            stage.champion = Some(champ.id.clone());
        }
        if stage.pulse != ui.pulse {
            stage.pulse = ui.pulse;
        }
    }
    let art = artworks.faded(&champ.id).unwrap_or_default();
    for mut node in &mut arts {
        if node.image != art {
            node.image = art.clone();
        }
    }
    for mut bg in &mut backgrounds {
        let tint = css(&champ.palette.primary);
        if bg.tint != tint {
            bg.tint = tint;
        }
    }
    for mut b in &mut plates {
        let c = BorderColor::all(css(&champ.palette.primary));
        if *b != c {
            *b = c;
        }
    }
}

/// Filtre des champs de saisie (nom ou code).
type InputField = Or<(With<NameInput>, With<CodeInput>)>;

/// Textes mis à jour chaque image : vitrine, initiale, case, indications, erreur, connexion.
type Labels = (
    &'static mut Text,
    Option<&'static Plate>,
    Has<Avatar>,
    Has<DevBox>,
    Has<JoinHint>,
    Has<ErrorText>,
    Has<ConnText>,
);

/// Entrée dans un champ : créer ou rejoindre, selon l'onglet.
fn submit_on_enter(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    fields: Query<(), InputField>,
    buttons: Query<(Entity, &Act)>,
    ui: Res<HomeUi>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Enter) || !focus.get().is_some_and(|e| fields.contains(e)) {
        return;
    }
    let wanted = if ui.mode == Mode::Create {
        Act::Create
    } else {
        Act::Join
    };
    if let Some((e, _)) = buttons.iter().find(|(_, a)| **a == wanted) {
        commands.trigger(Activate { entity: e });
    }
}

#[allow(clippy::too_many_arguments)] // un système Bevy lit autant de requêtes que l'écran a de zones
fn refresh(
    time: Res<Time>,
    state: Res<GameState>,
    mut ui: ResMut<HomeUi>,
    mut commands: Commands,
    showcase: Query<&Hovered, With<Showcase>>,
    mut code: Query<&mut EditableText, With<CodeInput>>,
    name: Query<&EditableText, (With<NameInput>, Without<CodeInput>)>,
    mut nodes: Query<(&ModePanel, &mut Node)>,
    mut tabs: Query<(&Act, &mut BorderColor), Without<Chip>>,
    mut chips: Query<(&Chip, &mut BorderColor), Without<Act>>,
    buttons: Query<(Entity, &Act)>,
    mut glow: Query<&mut BackgroundGradient, With<Glow>>,
    mut hints: Query<&mut Visibility, With<NameHint>>,
    mut texts: Query<Labels>,
) {
    let n = CHAMPIONS.len();
    if !showcase.iter().any(|h| h.0) {
        ui.since += time.delta_secs();
        if ui.since >= FEATURE_SECONDS {
            ui.since = 0.0;
            ui.featured = (ui.featured + 1) % n;
        }
    }
    // code en majuscules, comme le champ du web
    let mut code_value = String::new();
    if let Ok(mut field) = code.single_mut() {
        let raw = field.value().to_string();
        code_value = normalize_code(&raw);
        if code_value != raw {
            field.queue_edit(TextEdit::SelectAll);
            field.queue_edit(TextEdit::Insert(code_value.clone().into()));
        }
    }
    let name_value = name
        .single()
        .map(|t| t.value().to_string())
        .unwrap_or_default();
    for mut v in &mut hints {
        let want = if name_value.is_empty() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for (panel, mut node) in &mut nodes {
        let display = if panel.0 == ui.mode {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (act, mut border) in &mut tabs {
        if let Act::Tab(mode) = act {
            *border = BorderColor::all(if *mode == ui.mode { GOLD } else { Color::NONE });
        }
    }
    let champ = CHAMPIONS[ui.featured % n];
    let color = css(&champ.palette.primary);
    for (chip, mut border) in &mut chips {
        *border = BorderColor::all(if chip.0 == ui.featured {
            GOLD_HI
        } else {
            hexa(0xffffff, 0.15)
        });
    }
    if let Ok(mut g) = glow.single_mut() {
        *g = BackgroundGradient::from(RadialGradient {
            stops: vec![
                ColorStop::new(color.with_alpha(0.35), percent(0)),
                ColorStop::new(color.with_alpha(0.0), percent(70)),
            ],
            shape: RadialGradientShape::ClosestSide,
            ..default()
        });
    }
    let busy = now_ms() < ui.busy_until;
    for (e, act) in &buttons {
        match act {
            Act::Create => set_enabled(&mut commands, e, !busy),
            Act::Join => set_enabled(&mut commands, e, !busy && code_value.len() == 5),
            _ => {}
        }
    }
    let roles: Vec<&str> = champ.roles.iter().map(|r| role_label(*r)).collect();
    for (mut text, plate, avatar, dev, hint, error, conn) in &mut texts {
        let value = match plate {
            Some(Plate::Class) => class_label(champ.class).to_uppercase(),
            Some(Plate::Name) => champ.name.clone(),
            Some(Plate::Title) => champ.title.clone(),
            Some(Plate::Roles) => roles.join("  ·  "),
            None if avatar => name_value
                .trim()
                .chars()
                .next()
                .map_or("?".into(), |c| c.to_uppercase().collect()),
            None if dev => format!(
                "{}  Mode entraînement (moins de 8 joueurs)",
                if ui.dev { "■" } else { "□" }
            ),
            None if hint => {
                if code_value.len() == 5 {
                    format!("Salle {code_value}")
                } else {
                    "Entre le code à 5 caractères".into()
                }
            }
            None if error => state.error.clone().unwrap_or_default(),
            None if conn => if state.connected {
                "● Connecté au serveur"
            } else {
                "○ Prêt à se connecter"
            }
            .into(),
            None => continue,
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

pub struct HomePlugin;

impl Plugin for HomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HomeUi>()
            .add_systems(OnEnter(Screen::Home), on_enter)
            .add_systems(
                Update,
                (refresh, submit_on_enter, drive_stage).run_if(in_state(Screen::Home)),
            )
            .add_observer(on_activate)
            .add_observer(focus_on_press);
    }
}

/// Cliquer dans un champ (nom, code de salle) lui donne le focus clavier : sans ça, `TextInput` ne
/// reçoit aucune frappe (`DefaultPlugins` n'installe pas `PointerFocusPlugin`, et nos champs n'ont pas de
/// `TabIndex`). Le clic remonte la hiérarchie : un clic sur un enfant du champ compte aussi.
fn focus_on_press(
    ev: On<PointerPress>,
    fields: Query<(), InputField>,
    mut focus: ResMut<InputFocus>,
) {
    let e = ev.event_target();
    if fields.contains(e) && focus.get() != Some(e) {
        focus.set(e, bevy::input_focus::FocusCause::Navigated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_de_salle_en_majuscules_sur_5_caracteres() {
        assert_eq!(normalize_code("ab-c1d9z"), "ABC1D");
        assert_eq!(normalize_code("x y"), "XY");
    }

    #[test]
    fn nom_par_defaut() {
        assert_eq!(player_name("   "), "Invocateur");
        assert_eq!(player_name(" Lyra "), "Lyra");
    }
}
