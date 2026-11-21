//! Matériaux FX réutilisables : fresnel (boucliers, dômes, auras), givre cristallin, eau stylisée.
//!
//! Portage de `packages/client/src/fx/shaders/materials.ts` (la dissolution est dans `dissolve.rs` ; la
//! distorsion de chaleur arrive avec le post-traitement, qui la lit). Écarts avec le client web :
//! - l'horloge des animations est `globals.time` de Bevy au lieu d'un uniform `uTime` poussé à chaque frame ;
//! - le givre n'a pas de teinte par instance (`instanceColor`) : Bevy instancie seul les entités qui
//!   partagent maillage et matériau, la teinte passe par le matériau.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

const FRESNEL: &str = "embedded://lov_client/fx/shaders/fresnel.wesl";
const ICE: &str = "embedded://lov_client/fx/shaders/ice.wesl";
const WATER: &str = "embedded://lov_client/fx/shaders/water.wesl";

// ---------------------------------------------------------------- Fresnel / halo de bord

/// Uniforms du shader (`Fresnel` dans `fresnel.wesl`).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct FresnelUniform {
    pub color: Vec3,
    /// Exposant du halo : plus il est grand, plus le bord est fin.
    pub power: f32,
    /// Opacité de fond, face à la caméra.
    pub base: f32,
    /// Intensité des bandes horizontales qui défilent.
    pub bands: f32,
    pub opacity: f32,
    _pad: f32,
}

/// Halo additif, visible des deux côtés, sans écriture de profondeur.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct FresnelMaterial {
    #[uniform(0)]
    pub u: FresnelUniform,
}

impl Default for FresnelMaterial {
    fn default() -> Self {
        Self::new(Vec3::ONE)
    }
}

impl FresnelMaterial {
    /// Réglages par défaut du client web (`power` 2,5, `base` 0,05, sans bandes).
    pub fn new(color: Vec3) -> Self {
        Self {
            u: FresnelUniform {
                color,
                power: 2.5,
                base: 0.05,
                bands: 0.0,
                opacity: 1.0,
                _pad: 0.0,
            },
        }
    }
}

impl Material for FresnelMaterial {
    fn vertex_shader() -> ShaderRef {
        FRESNEL.into()
    }

    fn fragment_shader() -> ShaderRef {
        FRESNEL.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
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
        // `DoubleSide` : l'intérieur d'un dôme reste visible
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ---------------------------------------------------------------- Givre cristallin (facettes)

/// Uniforms du shader (`Ice` dans `ice.wesl`).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct IceUniform {
    pub color: Vec3,
    pub opacity: f32,
    /// Couleur des faces à l'ombre.
    pub deep: Vec3,
    _pad: f32,
}

/// Glace à facettes, translucide ; écrit la profondeur comme le client web (`depthWrite` par défaut).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct IceMaterial {
    #[uniform(0)]
    pub u: IceUniform,
}

impl Default for IceMaterial {
    fn default() -> Self {
        Self::new(Vec3::new(0.35, 0.65, 0.95), Vec3::new(0.03, 0.1, 0.25))
    }
}

impl IceMaterial {
    pub fn new(color: Vec3, deep: Vec3) -> Self {
        Self {
            u: IceUniform {
                color,
                opacity: 0.92,
                deep,
                _pad: 0.0,
            },
        }
    }
}

impl Material for IceMaterial {
    fn vertex_shader() -> ShaderRef {
        ICE.into()
    }

    fn fragment_shader() -> ShaderRef {
        ICE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- Eau stylisée

/// Uniforms du shader (`Water` dans `water.wesl`).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct WaterUniform {
    pub color: Vec3,
    pub opacity: f32,
    /// Écume du bord et reflets.
    pub foam: Vec3,
    _pad: f32,
}

/// Disque d'eau inscrit dans les UV du maillage (un plan, en général).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub u: WaterUniform,
}

impl Default for WaterMaterial {
    fn default() -> Self {
        Self::new(Vec3::new(0.03, 0.18, 0.42), Vec3::new(0.75, 0.9, 1.0))
    }
}

impl WaterMaterial {
    pub fn new(color: Vec3, foam: Vec3) -> Self {
        Self {
            u: WaterUniform {
                color,
                opacity: 0.85,
                foam,
                _pad: 0.0,
            },
        }
    }
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        WATER.into()
    }

    fn fragment_shader() -> ShaderRef {
        WATER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}
