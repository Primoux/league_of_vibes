//! Décor végétal et minéral : portage de la seconde moitié de `packages/client/src/render/map/vegetation.ts`
//! (placement des arbres, rochers, brousses, touffes et fleurs ; classe `Vegetation`).
//!
//! Placement identique au client web : même générateur (`mulberry32`, graines 4242 et 99), mêmes tirages
//! dans le même ordre. Écarts :
//! - pas d'`InstancedMesh` : une entité par instance, qui partage maillage et matériau avec ses sœurs ;
//!   Bevy les regroupe seul en un dessin instancié et les écarte une à une hors du champ de la caméra
//!   (le client web découpe en tuiles de 2000 pour ça). La teinte d'instance passe par le `MeshTag` ;
//! - qualité haute seulement (le client Bevy n'a pas encore de réglage de qualité) ;
//! - le brouillard de guerre (`applyFog`) n'est pas encore porté (area `render`).

use std::f64::consts::{FRAC_1_SQRT_2, TAU};

use bevy::light::NotShadowCaster;
use bevy::mesh::MeshTag;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Face};
use bevy::shader::ShaderRef;
use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{
    BRUSHES, CAMPS, FOUNTAINS, LANE_HALF_WIDTH, PITS, Polygon, RIVER_HALF_WIDTH, WALLS,
    is_in_brush, is_in_wall, point_in_polygon,
};
use lov_shared::types::Vec2 as GameVec2;

use super::cliffs::{CliffExt, CliffMaterial, CliffUniform, wall_geometry};
use super::geometry::{dist_to_lane, dist_to_river, terrain_height};
use super::noise::NoiseTexture;
use super::rng::Mulberry32;
use super::terrain::GROUND_MARGIN;
use super::vgeo::{Rgb, blade_tuft, flower, hex, offset_lightness, rock, tree_variants};
use crate::net::GameState;
use crate::render::RENDER_SCALE;

pub const SHADER: &str = "embedded://lov_client/render/map/foliage.wesl";

/// Taille des tuiles de végétation (unités de jeu) : une tuile entièrement hors de la carte n'est qu'un
/// fond, ses arbres ne portent pas d'ombre.
pub const TREE_TILE: f64 = 2000.0;

/// Teintes d'instance (proches du blanc) : variété de verts sans casser les dégradés des sommets.
const TREE_TINTS: [u32; 6] = [0xffffff, 0xe0f0d0, 0xfff2c8, 0xd2e6e0, 0xf0ffd8, 0xc8dcc0];
const FLOWER_TINTS: [u32; 6] = [0xffffff, 0xfff27a, 0xffa6d2, 0xc9a6ff, 0x9ad4ff, 0xffffff];

/// Extension du `StandardMaterial` de la végétation (voir `foliage.wesl`).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FoliageExt {
    /// x : force du vent, y : liseré.
    #[uniform(100)]
    pub params: Vec4,
}

impl MaterialExtension for FoliageExt {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub type FoliageMaterial = ExtendedMaterial<StandardMaterial, FoliageExt>;

/// Une instance placée : transformation monde, teinte, ombre portée.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub transform: Transform,
    pub tint: Rgb,
    pub shadow: bool,
}

/// Tout le décor placé, avant création des entités.
#[derive(Default)]
pub struct Layout {
    /// Arbres par variante (sapin élancé, sapin trapu, feuillu).
    pub trees: [Vec<Placed>; 3],
    pub rocks: Vec<Placed>,
    /// Touffes de chaque brousse, dans l'ordre de `BRUSHES`.
    pub brushes: Vec<Vec<Placed>>,
    pub tufts: Vec<Placed>,
    pub flowers: Vec<Placed>,
}

fn v(x: f64, y: f64) -> GameVec2 {
    GameVec2 { x, y }
}

/// Position de jeu → monde, posée sur le relief, surélevée de `lift` (unités monde).
fn ground(x: f64, y: f64, lift: f64) -> Vec3 {
    let s = f64::from(RENDER_SCALE);
    Vec3::new(
        (x * s) as f32,
        (terrain_height(x, y) * s + lift) as f32,
        (-y * s) as f32,
    )
}

