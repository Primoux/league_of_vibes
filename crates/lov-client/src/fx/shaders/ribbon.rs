//! Ruban de traînée (trail) : bande de triangles qui suit un point en mouvement (projectile, dash, lame).
//! Capacité fixe, maillage réécrit sur place à chaque frame (aucune allocation).
//! Mode `Billboard` : face caméra (projectiles) ; mode `Flat` : à plat au sol (dashes, glissades).
//!
//! Portage de `packages/client/src/fx/shaders/ribbon.ts`. Le ruban est une entité (`Ribbon` + maillage
//! propre + `RibbonMaterial`) ; on y pousse la position de la tête (`push`), `update_ribbons` fait le reste.
//! Bevy n'a pas de plage de dessin : les sommets inutilisés sont repliés sur le dernier point (triangles
//! vides) et un ruban sans segment est masqué.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::fx::render_order;

const SHADER: &str = "embedded://lov_client/fx/shaders/ribbon.wesl";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RibbonMode {
    #[default]
    Billboard,
    Flat,
}

/// Uniforms du shader (`Params` dans `ribbon.wesl`).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RibbonUniform {
    pub color: Vec3,
    pub opacity: f32,
    /// Couleur du cœur (HDR : blanc brillant par défaut).
    pub core: Vec3,
    _pad: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct RibbonMaterial {
    #[uniform(0)]
    pub u: RibbonUniform,
}

impl RibbonMaterial {
    pub fn new(color: Vec3, core: Option<Vec3>) -> Self {
        Self {
            u: RibbonUniform {
                color,
                opacity: 1.0,
                core: core.unwrap_or(Vec3::splat(2.0)),
                _pad: 0.0,
            },
        }
    }
}

impl Material for RibbonMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn depth_bias(&self) -> f32 {
        render_order(12.0)
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // visible des deux côtés (`DoubleSide` du client web)
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component, Clone, Debug)]
pub struct Ribbon {
    /// Largeur totale à la tête (monde).
    pub width: f32,
    /// Durée de vie d'un point de la traînée (s).
    pub life: f32,
    pub mode: RibbonMode,
    /// Distance minimale entre deux points (monde).
    pub min_dist: f32,
    p: Vec<Vec3>,
    age: Vec<f32>,
    /// Index du point le plus récent.
    head: usize,
    count: usize,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
}

impl Default for Ribbon {
    fn default() -> Self {
        Self::new(32)
    }
}

impl Ribbon {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(2);
        Self {
            width: 0.3,
            life: 0.35,
            mode: RibbonMode::Billboard,
            min_dist: 0.08,
            p: vec![Vec3::ZERO; capacity],
            age: vec![0.0; capacity],
            head: 0,
            count: 0,
            positions: vec![[0.0; 3]; capacity * 2],
            uvs: (0..capacity * 2).map(|v| [0.0, (v % 2) as f32]).collect(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.p.len()
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.head = 0;
    }

    /// Nombre de points vivants (0 = la traînée a fini de s'effacer).
    pub fn alive(&self) -> usize {
        self.count
    }

    /// Ajoute la position courante de la tête (si elle a assez bougé).
    pub fn push(&mut self, p: Vec3) {
        if self.count > 0 && p.distance_squared(self.p[self.head]) < self.min_dist * self.min_dist {
            // la tête suit quand même exactement le point
            self.p[self.head] = p;
            return;
        }
        self.head = (self.head + 1) % self.capacity();
        self.p[self.head] = p;
        self.age[self.head] = 0.0;
        if self.count < self.capacity() {
            self.count += 1;
        }
    }

    /// Vieillit les points, retire ceux qui ont expiré et recalcule les sommets ; renvoie le nombre de
    /// points dessinés (0 si moins de deux).
    pub fn update(&mut self, dt: f32, camera: Vec3) -> usize {
        let cap = self.capacity();
        let ring = |head: usize, k: usize| (head + cap - k) % cap;
        for k in 0..self.count {
            self.age[ring(self.head, k)] += dt;
        }
        // les plus anciens sont en queue
        while self.count > 0 && self.age[ring(self.head, self.count - 1)] >= self.life {
            self.count -= 1;
        }
        let n = self.count;
        if n < 2 {
            return 0;
        }
        // k = 0 → tête ; on écrit du plus récent au plus ancien
        for k in 0..n {
            let i = ring(self.head, k);
            let a = if k == n - 1 { i } else { (i + cap - 1) % cap };
            let b = if k == 0 { i } else { (i + 1) % cap };
            let mut tan = self.p[b] - self.p[a];
            if tan.length_squared() < 1e-10 {
                tan = Vec3::X;
            }
            let side = match self.mode {
                RibbonMode::Flat => tan.cross(Vec3::Y),
                RibbonMode::Billboard => tan.cross(self.p[i] - camera),
            };
            let life = (1.0 - self.age[i] / self.life).max(0.0);
            let taper = (1.0 - k as f32 / (n - 1) as f32) * 0.7 + 0.3;
            let side = side.normalize_or_zero() * (self.width / 2.0) * taper * life;
            self.positions[k * 2] = (self.p[i] + side).to_array();
            self.positions[k * 2 + 1] = (self.p[i] - side).to_array();
            self.uvs[k * 2][0] = life * taper;
            self.uvs[k * 2 + 1][0] = life * taper;
        }
        // sommets inutilisés repliés sur le dernier point : triangles vides
        let last = self.positions[n * 2 - 1];
        for v in n * 2..cap * 2 {
            self.positions[v] = last;
            self.uvs[v][0] = 0.0;
        }
        n
    }

    /// Maillage de la capacité entière, à donner à l'entité du ruban.
    pub fn mesh(&self) -> Mesh {
        let cap = self.capacity() as u32;
        let indices = (0..cap - 1)
            .flat_map(|i| {
                let a = i * 2;
                [a, a + 1, a + 2, a + 1, a + 3, a + 2]
            })
            .collect();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs.clone())
        .with_inserted_indices(Indices::U32(indices))
    }

