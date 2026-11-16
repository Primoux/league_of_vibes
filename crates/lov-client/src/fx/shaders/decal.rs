//! Décal au sol : télégraphies (cercle qui se remplit, ligne, cône), zones persistantes, ondes de choc.
//! Un seul shader paramétré par forme. Les distances (bord, motif) sont en unités monde, quelle que soit
//! l'échelle.
//!
//! Portage de `packages/client/src/fx/shaders/decal.ts`. Le décal du client web est un objet qui porte son
//! maillage ; ici c'est une entité (`GroundDecal` + plan partagé + `DecalMaterial` propre), placée par les
//! méthodes `circle` / `rect` / `cone` qui renvoient sa `Transform`. Écart : l'horloge du motif animé est
//! celle de Bevy (`globals.time`), ce qui évite de renvoyer chaque décal au GPU à chaque frame.

use std::sync::atomic::{AtomicU32, Ordering};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::fx::particles::system::Blend;
use crate::fx::render_order;

const SHADER: &str = "embedded://lov_client/fx/shaders/decal.wesl";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecalShape {
    Circle = 0,
    /// Onde de choc : anneau fin et halo intérieur.
    Ring = 1,
    Rect = 2,
    Cone = 3,
}

/// Uniforms du shader (`Decal` dans `decal.wesl`, même ordre).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct DecalUniform {
    pub color: Vec3,
    shape: u32,
    pub color2: Vec3,
    additive: f32,
    size: Vec2,
    pub opacity: f32,
    /// Opacité du remplissage de fond.
    pub fill: f32,
    /// Épaisseur du bord (monde).
    pub edge: f32,
    /// Remplissage temporel 0..1 (télégraphie), < 0 = désactivé.
    pub progress: f32,
    angle: f32,
    /// Intensité du motif animé.
    pub pattern: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct DecalMaterial {
    #[uniform(0)]
    pub u: DecalUniform,
    blend: Blend,
}

impl Default for DecalMaterial {
    fn default() -> Self {
        Self {
            u: DecalUniform {
                color: Vec3::ONE,
                shape: DecalShape::Circle as u32,
                color2: Vec3::ONE,
                additive: 1.0,
                size: Vec2::ONE,
                opacity: 1.0,
                fill: 0.15,
                edge: 0.08,
                progress: -1.0,
                angle: 0.5,
                pattern: 0.0,
            },
            blend: Blend::Add,
        }
    }
}

impl DecalMaterial {
    /// Réinitialise les réglages (avant réutilisation depuis un pool) ; garde couleurs et forme.
    pub fn reset(&mut self) -> &mut Self {
        let u = &mut self.u;
        u.opacity = 1.0;
        u.fill = 0.15;
        u.edge = 0.08;
        u.progress = -1.0;
        u.pattern = 0.0;
        self.set_blend(Blend::Add)
    }

    pub fn set_color(&mut self, c: Vec3, c2: Vec3) -> &mut Self {
        self.u.color = c;
        self.u.color2 = c2;
        self
    }

    pub fn set_blend(&mut self, blend: Blend) -> &mut Self {
        self.blend = blend;
        self.u.additive = if blend == Blend::Add { 1.0 } else { 0.0 };
        self
    }
}

