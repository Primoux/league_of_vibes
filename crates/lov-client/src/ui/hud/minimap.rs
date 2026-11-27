//! Minicarte : fond pré-rendu (terrain, rivière, fosses, voies, bases, brousses, murs) et, par-dessus,
//! les unités (structures, sbires, monstres, balises, champions en portraits), l'emprise de la caméra, les
//! délais de réapparition des camps et les pings. Liste des objectifs épiques à venir au-dessus.
//! Clic gauche ou glisser : déplacer la caméra ; clic droit : se déplacer ; Alt + clic : ping.
//!
//! Portage de `packages/client/src/ui/hud/Minimap.tsx`. Le web redessine un canvas 2D ; ici le fond est
//! une image dessinée une fois sur le processeur, et les marqueurs sont des nœuds d'interface recyclés
//! (aucune création pendant la partie).

use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use lov_shared::constants::MAP_SIZE;
use lov_shared::data::champions::get_champion;
use lov_shared::map::{BRUSHES, CAMPS, FOUNTAINS, LANE_PATHS, NEXUS, PITS, WALLS};
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{
    EntityKind, GameEvent, MonsterType, Team, TeamOrNeutral, Vec2 as GameVec2,
};

use super::{hud_root, label, set_bg, set_shown, set_text};
use crate::net::{Connection, GameState, ServerEvent};
use crate::render::camera::{CameraRig, MainCamera};
use crate::render::{RENDER_SCALE, to_world};
use crate::ui::icons::draw::IconCache;
use crate::ui::icons::raster::{Canvas, Path, rgb, stops};
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::format::{format_clock, monster_label};
use crate::ui::theme::{BLUE, GOLD_HI, MUTED, PANEL, RED, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, Screen, UiFonts};

/// Côté de la minicarte à l'écran (px).
pub const SIZE: f32 = 236.0;
/// Résolution du fond (px).
const RES: usize = 384;

const CHAMPS: usize = 10;
const MINIONS: usize = 96;
const MONSTERS: usize = 32;
const STRUCTURES: usize = 32;
const WARDS: usize = 24;
const PINGS: usize = 6;

/// Objectifs épiques listés au-dessus de la carte : clé de délai, glyphe.
const EPIC: [(&str, &str); 5] = [
    ("dragon", "dragon"),
    ("elder_dragon", "dragon"),
    ("grubs", "grub"),
    ("herald", "herald"),
    ("leviathan", "leviathan"),
];

// ---------------------------------------------------------------------------------- fond

fn to_px(p: GameVec2) -> Vec2 {
    Vec2::new(p.x as f32, (MAP_SIZE - p.y) as f32)
}

fn poly_path(points: &[GameVec2]) -> Path {
    Path {
        polys: vec![points.iter().map(|p| to_px(*p)).collect()],
    }
}

/// Masque d'une polyligne épaisse à bouts ronds (unités de jeu).
fn stroke(cv: &Canvas, points: &[GameVec2], width: f32) -> Vec<f32> {
    let pts: Vec<Vec2> = points.iter().map(|p| to_px(*p)).collect();
    let half = width / 2.0;
    let mut out = vec![0.0; cv.w * cv.h];
    for y in 0..cv.h {
        for x in 0..cv.w {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / cv.scale;
            let mut d = f32::INFINITY;
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                d = d.min(p.distance(a + ab * t));
            }
            out[y * cv.w + x] = ((half - d) * cv.scale + 0.5).clamp(0.0, 1.0);
        }
    }
    out
}

