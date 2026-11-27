//! Surcouches du HUD : score et chrono, KDA du joueur, fil des éliminations, annonces centrales, cadres
//! des alliés, vignette de PV bas, écran de mort.
//!
//! Portage de `packages/client/src/ui/hud/Overlays.tsx` et de la part de `ui/game/world.ts` qui tient le
//! fil et la file des annonces (`FeedTracker`, `trimFeed`, 3,2 s par annonce).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lov_shared::data::champions::get_champion;
use lov_shared::protocol::TeamScore;
use lov_shared::types::{EntityId, EntityKind, Team, TeamOrNeutral};

use super::{hud_root, label, set_shown, set_text};
use crate::input::PlayerInput;
use crate::net::{GameState, ServerEvent};
use crate::ui::icons::draw::IconCache;
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::feed::{
    AnnounceTone, Announcement, FeedEntry, FeedKind, FeedTracker, UnitRef, trim_feed,
};
use crate::ui::logic::format::{dragon_color, dragon_element, format_clock};
use crate::ui::theme::{BLUE, GOLD, GOLD_HI, MUTED, PANEL, RED, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, ButtonLook, Screen, UiFonts, button};

const ANNOUNCE_SECS: f32 = 3.2;
const FEED_MAX: usize = 6;
const FEED_TTL: f64 = 14.0;
const MAX_FRAMES: usize = 4;

/// Fil des éliminations et file des annonces.
#[derive(Resource, Default)]
pub struct HudFeed {
    tracker: Option<FeedTracker>,
    pub feed: Vec<FeedEntry>,
    queue: Vec<Announcement>,
    pub current: Option<Announcement>,
    left: f32,
    /// Le fil ou l'annonce ont changé : à redessiner.
    dirty: bool,
}

fn unit_ref(state: &GameState, id: EntityId) -> UnitRef {
    let Some(e) = state.entities().get(&id) else {
        return UnitRef::unknown(id);
    };
    UnitRef {
        id,
        kind: Some(e.kind),
        team: e.team,
        subtype: e.subtype.clone(),
        name: e.champ.as_ref().map(|c| c.name.clone()),
    }
}

/// Événements → fil et annonces ; avance la file.
fn track(
    mut events: MessageReader<ServerEvent>,
    state: Res<GameState>,
    time: Res<Time>,
    mut feed: ResMut<HudFeed>,
) {
    let Some(team) = state.game.as_ref().map(|g| g.team) else {
        events.clear();
        return;
    };
    let now = state.time;
    let feed = &mut *feed;
    let tracker = feed.tracker.get_or_insert_with(|| FeedTracker::new(team));
    tracker.set_team(team);
    for (id, e) in state.entities() {
        if e.kind == EntityKind::Champion && e.dead != Some(true) {
            tracker.mark_alive(*id);
        }
    }
    let batch: Vec<_> = events.read().map(|ServerEvent(e)| e.clone()).collect();
    if !batch.is_empty() {
        let out = tracker.process(&batch, now, |id| unit_ref(&state, id));
        if !out.feed.is_empty() {
            feed.feed.extend(out.feed);
            feed.dirty = true;
        }
        feed.queue.extend(out.announce);
        feed.queue.sort_by_key(|a| std::cmp::Reverse(a.priority));
        feed.queue.truncate(4);
    }
    let trimmed = trim_feed(&feed.feed, |e| e.time, now, FEED_MAX, FEED_TTL);
    if trimmed.len() != feed.feed.len() {
        feed.feed = trimmed;
        feed.dirty = true;
    }
    feed.left -= time.delta_secs();
    if feed.current.is_none() || feed.left <= 0.0 {
        let next = if feed.queue.is_empty() {
            None
        } else {
            Some(feed.queue.remove(0))
        };
        if next.is_some() || feed.current.is_some() {
            feed.current = next;
            feed.left = ANNOUNCE_SECS;
            feed.dirty = true;
        }
    }
}

fn team_color(t: TeamOrNeutral) -> Color {
    match t {
        TeamOrNeutral::Blue => BLUE,
        TeamOrNeutral::Red => RED,
        TeamOrNeutral::Neutral => hex(0xe8c46a),
    }
}

fn same_team(t: TeamOrNeutral, mine: Team) -> bool {
    matches!(
        (t, mine),
        (TeamOrNeutral::Blue, Team::Blue) | (TeamOrNeutral::Red, Team::Red)
    )
}

