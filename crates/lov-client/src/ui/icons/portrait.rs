//! Portrait de champion : portage de `ChampionPortrait` (`packages/client/src/ui/icons/Icons.tsx`).
//!
//! L'illustration peinte du champion en priorité ; à défaut, la photo 3D du studio (`screens::studio`),
//! puis son initiale. Fond radial aux couleurs du champion et vignettage, comme le SVG du web.
//! `Splash` : format 3:4 (cartes du chargement), recadré au centre ; `Round` : disque.
//!
//! Écarts : le motif procédural de secours (rayons, anneaux, éclats, emblème) n'est pas repris, seule
//! l'initiale l'est (les 13 champions ont une illustration) ; `grayscale` assombrit sans désaturer.

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;

use super::Artworks;
use crate::ui::UiFonts;
use crate::ui::screens::studio::{ShotKind, Studio};
use crate::ui::theme::{css, hex, hexa};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortraitKind {
    Square,
    /// Format portrait 3:4.
    Splash,
    Round,
}

/// Portrait d'un champion, sur un nœud dont l'appelant fixe la taille.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ChampionPortrait {
    pub id: String,
    pub kind: PortraitKind,
    /// Champion mort : portrait éteint.
    pub grayscale: bool,
}

impl ChampionPortrait {
    pub fn new(id: impl Into<String>, kind: PortraitKind) -> Self {
        Self {
            id: id.into(),
            kind,
            grayscale: false,
        }
    }

    pub fn gray(mut self, on: bool) -> Self {
        self.grayscale = on;
        self
    }

    /// Le portrait sur `node` (arrondi en disque pour `Round`).
    pub fn with(self, node: Node) -> impl Bundle {
        let radius = if self.kind == PortraitKind::Round {
            BorderRadius::MAX
        } else {
            node.border_radius
        };
        (
            Node {
                border_radius: radius,
                overflow: Overflow::clip(),
                ..node
            },
            self,
        )
    }

    /// Portrait carré (ou rond) de `size` px.
    pub fn sized(self, size: f32) -> impl Bundle {
        let h = if self.kind == PortraitKind::Splash {
            size * 4.0 / 3.0
        } else {
            size
        };
        self.with(Node {
            width: px(size),
            height: px(h),
            ..default()
        })
    }
}

/// Zone source de l'illustration (512 × 512) : recadrée au centre en 3:4 pour `Splash`
/// (`preserveAspectRatio="xMidYMid slice"`).
pub fn artwork_rect(kind: PortraitKind) -> Option<Rect> {
    (kind == PortraitKind::Splash).then(|| Rect::new(64.0, 0.0, 448.0, 512.0))
}

/// Calque plein cadre (illustration, vignettage), arrondi comme le portrait.
fn layer(round: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        border_radius: if round {
            BorderRadius::MAX
        } else {
            BorderRadius::ZERO
        },
        ..default()
    }
}

/// Dessine les portraits nouveaux ou modifiés.
fn draw(
    mut commands: Commands,
    portraits: Query<(Entity, &ChampionPortrait, &Node), Changed<ChampionPortrait>>,
    art: Res<Artworks>,
    fonts: Res<UiFonts>,
    mut studio: ResMut<Studio>,
    mut images: ResMut<Assets<Image>>,
) {
    for (e, p, node) in &portraits {
        let c = get_champion(&p.id);
        let (primary, secondary, accent) =
            c.map_or((hex(0x777777), hex(0x222222), hex(0xdddddd)), |c| {
                (
                    css(&c.palette.primary),
                    css(&c.palette.secondary),
                    css(&c.palette.accent),
                )
            });
        let round = p.kind == PortraitKind::Round;
        let cy = if p.kind == PortraitKind::Splash {
            0.42
        } else {
            0.45
        };
        let tint = if p.grayscale {
            Color::srgb(0.55, 0.55, 0.55)
        } else {
            Color::WHITE
        };
        let mut entity = commands.entity(e);
        entity.despawn_related::<Children>();
        entity.insert(BackgroundGradient::from(RadialGradient {
            position: UiPosition::new(Vec2::new(0.0, cy - 0.5), Val::ZERO, Val::ZERO),
            shape: RadialGradientShape::FarthestCorner,
            stops: vec![
                ColorStop::new(primary, percent(0)),
                ColorStop::new(secondary, percent(55)),
                ColorStop::new(hex(0x05060c), percent(100)),
            ],
            ..default()
        }));
        let art = art.get(&p.id);
        let shot = if art.is_none() {
            let kind = if p.kind == PortraitKind::Splash {
                ShotKind::Full
            } else {
                ShotKind::Bust
            };
            studio.shot(&mut images, &p.id, kind)
        } else {
            None
        };
        entity.with_children(|layers| {
            if let Some(image) = art {
                layers.spawn((
                    layer(round),
                    ImageNode {
                        image,
                        rect: artwork_rect(p.kind),
                        image_mode: NodeImageMode::Stretch,
                        color: tint,
                        ..default()
                    },
                ));
            } else if let Some(image) = shot {
                layers.spawn((
                    layer(round),
                    ImageNode {
                        image,
                        image_mode: NodeImageMode::Stretch,
                        color: tint,
                        ..default()
                    },
                ));
            } else {
                // initiale, sur 44 % de la hauteur du portrait
                let size = match node.height {
                    Val::Px(h) => h * 0.44,
                    _ => 32.0,
                };
                let letter: String = c
                    .map_or(p.id.clone(), |c| c.name.clone())
                    .chars()
                    .take(1)
                    .collect();
                layers
                    .spawn(Node {
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..layer(round)
                    })
                    .with_child((
                        Text::new(letter.to_uppercase()),
                        fonts.display(size, 800),
                        TextColor(accent.mix(&Color::WHITE, 0.4)),
                    ));
            }
            layers.spawn((
                layer(round),
                BackgroundGradient::from(RadialGradient {
                    shape: RadialGradientShape::FarthestCorner,
                    stops: vec![
                        ColorStop::new(Color::NONE, percent(60)),
                        ColorStop::new(hexa(0x000000, 0.65), percent(100)),
                    ],
                    ..default()
                }),
                Pickable::IGNORE,
            ));
        });
    }
}

pub struct PortraitPlugin;

impl Plugin for PortraitPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, draw.before(bevy::ui::UiSystems::Prepare));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recadrage_du_format_splash() {
        assert_eq!(artwork_rect(PortraitKind::Square), None);
        let Some(r) = artwork_rect(PortraitKind::Splash) else {
            panic!("pas de recadrage");
        };
        assert_eq!(r.width() / r.height(), 0.75);
        assert_eq!(r.center().x, 256.0);
    }
}