    /// Composants d'une entité ruban (le maillage vient de `Ribbon::mesh`).
    pub fn bundle(self, mesh: Handle<Mesh>, material: Handle<RibbonMaterial>) -> impl Bundle {
        (
            Name::new("fx-ribbon"),
            self,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::Hidden,
            NoFrustumCulling,
        )
    }
}

/// Avance tous les rubans et réécrit leurs sommets.
pub fn update_ribbons(
    time: Res<Time>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    mut ribbons: Query<(&mut Ribbon, &Mesh3d, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cam = camera
        .iter()
        .next()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    for (mut ribbon, mesh, mut visibility) in &mut ribbons {
        let n = ribbon.update(time.delta_secs(), cam);
        let want = if n < 2 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        visibility.set_if_neq(want);
        if n < 2 {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        if let Some(VertexAttributeValues::Float32x3(dst)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            dst.copy_from_slice(&ribbon.positions);
        }
        if let Some(VertexAttributeValues::Float32x2(dst)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            dst.copy_from_slice(&ribbon.uvs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suit_la_tete_sans_doublons() {
        let mut r = Ribbon::new(4);
        r.push(Vec3::ZERO);
        r.push(Vec3::new(0.01, 0.0, 0.0)); // trop près : la tête bouge, pas de nouveau point
        assert_eq!(r.alive(), 1);
        assert_eq!(r.p[r.head], Vec3::new(0.01, 0.0, 0.0));
        for x in 1..10 {
            r.push(Vec3::new(x as f32, 0.0, 0.0));
        }
        assert_eq!(r.alive(), 4, "plafonné à la capacité");
    }

    #[test]
    fn effile_puis_expire() {
        let mut r = Ribbon::new(8);
        r.mode = RibbonMode::Flat;
        for x in 0..3 {
            r.push(Vec3::new(x as f32, 0.0, 0.0));
        }
        assert_eq!(r.update(0.0, Vec3::Y * 10.0), 3);
        // à plat : la largeur est le long de z, pleine à la tête, 30 % à la queue
        let head = Vec3::from(r.positions[0]) - Vec3::from(r.positions[1]);
        let tail = Vec3::from(r.positions[4]) - Vec3::from(r.positions[5]);
        assert!(
            (head.length() - 0.3).abs() < 1e-5 && head.x.abs() < 1e-6,
            "{head}"
        );
        assert!((tail.length() - 0.09).abs() < 1e-5, "{tail}");
        assert_eq!(r.uvs[0][0], 1.0);
        // sommets inutilisés repliés sur le dernier
        assert_eq!(r.positions[15], r.positions[5]);
        assert_eq!(r.uvs[15][0], 0.0);
        assert_eq!(r.update(0.4, Vec3::ZERO), 0);
        assert_eq!(r.alive(), 0);
    }

    #[test]
    fn face_camera_en_billboard() {
        let mut r = Ribbon::new(8);
        r.push(Vec3::ZERO);
        r.push(Vec3::X);
        r.update(0.0, Vec3::new(1.0, 10.0, 0.0));
        // caméra au-dessus : le ruban s'étale dans le plan du sol (perpendiculaire à la vue)
        let side = Vec3::from(r.positions[0]) - Vec3::from(r.positions[1]);
        assert!(side.y.abs() < 1e-5 && side.z.abs() > 0.29, "{side}");
    }
}
