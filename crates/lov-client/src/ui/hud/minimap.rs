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
    EntityId, EntityKind, GameEvent, MonsterType, Team, TeamOrNeutral, Vec2 as GameVec2,
};

use super::{hud_root, label, set_bg, set_if, set_shown, set_text, set_val};
use crate::net::{Connection, GameState, ServerEvent};
use crate::render::camera::{CameraRig, MainCamera};
use crate::render::{RENDER_SCALE, to_world};
use crate::ui::icons::draw::IconCache;
use crate::ui::icons::raster::{Canvas, Path, dilate, rgb, stops};
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
/// Épaisseur des bords du cadre de la caméra (px).
const CAM_LINE: f32 = 1.5;

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

/// Silhouette blanche cerclée de noir, teintée par `ImageNode::color` : tour (triangle), inhibiteur
/// (disque), nexus (losange), comme `drawStructure`.
fn shape_image(points: &[Vec2]) -> Image {
    let mut cv = Canvas::new(32, 16.0);
    let m = cv.mask(
        &Path {
            polys: vec![points.to_vec()],
        },
        Affine2::IDENTITY,
    );
    let outline = dilate(&m, cv.w, cv.h, 2.5);
    cv.paint(&outline, 1.0, |_| rgb(0x0a0a0a, 1.0));
    cv.paint(&m, 1.0, |_| rgb(0xffffff, 1.0));
    cv.into_image()
}

/// Images des structures, dessinées une fois.
#[derive(Resource, Clone)]
struct Shapes {
    tower: Handle<Image>,
    inhibitor: Handle<Image>,
    nexus: Handle<Image>,
}

impl Shapes {
    fn new(images: &mut Assets<Image>) -> Self {
        let circle: Vec<Vec2> = (0..24)
            .map(|i| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                Vec2::new(8.0 + 5.5 * a.cos(), 8.0 + 5.5 * a.sin())
            })
            .collect();
        Self {
            tower: images.add(shape_image(&[
                Vec2::new(8.0, 1.5),
                Vec2::new(14.0, 13.5),
                Vec2::new(2.0, 13.5),
            ])),
            inhibitor: images.add(shape_image(&circle)),
            nexus: images.add(shape_image(&[
                Vec2::new(8.0, 1.5),
                Vec2::new(14.5, 8.0),
                Vec2::new(8.0, 14.5),
                Vec2::new(1.5, 8.0),
            ])),
        }
    }
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
    /// Un des quatre bords du trapèze vu par la caméra.
    CamEdge(usize),
}

/// Texte d'un délai de camp, avec ce qu'il affiche (minutes négatives, secondes positives).
#[derive(Component)]
struct CampText(usize, Option<i64>);

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
    mut cached: Local<Option<(Handle<Image>, Shapes)>>,
    fog: Option<Res<crate::render::map::fog::FogOfWar>>,
) {
    let veil = fog.map(|f| f.minimap.clone());
    let f = &*fonts;
    let (bg, shapes) = cached
        .get_or_insert_with(|| (images.add(draw_static()), Shapes::new(&mut images)))
        .clone();
    commands.insert_resource(shapes);
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
                        // brouillard de guerre (`drawFog` du web) : voile sur les zones non visibles,
                        // sous les marqueurs
                        if let Some(veil) = veil {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(SIZE),
                                    height: px(SIZE),
                                    ..default()
                                },
                                ImageNode::new(veil),
                                Pickable::IGNORE,
                            ));
                        }
                        let hidden = Visibility::Hidden;
                        for i in 0..STRUCTURES {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    ..default()
                                },
                                ImageNode::default(),
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
                            .with_child((label(f, "", 8.0, 700, hex(0xcfc9b8)), CampText(i, None)));
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
                        // emprise de la caméra : quatre bords tournés
                        for i in 0..4 {
                            map.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    height: px(CAM_LINE),
                                    ..default()
                                },
                                BackgroundColor(hexa(0xffffff, 0.9)),
                                UiTransform::default(),
                                Mark::CamEdge(i),
                                hidden,
                                Pickable::IGNORE,
                            ));
                        }
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

/// Coin haut-gauche (px) d'un marqueur de côté `size` centré sur `p` (unités de jeu).
fn corner(p: GameVec2, size: f32) -> (f32, f32) {
    let x = (p.x / MAP_SIZE) as f32 * SIZE - size / 2.0;
    let y = (1.0 - p.y / MAP_SIZE) as f32 * SIZE - size / 2.0;
    (x, y)
}

/// Point de jeu → pixel de la minicarte.
fn map_px(p: GameVec2) -> Vec2 {
    Vec2::new(
        (p.x / MAP_SIZE) as f32 * SIZE,
        (1.0 - p.y / MAP_SIZE) as f32 * SIZE,
    )
}

