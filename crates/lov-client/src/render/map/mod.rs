//! Rendu de la carte : sol, murs, brousses.
//!
//! PROTOTYPE : la géométrie est recopiée de `packages/shared/src/map.ts` (quadrant ouest de la jungle
//! bleue, puis symétries). Elle viendra de `lov-shared` dès que la carte y est portée.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::to_world;

pub const MAP_SIZE: f32 = 12_000.0;
const WALL_HEIGHT: f32 = 1.4;

type Polygon = Vec<Vec2>;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Polygon {
    vec![
        Vec2::new(x, y),
        Vec2::new(x + w, y),
        Vec2::new(x + w, y + h),
        Vec2::new(x, y + h),
    ]
}

fn poly(points: &[[f32; 2]]) -> Polygon {
    points.iter().map(|p| Vec2::from_array(*p)).collect()
}

/// Met le polygone en sens anti-horaire (repère jeu : x à droite, y vers le haut).
fn ccw(mut p: Polygon) -> Polygon {
    let n = p.len();
    let area: f32 = (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum();
    if area < 0.0 {
        p.reverse();
    }
    p
}

fn west_walls() -> Vec<Polygon> {
    vec![
        rect(1650.0, 3350.0, 420.0, 1700.0),
        rect(1650.0, 5550.0, 380.0, 750.0),
        poly(&[
            [1650.0, 7350.0],
            [2050.0, 7550.0],
            [2100.0, 9000.0],
            [1650.0, 9300.0],
        ]),
        poly(&[
            [2450.0, 3700.0],
            [3050.0, 3700.0],
            [3150.0, 4250.0],
            [2450.0, 4250.0],
        ]),
        poly(&[
            [2500.0, 4950.0],
            [3150.0, 4850.0],
            [3250.0, 5700.0],
            [2650.0, 6000.0],
            [2450.0, 5600.0],
        ]),
        poly(&[
            [2550.0, 7450.0],
            [3250.0, 7250.0],
            [3450.0, 7500.0],
            [2900.0, 8100.0],
            [2500.0, 7950.0],
        ]),
        poly(&[
            [4250.0, 5350.0],
            [4750.0, 5850.0],
            [4550.0, 6650.0],
            [4250.0, 6800.0],
            [4050.0, 6150.0],
        ]),
    ]
}

fn west_brushes() -> Vec<Polygon> {
    vec![
        rect(420.0, 8900.0, 340.0, 950.0),
        poly(&[
            [1700.0, 9550.0],
            [2150.0, 9350.0],
            [2300.0, 9650.0],
            [1850.0, 9900.0],
        ]),
        rect(2150.0, 5150.0, 300.0, 380.0),
        poly(&[
            [3300.0, 4300.0],
            [3650.0, 4300.0],
            [3700.0, 4550.0],
            [3300.0, 4550.0],
        ]),
        poly(&[
            [3350.0, 7000.0],
            [3750.0, 7000.0],
            [3700.0, 7300.0],
            [3350.0, 7250.0],
        ]),
        poly(&[
            [4850.0, 5650.0],
            [5150.0, 5600.0],
            [5150.0, 5900.0],
            [4900.0, 5950.0],
        ]),
        poly(&[
            [2900.0, 8650.0],
            [3300.0, 8350.0],
            [3500.0, 8550.0],
            [3100.0, 8900.0],
        ]),
        poly(&[
            [4750.0, 7300.0],
            [5100.0, 7000.0],
            [5250.0, 7150.0],
            [4900.0, 7450.0],
        ]),
    ]
}

/// Quadrant ouest + son reflet sur la diagonale x = y (moitié bleue), puis rotation de 180° (moitié rouge).
fn whole_map(west: Vec<Polygon>) -> Vec<Polygon> {
    let south: Vec<Polygon> = west
        .iter()
        .map(|p| p.iter().map(|v| v.yx()).collect())
        .collect();
    let blue: Vec<Polygon> = west.into_iter().chain(south).collect();
    let red: Vec<Polygon> = blue
        .iter()
        .map(|p| p.iter().map(|v| Vec2::splat(MAP_SIZE) - *v).collect())
        .collect();
    blue.into_iter().chain(red).map(ccw).collect()
}

/// Un seul maillage pour tous les polygones : une face supérieure à `height`, et des flancs si `height > 0`.
fn prisms(polygons: &[Polygon], height: f32, with_sides: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for p in polygons {
        let base = positions.len() as u32;
        for v in p {
            positions.push(to_world(*v, height).to_array());
            normals.push([0.0, 1.0, 0.0]);
        }
        for i in 1..p.len() as u32 - 1 {
            indices.extend([base, base + i, base + i + 1]);
        }
        if !with_sides {
            continue;
        }
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            let d = (b - a).normalize_or_zero();
            // Normale sortante d'un polygone anti-horaire : (dy, -dx) en jeu, soit (dy, 0, dx) en monde.
            let normal = [d.y, 0.0, d.x];
            let base = positions.len() as u32;
            for corner in [
                to_world(a, 0.0),
                to_world(b, 0.0),
                to_world(b, height),
                to_world(a, height),
            ] {
                positions.push(corner.to_array());
                normals.push(normal);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_map);
    }
}

fn spawn_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = MAP_SIZE * super::RENDER_SCALE;
    let mut matte = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.95,
            ..default()
        })
    };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(size, size))),
        MeshMaterial3d(matte(Color::srgb(0.22, 0.33, 0.24))),
        Transform::from_xyz(size / 2.0, 0.0, -size / 2.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(prisms(&whole_map(west_walls()), WALL_HEIGHT, true))),
        MeshMaterial3d(matte(Color::srgb(0.36, 0.37, 0.42))),
    ));
    commands.spawn((
        Mesh3d(meshes.add(prisms(&whole_map(west_brushes()), 0.03, false))),
        MeshMaterial3d(matte(Color::srgb(0.16, 0.5, 0.2))),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carte_a_28_murs_et_32_brousses_en_sens_anti_horaire() {
        let walls = whole_map(west_walls());
        let brushes = whole_map(west_brushes());
        assert_eq!(walls.len(), 28);
        assert_eq!(brushes.len(), 32);
        for p in walls.iter().chain(&brushes) {
            let area: f32 = (0..p.len())
                .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                .sum();
            assert!(area > 0.0);
        }
    }

    #[test]
    fn la_carte_est_symetrique_par_rotation_de_180_degres() {
        let walls = whole_map(west_walls());
        let center = |p: &Polygon| p.iter().copied().sum::<Vec2>() / p.len() as f32;
        for (blue, red) in walls[..14].iter().zip(&walls[14..]) {
            assert!((center(blue) + center(red) - Vec2::splat(MAP_SIZE)).length() < 0.01);
        }
    }
}
