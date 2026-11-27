//! Panneaux du HUD : tableau des scores (Tab maintenu), statistiques détaillées (C), réglages (Échap).
//!
//! Portage de `packages/client/src/ui/hud/Panels.tsx`. Les réglages couvrent ce que le client natif
//! sait régler : volumes des canaux, lancement rapide, attaque-déplacement au clic gauche, plein écran ;
//! et la liste des raccourcis.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy::window::{MonitorSelection, WindowMode};
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::{ITEM_LIST, item};
use lov_shared::types::{Stats, Team};

use super::{HUD_Z, label};
use crate::audio::AudioManager;
use crate::audio::mixer::Channel;
use crate::input::PlayerInput;
use crate::net::GameState;
use crate::ui::icons::draw::IconCache;
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::format::{format_gold_short, format_stat, stat_label};
use crate::ui::logic::game::{ScoreRow, sort_score_rows};
use crate::ui::logic::shop::inventory_value;
use crate::ui::theme::{BLUE, GOLD, GOLD_HI, MUTED, RED, TEAL, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, ButtonLook, Screen, UiFonts, button};

#[derive(Component)]
struct ScoreboardPanel;

#[derive(Component)]
struct StatsPanel;

#[derive(Component)]
struct SettingsPanel;

/// Contenu à redessiner, et la clé de ce qui est affiché.
#[derive(Component, Default)]
struct Body(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum SetTab {
    #[default]
    Audio,
    Game,
    Video,
    Keys,
}

#[derive(Resource, Default)]
struct PanelsUi {
    tab: SetTab,
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum Act {
    Tab(SetTab),
    Volume(Channel, i32),
    Mute,
    QuickCast,
    LeftClickAm,
    Fullscreen,
    Close,
}

const CHANNELS: [(Channel, &str); 5] = [
    (Channel::Master, "Général"),
    (Channel::Sfx, "Effets"),
    (Channel::Voice, "Annonces"),
    (Channel::Music, "Musique"),
    (Channel::Ui, "Interface"),
];

const KEYS: [(&str, &str); 20] = [
    ("Clic droit", "Se déplacer / attaquer"),
    ("A + clic", "Attaque-déplacement"),
    (
        "Q W E R",
        "Sorts (maintenir pour viser, relâcher pour lancer)",
    ),
    ("Maj + Q/W/E/R", "Inverser lancement rapide / normal"),
    ("Alt (ou Ctrl) + Q/W/E/R", "Monter un sort"),
    ("D / F", "Sorts d’invocateur"),
    ("1-3, 5-7 / 4", "Objets / balise"),
    ("B", "Rappel"),
    ("S", "Stop"),
    ("P", "Boutique"),
    ("Tab", "Tableau des scores (maintenir)"),
    ("C", "Statistiques"),
    ("² (maintenir)", "Ne cibler que les champions"),
    ("Entrée", "Chat d’équipe (Maj + Entrée : tous)"),
    ("Espace", "Centrer la caméra"),
    ("Y", "Verrouiller la caméra"),
    ("Alt + clic", "Ping"),
    ("Échap", "Fermer / réglages"),
    ("F11", "Plein écran"),
    ("V", "Synchronisation verticale"),
];

const STAT_ORDER: [&str; 14] = [
    "ad",
    "ap",
    "armor",
    "mr",
    "attackSpeed",
    "abilityHaste",
    "critChance",
    "moveSpeed",
    "attackRange",
    "lifesteal",
    "armorPen",
    "magicPen",
    "hpRegen",
    "manaRegen",
];

fn stat_value(s: &Stats, k: &str) -> f64 {
    match k {
        "ad" => s.ad,
        "ap" => s.ap,
        "armor" => s.armor,
        "mr" => s.mr,
        "attackSpeed" => s.attack_speed,
        "abilityHaste" => s.ability_haste,
        "critChance" => s.crit_chance,
        "moveSpeed" => s.move_speed,
        "attackRange" => s.attack_range,
        "lifesteal" => s.lifesteal,
        "armorPen" => s.armor_pen,
        "magicPen" => s.magic_pen,
        "hpRegen" => s.hp_regen,
        _ => s.mana_regen,
    }
}

/// Lignes du tableau des scores (`buildRows`).
fn rows(state: &GameState) -> Vec<ScoreRow> {
    let Some(game) = &state.game else {
        return vec![];
    };
    let roles: Vec<(String, Option<lov_shared::types::Role>)> = state
        .lobby
        .as_ref()
        .map(|l| l.players.iter().map(|p| (p.id.clone(), p.role)).collect())
        .unwrap_or_default();
    game.players
        .iter()
        .map(|p| {
            let snap = state
                .entities()
                .values()
                .find(|e| e.champ.as_ref().is_some_and(|c| c.player_id == p.id));
            let c = snap.and_then(|e| e.champ.as_ref());
            ScoreRow {
                player_id: p.id.clone(),
                name: p.name.clone(),
                team: p.team,
                champion_id: p.champion_id.clone(),
                role: roles
                    .iter()
                    .find(|(id, _)| *id == p.id)
                    .and_then(|(_, r)| *r),
                level: snap.and_then(|e| e.level).unwrap_or(1),
                kills: c.map_or(0, |c| c.kills),
                deaths: c.map_or(0, |c| c.deaths),
                assists: c.map_or(0, |c| c.assists),
                cs: c.map_or(0, |c| c.cs),
                items: c.map(|c| c.items.clone()).unwrap_or_default(),
                summoners: p.summoners,
                dead: snap.is_some_and(|e| e.dead == Some(true)),
                respawn_in: c.and_then(|c| c.respawn_in),
                is_me: p.id == game.you,
            }
        })
        .collect()
}

fn modal_root(marker: impl Bundle) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GlobalZIndex(HUD_Z + 3),
        Pickable::IGNORE,
        Visibility::Hidden,
        DespawnOnExit(Screen::Game),
        marker,
    )
}

