//! Fond animé des menus : portage de `packages/client/src/ui/stage/RiftBackground.tsx`.
//!
//! Faille lumineuse en diagonale (la rivière de la carte) qui ondule, étoiles qui scintillent et braises
//! qui montent, teintées par `tint` (couleur du champion en vedette). Le canvas 2D du web devient un
//! matériau d'interface (`rift.wesl`) : tout y est fonction du temps, l'écran peut être reconstruit sans
//! que le fond saute.

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

const SHADER: &str = "embedded://lov_client/ui/screens/rift.wesl";

/// Fond animé. Le changer (teinte, intensité) met à jour son matériau.
#[derive(Component, Clone, Copy, PartialEq)]
pub struct RiftBackground {
    pub tint: Color,
    pub intensity: f32,
}

impl RiftBackground {
    pub fn new(tint: Color, intensity: f32) -> Self {
        Self { tint, intensity }
    }

    /// Nœud plein écran, derrière le contenu (premier enfant de la racine de l'écran).
    pub fn bundle(self) -> impl Bundle {
        (
            self,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        )
    }

    /// Nombre de braises (70 à l'intensité 1).
    pub fn embers(&self) -> u32 {
        (70.0 * self.intensity).round().max(0.0) as u32
    }
}

/// Uniforme du shader (`Rift` de `rift.wesl`).
#[derive(ShaderType, Clone, Copy, Debug)]
pub struct RiftUniform {
    /// Teinte (linéaire).
    tint: Vec4,
    /// Intensité, nombre de braises.
    params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct RiftMaterial {
    #[uniform(0)]
    u: RiftUniform,
}

impl RiftMaterial {
    fn of(bg: &RiftBackground) -> Self {
        let t = bg.tint.to_linear();
        Self {
            u: RiftUniform {
                tint: Vec4::new(t.red, t.green, t.blue, 1.0),
                params: Vec4::new(bg.intensity, bg.embers() as f32, 0.0, 0.0),
            },
        }
    }
}

impl UiMaterial for RiftMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Pose le matériau des nouveaux fonds, et le met à jour quand la teinte ou l'intensité change.
fn sync(
    mut commands: Commands,
    added: Query<(Entity, &RiftBackground), Without<MaterialNode<RiftMaterial>>>,
    changed: Query<(&RiftBackground, &MaterialNode<RiftMaterial>), Changed<RiftBackground>>,
    mut materials: ResMut<Assets<RiftMaterial>>,
) {
    for (e, bg) in &added {
        commands
            .entity(e)
            .insert(MaterialNode(materials.add(RiftMaterial::of(bg))));
    }
    for (bg, node) in &changed {
        if let Some(mut m) = materials.get_mut(&node.0) {
            *m = RiftMaterial::of(bg);
        }
    }
}

pub struct RiftPlugin;

impl Plugin for RiftPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "rift.wesl");
        app.add_plugins(UiMaterialPlugin::<RiftMaterial>::default())
            .add_systems(PostUpdate, sync.before(bevy::ui::UiSystems::Prepare));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braises_selon_l_intensite() {
        assert_eq!(RiftBackground::new(Color::WHITE, 1.0).embers(), 70);
        assert_eq!(RiftBackground::new(Color::WHITE, 0.7).embers(), 49);
        assert_eq!(RiftBackground::new(Color::WHITE, 1.2).embers(), 84);
    }

    #[test]
    fn teinte_lineaire_dans_le_materiau() {
        let m = RiftMaterial::of(&RiftBackground::new(Color::srgb_u8(0x3e, 0xe6, 0xc8), 0.8));
        let t = Color::srgb_u8(0x3e, 0xe6, 0xc8).to_linear();
        assert_eq!(m.u.tint.x, t.red);
        assert_eq!(m.u.params.x, 0.8);
        assert_eq!(m.u.params.y, 56.0);
    }
}
