//! Écrans hors partie : portage de `packages/client/src/ui/screens/` (area `ui-screens`).
//!
//! Accueil, salon, draft, chargement et fin de partie, avec la scène 3D des champions (`stage`) et le
//! fond animé (`rift`).

use bevy::prelude::*;
use lov_shared::types::Team;

use super::Screen;
use super::theme::{BLUE, RED};

pub mod draft;
pub mod end;
pub mod home;
pub mod loading;
pub mod lobby;
pub mod rift;
pub mod stage;
pub mod studio;

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

/// Couleur d'une équipe.
pub fn team_color(team: Team) -> Color {
    match team {
        Team::Blue => BLUE,
        Team::Red => RED,
    }
}

/// Nom d'une équipe.
pub fn team_name(team: Team) -> &'static str {
    match team {
        Team::Blue => "Équipe bleue",
        Team::Red => "Équipe rouge",
    }
}

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            home::HomePlugin,
            lobby::LobbyPlugin,
            draft::DraftPlugin,
            loading::LoadingPlugin,
            end::EndPlugin,
            stage::StagePlugin,
            rift::RiftPlugin,
            studio::StudioPlugin,
        ));
    }
}