fn frame(width: f32) -> impl Bundle {
    (
        Node {
            width: px(width),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            padding: UiRect::all(px(14)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BorderColor::all(hexa(0xd9b25f, 0.55)),
        BackgroundColor(hexa(0x080a16, 0.96)),
        BlocksWorld,
        Body::default(),
    )
}

fn spawn(mut commands: Commands) {
    commands
        .spawn(modal_root(ScoreboardPanel))
        .with_child(frame(860.0));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(8),
                bottom: px(170),
                ..default()
            },
            GlobalZIndex(HUD_Z + 1),
            Pickable::IGNORE,
            Visibility::Hidden,
            DespawnOnExit(Screen::Game),
            StatsPanel,
        ))
        .with_child(frame(230.0));
    commands
        .spawn(modal_root(SettingsPanel))
        .with_child(frame(620.0));
}

fn draw_scoreboard(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    state: &GameState,
) {
    let all = rows(state);
    for team in [Team::Blue, Team::Red] {
        let mut tr: Vec<ScoreRow> = all.iter().filter(|r| r.team == team).cloned().collect();
        sort_score_rows(&mut tr);
        let kills: u32 = tr.iter().map(|r| r.kills).sum();
        // `fold` depuis 0 : la somme d'une équipe vide ne vaut pas « -0 »
        let value = tr
            .iter()
            .fold(0.0, |acc, r| acc + inventory_value(&r.items, &ITEM_LIST));
        let color = if team == Team::Blue { BLUE } else { RED };
        p.spawn(Node {
            column_gap: px(14),
            align_items: AlignItems::Center,
            margin: UiRect::top(px(4)),
            ..default()
        })
        .with_children(|h| {
            h.spawn(label(
                f,
                if team == Team::Blue {
                    "Équipe bleue"
                } else {
                    "Équipe rouge"
                },
                15.0,
                800,
                color,
            ));
            h.spawn(label(f, format!("{kills} éliminations"), 12.0, 600, TEXT));
            h.spawn(label(
                f,
                format!("{} or en objets", format_gold_short(value)),
                12.0,
                600,
                GOLD_HI,
            ));
        });
        for r in &tr {
            p.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::axes(px(8), px(4)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(if r.is_me {
                    hexa(0xd9b25f, 0.14)
                } else {
                    hexa(0xffffff, 0.03)
                }),
            ))
            .with_children(|row| {
                row.spawn(Node {
                    width: px(40),
                    height: px(40),
                    ..default()
                })
                .with_children(|pic| {
                    pic.spawn(
                        ChampionPortrait::new(r.champion_id.clone(), PortraitKind::Round)
                            .gray(r.dead)
                            .sized(40.0),
                    );
                    pic.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            right: px(-4),
                            bottom: px(-4),
                            padding: UiRect::horizontal(px(4)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(hex(0x10142a)),
                        children![label(f, r.level.to_string(), 10.0, 800, GOLD_HI)],
                    ));
                });
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|s| {
                    for id in r.summoners {
                        s.spawn((
                            Node {
                                width: px(18),
                                height: px(18),
                                ..default()
                            },
                            ImageNode::new(icons.summoner(images, id)),
                        ));
                    }
                });
                row.spawn(Node {
                    width: px(170),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|who| {
                    who.spawn(label(f, r.name.clone(), 13.0, 700, TEXT));
                    let champ = get_champion(&r.champion_id)
                        .map_or(r.champion_id.clone(), |c| c.name.clone());
                    let sub = match r.respawn_in.filter(|_| r.dead) {
                        Some(t) => format!("{champ} · réapparaît dans {}", t.ceil()),
                        None => champ,
                    };
                    who.spawn(label(f, sub, 11.0, 500, MUTED));
                });
                row.spawn(Node {
                    column_gap: px(3),
                    ..default()
                })
                .with_children(|items| {
                    for i in 0..7 {
                        let id = r.items.get(i).cloned().flatten();
                        items
                            .spawn((
                                Node {
                                    width: px(28),
                                    height: px(28),
                                    margin: UiRect::left(px(if i == 6 { 6.0 } else { 0.0 })),
                                    border_radius: BorderRadius::all(px(3)),
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                BackgroundColor(hexa(0x000000, 0.4)),
                            ))
                            .with_children(|s| {
                                if let Some(d) = id.as_deref().and_then(item) {
                                    s.spawn((
                                        Node {
                                            width: percent(100),
                                            height: percent(100),
                                            ..default()
                                        },
                                        ImageNode::new(icons.item(images, d)),
                                    ));
                                }
                            });
                    }
                });
                row.spawn(Node {
                    width: px(90),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_child(label(
                    f,
                    format!("{} / {} / {}", r.kills, r.deaths, r.assists),
                    14.0,
                    800,
                    TEXT,
                ));
                row.spawn(label(f, r.cs.to_string(), 13.0, 600, MUTED));
            });
        }
    }
    p.spawn(label(
        f,
        format!("Maintiens Tab · {} min de jeu", (state.time / 60.0).floor()),
        11.0,
        500,
        MUTED,
    ));
}

fn draw_stats(p: &mut ChildSpawnerCommands, f: &UiFonts, state: &GameState) {
    let Some(st) = state.self_state.as_ref() else {
        return;
    };
    p.spawn((
        Text::new("Statistiques"),
        f.display(16.0, 800),
        TextColor(GOLD_HI),
    ));
    for k in STAT_ORDER {
        p.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|r| {
            r.spawn(label(f, stat_label(k).to_string(), 12.0, 500, MUTED));
            r.spawn(label(
                f,
                format_stat(k, stat_value(&st.stats, k), false),
                12.0,
                800,
                TEXT,
            ));
        });
    }
}