impl Material for DecalMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.blend {
            Blend::Add => AlphaMode::Add,
            Blend::Normal => AlphaMode::Blend,
        }
    }

    fn depth_bias(&self) -> f32 {
        render_order(5.0)
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
        // Un cône ou un rectangle tourné reste visible quel que soit le sens des triangles.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

static LAYER_SEQ: AtomicU32 = AtomicU32::new(0);

/// Place un décal au sol. La hauteur au-dessus du sol diffère légèrement d'un décal à l'autre, pour éviter
/// le z-fighting.
#[derive(Component, Clone, Copy, Debug)]
pub struct GroundDecal {
    lift: f32,
}

impl Default for GroundDecal {
    fn default() -> Self {
        let seq = LAYER_SEQ.fetch_add(1, Ordering::Relaxed) % 16;
        Self {
            lift: 0.03 + seq as f32 * 0.002,
        }
    }
}

impl GroundDecal {
    /// Cercle (ou anneau d'onde de choc) centré en `center` (monde), rayon en unités monde.
    pub fn circle(
        &self,
        m: &mut DecalMaterial,
        center: Vec3,
        radius: f32,
        shape: DecalShape,
    ) -> Transform {
        m.u.shape = shape as u32;
        m.u.size = Vec2::splat(radius);
        Transform::from_xyz(center.x, center.y + self.lift, center.z)
            .with_scale(Vec3::new(radius, 1.0, radius))
    }

    /// Rectangle de `from` à `to` (monde), largeur totale `width`.
    pub fn rect(&self, m: &mut DecalMaterial, from: Vec3, to: Vec3, width: f32) -> Transform {
        let (dx, dz) = (to.x - from.x, to.z - from.z);
        let half = (dx.hypot(dz) / 2.0).max(1e-3);
        m.u.shape = DecalShape::Rect as u32;
        m.u.size = Vec2::new(half, width / 2.0);
        Transform::from_xyz(
            (from.x + to.x) / 2.0,
            from.y + self.lift,
            (from.z + to.z) / 2.0,
        )
        .with_rotation(Quat::from_rotation_y((-dz).atan2(dx)))
        .with_scale(Vec3::new(half, 1.0, width / 2.0))
    }

    /// Cône depuis `origin` vers `toward`, portée `range`, demi-angle `half_angle` (rad).
    pub fn cone(
        &self,
        m: &mut DecalMaterial,
        origin: Vec3,
        toward: Vec3,
        range: f32,
        half_angle: f32,
    ) -> Transform {
        m.u.shape = DecalShape::Cone as u32;
        m.u.size = Vec2::splat(range);
        m.u.angle = half_angle;
        Transform::from_xyz(origin.x, origin.y + self.lift, origin.z)
            .with_rotation(Quat::from_rotation_y(
                (-(toward.z - origin.z)).atan2(toward.x - origin.x),
            ))
            .with_scale(Vec3::new(range, 1.0, range))
    }
}

/// Plan 2×2 couché au sol, partagé par tous les décals.
#[derive(Resource)]
pub struct DecalMesh(pub Handle<Mesh>);

impl FromWorld for DecalMesh {
    fn from_world(world: &mut World) -> Self {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-1.0, 0.0, -1.0],
                [1.0, 0.0, -1.0],
                [1.0, 0.0, 1.0],
                [-1.0, 0.0, 1.0],
            ],
        )
        .with_inserted_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
        Self(world.resource_mut::<Assets<Mesh>>().add(mesh))
    }
}

impl DecalMesh {
    /// Composants d'une entité décal.
    pub fn bundle(
        &self,
        decal: GroundDecal,
        material: Handle<DecalMaterial>,
        transform: Transform,
    ) -> impl Bundle {
        (
            Name::new("fx-decal"),
            decal,
            Mesh3d(self.0.clone()),
            MeshMaterial3d(material),
            transform,
            NoFrustumCulling,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    #[test]
    fn rect_couvre_le_segment() {
        let d = GroundDecal::default();
        let mut m = DecalMaterial::default();
        let from = Vec3::new(1.0, 0.0, 2.0);
        let to = Vec3::new(1.0, 0.0, -4.0);
        let tf = d.rect(&mut m, from, to, 2.0);
        assert_eq!(m.u.shape, DecalShape::Rect as u32);
        assert_eq!(m.u.size, Vec2::new(3.0, 1.0));
        // le bout local (1, 0, 0) du plan tombe sur `to`, le bout (−1, 0, 0) sur `from`
        let end = tf.transform_point(Vec3::X);
        assert!(end.xz().distance(to.xz()) < 1e-5, "{end}");
        let start = tf.transform_point(-Vec3::X);
        assert!(start.xz().distance(from.xz()) < 1e-5, "{start}");
        assert!((tf.translation.y - d.lift).abs() < 1e-6);
    }

    #[test]
    fn cone_vise_la_cible() {
        let d = GroundDecal::default();
        let mut m = DecalMaterial::default();
        let tf = d.cone(&mut m, Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), 4.0, 0.4);
        assert_eq!(m.u.angle, 0.4);
        // l'axe du cône (x local) pointe vers la cible
        let tip = tf.transform_point(Vec3::X);
        assert!(tip.xz().distance(Vec2::new(0.0, 4.0)) < 1e-5, "{tip}");
        let (_, angle) = tf.rotation.to_axis_angle();
        assert!((angle - FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn hauteurs_distinctes_et_reset() {
        let lifts: Vec<f32> = (0..16).map(|_| GroundDecal::default().lift).collect();
        assert!(lifts.iter().all(|l| (0.03..0.0601).contains(l)));
        let mut m = DecalMaterial::default();
        m.u.progress = 0.5;
        m.u.opacity = 0.2;
        m.set_blend(Blend::Normal);
        m.reset();
        assert_eq!((m.u.progress, m.u.opacity, m.u.additive), (-1.0, 1.0, 1.0));
        assert_eq!(m.alpha_mode(), AlphaMode::Add);
    }
}