/// Place et dimensionne un marqueur carré sans rien réécrire de ce qui n'a pas changé.
fn place(node: &mut Mut<Node>, p: GameVec2, size: f32) {
    let (x, y) = corner(p, size);
    set_val(node, |n| &mut n.left, px(x));
    set_val(node, |n| &mut n.top, px(y));
    set_val(node, |n| &mut n.width, px(size));
    set_val(node, |n| &mut n.height, px(size));
}

fn set_border(b: &mut Mut<BorderColor>, c: Color) {
    if b.top != c {
        **b = BorderColor::all(c);
    }
}

fn team_of(t: TeamOrNeutral) -> Color {
    match t {
        TeamOrNeutral::Blue => BLUE,
        TeamOrNeutral::Red => RED,
        TeamOrNeutral::Neutral => hex(0xe8c46a),
    }
}

/// Emprise de la caméra au sol, en coordonnées de jeu : les quatre coins de l'écran projetés sur le
/// sol (haut-gauche, haut-droit, bas-droit, bas-gauche), un trapèze en vue inclinée.
fn camera_quad(camera: &Camera, tf: &GlobalTransform) -> Option<[GameVec2; 4]> {
    let size = camera.logical_viewport_size()?;
    let m = MAP_SIZE as f32;
    let mut out = [GameVec2 { x: 0.0, y: 0.0 }; 4];
    for (o, c) in out.iter_mut().zip([
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        size,
        Vec2::new(0.0, size.y),
    ]) {
        let ray = camera.viewport_to_world(tf, c).ok()?;
        let t = if ray.direction.y < -1e-3 {
            (-ray.origin.y / ray.direction.y).min(300.0)
        } else {
            300.0
        };
        let p = ray.origin + *ray.direction * t;
        *o = GameVec2 {
            x: f64::from((p.x / RENDER_SCALE).clamp(0.0, m)),
            y: f64::from((-p.z / RENDER_SCALE).clamp(0.0, m)),
        };
    }
    Some(out)
}

/// Un bord `a → b` (px de la minicarte) : position, longueur et angle du trait.
fn edge(a: Vec2, b: Vec2) -> (Vec2, f32, f32) {
    let d = b - a;
    let len = d.length();
    let mid = (a + b) / 2.0;
    (
        Vec2::new(mid.x - len / 2.0, mid.y - CAM_LINE / 2.0),
        len,
        d.y.atan2(d.x),
    )
}

/// Unités de la carte par catégorie, gardées d'une image à l'autre pour ne rien allouer.
#[derive(Default)]
struct Lists {
    champs: Vec<(EntityId, GameVec2)>,
    minions: Vec<(EntityId, GameVec2)>,
    monsters: Vec<(EntityId, GameVec2)>,
    structures: Vec<(EntityId, GameVec2)>,
    wards: Vec<(EntityId, GameVec2)>,
}

/// Ce qu'un marqueur peut changer : tous n'ont pas de fond, de bord, d'image ni de rotation.
type MarkParts = (
    &'static Mark,
    &'static mut Node,
    &'static mut Visibility,
    Option<&'static mut BackgroundColor>,
    Option<&'static mut BorderColor>,
    Option<&'static mut ImageNode>,
    Option<&'static mut UiTransform>,
);