// ---------------------------------------------------------------------------------- marqueurs

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum T {
    Clock,
    Kills(Team),
    Towers(Team),
    Grubs(Team),
    Kda,
    Cs,
    DeathName,
    DeathTimer,
    TfLevel(usize),
    TfRespawn(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum V {
    Top,
    Strip,
    Grubs(Team),
    Death,
    Tf(usize),
    TfRespawn(usize),
    LowHp,
}

/// Pastilles des drakes d'une équipe et leur nombre affiché (reconstruites quand il change).
#[derive(Component)]
struct Drakes(Team, usize);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Fill {
    TfHp(usize),
    TfMana(usize),
}

#[derive(Component)]
struct FeedList;

#[derive(Component)]
struct AnnounceBox;

/// Portrait d'un cadre d'allié.
#[derive(Component)]
struct TeamFrame(usize);

/// Bouton des réglages.
#[derive(Component)]
struct Settings;

/// Vignette de PV bas (pulsation).
#[derive(Component)]
struct LowHp;

// ---------------------------------------------------------------------------------- construction

fn glyph_node(
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    name: &str,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Node {
            width: px(size),
            height: px(size),
            ..default()
        },
        ImageNode {
            image: icons.glyph(images, name),
            color,
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn score_side(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    team: Team,
) {
    let blue = team == Team::Blue;
    p.spawn(Node {
        flex_direction: if blue {
            FlexDirection::Row
        } else {
            FlexDirection::RowReverse
        },
        align_items: AlignItems::Center,
        column_gap: px(10),
        padding: UiRect::axes(px(12), px(4)),
        ..default()
    })
    .with_children(|s| {
        s.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(3),
            ..default()
        })
        .with_children(|t| {
            t.spawn(glyph_node(icons, images, "tower", 14.0, MUTED));
            t.spawn((label(f, "0", 13.0, 700, TEXT), T::Towers(team)));
        });
        s.spawn((
            Node {
                column_gap: px(3),
                align_items: AlignItems::Center,
                ..default()
            },
            Drakes(team, usize::MAX),
        ));
        s.spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(3),
                ..default()
            },
            V::Grubs(team),
            Visibility::Hidden,
        ))
        .with_children(|t| {
            t.spawn(glyph_node(icons, images, "grub", 14.0, hex(0xc48bff)));
            t.spawn((label(f, "0", 13.0, 700, TEXT), T::Grubs(team)));
        });
        s.spawn((
            label(f, "0", 22.0, 800, if blue { BLUE } else { RED }),
            T::Kills(team),
        ));
    });
}

fn spawn_team_frames(p: &mut ChildSpawnerCommands, f: &UiFonts) {
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(8),
            top: percent(30),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        for i in 0..MAX_FRAMES {
            col.spawn((
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    ..default()
                },
                V::Tf(i),
                Visibility::Hidden,
            ))
            .with_children(|tf| {
                tf.spawn((
                    Node {
                        width: px(44),
                        height: px(44),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(BLUE),
                ))
                .with_children(|pic| {
                    pic.spawn((
                        Node {
                            width: percent(100),
                            height: percent(100),
                            border_radius: BorderRadius::MAX,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        TeamFrame(i),
                    ));
                    pic.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(-6),
                            right: px(-6),
                            min_width: px(18),
                            height: px(18),
                            border_radius: BorderRadius::MAX,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(hex(0x10142a)),
                        children![(label(f, "1", 10.0, 800, GOLD_HI), T::TfLevel(i))],
                    ));
                    pic.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        V::TfRespawn(i),
                        Visibility::Hidden,
                        children![(label(f, "", 16.0, 800, Color::WHITE), T::TfRespawn(i))],
                    ));
                });
                tf.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    width: px(56),
                    ..default()
                })
                .with_children(|bars| {
                    for (fill, color, h) in [
                        (Fill::TfHp(i), hex(0x3ecf6a), 6.0),
                        (Fill::TfMana(i), hex(0x4f8dff), 4.0),
                    ] {
                        bars.spawn((
                            Node {
                                width: percent(100),
                                height: px(h),
                                border_radius: BorderRadius::all(px(2)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(hexa(0x000000, 0.7)),
                        ))
                        .with_child((
                            Node {
                                width: percent(100),
                                height: percent(100),
                                ..default()
                            },
                            BackgroundColor(color),
                            fill,
                        ));
                    }
                });
            });
        }
    });
}

