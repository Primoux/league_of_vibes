//! Matériau du contour des champions (coque inversée), portage de `getOutlineMaterial`.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

const SHADER: &str = "embedded://lov_client/render/champions/outline.wesl";

/// Contour : couleur (linéaire) et épaisseur.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct OutlineMaterial {
    /// rgb : couleur, a : épaisseur (`uThickness`).
    #[uniform(0)]
    pub color: Vec4,
}

impl Default for OutlineMaterial {
    fn default() -> Self {
        Self {
            color: Vec4::new(0.03, 0.025, 0.04, 0.14),
        }
    }
}

impl Material for OutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
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
        // `side: BackSide` : seules les faces arrière de la coque dépassent du modèle
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}