/// Fond de la minicarte (`drawStatic`).
pub fn draw_static() -> Image {
    let size = MAP_SIZE as f32;
    let mut cv = Canvas::new(RES, size);
    let full = cv.full();
    // fond jungle, en diagonale du bas-gauche au haut-droit
    let bg = [
        (0.0, rgb(0x16261c, 1.0)),
        (0.5, rgb(0x1b2a22, 1.0)),
        (1.0, rgb(0x261b1e, 1.0)),
    ];
    cv.paint(&full, 1.0, |p| {
        stops(&bg, ((p.x + (size - p.y)) / (2.0 * size)).clamp(0.0, 1.0))
    });
    // rivière (diagonale x + y = MAP_SIZE)
    let river = [
        GameVec2 {
            x: 600.0,
            y: MAP_SIZE - 600.0,
        },
        GameVec2 {
            x: MAP_SIZE - 600.0,
            y: 600.0,
        },
    ];
    let k = size / 512.0;
    let r = stroke(&cv, &river, 46.0 * k);
    cv.paint(&r, 0.55, |_| rgb(0x4696c8, 1.0));
    let r = stroke(&cv, &river, 20.0 * k);
    cv.paint(&r, 0.18, |_| rgb(0x8cd2ff, 1.0));
    for pit in [PITS.dragon, PITS.leviathan] {
        let m = cv.ring(
            to_px(pit.pos),
            pit.radius as f32 / 2.0,
            pit.radius as f32,
            None,
        );
        cv.paint(&m, 0.6, |_| rgb(0x285a78, 1.0));
    }
    // voies
    for path in [&LANE_PATHS.top, &LANE_PATHS.mid, &LANE_PATHS.bot] {
        let m = stroke(&cv, path, 22.0 * k);
        cv.paint(&m, 0.32, |_| rgb(0xd6be8c, 1.0));
        let m = stroke(&cv, path, 8.0 * k);
        cv.paint(&m, 0.12, |_| rgb(0xf0dcaa, 1.0));
    }
    // bases
    for (team, color) in [(Team::Blue, 0x4ea2ff), (Team::Red, 0xff5a5a)] {
        let c = to_px(*NEXUS.get(team));
        let r = 2600.0;
        let glow = [(0.0, rgb(color, 0.35)), (1.0, rgb(color, 0.0))];
        let m = cv.ring(c, r / 2.0, r, None);
        cv.paint(&m, 1.0, |p| stops(&glow, (p - c).length() / r));
        let fc = to_px(*FOUNTAINS.get(team));
        let m = cv.ring(fc, 270.0, 540.0, None);
        cv.paint(&m, 0.5, |_| rgb(color, 1.0));
    }
    // brousses, puis murs
    for b in BRUSHES.iter() {
        let m = cv.mask(&poly_path(b), Affine2::IDENTITY);
        cv.paint(&m, 0.55, |_| rgb(0x468c3c, 1.0));
    }
    for w in WALLS.iter() {
        let m = cv.mask(&poly_path(w), Affine2::IDENTITY);
        cv.paint(&m, 1.0, |_| rgb(0x0c120e, 1.0));
    }
    cv.into_image()
}

// ---------------------------------------------------------------------------------- marqueurs

/// Un marqueur recyclé, par catégorie et rang dans son pool.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Mark {
    Champ(usize),
    Minion(usize),
    Monster(usize),
    Structure(usize),
    Ward(usize),
    Ping(usize),
    Camp(usize),
    Camera,
}

/// Texte d'un délai de camp.
#[derive(Component)]
struct CampText(usize);

/// Portrait d'un champion de la carte.
#[derive(Component)]
struct ChampPic(usize);

/// La surface cliquable.
#[derive(Component)]
struct MapArea;

/// Liste des objectifs épiques.
#[derive(Component)]
struct Timers;

/// Pings affichés (position, âge, type).
#[derive(Resource, Default)]
struct Pings(Vec<(GameVec2, f32, bool)>);

fn dot(size: f32, round: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: px(size),
        height: px(size),
        border: UiRect::all(px(1)),
        border_radius: if round {
            BorderRadius::MAX
        } else {
            BorderRadius::all(px(1))
        },
        ..default()
    }
}