fn spawn(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let f = &*fonts;
    let images = &mut *images;
    let icons = &mut *icons;
    commands.spawn(hud_root()).with_children(|root| {
        // vignette de PV bas (sous tout le reste)
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundGradient::from(RadialGradient {
                shape: RadialGradientShape::FarthestCorner,
                stops: vec![
                    ColorStop::new(Color::NONE, percent(55)),
                    ColorStop::new(hexa(0xd01818, 0.55), percent(100)),
                ],
                ..default()
            }),
            V::LowHp,
            Visibility::Hidden,
            Pickable::IGNORE,
            LowHp,
        ));
        // score et chrono
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            V::Top,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    border_radius: BorderRadius::bottom(px(8)),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.5)),
                BackgroundColor(PANEL),
                BlocksWorld,
            ))
            .with_children(|bar| {
                score_side(bar, f, icons, images, Team::Blue);
                bar.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(5),
                        padding: UiRect::axes(px(14), px(6)),
                        border: UiRect::horizontal(px(1)),
                        ..default()
                    },
                    BorderColor::all(hexa(0xd9b25f, 0.3)),
                ))
                .with_children(|c| {
                    c.spawn(glyph_node(icons, images, "sword", 12.0, GOLD));
                    c.spawn((label(f, "0:00", 15.0, 800, GOLD_HI), T::Clock));
                });
                score_side(bar, f, icons, images, Team::Red);
            });
        });
        // KDA, CS et réglages, en haut à droite
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(6),
                right: px(8),
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::axes(px(10), px(4)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(PANEL),
            BlocksWorld,
            V::Strip,
        ))
        .with_children(|s| {
            s.spawn(glyph_node(icons, images, "skull", 13.0, MUTED));
            s.spawn((label(f, "0/0/0", 13.0, 700, TEXT), T::Kda));
            s.spawn(glyph_node(icons, images, "minion", 13.0, MUTED));
            s.spawn((label(f, "0", 13.0, 700, TEXT), T::Cs));
            s.spawn((
                button(
                    ButtonLook::new(hexa(0xffffff, 0.06), hexa(0xffffff, 0.16)),
                    Node {
                        width: px(26),
                        height: px(26),
                        border_radius: BorderRadius::all(px(4)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ),
                Settings,
            ))
            .with_child(glyph_node(icons, images, "gear", 16.0, TEXT));
        });
        // fil des éliminations, sous le bandeau de droite
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(46),
                right: px(8),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4),
                ..default()
            },
            Pickable::IGNORE,
            FeedList,
        ));
        // annonce centrale
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(17),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6),
                ..default()
            },
            Pickable::IGNORE,
            AnnounceBox,
        ));
        spawn_team_frames(root, f);
        // écran de mort
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(28),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                ..default()
            },
            V::Death,
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_children(|d| {
            d.spawn((label(f, "", 14.0, 600, MUTED), T::DeathName));
            d.spawn((
                Text::new("Réapparition dans"),
                f.display(26.0, 700),
                TextColor(TEXT),
            ));
            d.spawn((
                Text::new(""),
                f.display(64.0, 800),
                TextColor(GOLD_HI),
                T::DeathTimer,
            ));
            d.spawn(label(
                f,
                "Tu peux acheter en boutique (P) pendant ce temps.",
                13.0,
                500,
                MUTED,
            ));
        });
    });
}

// ---------------------------------------------------------------------------------- mise à jour

fn score(state: &GameState, t: Team) -> Option<&TeamScore> {
    state.score.as_ref().map(|s| match t {
        Team::Blue => &s.blue,
        Team::Red => &s.red,
    })
}

/// Alliés du joueur (sans lui), dans un ordre stable.
fn allies(state: &GameState) -> Vec<EntityId> {
    let Some(team) = state.game.as_ref().map(|g| g.team) else {
        return vec![];
    };
    let me = state.my_entity;
    let mut v: Vec<EntityId> = state
        .entities()
        .values()
        .filter(|e| e.kind == EntityKind::Champion && Some(e.id) != me && same_team(e.team, team))
        .map(|e| e.id)
        .collect();
    v.sort_unstable();
    v.truncate(MAX_FRAMES);
    v
}

