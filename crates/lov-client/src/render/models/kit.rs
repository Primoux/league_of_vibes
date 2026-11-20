//! Assemblage des modèles procéduraux : caches de géométries et de matériaux, arbre de nœuds (pivots et
//! pièces), puis apparition dans le monde Bevy.
//!
//! Portage des utilitaires de `packages/client/src/render/models/rig.ts` (`cachedGeo`, `material`, `glow`,
//! `part`, `pivot`). Un modèle se décrit d'abord comme un arbre de `Node` (comme un `THREE.Group` qu'on
//! remplit), puis `spawn` crée une entité par nœud : les rigs gardent l'indice de leurs articulations et
//! les traduisent en entités à l'apparition.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Face};
use bevy::shader::ShaderRef;

use super::geometry::Geo;

pub const RIM_SHADER: &str = "embedded://lov_client/render/models/rim.wesl";

/// Extension du `StandardMaterial` : liseré et occlusion de contact (voir `rim.wesl`).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct RimAo {
    /// x : 1 = actif (matériaux mats), 0 = matériaux lumineux.
    #[uniform(100)]
    pub rim: Vec4,
}

impl MaterialExtension for RimAo {
    fn fragment_shader() -> ShaderRef {
        RIM_SHADER.into()
    }
}

/// Matériau de tous les modèles procéduraux.
pub type ModelMaterial = ExtendedMaterial<StandardMaterial, RimAo>;

/// Options de `material` (mêmes défauts que le client web).
#[derive(Clone, Copy, Debug)]
pub struct MatOpts {
    /// Couleur émissive, ou `None`.
    pub emissive: Option<u32>,
    pub intensity: f32,
    pub rough: f32,
    pub metal: f32,
    /// Matériau visible des deux côtés (ailes de drake).
    pub double_sided: bool,
    /// Liseré et occlusion de contact (`addRim`) ; les vues par défaut (`mat` de `DefaultViews.ts`) n'en
    /// ont pas, les matériaux émissifs non plus.
    pub rim: bool,
}

impl Default for MatOpts {
    fn default() -> Self {
        Self {
            emissive: None,
            intensity: 1.0,
            rough: 0.8,
            metal: 0.0,
            double_sided: false,
            rim: true,
        }
    }
}

impl MatOpts {
    pub fn rough(rough: f32) -> Self {
        Self { rough, ..default() }
    }

    pub fn rm(rough: f32, metal: f32) -> Self {
        Self {
            rough,
            metal,
            ..default()
        }
    }
}

/// Couleur hexadécimale three.js (sRGB) → couleur Bevy.
pub fn hex(c: u32) -> Color {
    Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MatKey {
    color: u32,
    emissive: u32,
    intensity: u32,
    rough: u32,
    metal: u32,
    double_sided: bool,
    rim: bool,
}

/// Caches partagés : une géométrie par clé, un matériau par jeu d'options (pas d'allocation par entité
/// au-delà des nœuds).
#[derive(Resource, Default)]
pub struct ModelCache {
    meshes: HashMap<String, Handle<Mesh>>,
    materials: HashMap<MatKey, Handle<ModelMaterial>>,
}

/// Accès aux caches et aux banques d'assets, pour construire des modèles depuis un système.
#[derive(SystemParam)]
pub struct Kit<'w> {
    cache: ResMut<'w, ModelCache>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ModelMaterial>>,
}

impl Kit<'_> {
    /// `cachedGeo` : la géométrie n'est construite qu'au premier appel pour cette clé.
    pub fn geo(&mut self, key: &str, make: impl FnOnce() -> Geo) -> Handle<Mesh> {
        self.mesh(key, || make().to_flat_mesh())
    }

    /// Comme `geo`, pour un maillage déjà construit.
    pub fn mesh(&mut self, key: &str, make: impl FnOnce() -> Mesh) -> Handle<Mesh> {
        if let Some(h) = self.cache.meshes.get(key) {
            return h.clone();
        }
        let h = self.meshes.add(make());
        self.cache.meshes.insert(key.to_owned(), h.clone());
        h
    }

    /// `material(color, opts)` : matériau à facettes, avec liseré s'il n'est pas émissif.
    pub fn material(&mut self, color: u32, o: MatOpts) -> Handle<ModelMaterial> {
        let key = MatKey {
            color,
            emissive: o.emissive.unwrap_or(0),
            intensity: o.intensity.to_bits(),
            rough: o.rough.to_bits(),
            metal: o.metal.to_bits(),
            double_sided: o.double_sided,
            rim: o.rim,
        };
        if let Some(h) = self.cache.materials.get(&key) {
            return h.clone();
        }
        let emissive = o.emissive.filter(|&e| e != 0);
        let base = StandardMaterial {
            base_color: hex(color),
            perceptual_roughness: o.rough,
            metallic: o.metal,
            emissive: emissive.map_or(LinearRgba::BLACK, |e| {
                LinearRgba::from(hex(e)) * o.intensity
            }),
            double_sided: o.double_sided,
            cull_mode: (!o.double_sided).then_some(Face::Back),
            ..default()
        };
        let rim = if o.rim && emissive.is_none() {
            1.0
        } else {
            0.0
        };
        let h = self.materials.add(ModelMaterial {
            base,
            extension: RimAo {
                rim: Vec4::new(rim, 0.0, 0.0, 0.0),
            },
        });
        self.cache.materials.insert(key, h.clone());
        h
    }

    /// Matériau mat aux réglages par défaut.
    pub fn mat(&mut self, color: u32) -> Handle<ModelMaterial> {
        self.material(color, MatOpts::default())
    }

    /// `glow(color, intensity)` : matériau lumineux.
    pub fn glow(&mut self, color: u32, intensity: f32) -> Handle<ModelMaterial> {
        self.material(
            color,
            MatOpts {
                emissive: Some(color),
                intensity,
                rough: 0.4,
                ..default()
            },
        )
    }
}

