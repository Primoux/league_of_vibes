//! Rendu de la carte : sol, murs, brousses.
//!
//! Murs et brousses viennent de `lov_shared::map` (`WALLS`, `BRUSHES`).

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::{from_shared, to_world};
use lov_shared::constants;
use lov_shared::map::{BRUSHES, WALLS};

pub const MAP_SIZE: f32 = constants::MAP_SIZE as f32;
const WALL_HEIGHT: f32 = 1.4;

type Polygon = Vec<Vec2>;

/// Polygones `lov-shared` (f64, anti-horaires) → polygones de jeu Bevy (f32).
fn polygons(source: &[lov_shared::map::Polygon]) -> Vec<Polygon> {
    source
        .iter()
        .map(|p| p.iter().copied().map(from_shared).collect())
        .collect()
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
        Mesh3d(meshes.add(prisms(&polygons(&WALLS), WALL_HEIGHT, true))),
        MeshMaterial3d(matte(Color::srgb(0.36, 0.37, 0.42))),
    ));
    commands.spawn((
        Mesh3d(meshes.add(prisms(&polygons(&BRUSHES), 0.03, false))),
        MeshMaterial3d(matte(Color::srgb(0.16, 0.5, 0.2))),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_carte_a_28_murs_et_32_brousses_en_sens_anti_horaire() {
        let walls = polygons(&WALLS);
        let brushes = polygons(&BRUSHES);
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
        let walls = polygons(&WALLS);
        let center = |p: &Polygon| p.iter().copied().sum::<Vec2>() / p.len() as f32;
        for (blue, red) in walls[..14].iter().zip(&walls[14..]) {
            assert!((center(blue) + center(red) - Vec2::splat(MAP_SIZE)).length() < 0.01);
        }
    }
}