fn update_texts(state: Res<GameState>, mut q: Query<(&T, &mut Text)>) {
    let me = state.my_entity.and_then(|id| state.entities().get(&id));
    let allies = allies(&state);
    let ally = |i: usize| allies.get(i).and_then(|id| state.entities().get(id));
    for (t, mut text) in &mut q {
        let s = match *t {
            T::Clock => format_clock(state.time),
            T::Kills(team) => score(&state, team).map_or(0, |s| s.kills).to_string(),
            T::Towers(team) => score(&state, team).map_or(0, |s| s.towers).to_string(),
            T::Grubs(team) => score(&state, team).map_or(0, |s| s.grubs).to_string(),
            T::Kda => me
                .and_then(|e| e.champ.as_ref())
                .map_or("0/0/0".into(), |c| {
                    format!("{}/{}/{}", c.kills, c.deaths, c.assists)
                }),
            T::Cs => me
                .and_then(|e| e.champ.as_ref())
                .map_or(0, |c| c.cs)
                .to_string(),
            T::DeathName => me
                .and_then(|e| e.subtype.as_deref())
                .and_then(get_champion)
                .map_or(String::new(), |c| {
                    format!("{} est tombé·e au combat", c.name)
                }),
            T::DeathTimer => me
                .and_then(|e| e.champ.as_ref())
                .and_then(|c| c.respawn_in)
                .map_or(String::new(), |r| format!("{}", r.ceil())),
            T::TfLevel(i) => ally(i).map_or(String::new(), |e| e.level.unwrap_or(1).to_string()),
            T::TfRespawn(i) => ally(i)
                .and_then(|e| e.champ.as_ref())
                .and_then(|c| c.respawn_in)
                .map_or(String::new(), |r| format!("{}", r.ceil())),
        };
        set_text(&mut text, &s);
    }
}

fn update_shown(
    state: Res<GameState>,
    time: Res<Time>,
    mut q: Query<(&V, &mut Visibility)>,
    mut low: Query<&mut BackgroundGradient, With<LowHp>>,
) {
    let me = state.my_entity.and_then(|id| state.entities().get(&id));
    let dead = me.is_some_and(|e| e.dead == Some(true));
    let ratio = me
        .filter(|e| e.dead != Some(true) && e.max_hp > 0.0)
        .map_or(1.0, |e| e.hp / e.max_hp);
    let allies = allies(&state);
    for (v, mut vis) in &mut q {
        let on = match *v {
            V::Top | V::Strip => state.game.is_some(),
            V::Grubs(t) => score(&state, t).is_some_and(|s| s.grubs > 0),
            V::Death => dead,
            V::Tf(i) => i < allies.len(),
            V::TfRespawn(i) => allies
                .get(i)
                .and_then(|id| state.entities().get(id))
                .is_some_and(|e| e.dead == Some(true)),
            V::LowHp => ratio < 0.3,
        };
        set_shown(&mut vis, on);
    }
    if ratio >= 0.3 {
        return;
    }
    // pulsation, plus forte sous 15 %
    let strong = ratio < 0.15;
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * if strong { 6.0 } else { 3.5 }).sin();
    let a = if strong {
        0.45 + 0.3 * pulse
    } else {
        0.25 + 0.2 * pulse
    };
    for mut g in &mut low {
        if let Some(bevy::ui::Gradient::Radial(r)) = g.0.first_mut()
            && let Some(stop) = r.stops.last_mut()
        {
            stop.color = hexa(0xd01818, a);
        }
    }
}

fn update_frames(
    mut commands: Commands,
    state: Res<GameState>,
    frames: Query<(Entity, &TeamFrame, Option<&ChampionPortrait>)>,
    mut fills: Query<(&Fill, &mut Node)>,
) {
    let allies = allies(&state);
    for (e, tf, current) in &frames {
        let snap = allies.get(tf.0).and_then(|id| state.entities().get(id));
        let want = snap.and_then(|s| s.subtype.clone()).map(|c| {
            ChampionPortrait::new(c, PortraitKind::Round)
                .gray(snap.is_some_and(|s| s.dead == Some(true)))
        });
        if let Some(p) = want
            && current != Some(&p)
        {
            commands.entity(e).insert(p);
        }
    }
    for (fill, mut node) in &mut fills {
        let (i, hp) = match *fill {
            Fill::TfHp(i) => (i, true),
            Fill::TfMana(i) => (i, false),
        };
        let Some(e) = allies.get(i).and_then(|id| state.entities().get(id)) else {
            continue;
        };
        let k = if hp {
            e.hp / e.max_hp.max(1.0)
        } else {
            e.mana.unwrap_or(0.0) / e.max_mana.unwrap_or(1.0).max(1.0)
        };
        let w = percent((k * 100.0) as f32);
        if node.width != w {
            node.width = w;
        }
    }
}

