//! Dissolution (mort) : le modèle se défait en bruit, du haut vers le bas, avec un liseré lumineux.
//!
//! Portage de `makeDissolvable` / `makeDissolveDepthMaterial` / `applyDissolve`
//! (`packages/client/src/fx/shaders/materials.ts`). Le client web injecte le code dans les shaders de three.js
//! (`onBeforeCompile`) ; ici c'est une extension du `StandardMaterial` (`DissolveMaterial`), dont le shader
//! sert aussi à la passe d'ombre. Écarts :
//! - les uniforms ne sont pas partagés entre les pièces d'un modèle (chaque pièce garde son matériau, donc
//!   sa couleur) : on règle la dissolution de toutes les pièces avec `set_dissolve` ;
//! - pour que la passe d'ombre exécute le fragment shader (et donc la découpe), Bevy exige un mode alpha
//!   « peut jeter des fragments » : `set_dissolve` passe la pièce en `AlphaMode::Mask` pendant la
//!   dissolution et la remet en opaque ensuite.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

const SHADER: &str = "embedded://lov_client/fx/shaders/dissolve.wesl";

/// Pièce de modèle dissolvable.
pub type DissolveMaterial = ExtendedMaterial<StandardMaterial, Dissolve>;

/// Uniforms du shader (`Dissolve` dans `dissolve.wesl`).
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct DissolveUniform {
    /// Liseré de découpe (HDR).
    pub edge_color: Vec3,
    /// 0 = intact, 1 = entièrement dissous.
    pub amount: f32,
    pub noise_scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Clone, Debug)]
pub struct Dissolve {
    #[uniform(100)]
    pub u: DissolveUniform,
}

impl Default for Dissolve {
    fn default() -> Self {
        Self::new(Vec3::new(3.0, 1.6, 0.6))
    }
}

impl Dissolve {
    pub fn new(edge_color: Vec3) -> Self {
        Self {
            u: DissolveUniform {
                edge_color,
                amount: 0.0,
                noise_scale: 3.5,
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
            },
        }
    }
}

impl MaterialExtension for Dissolve {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Rend une pièce dissolvable, avec le liseré par défaut (orange brûlant).
pub fn dissolvable(base: StandardMaterial) -> DissolveMaterial {
    ExtendedMaterial {
        base,
        extension: Dissolve::default(),
    }
}

/// Règle l'avancement de la dissolution (0..1) d'une pièce, et son mode alpha (voir l'en-tête).
pub fn set_dissolve(m: &mut DissolveMaterial, amount: f32) {
    m.extension.u.amount = amount;
    let dissolving = amount > 0.0;
    match m.base.alpha_mode {
        AlphaMode::Opaque if dissolving => m.base.alpha_mode = AlphaMode::Mask(0.5),
        AlphaMode::Mask(_) if !dissolving => m.base.alpha_mode = AlphaMode::Opaque,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_dissolution_bascule_le_mode_alpha() {
        let mut m = dissolvable(StandardMaterial::default());
        assert_eq!(m.base.alpha_mode, AlphaMode::Opaque);
        set_dissolve(&mut m, 0.3);
        assert_eq!(m.extension.u.amount, 0.3);
        assert_eq!(m.base.alpha_mode, AlphaMode::Mask(0.5));
        set_dissolve(&mut m, 0.0);
        assert_eq!(m.base.alpha_mode, AlphaMode::Opaque);
        // une pièce transparente le reste
        m.base.alpha_mode = AlphaMode::Blend;
        set_dissolve(&mut m, 0.5);
        assert_eq!(m.base.alpha_mode, AlphaMode::Blend);
    }
}