fn yaw(rot: f64, pos: Vec3, scale: Vec3) -> Transform {
    Transform {
        translation: pos,
        rotation: Quat::from_rotation_y(rot as f32),
        scale,
    }
}

fn bounds(poly: &[GameVec2]) -> (f64, f64, f64, f64) {
    poly.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

fn near_fountain(x: f64, y: f64, d: f64) -> bool {
    (x - FOUNTAINS.blue.x).hypot(y - FOUNTAINS.blue.y) < d
        || (x - FOUNTAINS.red.x).hypot(y - FOUNTAINS.red.y) < d
}

// ---------------------------------------------------------------- arbres

struct TreeSpot {
    x: f64,
    y: f64,
    s: f64,
}

fn inset_inside(p: GameVec2, poly: &Polygon, inset: f64) -> bool {
    point_in_polygon(p, poly)
        && (0..6).all(|a| {
            let a = f64::from(a);
            point_in_polygon(v(p.x + a.cos() * inset, p.y + a.sin() * inset), poly)
        })
}

fn wall_tree_spots(r: &mut Mulberry32) -> Vec<TreeSpot> {
    let mut out = Vec::new();
    for w in WALLS.iter() {
        let (min_x, min_y, max_x, max_y) = bounds(w);
        let mut y = min_y + 40.0;
        while y < max_y {
            let mut x = min_x + 40.0;
            while x < max_x {
                let px = x + (r.next() - 0.5) * 90.0;
                let py = y + (r.next() - 0.5) * 90.0;
                if inset_inside(v(px, py), w, 55.0) {
                    out.push(TreeSpot {
                        x: px,
                        y: py,
                        s: 0.85 + r.next() * 0.5,
                    });
                }
                x += 125.0;
            }
            y += 125.0;
        }
    }
    out
}

fn border_tree_spots(r: &mut Mulberry32) -> Vec<TreeSpot> {
    let mut out = Vec::new();
    let mut y = -GROUND_MARGIN + 100.0;
    while y < MAP_SIZE + GROUND_MARGIN {
        let mut x = -GROUND_MARGIN + 100.0;
        while x < MAP_SIZE + GROUND_MARGIN {
            let px = x + (r.next() - 0.5) * 140.0;
            let py = y + (r.next() - 0.5) * 140.0;
            x += 210.0;
            let inside = px > 0.0 && py > 0.0 && px < MAP_SIZE && py < MAP_SIZE;
            let edge = px.min(py).min(MAP_SIZE - px).min(MAP_SIZE - py);
            if inside {
                if edge > 750.0
                    || dist_to_lane(px, py) < LANE_HALF_WIDTH + 170.0
                    || is_in_brush(v(px, py))
                    || near_fountain(px, py, 2400.0)
                    || (dist_to_river(px, py) < 700.0 && (px - py).abs() < 9400.0)
                {
                    continue;
                }
            } else if edge < -1300.0 {
                continue;
            }
            out.push(TreeSpot {
                x: px,
                y: py,
                s: 1.0 + r.next() * 0.7,
            });
        }
        y += 210.0;
    }
    out
}

fn place_trees(spots: &[TreeSpot], r: &mut Mulberry32) -> [Vec<Placed>; 3] {
    // tirages aléatoires d'abord (ordre stable quel que soit le découpage en tuiles)
    struct Draw {
        rot: f64,
        sy: f64,
        tint: u32,
        light: f64,
        variant: usize,
    }
    let draws: Vec<Draw> = spots
        .iter()
        .map(|t| {
            let rot = r.next() * TAU;
            let sy = 0.9 + r.next() * 0.35;
            let tint = TREE_TINTS[(r.next() * TREE_TINTS.len() as f64) as usize];
            let light = (r.next() - 0.5) * 0.08;
            // les feuillus restent en lisière (bordure, bas des murs) : les murs sont surtout des sapins
            let broadleaf = if is_in_wall(v(t.x, t.y)) { 0.18 } else { 0.4 };
            let variant = if r.next() < broadleaf {
                2
            } else if r.next() < 0.55 {
                0
            } else {
                1
            };
            Draw {
                rot,
                sy,
                tint,
                light,
                variant,
            }
        })
        .collect();
    // une tuile entièrement hors de la carte n'est qu'un fond : pas d'ombre portée
    let tile = |t: &TreeSpot| {
        (
            (t.x / TREE_TILE).floor() as i64,
            (t.y / TREE_TILE).floor() as i64,
        )
    };
    let inside = |t: &TreeSpot| t.x > 0.0 && t.y > 0.0 && t.x < MAP_SIZE && t.y < MAP_SIZE;
    let lit_tiles: std::collections::HashSet<(i64, i64)> =
        spots.iter().filter(|t| inside(t)).map(tile).collect();
    let mut out: [Vec<Placed>; 3] = Default::default();
    for (t, d) in spots.iter().zip(draws) {
        let lift = if is_in_wall(v(t.x, t.y)) { 0.95 } else { 0.0 };
        let s = t.s as f32;
        out[d.variant].push(Placed {
            transform: yaw(
                d.rot,
                ground(t.x, t.y, lift),
                Vec3::new(s, s * d.sy as f32, s),
            ),
            tint: offset_lightness(hex(d.tint), d.light as f32),
            shadow: lit_tiles.contains(&tile(t)),
        });
    }
    out
}

// ---------------------------------------------------------------- rochers

fn place_rocks(r: &mut Mulberry32) -> Vec<Placed> {
    let mut spots = Vec::new();
    let mut i = 0;
    while i < 1400 && spots.len() < 220 {
        i += 1;
        let x = 600.0 + r.next() * (MAP_SIZE - 1200.0);
        let y = 600.0 + r.next() * (MAP_SIZE - 1200.0);
        let p = v(x, y);
        if is_in_wall(p) || is_in_brush(p) || dist_to_lane(x, y) < LANE_HALF_WIDTH + 80.0 {
            continue;
        }
        if CAMPS
            .iter()
            .any(|c| (c.pos.x - x).hypot(c.pos.y - y) < 450.0)
        {
            continue;
        }
        if [PITS.dragon, PITS.leviathan]
            .iter()
            .any(|pit| (pit.pos.x - x).hypot(pit.pos.y - y) < pit.radius + 100.0)
        {
            continue;
        }
        if near_fountain(x, y, 2500.0) {
            continue;
        }
        // les rochers longent les murs : on garde surtout ceux proches d'un mur
        let near_wall = (0..8).any(|a| {
            let a = f64::from(a);
            is_in_wall(v(x + a.cos() * 220.0, y + a.sin() * 220.0))
        });
        if !near_wall && r.next() < 0.8 {
            continue;
        }
        spots.push(p);
    }
    spots
        .iter()
        .map(|p| {
            // rotation surtout autour de la verticale : la mousse reste sur le dessus
            let ex = (r.next() - 0.5) * 0.5;
            let ey = r.next() * TAU;
            let ez = (r.next() - 0.5) * 0.5;
            let s = 0.6 + r.next() * 1.3;
            let sy = s * (0.5 + r.next() * 0.4);
            let sz = s * (0.8 + r.next() * 0.4);
            let light = (r.next() - 0.5) * 0.12;
            Placed {
                transform: Transform {
                    translation: ground(p.x, p.y, 0.02),
                    rotation: Quat::from_euler(EulerRot::XYZ, ex as f32, ey as f32, ez as f32),
                    scale: Vec3::new(s as f32, sy as f32, sz as f32),
                },
                tint: offset_lightness(Vec3::ONE, light as f32),
                shadow: true,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- brousses

fn place_brushes(r: &mut Mulberry32) -> Vec<Vec<Placed>> {
    BRUSHES
        .iter()
        .map(|poly| {
            let (min_x, min_y, max_x, max_y) = bounds(poly);
            let mut pts = Vec::new();
            let mut y = min_y + 10.0;
            while y < max_y {
                let mut x = min_x + 10.0;
                while x < max_x {
                    let p = v(x + (r.next() - 0.5) * 26.0, y + (r.next() - 0.5) * 26.0);
                    if point_in_polygon(p, poly) {
                        pts.push(p);
                    }
                    x += 34.0;
                }
                y += 34.0;
            }
            pts.iter()
                .map(|p| {
                    let rot = r.next() * TAU;
                    let s = 0.9 + r.next() * 0.5;
                    let sy = s * (0.85 + r.next() * 0.4);
                    let base = if r.next() < 0.5 { 0xffffff } else { 0xe2f0c8 };
                    let light = (r.next() - 0.5) * 0.08;
                    Placed {
                        transform: yaw(
                            rot,
                            ground(p.x, p.y, 0.0),
                            Vec3::new(s as f32, sy as f32, s as f32),
                        ),
                        tint: offset_lightness(hex(base), light as f32),
                        shadow: true,
                    }
                })
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------- touffes et fleurs au sol (détail)

/// Emplacement libre pour du détail au sol (herbe de la jungle, bords des voies).
fn free_ground(x: f64, y: f64) -> bool {
    if x < 150.0 || y < 150.0 || x > MAP_SIZE - 150.0 || y > MAP_SIZE - 150.0 {
        return false;
    }
    if dist_to_lane(x, y) < LANE_HALF_WIDTH - 30.0 {
        return false;
    }
    if dist_to_river(x, y) < RIVER_HALF_WIDTH + 60.0 && (x - y).abs() * FRAC_1_SQRT_2 < 6600.0 {
        return false;
    }
    if near_fountain(x, y, 2380.0) || is_in_wall(v(x, y)) || is_in_brush(v(x, y)) {
        return false;
    }
    ![PITS.dragon, PITS.leviathan]
        .iter()
        .any(|pit| (pit.pos.x - x).hypot(pit.pos.y - y) < pit.radius + 60.0)
}

fn place_ground_detail(r: &mut Mulberry32) -> (Vec<Placed>, Vec<Placed>) {
    let (mut tufts, mut flowers) = (Vec::new(), Vec::new());
    let step = 62.0;
    let mut y = 100.0;
    while y < MAP_SIZE {
        let mut x = 100.0;
        while x < MAP_SIZE {
            let px = x + (r.next() - 0.5) * step;
            let py = y + (r.next() - 0.5) * step;
            x += step;
            // densité en grappes (somme de sinus : pas de motif visible à l'échelle du jeu)
            let clump =
                (px * 0.0031).sin() * (py * 0.0027).cos() + ((px + py) * 0.0013).sin() * 0.6;
            let edge = dist_to_lane(px, py) - LANE_HALF_WIDTH;
            let near_lane = edge > -30.0 && edge < 120.0;
            let keep = if near_lane {
                0.85
            } else {
                0.18 + clump.max(0.0) * 0.5
            };
            let roll = r.next();
            let rot = r.next() * TAU;
            let s = (0.75 + r.next() * 0.6) as f32;
            let c = r.next();
            if roll > keep || !free_ground(px, py) {
                continue;
            }
            let transform = yaw(rot, ground(px, py, 0.0), Vec3::splat(s));
            if !near_lane && c < 0.09 && clump > 0.2 {
                let tint = FLOWER_TINTS[(c * 66.0).floor() as usize % FLOWER_TINTS.len()];
                flowers.push(Placed {
                    transform,
                    tint: hex(tint),
                    shadow: false,
                });
            } else {
                let tint = if c < 0.3 {
                    0xfff0c0
                } else if c < 0.6 {
                    0xe8ffe0
                } else {
                    0xffffff
                };
                tufts.push(Placed {
                    transform,
                    tint: hex(tint),
                    shadow: false,
                });
            }
        }
        y += step;
    }
    (tufts, flowers)
}

/// Placement de tout le décor, dans l'ordre des tirages du client web (`new Vegetation('high')`).
pub fn layout() -> Layout {
    let mut r = Mulberry32::new(4242);
    let mut spots = wall_tree_spots(&mut r);
    spots.extend(border_tree_spots(&mut r));
    let trees = place_trees(&spots, &mut r);
    let rocks = place_rocks(&mut r);
    let brushes = place_brushes(&mut r);
    let (tufts, flowers) = place_ground_detail(&mut Mulberry32::new(99));
    Layout {
        trees,
        rocks,
        brushes,
        tufts,
        flowers,
    }
}

// ---------------------------------------------------------------- entités

/// Teinte d'instance → `MeshTag` (RGB8).
fn tag(c: Rgb) -> MeshTag {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    MeshTag::new(b(c.x) | (b(c.y) << 8) | (b(c.z) << 16))
}

/// Brousse rendue semi-transparente quand le joueur local y entre.
#[derive(Component)]
pub struct BrushView {
    pub poly: usize,
    pub material: Handle<FoliageMaterial>,
    pub opacity: f32,
}

fn foliage(rough: f32, wind: f32, rim: f32, double_sided: bool) -> FoliageMaterial {
    FoliageMaterial {
        base: StandardMaterial {
            perceptual_roughness: rough,
            double_sided,
            cull_mode: if double_sided { None } else { Some(Face::Back) },
            ..default()
        },
        extension: FoliageExt {
            params: Vec4::new(wind, rim, 0.0, 0.0),
        },
    }
}

fn spawn_all(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<FoliageMaterial>,
    items: &[Placed],
) {
    for p in items {
        let mut e = commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            p.transform,
            tag(p.tint),
        ));
        if !p.shadow {
            e.insert(NotShadowCaster);
        }
    }
}

pub(super) fn spawn_vegetation(
    mut commands: Commands,
    noise: Res<NoiseTexture>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FoliageMaterial>>,
    mut cliffs: ResMut<Assets<CliffMaterial>>,
) {
    // murs (falaises rocheuses)
    let cliff = cliffs.add(CliffMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.95,
            ..default()
        },
        extension: CliffExt {
            u: CliffUniform::new(),
            noise: noise.0.clone(),
        },
    });
    commands.spawn((
        Name::new("walls"),
        Mesh3d(meshes.add(wall_geometry().to_mesh())),
        MeshMaterial3d(cliff),
        Transform::IDENTITY,
    ));
    let layout = layout();
    // arbres
    let trees = materials.add(foliage(0.88, 0.025, 0.55, false));
    for (geo, items) in tree_variants().iter().zip(&layout.trees) {
        spawn_all(&mut commands, &meshes.add(geo.to_mesh()), &trees, items);
    }
    // rochers
    let rocks = materials.add(foliage(1.0, 0.0, 0.0, false));
    spawn_all(
        &mut commands,
        &meshes.add(rock().to_mesh()),
        &rocks,
        &layout.rocks,
    );
    // brousses : un matériau par brousse (opacité propre)
    let blade = meshes.add(blade_tuft(7, 0.85, 0.07, 0.05, hex(0x173d1c), hex(0x9ccf55)).to_mesh());
    for (poly, items) in layout.brushes.iter().enumerate() {
        let material = materials.add(foliage(0.8, 0.09, 0.0, true));
        let view = BrushView {
            poly,
            material: material.clone(),
            opacity: 1.0,
        };
        commands.spawn((Name::new("brush"), view));
        spawn_all(&mut commands, &blade, &material, items);
    }
    // touffes et fleurs au sol
    let tuft = meshes.add(blade_tuft(6, 0.36, 0.05, 0.035, hex(0x3b6e30), hex(0xb6d870)).to_mesh());
    let tuft_mat = materials.add(foliage(0.85, 0.6, 0.0, true));
    spawn_all(&mut commands, &tuft, &tuft_mat, &layout.tufts);
    let flower_mat = materials.add(foliage(0.7, 0.0, 0.0, true));
    spawn_all(
        &mut commands,
        &meshes.add(flower().to_mesh()),
        &flower_mat,
        &layout.flowers,
    );
}

/// Opacité d'une brousse après `dt` secondes, ou `None` si elle a atteint sa cible : 0,4 quand le joueur
/// est dedans, 1 sinon, approche exponentielle (×8 par seconde) arrondie à 0,01 près.
pub fn fade(opacity: f32, inside: bool, dt: f32) -> Option<f32> {
    let target = if inside { 0.4 } else { 1.0 };
    if opacity == target {
        return None;
    }
    let next = opacity + (target - opacity) * (dt * 8.0).min(1.0);
    Some(if (next - target).abs() < 0.01 {
        target
    } else {
        next
    })
}

/// Rend semi-transparente la brousse où se trouve le joueur local.
pub(super) fn fade_brushes(
    state: Res<GameState>,
    time: Res<Time>,
    mut brushes: Query<&mut BrushView>,
    mut materials: ResMut<Assets<FoliageMaterial>>,
) {
    let me = state
        .my_entity
        .and_then(|id| state.poses.get(&id))
        .map(|p| v(p.x, p.y));
    let dt = time.delta_secs();
    for mut b in &mut brushes {
        let inside = me.is_some_and(|p| point_in_polygon(p, &BRUSHES[b.poly]));
        let Some(opacity) = fade(b.opacity, inside, dt) else {
            continue;
        };
        b.opacity = opacity;
        if let Some(mut m) = materials.get_mut(&b.material) {
            m.base.base_color = Color::WHITE.with_alpha(b.opacity);
            m.base.alpha_mode = if b.opacity > 0.99 {
                AlphaMode::Opaque
            } else {
                AlphaMode::Blend
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chiffres de `new Vegetation('high')` du client web (Node 24, three r187).
    #[test]
    fn meme_decor_que_le_client_web() {
        let l = layout();
        let counts: Vec<usize> = l.trees.iter().map(Vec::len).collect();
        assert_eq!(counts, [999, 847, 970]);
        let shadowless = l.trees.iter().flatten().filter(|p| !p.shadow).count();
        assert_eq!(shadowless, 1586);
        assert_eq!(l.rocks.len(), 220);
        assert_eq!(l.brushes.len(), 32);
        assert_eq!(l.brushes[0].len(), 279);
        assert_eq!(l.brushes.iter().map(Vec::len).sum::<usize>(), 3967);
        assert_eq!(l.tufts.len(), 6661);
        assert_eq!(l.flowers.len(), 248);
    }

    #[test]
    fn la_brousse_s_estompe_autour_du_joueur_puis_redevient_opaque() {
        assert_eq!(fade(1.0, false, 0.016), None);
        let mut o = 1.0;
        let mut frames = 0;
        while let Some(next) = fade(o, true, 1.0 / 60.0) {
            o = next;
            frames += 1;
        }
        assert_eq!(o, 0.4);
        assert!((20..40).contains(&frames), "{frames} images");
        assert_eq!(fade(0.4, false, 1.0), Some(1.0));
    }

    #[test]
    fn memes_transformations_et_teintes_que_le_client_web() {
        let l = layout();
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-4;
        let rock = &l.rocks[0].transform;
        assert!(close(
            rock.translation,
            Vec3::new(113.55009, 0.01315, -21.21893)
        ));
        // première colonne de la matrice du client web : (0.09536, 0.34755, -1.05249)
        let m = rock.to_matrix();
        assert!(
            close(m.x_axis.truncate(), Vec3::new(0.09536, 0.34755, -1.05249)),
            "{m}"
        );
        let brush = &l.brushes[0][0];
        assert!(close(
            brush.transform.translation,
            Vec3::new(4.21707, 0.04125, -89.16773)
        ));
        let m = brush.transform.to_matrix();
        assert!(
            close(m.x_axis.truncate(), Vec3::new(0.49472, 0.0, -0.94424)),
            "{m}"
        );
        assert!(
            close(brush.tint, Vec3::new(0.785377, 0.884716, 0.621418)),
            "{}",
            brush.tint
        );
    }
}
