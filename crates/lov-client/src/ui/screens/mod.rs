//! Écrans hors partie : portage de `packages/client/src/ui/screens/` (area `ui-screens`).
//!
//! Accueil dans cette PR ; salon, draft, chargement et fin de partie dans les suivantes, puis la scène
//! 3D des champions et les portraits (`stage/`).

use bevy::prelude::*;

use super::Screen;

pub mod home;

/// Racine plein écran d'un écran, détruite en sortie d'état. Fond : dégradé radial sombre de
/// `.lov-screen` (`#17112e` → `#04050c`). `layout` : disposition des enfants (taille et position sont
/// imposées).
pub fn screen_root(screen: Screen, layout: Node) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..layout
        },
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::anchor(Vec2::new(0.2, -0.1)),
            shape: RadialGradientShape::FarthestCorner,
            stops: vec![
                ColorStop::new(Color::srgb_u8(0x17, 0x11, 0x2e), percent(0)),
                ColorStop::new(Color::srgb_u8(0x08, 0x09, 0x1a), percent(55)),
                ColorStop::new(Color::srgb_u8(0x04, 0x05, 0x0c), percent(100)),
            ],
            ..default()
        }),
        GlobalZIndex(10),
        DespawnOnExit(screen),
    )
}

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((home::HomePlugin,));
    }
}