fn spawn(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut images: ResMut<Assets<Image>>,
    mut cached: Local<Option<Handle<Image>>>,
) {
    let f = &*fonts;
    let bg = cached
        .get_or_insert_with(|| images.add(draw_static()))
        .clone();
    commands.spawn(hud_root()).with_children(|root| {
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(8),
                bottom: px(8),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|col| {
            col.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: px(2),
                    ..default()
                },
                Timers,
                Pickable::IGNORE,
            ));
            col.spawn((
                Node {
                    width: px(SIZE + 6.0),
                    height: px(SIZE + 6.0),
                    padding: UiRect::all(px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.6)),
                BackgroundColor(PANEL),
            ))
            .with_children(|frame| {
                frame
                    .spawn((
                        Node {
                            width: px(SIZE),
                            height: px(SIZE),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        ImageNode::new(bg),
                        MapArea,
                        BlocksWorld,
                    ))
                    .with_children(|map| {
                        let hidden = Visibility::Hidden;
                        for i in 0..STRUCTURES {
                            map.spawn((
                                dot(8.0, false),
                                BackgroundColor(BLUE),
                                BorderColor::all(hex(0x0a0a0a)),
                                Mark::Structure(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
                        for i in 0..CAMPS.len().min(24) {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(17),
                                    height: px(17),
                                    border: UiRect::all(px(1)),
                                    border_radius: BorderRadius::MAX,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(hexa(0x05070f, 0.82)),
                                BorderColor::all(hexa(0xe8c46a, 0.5)),
                                Mark::Camp(i),
                                hidden,
                                Pickable::IGNORE,
                            ))
                            .with_child((label(f, "", 8.0, 700, hex(0xcfc9b8)), CampText(i)));
                        }
                        for i in 0..WARDS {
                            map.spawn((
                                dot(5.0, true),
                                BackgroundColor(hex(0xffe680)),
                                BorderColor::all(Color::NONE),
                                Mark::Ward(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
                        for i in 0..MINIONS {
                            map.spawn((
                                dot(4.0, false),
                                BackgroundColor(BLUE),
                                BorderColor::all(Color::NONE),
                                Mark::Minion(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
                        for i in 0..MONSTERS {
                            map.spawn((
                                dot(7.0, true),
                                BackgroundColor(hex(0xe8c46a)),
                                BorderColor::all(hex(0x0a0a0a)),
                                Mark::Monster(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
                        // emprise de la caméra
                        map.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                border: UiRect::all(px(1.5)),
                                ..default()
                            },
                            BorderColor::all(hexa(0xffffff, 0.9)),
                            Mark::Camera,
                            hidden,
                            Pickable::IGNORE,
                        ));
                        for i in 0..CHAMPS {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(20),
                                    height: px(20),
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::MAX,
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                BorderColor::all(BLUE),
                                BackgroundColor(hex(0x222222)),
                                Mark::Champ(i),
                                hidden,
                                Pickable::IGNORE,
                            ))
                            .with_child((
                                Node {
                                    width: percent(100),
                                    height: percent(100),
                                    ..default()
                                },
                                ChampPic(i),
                                Pickable::IGNORE,
                            ));
                        }
                        for i in 0..PINGS {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BorderColor::all(hex(0xffdc5a)),
                                Mark::Ping(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
                    });
            });
        });
    });
}

/// Position d'un marqueur centré sur `p` (unités de jeu), de côté `size` px.
fn place(node: &mut Node, p: GameVec2, size: f32) {
    let x = (p.x / MAP_SIZE) as f32 * SIZE - size / 2.0;
    let y = (1.0 - p.y / MAP_SIZE) as f32 * SIZE - size / 2.0;
    let (l, t) = (px(x), px(y));
    if node.left != l {
        node.left = l;
    }
    if node.top != t {
        node.top = t;
    }
}

fn team_of(t: TeamOrNeutral) -> Color {
    match t {
        TeamOrNeutral::Blue => BLUE,
        TeamOrNeutral::Red => RED,
        TeamOrNeutral::Neutral => hex(0xe8c46a),
    }
}

/// Emprise de la caméra au sol, en coordonnées de jeu (boîte englobante des coins de l'écran).
fn camera_box(camera: &Camera, tf: &GlobalTransform) -> Option<(GameVec2, GameVec2)> {
    let size = camera.logical_viewport_size()?;
    let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for c in [
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        Vec2::new(0.0, size.y),
        size,
    ] {
        let ray = camera.viewport_to_world(tf, c).ok()?;
        let t = if ray.direction.y < -1e-3 {
            (-ray.origin.y / ray.direction.y).min(300.0)
        } else {
            300.0
        };
        let p = ray.origin + *ray.direction * t;
        let g = Vec2::new(p.x / RENDER_SCALE, -p.z / RENDER_SCALE);
        lo = lo.min(g);
        hi = hi.max(g);
    }
    let m = MAP_SIZE as f32;
    let clamp = |v: Vec2| GameVec2 {
        x: f64::from(v.x.clamp(0.0, m)),
        y: f64::from(v.y.clamp(0.0, m)),
    };
    Some((clamp(lo), clamp(hi)))
}

#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn update_marks(
    mut commands: Commands,
    state: Res<GameState>,
    time: Res<Time>,
    mut pings: ResMut<Pings>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut marks: Query<(
        &Mark,
        &mut Node,
        &mut Visibility,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    pics: Query<(Entity, &ChampPic, Option<&ChampionPortrait>)>,
    mut camp_texts: Query<(&CampText, &mut Text)>,
) {
    let me = state.my_entity;
    let my_team = state.game.as_ref().map(|g| g.team);
    let dt = time.delta_secs();
    for p in &mut pings.0 {
        p.1 += dt;
    }
    pings.0.retain(|p| p.1 < 1.6);
    // listes d'unités par catégorie (le joueur en dernier : dessiné au-dessus)
    let mut champs = Vec::new();
    let mut minions = Vec::new();
    let mut monsters = Vec::new();
    let mut structures = Vec::new();
    let mut wards = Vec::new();
    for e in state.entities().values() {
        let p = state
            .pose(e.id)
            .map_or(GameVec2 { x: e.x, y: e.y }, |p| GameVec2 { x: p.x, y: p.y });
        match e.kind {
            EntityKind::Champion => champs.push((e, p)),
            EntityKind::Minion if e.dead != Some(true) => minions.push((e, p)),
            EntityKind::Monster if e.dead != Some(true) => monsters.push((e, p)),
            EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus => {
                structures.push((e, p))
            }
            EntityKind::Ward => wards.push((e, p)),
            _ => {}
        }
    }
    champs.sort_by_key(|(e, _)| (Some(e.id) == me, e.id));
    let cam = camera.iter().next().and_then(|(c, tf)| camera_box(c, tf));
    let now = state.time;
    let timers = state.objective_timers.as_ref();
    let camp_left = |i: usize| -> Option<f64> {
        let c = CAMPS.get(i)?;
        let at = *timers?.get(&c.id)?;
        (at > now).then_some(at - now)
    };
    for (mark, mut node, mut vis, mut bg, mut border) in &mut marks {
        let shown = match *mark {
            Mark::Champ(i) => champs.get(i).is_some_and(|(e, p)| {
                let mine = Some(e.id) == me;
                let size = if mine { 24.0 } else { 20.0 };
                node.width = px(size);
                node.height = px(size);
                place(&mut node, *p, size);
                let c = if mine { hex(0xffd76a) } else { team_of(e.team) };
                if border.top != c {
                    *border = BorderColor::all(c);
                }
                true
            }),
            Mark::Minion(i) => minions.get(i).is_some_and(|(e, p)| {
                place(&mut node, *p, 4.0);
                set_bg(&mut bg, team_of(e.team));
                true
            }),
            Mark::Monster(i) => monsters.get(i).is_some_and(|(e, p)| {
                let epic = matches!(
                    e.subtype.as_deref(),
                    Some("dragon" | "elder_dragon" | "leviathan" | "herald")
                );
                let size = if epic { 13.0 } else { 7.0 };
                node.width = px(size);
                node.height = px(size);
                place(&mut node, *p, size);
                set_bg(&mut bg, if epic { hex(0xc48bff) } else { hex(0xe8c46a) });
                true
            }),
            Mark::Structure(i) => structures.get(i).is_some_and(|(e, p)| {
                let (size, round) = match e.kind {
                    EntityKind::Nexus => (14.0, false),
                    EntityKind::Inhibitor => (9.0, true),
                    _ => (8.0, false),
                };
                node.width = px(size);
                node.height = px(size);
                node.border_radius = if round {
                    BorderRadius::MAX
                } else {
                    BorderRadius::all(px(2))
                };
                place(&mut node, *p, size);
                let dead = e.dead == Some(true) || e.hp <= 0.0;
                set_bg(
                    &mut bg,
                    team_of(e.team).with_alpha(if dead { 0.3 } else { 1.0 }),
                );
                true
            }),
            Mark::Ward(i) => wards.get(i).is_some_and(|(e, p)| {
                place(&mut node, *p, 5.0);
                let ally = my_team.is_some_and(|t| {
                    matches!(
                        (e.team, t),
                        (TeamOrNeutral::Blue, Team::Blue) | (TeamOrNeutral::Red, Team::Red)
                    )
                });
                set_bg(&mut bg, if ally { hex(0xffe680) } else { hex(0xff7070) });
                true
            }),
            Mark::Ping(i) => pings.0.get(i).is_some_and(|(p, age, mv)| {
                let size = 12.0 + (age * 40.0) % 30.0;
                node.width = px(size);
                node.height = px(size);
                place(&mut node, *p, size);
                let a = 1.0 - age / 1.6;
                let c = if *mv {
                    hexa(0x78ffa0, a)
                } else {
                    hexa(0xffdc5a, a)
                };
                *border = BorderColor::all(c);
                true
            }),
            Mark::Camp(i) => camp_left(i).is_some_and(|left| {
                if let Some(c) = CAMPS.get(i) {
                    place(&mut node, c.pos, 17.0);
                }
                let c = if left <= 15.0 {
                    GOLD_HI
                } else {
                    hexa(0xe8c46a, 0.5)
                };
                if border.top != c {
                    *border = BorderColor::all(c);
                }
                true
            }),
            Mark::Camera => cam.is_some_and(|(lo, hi)| {
                let k = SIZE / MAP_SIZE as f32;
                node.left = px(lo.x as f32 * k);
                node.top = px((MAP_SIZE - hi.y) as f32 * k);
                node.width = px((hi.x - lo.x) as f32 * k);
                node.height = px((hi.y - lo.y) as f32 * k);
                true
            }),
        };
        set_shown(&mut vis, shown);
    }
    for (CampText(i), mut text) in &mut camp_texts {
        let s = camp_left(*i).map_or(String::new(), |left| {
            if left >= 60.0 {
                format!("{}m", (left / 60.0).ceil())
            } else {
                format!("{}", left.ceil())
            }
        });
        set_text(&mut text, &s);
    }
    for (e, ChampPic(i), current) in &pics {
        let Some((snap, _)) = champs.get(*i) else {
            continue;
        };
        let Some(id) = snap.subtype.clone().filter(|id| get_champion(id).is_some()) else {
            continue;
        };
        let want = ChampionPortrait::new(id, PortraitKind::Round).gray(snap.dead == Some(true));
        if current != Some(&want) {
            commands.entity(e).insert(want);
        }
    }
}

fn epic_label(key: &str) -> String {
    let m = match key {
        "dragon" => Some(MonsterType::Dragon),
        "elder_dragon" => Some(MonsterType::ElderDragon),
        "herald" => Some(MonsterType::Herald),
        "leviathan" => Some(MonsterType::Leviathan),
        "grubs" => Some(MonsterType::Grub),
        _ => None,
    };
    match (key, m.and_then(monster_label)) {
        ("grubs", _) => "Larves".into(),
        (_, Some(l)) => l.into(),
        _ => key.into(),
    }
}

/// Liste des objectifs épiques à venir (reconstruite à chaque seconde).
fn update_timers(
    mut commands: Commands,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    q: Query<Entity, With<Timers>>,
    mut last: Local<(i64, usize)>,
) {
    let now = state.time;
    let list: Vec<(&str, &str, f64)> = state
        .objective_timers
        .as_ref()
        .map(|t| {
            let mut v: Vec<_> = EPIC
                .iter()
                .filter_map(|(k, g)| t.get(*k).filter(|at| **at > now).map(|at| (*k, *g, *at)))
                .collect();
            v.sort_by(|a, b| a.2.total_cmp(&b.2));
            v
        })
        .unwrap_or_default();
    let key = (now.floor() as i64, list.len());
    if *last == key {
        return;
    }
    *last = key;
    for e in &q {
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|col| {
            for (k, g, at) in &list {
                let soon = at - now <= 30.0;
                col.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(5),
                        padding: UiRect::axes(px(8), px(2)),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                ))
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: px(13),
                            height: px(13),
                            ..default()
                        },
                        ImageNode {
                            image: icons.glyph(&mut images, g),
                            color: if soon { GOLD_HI } else { MUTED },
                            ..default()
                        },
                    ));
                    row.spawn(label(&fonts, epic_label(k), 11.0, 600, TEXT));
                    row.spawn(label(
                        &fonts,
                        format_clock(at - now),
                        11.0,
                        800,
                        if soon { GOLD_HI } else { TEXT },
                    ));
                });
            }
        });
    }
}