/// Pastilles des drakes, reconstruites quand leur nombre change.
fn update_drakes(
    mut commands: Commands,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut q: Query<(Entity, &mut Drakes)>,
) {
    for (e, mut d) in &mut q {
        let list = score(&state, d.0)
            .map(|s| s.dragons.clone())
            .unwrap_or_default();
        if d.1 == list.len() {
            continue;
        }
        d.1 = list.len();
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|row| {
            if list.is_empty() {
                row.spawn(label(&fonts, "—", 12.0, 600, MUTED));
            }
            for el in &list {
                let c = dragon_element(el).map_or(hex(0xaaaaaa), |e| hex(dragon_color(e)));
                row.spawn((
                    Node {
                        width: px(10),
                        height: px(10),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(c),
                ));
            }
            if list.len() >= 4 {
                let c = list
                    .get(2)
                    .or(list.first())
                    .and_then(|e| dragon_element(e))
                    .map_or(Color::WHITE, |e| hex(dragon_color(e)));
                row.spawn(label(&fonts, "Âme", 11.0, 800, c));
            }
        });
    }
}

fn unit_badge(
    p: &mut ChildSpawnerCommands,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    u: Option<&UnitRef>,
    tint: TeamOrNeutral,
    size: f32,
) {
    let team = u.map_or(TeamOrNeutral::Neutral, |u| {
        if u.team == TeamOrNeutral::Neutral {
            tint
        } else {
            u.team
        }
    });
    let mut node = p.spawn((
        Node {
            width: px(size),
            height: px(size),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(team_color(team)),
        BackgroundColor(hexa(0x05060c, 0.9)),
    ));
    match u {
        Some(u) if u.kind == Some(EntityKind::Champion) && u.subtype.is_some() => {
            node.with_child(
                ChampionPortrait::new(u.subtype.clone().unwrap_or_default(), PortraitKind::Round)
                    .with(Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    }),
            );
        }
        _ => {
            let name = match u.and_then(|u| u.kind) {
                Some(EntityKind::Tower) => "tower",
                Some(EntityKind::Inhibitor) => "inhibitor",
                Some(EntityKind::Nexus) => "nexus",
                Some(EntityKind::Monster) => match u.and_then(|u| u.subtype.as_deref()) {
                    Some("dragon" | "elder_dragon") => "dragon",
                    Some("leviathan") => "leviathan",
                    Some("grub") => "grub",
                    Some("herald") => "herald",
                    _ => "skull",
                },
                _ => "minion",
            };
            node.with_child(glyph_node(icons, images, name, size * 0.6, TEXT));
        }
    }
}

fn feed_row(
    row: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    entry: &FeedEntry,
) {
    match entry.kind {
        FeedKind::Objective => {
            unit_badge(row, icons, images, entry.killer.as_ref(), entry.team, 30.0);
            row.spawn(label(f, "⟶", 13.0, 700, TEXT));
            let c = entry.element.map_or(GOLD_HI, |e| hex(dragon_color(e)));
            row.spawn(label(
                f,
                entry.label.clone().unwrap_or_default(),
                12.0,
                700,
                c,
            ));
        }
        FeedKind::Structure => {
            let text = format!("{} détruite", entry.label.clone().unwrap_or_default());
            row.spawn(label(f, text, 12.0, 700, TEXT));
            unit_badge(row, icons, images, entry.victim.as_ref(), entry.team, 30.0);
        }
        FeedKind::Kill | FeedKind::Execute => {
            unit_badge(row, icons, images, entry.killer.as_ref(), entry.team, 30.0);
            for a in entry.assists.iter().take(3) {
                unit_badge(row, icons, images, Some(a), entry.team, 18.0);
            }
            row.spawn(glyph_node(icons, images, "sword", 14.0, TEXT));
            unit_badge(row, icons, images, entry.victim.as_ref(), entry.team, 30.0);
        }
    }
}

