//! HUD en partie : portage de `packages/client/src/ui/hud/` (area `ui-hud`).
//!
//! Construit une fois en entrant en partie (`Screen::Game`), puis mis à jour à chaque image : chaque
//! élément dynamique porte un marqueur (texte, largeur, image, visibilité, couleur) que les systèmes de son
//! module remplissent depuis l'état du jeu. Rien n'est recréé tant que la partie dure, hors listes
//! courtes (fil des éliminations) ; un texte ou une couleur n'est réécrit que s'il change.

use bevy::prelude::*;

use super::Screen;
use super::UiFonts;

pub mod bottom;
pub mod chat;
pub mod minimap;
pub mod overlays;
pub mod panels;
pub mod shop;

/// Racine du HUD (plein écran, transparente aux clics).
#[derive(Component)]
pub struct HudRoot;

/// Le HUD passe sous les écrans (`GlobalZIndex(10)`) et au-dessus des étiquettes du monde.
pub const HUD_Z: i32 = 2;

/// Racine plein écran, détruite en sortie de partie.
pub fn hud_root() -> impl Bundle {
    (
        HudRoot,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(HUD_Z),
        Pickable::IGNORE,
        DespawnOnExit(Screen::Game),
    )
}

/// Texte du HUD (police Inter) avec ombre portée : lisible sur la carte.
pub fn label(
    f: &UiFonts,
    s: impl Into<String>,
    size: f32,
    weight: u16,
    color: Color,
) -> impl Bundle {
    (
        Text::new(s),
        f.text(size, weight),
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.85),
        },
        Pickable::IGNORE,
    )
}

/// Remplace le texte s'il a changé (évite de relancer la mise en page).
pub fn set_text(t: &mut Mut<Text>, s: &str) {
    if t.0 != s {
        s.clone_into(&mut t.0);
    }
}

/// Couleur de fond, réécrite seulement si elle change.
pub fn set_bg(bg: &mut Mut<BackgroundColor>, c: Color) {
    if bg.0 != c {
        bg.0 = c;
    }
}

/// Visible ou non (sans marquer de changement inutile).
pub fn set_shown(v: &mut Mut<Visibility>, on: bool) {
    v.set_if_neq(if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            bottom::BottomBarPlugin,
            overlays::OverlaysPlugin,
            minimap::MinimapPlugin,
            shop::ShopPlugin,
            panels::PanelsPlugin,
            chat::ChatPlugin,
        ));
    }
}