#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn update_marks(
    mut commands: Commands,
    state: Res<GameState>,
    time: Res<Time>,
    shapes: Res<Shapes>,
    mut pings: ResMut<Pings>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut marks: Query<MarkParts>,
    pics: Query<(Entity, &ChampPic, Option<&ChampionPortrait>)>,
    mut camp_texts: Query<(&mut CampText, &mut Text)>,
    mut lists: Local<Lists>,
) {
    let me = state.my_entity;
    let my_team = state.game.as_ref().map(|g| g.team);
    let dt = time.delta_secs();
    for p in &mut pings.0 {
        p.1 += dt;
    }
    pings.0.retain(|p| p.1 < 1.6);
    // unités par catégorie (le joueur en dernier : dessiné au-dessus)
    let l = &mut *lists;
    for v in [
        &mut l.champs,
        &mut l.minions,
        &mut l.monsters,
        &mut l.structures,
        &mut l.wards,
    ] {
        v.clear();
    }
    let entities = state.entities();
    for e in entities.values() {
        let p = state
            .pose(e.id)
            .map_or(GameVec2 { x: e.x, y: e.y }, |p| GameVec2 { x: p.x, y: p.y });
        let alive = e.dead != Some(true);
        let list = match e.kind {
            EntityKind::Champion => &mut l.champs,
            EntityKind::Minion if alive => &mut l.minions,
            EntityKind::Monster if alive => &mut l.monsters,
            EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus => &mut l.structures,
            EntityKind::Ward => &mut l.wards,
            _ => continue,
        };
        list.push((e.id, p));
    }
    l.champs.sort_by_key(|(id, _)| (Some(*id) == me, *id));
    let get = |list: &Vec<(EntityId, GameVec2)>, i: usize| {
        list.get(i)
            .and_then(|(id, p)| entities.get(id).map(|e| (e, *p)))
    };
    let quad = camera.iter().next().and_then(|(c, tf)| camera_quad(c, tf));
    let now = state.time;
    let timers = state.objective_timers.as_ref();
    let camp_left = |i: usize| -> Option<f64> {
        let c = CAMPS.get(i)?;
        let at = *timers?.get(&c.id)?;
        (at > now).then_some(at - now)
    };
    for (mark, mut node, mut vis, mut bg, mut border, mut img, tf) in &mut marks {
        let shown = match *mark {
            Mark::Champ(i) => get(&l.champs, i).is_some_and(|(e, p)| {
                let mine = Some(e.id) == me;
                place(&mut node, p, if mine { 24.0 } else { 20.0 });
                // mort : estompé, comme `drawChamp`
                let a = if e.dead == Some(true) { 0.35 } else { 1.0 };
                let c = if mine { hex(0xffd76a) } else { team_of(e.team) };
                if let Some(b) = border.as_mut() {
                    set_border(b, c.with_alpha(a));
                }
                true
            }),
            Mark::Minion(i) => get(&l.minions, i).is_some_and(|(e, p)| {
                place(&mut node, p, 4.0);
                if let Some(bg) = bg.as_mut() {
                    set_bg(bg, team_of(e.team));
                }
                true
            }),
            Mark::Monster(i) => get(&l.monsters, i).is_some_and(|(e, p)| {
                let epic = matches!(
                    e.subtype.as_deref(),
                    Some("dragon" | "elder_dragon" | "leviathan" | "herald")
                );
                place(&mut node, p, if epic { 13.0 } else { 7.0 });
                if let Some(bg) = bg.as_mut() {
                    set_bg(bg, if epic { hex(0xc48bff) } else { hex(0xe8c46a) });
                }
                true
            }),
            Mark::Structure(i) => get(&l.structures, i).is_some_and(|(e, p)| {
                let (size, shape) = match e.kind {
                    EntityKind::Nexus => (14.0, &shapes.nexus),
                    EntityKind::Inhibitor => (10.0, &shapes.inhibitor),
                    _ => (10.0, &shapes.tower),
                };
                place(&mut node, p, size);
                let dead = e.dead == Some(true) || e.hp <= 0.0;
                if let Some(img) = img.as_mut() {
                    if img.image != *shape {
                        img.image = shape.clone();
                    }
                    let c = team_of(e.team).with_alpha(if dead { 0.3 } else { 1.0 });
                    set_if(img, |n| &mut n.color, c);
                }
                true
            }),
            Mark::Ward(i) => get(&l.wards, i).is_some_and(|(e, p)| {
                place(&mut node, p, 5.0);
                let ally = my_team.is_some_and(|t| {
                    matches!(
                        (e.team, t),
                        (TeamOrNeutral::Blue, Team::Blue) | (TeamOrNeutral::Red, Team::Red)
                    )
                });
                if let Some(bg) = bg.as_mut() {
                    set_bg(bg, if ally { hex(0xffe680) } else { hex(0xff7070) });
                }
                true
            }),
            Mark::Ping(i) => pings.0.get(i).is_some_and(|(p, age, mv)| {
                place(&mut node, *p, 12.0 + (age * 40.0) % 30.0);
                let a = 1.0 - age / 1.6;
                let c = if *mv {
                    hexa(0x78ffa0, a)
                } else {
                    hexa(0xffdc5a, a)
                };
                if let Some(b) = border.as_mut() {
                    set_border(b, c);
                }
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
                if let Some(b) = border.as_mut() {
                    set_border(b, c);
                }
                true
            }),
            Mark::CamEdge(i) => quad.is_some_and(|q| {
                let (at, len, angle) = edge(map_px(q[i]), map_px(q[(i + 1) % 4]));
                set_val(&mut node, |n| &mut n.left, px(at.x));
                set_val(&mut node, |n| &mut n.top, px(at.y));
                set_val(&mut node, |n| &mut n.width, px(len));
                if let Some(tf) = tf {
                    let mut tf = tf;
                    set_if(&mut tf, |t| &mut t.rotation, Rot2::radians(angle));
                }
                true
            }),
        };
        set_shown(&mut vis, shown);
    }
    for (mut ct, mut text) in &mut camp_texts {
        // minutes (négatives) au-delà d'une minute, secondes sinon
        let shown = camp_left(ct.0).map(|left| {
            if left >= 60.0 {
                -((left / 60.0).ceil() as i64)
            } else {
                left.ceil() as i64
            }
        });
        if ct.1 == Some(shown.unwrap_or(i64::MIN)) {
            continue;
        }
        ct.1 = Some(shown.unwrap_or(i64::MIN));
        let s = match shown {
            Some(m) if m < 0 => format!("{}m", -m),
            Some(s) => s.to_string(),
            None => String::new(),
        };
        set_text(&mut text, &s);
    }
    for (e, ChampPic(i), current) in &pics {
        let Some((snap, _)) = get(&l.champs, *i) else {
            continue;
        };
        let Some(id) = snap
            .subtype
            .as_deref()
            .filter(|id| get_champion(id).is_some())
        else {
            continue;
        };
        let dead = snap.dead == Some(true);
        let same = current
            .is_some_and(|c| c.id == id && c.kind == PortraitKind::Round && c.grayscale == dead);
        if !same {
            commands
                .entity(e)
                .insert(ChampionPortrait::new(id, PortraitKind::Round).gray(dead));
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
    let timers = state.objective_timers.as_ref();
    let upcoming = || {
        EPIC.iter().filter_map(move |(k, g)| {
            let at = *timers?.get(*k)?;
            (at > now).then_some((*k, *g, at))
        })
    };
    // reconstruite une fois par seconde au plus, et la liste n'est faite qu'à ce moment-là
    let key = (now.floor() as i64, upcoming().count());
    if *last == key {
        return;
    }
    *last = key;
    let mut list: Vec<(&str, &str, f64)> = upcoming().collect();
    list.sort_by(|a, b| a.2.total_cmp(&b.2));
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
        let (x, y) = corner(
            GameVec2 {
                x: MAP_SIZE / 2.0,
                y: MAP_SIZE,
            },
            10.0,
        );
        assert_eq!((x, y), (SIZE / 2.0 - 5.0, -5.0));
        assert_eq!(epic_label("grubs"), "Larves");
    }

    #[test]
    fn un_bord_du_cadre_de_camera() {
        // trait horizontal de 10 px : centré sur son milieu, angle nul
        let (at, len, angle) = edge(Vec2::new(0.0, 4.0), Vec2::new(10.0, 4.0));
        assert_eq!(len, 10.0);
        assert_eq!(angle, 0.0);
        assert_eq!(at, Vec2::new(0.0, 4.0 - CAM_LINE / 2.0));
        // vertical vers le bas : un quart de tour, milieu au même endroit
        let (at, len, angle) = edge(Vec2::new(5.0, 0.0), Vec2::new(5.0, 10.0));
        assert_eq!(len, 10.0);
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(
            at + Vec2::new(len / 2.0, CAM_LINE / 2.0),
            Vec2::new(5.0, 5.0)
        );
    }

    /// Une minicarte stable ne relance pas la mise en page : aucun `Node` marqué modifié.
    #[test]
    fn rien_n_est_reecrit_quand_rien_ne_bouge() {
        use lov_shared::types::EntitySnapshot;
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.init_resource::<Pings>();
        let mut images = Assets::<Image>::default();
        app.insert_resource(Shapes::new(&mut images));
        let mut state = GameState::default();
        for (id, kind) in [(1, EntityKind::Minion), (2, EntityKind::Tower)] {
            state.buffer.entities.insert(
                id,
                EntitySnapshot {
                    id,
                    kind,
                    team: TeamOrNeutral::Blue,
                    x: 3000.0,
                    y: 3000.0,
                    facing: 0.0,
                    hp: 100.0,
                    max_hp: 100.0,
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
                },
            );
        }
        app.insert_resource(state);
        app.add_systems(Update, update_marks);
        let minion = app
            .world_mut()
            .spawn((
                Node::default(),
                Visibility::Hidden,
                BackgroundColor(BLUE),
                Mark::Minion(0),
            ))
            .id();
        let tower = app
            .world_mut()
            .spawn((
                Node::default(),
                Visibility::Hidden,
                ImageNode::default(),
                Mark::Structure(0),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Node>(minion).unwrap().width, px(4.0));
        let tick = app.world().read_change_tick();
        app.update();
        for e in [minion, tower] {
            let changed = app
                .world()
                .entity(e)
                .get_ref::<Node>()
                .unwrap()
                .last_changed()
                .is_newer_than(tick, app.world().read_change_tick());
            assert!(!changed, "{e:?} réécrit sans raison");
        }
    }
}