/// Indice d'un nœud dans son `Model`.
pub type N = usize;

/// Nœud du modèle : pivot vide ou pièce visible.
#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<N>,
    pub pos: Vec3,
    /// Angles d'Euler, ordre XYZ de three.js.
    pub rot: Vec3,
    pub scale: Vec3,
    pub part: Option<(Handle<Mesh>, Handle<ModelMaterial>)>,
    pub cast_shadow: bool,
}

impl Node {
    pub fn transform(&self) -> Transform {
        Transform {
            translation: self.pos,
            rotation: euler(self.rot),
            scale: self.scale,
        }
    }
}

/// Rotation d'Euler dans l'ordre de three.js (`'XYZ'` : X, puis Y, puis Z, axes locaux).
pub fn euler(r: Vec3) -> Quat {
    Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z)
}

/// Arbre d'un modèle ; le nœud 0 est la racine.
#[derive(Clone, Debug)]
pub struct Model {
    pub nodes: Vec<Node>,
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Model {
    pub const ROOT: N = 0;

    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                parent: None,
                pos: Vec3::ZERO,
                rot: Vec3::ZERO,
                scale: Vec3::ONE,
                part: None,
                cast_shadow: true,
            }],
        }
    }

    fn push(
        &mut self,
        parent: N,
        pos: Vec3,
        part: Option<(Handle<Mesh>, Handle<ModelMaterial>)>,
    ) -> N {
        self.nodes.push(Node {
            parent: Some(parent),
            pos,
            rot: Vec3::ZERO,
            scale: Vec3::ONE,
            part,
            cast_shadow: true,
        });
        self.nodes.len() - 1
    }

    /// `pivot(x, y, z, parent)` : articulation vide.
    pub fn pivot(&mut self, parent: N, x: f32, y: f32, z: f32) -> N {
        self.push(parent, Vec3::new(x, y, z), None)
    }

    /// `part(geo, mat, x, y, z, parent)` : pièce visible.
    pub fn part(
        &mut self,
        parent: N,
        mesh: Handle<Mesh>,
        mat: Handle<ModelMaterial>,
        x: f32,
        y: f32,
        z: f32,
    ) -> N {
        self.push(parent, Vec3::new(x, y, z), Some((mesh, mat)))
    }

    /// `rotation.set(x, y, z)`.
    pub fn rot(&mut self, n: N, x: f32, y: f32, z: f32) -> N {
        self.nodes[n].rot = Vec3::new(x, y, z);
        n
    }

    /// `scale.setScalar(s)`.
    pub fn scale(&mut self, n: N, s: f32) -> N {
        self.nodes[n].scale = Vec3::splat(s);
        n
    }

    /// `castShadow = false`.
    pub fn no_shadow(&mut self, n: N) -> N {
        self.nodes[n].cast_shadow = false;
        n
    }

    /// Crée une entité par nœud, enfants rattachés à leurs parents ; renvoie les entités par indice.
    /// La racine reçoit `Visibility` et peut être rattachée à une vue.
    pub fn spawn(&self, commands: &mut Commands) -> Vec<Entity> {
        let mut out: Vec<Entity> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let mut e = commands.spawn((node.transform(), Visibility::Inherited));
            if let Some((mesh, mat)) = &node.part {
                e.insert((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
                if !node.cast_shadow {
                    e.insert(NotShadowCaster);
                }
            }
            if let Some(p) = node.parent {
                e.insert(ChildOf(out[p]));
            }
            out.push(e.id());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_dans_l_ordre_de_three() {
        // three : matrice = Rx · Ry · Rz
        let (x, y, z) = (0.3, -0.7, 1.1);
        let q = euler(Vec3::new(x, y, z));
        let m = Quat::from_rotation_x(x) * Quat::from_rotation_y(y) * Quat::from_rotation_z(z);
        assert!(q.angle_between(m) < 1e-5);
    }

    #[test]
    fn arbre_de_noeuds() {
        let mut m = Model::new();
        let p = m.pivot(Model::ROOT, 0.0, 1.0, 0.0);
        m.rot(p, 0.0, 0.0, 0.5);
        let q = m.pivot(p, 1.0, 0.0, 0.0);
        assert_eq!(m.nodes[q].parent, Some(p));
        assert_eq!(m.nodes[p].transform().translation, Vec3::Y);
        assert_eq!(hex(0xff0000), Color::srgb(1.0, 0.0, 0.0));
    }
}