fn announce(
    b: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    a: &Announcement,
) {
    let color = match a.tone {
        AnnounceTone::Ally => hex(0x8cc8ff),
        AnnounceTone::Enemy => hex(0xff8a7a),
        AnnounceTone::Neutral => TEXT,
        AnnounceTone::Gold => GOLD_HI,
    };
    let color = a.element.map_or(color, |e| hex(dragon_color(e)));
    let champ = |u: &Option<UnitRef>| {
        u.as_ref()
            .filter(|u| u.kind == Some(EntityKind::Champion))
            .and_then(|u| u.subtype.clone())
    };
    let (k, v) = (champ(&a.killer), champ(&a.victim));
    if k.is_some() || v.is_some() {
        b.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: px(10),
            ..default()
        })
        .with_children(|faces| {
            if let Some(k) = k {
                faces.spawn(ChampionPortrait::new(k, PortraitKind::Round).sized(56.0));
            }
            if let Some(v) = v {
                faces.spawn(glyph_node(icons, images, "sword", 20.0, TEXT));
                faces.spawn(
                    ChampionPortrait::new(v, PortraitKind::Round)
                        .gray(true)
                        .sized(56.0),
                );
            }
        });
    }
    b.spawn((
        Text::new(a.title.clone()),
        f.display(30.0, 800),
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.9),
        },
    ));
    if let Some(sub) = &a.subtitle {
        b.spawn(label(f, sub.clone(), 15.0, 600, TEXT));
    }
}

/// Fil et annonce, redessinés quand ils changent.
#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn redraw_feed(
    mut commands: Commands,
    mut feed: ResMut<HudFeed>,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    list: Query<Entity, With<FeedList>>,
    boxes: Query<Entity, With<AnnounceBox>>,
) {
    if !feed.dirty {
        return;
    }
    feed.dirty = false;
    let mine = state.game.as_ref().map_or(Team::Blue, |g| g.team);
    let f = &*fonts;
    let icons = &mut *icons;
    let images = &mut *images;
    for e in &list {
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|col| {
            for entry in &feed.feed {
                let tone = match entry.team {
                    TeamOrNeutral::Neutral => hexa(0x3a3420, 0.85),
                    t if same_team(t, mine) => hexa(0x12305a, 0.85),
                    _ => hexa(0x5a1818, 0.85),
                };
                col.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(6),
                        padding: UiRect::axes(px(6), px(3)),
                        border_radius: BorderRadius::all(px(16)),
                        ..default()
                    },
                    BackgroundColor(tone),
                ))
                .with_children(|row| feed_row(row, f, icons, images, entry));
            }
        });
    }
    for e in &boxes {
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        if let Some(a) = &feed.current {
            ec.with_children(|b| announce(b, f, icons, images, a));
        }
    }
}

fn on_settings(ev: On<Activate>, q: Query<(), With<Settings>>, mut input: ResMut<PlayerInput>) {
    if q.get(ev.entity).is_ok() {
        input.settings_open = !input.settings_open;
    }
}

fn reset(mut feed: ResMut<HudFeed>) {
    *feed = HudFeed::default();
}

pub struct OverlaysPlugin;

impl Plugin for OverlaysPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudFeed>()
            .add_systems(OnEnter(Screen::Game), (reset, spawn))
            .add_systems(
                Update,
                (
                    track,
                    redraw_feed,
                    update_texts,
                    update_shown,
                    update_frames,
                    update_drakes,
                )
                    .chain()
                    .run_if(in_state(Screen::Game)),
            )
            .add_observer(on_settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_file_des_annonces_avance() {
        let mut feed = HudFeed::default();
        feed.queue.push(Announcement {
            key: 1,
            title: "Premier sang".into(),
            subtitle: None,
            tone: AnnounceTone::Gold,
            killer: None,
            victim: None,
            priority: 5,
            element: None,
        });
        assert!(feed.current.is_none());
        assert_eq!(team_color(TeamOrNeutral::Blue), BLUE);
        assert!(same_team(TeamOrNeutral::Red, Team::Red));
        assert!(!same_team(TeamOrNeutral::Neutral, Team::Red));
    }
}