/// Clics sur la carte : caméra, déplacement, ping.
fn clicks(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    area: Query<&RelativeCursorPosition, With<MapArea>>,
    conn: Res<Connection>,
    mut rig: ResMut<CameraRig>,
    mut pings: ResMut<Pings>,
    mut dragging: Local<bool>,
) {
    if buttons.just_released(MouseButton::Left) {
        *dragging = false;
    }
    let Some(rel) = area.iter().next() else {
        return;
    };
    let Some(n) = rel.normalized else { return };
    let pos = GameVec2 {
        x: f64::from((n.x + 0.5).clamp(0.0, 1.0)) * MAP_SIZE,
        y: f64::from((0.5 - n.y).clamp(0.0, 1.0)) * MAP_SIZE,
    };
    let over = rel.cursor_over();
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if over && buttons.just_pressed(MouseButton::Right) {
        conn.send(ClientMsg::Move { pos });
        pings.0.push((pos, 0.0, true));
        return;
    }
    if over && buttons.just_pressed(MouseButton::Left) {
        if alt {
            conn.send(ClientMsg::Ping {
                pos,
                kind: "alert".into(),
            });
            pings.0.push((pos, 0.0, false));
            return;
        }
        *dragging = true;
    }
    if *dragging && buttons.pressed(MouseButton::Left) {
        rig.locked = false;
        rig.focus = to_world(Vec2::new(pos.x as f32, pos.y as f32), 0.0);
    }
}

