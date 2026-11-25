//! Sol de la carte : géométrie avec relief + matériau procédural. Portage de `buildTerrain`
//! (`packages/client/src/render/map/terrain.ts`).
//!
//! Les masques (`masks.rs`) ne portent pas des couleurs mais des zones (voies, sable/lit de rivière,
//! dallage des bases, occlusion) ; le shader (`terrain.wesl`) compose herbe, terre, cailloux, sable
//! humide, pavés et caustiques à partir de la texture de bruit échantillonnée en coordonnées monde.
//!
//! Écart avec le client web : le brouillard de guerre (`applyFog`) n'est pas encore porté (area `render`).

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use lov_shared::constants::MAP_SIZE;
use lov_shared::map::FOUNTAINS;

use super::geometry::terrain_height;
use super::masks::{TEX, paint_masks};
use super::noise::{NoiseTexture, image_with_mips};
use crate::render::RENDER_SCALE;

const SHADER: &str = "embedded://lov_client/render/map/terrain.wesl";

/// Le sol déborde de la carte pour remplir la vue aux bords (unités de jeu).
pub const GROUND_MARGIN: f64 = 2000.0;
/// Subdivisions du plan en qualité haute (120 en basse dans le client web).
const SEGMENTS: u32 = 200;

/// Couleur sRGB `0xRRGGBB` → linéaire (`new THREE.Color(hex)`).
pub fn lin(hex: u32) -> Vec4 {
    let c = Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8).to_linear();
    Vec4::new(c.red, c.green, c.blue, 1.0)
}

/// Uniforms de `terrain.wesl`.
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TerrainUniform {
    fount_b: Vec4,
    fount_r: Vec4,
    grass: Vec4,
    grass_dark: Vec4,
    grass_dry: Vec4,
    dirt: Vec4,
    dirt_dark: Vec4,
    pebble: Vec4,
    sand: Vec4,
    sand_wet: Vec4,
    stone_a: Vec4,
    stone_b: Vec4,
    mortar: Vec4,
    caustic: Vec4,
    team_b: Vec4,
    team_r: Vec4,
}

impl TerrainUniform {
    /// Palette du client web (`PALETTE`, couleurs sRGB converties en linéaire).
    fn new() -> Self {
        let s = RENDER_SCALE as f64;
        let fount =
            |p: lov_shared::types::Vec2| Vec4::new((p.x * s) as f32, (-p.y * s) as f32, 0.0, 0.0);
        Self {
            fount_b: fount(FOUNTAINS.blue),
            fount_r: fount(FOUNTAINS.red),
            grass: lin(0x5a8c3e),
            grass_dark: lin(0x356a31),
            grass_dry: lin(0x9aa64a),
            dirt: lin(0xb4935f),
            dirt_dark: lin(0x8a6c47),
            pebble: lin(0xd8ccb0),
            sand: lin(0xcdb98a),
            sand_wet: lin(0x6d6a52),
            stone_a: lin(0x9d9584),
            stone_b: lin(0xb8ae97),
            mortar: lin(0x5a5244),
            caustic: lin(0xbff4ff),
            team_b: lin(0x3d8bff),
            team_r: lin(0xff4a3d),
        }
    }
}

/// Extension du `StandardMaterial` : composition des matériaux du sol (voir `terrain.wesl`).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainExt {
    #[uniform(100)]
    u: TerrainUniform,
    #[texture(101)]
    #[sampler(102)]
    masks: Handle<Image>,
    #[texture(103)]
    #[sampler(104)]
    noise: Handle<Image>,
}

impl MaterialExtension for TerrainExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExt>;

/// `PlaneGeometry(size, size, seg, seg).rotateX(-π/2)`, sommets surélevés à `terrainHeight`, normales
/// lissées (`computeVertexNormals`), UV = position sur la carte (0..1 de 0 à `MAP_SIZE`, au-delà le
/// masque est étiré). Centré sur l'origine : l'entité est placée au centre de la carte.
pub fn terrain_mesh(segments: u32) -> Mesh {
    let s = f64::from(RENDER_SCALE);
    let size = (MAP_SIZE + GROUND_MARGIN * 2.0) * s;
    let row = segments + 1;
    let step = size / f64::from(segments);
    let mut positions: Vec<Vec3> = Vec::with_capacity((row * row) as usize);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(positions.capacity());
    for iy in 0..row {
        // après `rotateX(-π/2)`, la ligne `iy` du plan est à z = iy · pas − taille / 2
        let z = f64::from(iy) * step - size / 2.0;
        for ix in 0..row {
            let x = f64::from(ix) * step - size / 2.0;
            let gx = x / s + MAP_SIZE / 2.0;
            let gy = -z / s + MAP_SIZE / 2.0;
            positions.push(Vec3::new(
                x as f32,
                (terrain_height(gx, gy) * s) as f32,
                z as f32,
            ));
            uvs.push([(gx / MAP_SIZE) as f32, (gy / MAP_SIZE) as f32]);
        }
    }
    let mut indices: Vec<u32> = Vec::with_capacity((segments * segments * 6) as usize);
    for iy in 0..segments {
        for ix in 0..segments {
            let a = ix + row * iy;
            let b = ix + row * (iy + 1);
            let c = ix + 1 + row * (iy + 1);
            let d = ix + 1 + row * iy;
            indices.extend([a, b, d, b, c, d]);
        }
    }
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for t in indices.as_chunks::<3>().0 {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let n = (positions[c] - positions[b]).cross(positions[a] - positions[b]);
        normals[a] += n;
        normals[b] += n;
        normals[c] += n;
    }
    let normals: Vec<Vec3> = normals.into_iter().map(Vec3::normalize_or_zero).collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

pub(super) fn spawn_terrain(
    mut commands: Commands,
    noise: Res<NoiseTexture>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let masks = images.add(image_with_mips(TEX, paint_masks(), false));
    let material = materials.add(TerrainMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.93,
            metallic: 0.0,
            ..default()
        },
        extension: TerrainExt {
            u: TerrainUniform::new(),
            masks,
            noise: noise.0.clone(),
        },
    });
    let half = (MAP_SIZE / 2.0) as f32 * RENDER_SCALE;
    commands.spawn((
        Name::new("terrain"),
        Mesh3d(meshes.add(terrain_mesh(SEGMENTS))),
        MeshMaterial3d(material),
        Transform::from_xyz(half, 0.0, -half),
        NotShadowCaster,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_sol_suit_le_relief_et_couvre_la_carte_avec_sa_marge() {
        let mesh = terrain_mesh(4);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        assert_eq!(pos.len(), 25);
        let s = f64::from(RENDER_SCALE);
        let half = ((MAP_SIZE / 2.0 + GROUND_MARGIN) * s) as f32;
        assert_eq!(pos[0][0], -half);
        assert_eq!(pos[0][2], -half);
        assert_eq!(pos[24][0], half);
        // sommet central = centre de la carte, au fond de la rivière
        let center = pos[12];
        let expected = (terrain_height(MAP_SIZE / 2.0, MAP_SIZE / 2.0) * s) as f32;
        assert!((center[1] - expected).abs() < 1e-6);
        assert!(center[1] < 0.0);
    }

    #[test]
    fn les_couleurs_sont_converties_en_lineaire() {
        let c = lin(0xffffff);
        assert!((c.x - 1.0).abs() < 1e-6);
        let g = lin(0x808080).x;
        assert!((g - 0.2158605).abs() < 1e-4);
    }
}