fn act_button(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    text: impl Into<String>,
    act: Act,
    on: bool,
) {
    p.spawn((
        button(
            ButtonLook::new(
                if on {
                    hexa(0xd9b25f, 0.3)
                } else {
                    hexa(0xffffff, 0.07)
                },
                hexa(0xffffff, 0.16),
            ),
            Node {
                padding: UiRect::axes(px(12), px(6)),
                border_radius: BorderRadius::all(px(6)),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ),
        act,
    ))
    .with_child(label(f, text, 13.0, 700, if on { GOLD_HI } else { TEXT }));
}

fn setting_row(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    text: &str,
    ctrl: impl FnOnce(&mut ChildSpawnerCommands),
) {
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: px(12),
        padding: UiRect::vertical(px(3)),
        ..default()
    })
    .with_children(|r| {
        r.spawn(label(f, text.to_string(), 13.0, 500, TEXT));
        r.spawn(Node {
            column_gap: px(6),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(ctrl);
    });
}

fn draw_settings(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    ui: &PanelsUi,
    input: &PlayerInput,
    audio: Option<&AudioManager>,
    window: Option<&Window>,
) {
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|h| {
        h.spawn((
            Text::new("Réglages"),
            f.display(20.0, 800),
            TextColor(GOLD_HI),
        ));
        act_button(h, f, "Fermer", Act::Close, false);
    });
    p.spawn(Node {
        column_gap: px(6),
        ..default()
    })
    .with_children(|tabs| {
        for (t, name) in [
            (SetTab::Audio, "Son"),
            (SetTab::Game, "Jeu"),
            (SetTab::Video, "Affichage"),
            (SetTab::Keys, "Raccourcis"),
        ] {
            act_button(tabs, f, name, Act::Tab(t), ui.tab == t);
        }
    });
    p.spawn((
        Node {
            height: px(1),
            ..default()
        },
        BackgroundColor(hexa(0xd9b25f, 0.25)),
    ));
    match ui.tab {
        SetTab::Audio => {
            for (ch, name) in CHANNELS {
                let v = audio.map_or(1.0, |a| a.volume(ch));
                setting_row(p, f, name, |c| {
                    act_button(c, f, "−", Act::Volume(ch, -10), false);
                    c.spawn((
                        Node {
                            width: px(120),
                            height: px(8),
                            border_radius: BorderRadius::all(px(4)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(hexa(0xffffff, 0.1)),
                    ))
                    .with_child((
                        Node {
                            width: percent(v * 100.0),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(TEAL),
                    ));
                    c.spawn(label(
                        f,
                        format!("{}", (v * 100.0).round()),
                        12.0,
                        800,
                        TEXT,
                    ));
                    act_button(c, f, "+", Act::Volume(ch, 10), false);
                });
            }
            let muted = audio.is_some_and(AudioManager::muted);
            setting_row(p, f, "Couper le son", |c| {
                act_button(
                    c,
                    f,
                    if muted { "Coupé" } else { "Actif" },
                    Act::Mute,
                    muted,
                );
            });
        }
        SetTab::Game => {
            let s = input.settings;
            setting_row(
                p,
                f,
                "Lancement rapide des sorts (dès l’appui, sans viser)",
                |c| {
                    act_button(
                        c,
                        f,
                        if s.quick_cast { "Oui" } else { "Non" },
                        Act::QuickCast,
                        s.quick_cast,
                    );
                },
            );
            setting_row(p, f, "Attaque-déplacement au clic gauche", |c| {
                act_button(
                    c,
                    f,
                    if s.left_click_attack_move {
                        "Oui"
                    } else {
                        "Non"
                    },
                    Act::LeftClickAm,
                    s.left_click_attack_move,
                );
            });
        }
        SetTab::Video => {
            let full = window.is_some_and(|w| !matches!(w.mode, WindowMode::Windowed));
            setting_row(p, f, "Plein écran (F11)", |c| {
                act_button(
                    c,
                    f,
                    if full { "Oui" } else { "Non" },
                    Act::Fullscreen,
                    full,
                );
            });
        }
        SetTab::Keys => {
            for (k, v) in KEYS {
                p.spawn(Node {
                    column_gap: px(12),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((
                        Node {
                            width: px(180),
                            ..default()
                        },
                        children![label(f, k, 12.0, 800, GOLD)],
                    ));
                    r.spawn(label(f, v, 12.0, 500, TEXT));
                });
            }
        }
    }
}

/// Racine d'un panneau : visibilité, type, enfants.
type PanelRoot = (
    &'static mut Visibility,
    Has<ScoreboardPanel>,
    Has<StatsPanel>,
    &'static Children,
);
/// Seulement les trois panneaux.
type PanelFilter = Or<(With<ScoreboardPanel>, With<StatsPanel>, With<SettingsPanel>)>;

#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn redraw(
    mut commands: Commands,
    state: Res<GameState>,
    input: Res<PlayerInput>,
    ui: Res<PanelsUi>,
    fonts: Res<UiFonts>,
    audio: Option<Res<AudioManager>>,
    windows: Query<&Window>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    mut roots: Query<PanelRoot, PanelFilter>,
    mut bodies: Query<(Entity, &mut Body)>,
) {
    let window = windows.iter().next();
    for (mut vis, score, stats, children) in &mut roots {
        let open = if score {
            input.scoreboard_open
        } else if stats {
            input.stats_open
        } else {
            input.settings_open
        };
        vis.set_if_neq(if open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !open {
            continue;
        }
        let key = if score {
            let r = rows(&state);
            format!("{:?}|{}", r, (state.time / 60.0).floor())
        } else if stats {
            format!("{:?}", state.self_state.as_ref().map(|s| &s.stats))
        } else {
            let vols: Vec<f32> = CHANNELS
                .iter()
                .map(|(c, _)| audio.as_ref().map_or(1.0, |a| a.volume(*c)))
                .collect();
            format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}",
                ui.tab,
                vols,
                audio.as_ref().map(|a| a.muted()),
                input.settings,
                window.map(|w| w.mode)
            )
        };
        for child in children.iter() {
            let Ok((e, mut body)) = bodies.get_mut(child) else {
                continue;
            };
            if body.0 == key {
                continue;
            }
            body.0.clone_from(&key);
            let mut ec = commands.entity(e);
            ec.despawn_related::<Children>();
            ec.with_children(|p| {
                if score {
                    draw_scoreboard(p, &fonts, &mut icons, &mut images, &state);
                } else if stats {
                    draw_stats(p, &fonts, &state);
                } else {
                    draw_settings(p, &fonts, &ui, &input, audio.as_deref(), window);
                }
            });
        }
    }
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    mut ui: ResMut<PanelsUi>,
    mut input: ResMut<PlayerInput>,
    audio: Option<ResMut<AudioManager>>,
    mut windows: Query<&mut Window>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    match *act {
        Act::Tab(t) => ui.tab = t,
        Act::Volume(ch, d) => {
            if let Some(mut a) = audio {
                let v = (a.volume(ch) + d as f32 / 100.0).clamp(0.0, 1.0);
                a.set_volume(ch, (v * 10.0).round() / 10.0);
            }
        }
        Act::Mute => {
            if let Some(mut a) = audio {
                let m = !a.muted();
                a.set_muted(m);
            }
        }
        Act::QuickCast => input.settings.quick_cast = !input.settings.quick_cast,
        Act::LeftClickAm => {
            input.settings.left_click_attack_move = !input.settings.left_click_attack_move;
        }
        Act::Fullscreen => {
            for mut w in &mut windows {
                w.mode = match w.mode {
                    WindowMode::Windowed => {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                    }
                    _ => WindowMode::Windowed,
                };
            }
        }
        Act::Close => input.settings_open = false,
    }
}

pub struct PanelsPlugin;

impl Plugin for PanelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelsUi>()
            .add_systems(OnEnter(Screen::Game), spawn)
            .add_systems(Update, redraw.run_if(in_state(Screen::Game)))
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valeurs_des_statistiques() {
        let s = Stats {
            max_hp: 1.0,
            hp_regen: 2.0,
            max_mana: 3.0,
            mana_regen: 4.0,
            ad: 5.0,
            ap: 6.0,
            armor: 7.0,
            mr: 8.0,
            attack_speed: 0.9,
            attack_range: 10.0,
            move_speed: 11.0,
            crit_chance: 0.12,
            ability_haste: 13.0,
            lifesteal: 0.14,
            armor_pen: 15.0,
            magic_pen: 16.0,
        };
        assert_eq!(stat_value(&s, "manaRegen"), 4.0);
        assert_eq!(stat_value(&s, "attackRange"), 10.0);
        assert!(rows(&GameState::default()).is_empty());
    }
}