/// Pings des alliés reçus du serveur.
fn pings_from_events(mut events: MessageReader<ServerEvent>, mut pings: ResMut<Pings>) {
    for ServerEvent(e) in events.read() {
        if let GameEvent::Ping { pos, .. } = e {
            pings.0.push((*pos, 0.0, false));
        }
    }
    let n = pings.0.len();
    if n > PINGS {
        pings.0.drain(..n - PINGS);
    }
}

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pings>()
            .add_systems(OnEnter(Screen::Game), spawn)
            .add_systems(
                Update,
                (pings_from_events, clicks, update_marks, update_timers)
                    .chain()
                    .run_if(in_state(Screen::Game))
                    .after(crate::render::camera::drive_camera),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fond_dessine_sans_trou() {
        let img = draw_static();
        assert_eq!(img.width(), RES as u32);
        let a = img.data.as_ref().map_or(0, |d| d[3]);
        assert_eq!(a, 255, "fond opaque");
    }

    #[test]
    fn placement_centre_et_y_inverse() {
        let mut n = Node::default();
        place(
            &mut n,
            GameVec2 {
                x: MAP_SIZE / 2.0,
                y: MAP_SIZE,
            },
            10.0,
        );
        assert_eq!(n.left, px(SIZE / 2.0 - 5.0));
        assert_eq!(n.top, px(-5.0));
        assert_eq!(epic_label("grubs"), "Larves");
    }
}
