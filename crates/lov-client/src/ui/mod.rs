//! Interface hors carte : portage du socle de `packages/client/src/ui/` (area `ui-core`) — polices,
//! couleurs, routage entre écrans (`App.tsx`), boutons et compte à rebours (`components.tsx`). Les
//! écrans eux-mêmes sont dans `screens/` (area `ui-screens`), le HUD arrivera dans `hud/`.
//!
//! Routage par état, comme le `Router` du client web : fin > partie > chargement > draft > lobby >
//! accueil. Chaque écran est un arbre d'entités `bevy_ui` détruit en sortie d'état (`DespawnOnExit`).
//!
//! Le plugin coupe le pilote automatique du réseau : c'est le joueur qui choisit, sauf `LOV_AUTOPLAY=1`.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Button;

use crate::net::{Autoplay, GameState};
use lov_shared::protocol::RoomPhase;

pub mod lab;
pub mod logic;
pub mod screens;

/// Polices du jeu : Cinzel (titres) et Inter (texte), comme le client web (OFL 1.1, voir `ASSETS.md`).
#[derive(Resource, Clone)]
pub struct UiFonts {
    pub display: Handle<Font>,
    pub text: Handle<Font>,
}

impl UiFonts {
    /// Texte courant (Inter) à la graisse donnée.
    pub fn text(&self, size: f32, weight: u16) -> TextFont {
        TextFont {
            font: self.text.clone().into(),
            font_size: size.into(),
            weight: FontWeight(weight),
            ..default()
        }
    }

    /// Titre (Cinzel, repli sur Inter pour les glyphes absents de Cinzel).
    pub fn display(&self, size: f32, weight: u16) -> TextFont {
        TextFont {
            font: bevy::text::FontSource::list([self.display.clone(), self.text.clone()]),
            font_size: size.into(),
            weight: FontWeight(weight),
            ..default()
        }
    }
}

/// Couleurs du thème (`styles.css`).
pub mod theme {
    use bevy::prelude::Color;

    /// `#rrggbb` → couleur sRGB.
    pub const fn hex(c: u32) -> Color {
        Color::srgb(
            ((c >> 16) & 255) as f32 / 255.0,
            ((c >> 8) & 255) as f32 / 255.0,
            (c & 255) as f32 / 255.0,
        )
    }

    /// `#rrggbb` avec opacité.
    pub const fn hexa(c: u32, a: f32) -> Color {
        Color::srgba(
            ((c >> 16) & 255) as f32 / 255.0,
            ((c >> 8) & 255) as f32 / 255.0,
            (c & 255) as f32 / 255.0,
            a,
        )
    }

    /// Couleur CSS `#rrggbb` d'une palette de champion (blanc si illisible).
    pub fn css(s: &str) -> Color {
        u32::from_str_radix(s.trim_start_matches('#'), 16).map_or(Color::WHITE, hex)
    }

    pub const PANEL: Color = hexa(0x090c18, 0.9);
    pub const TEXT: Color = hex(0xece7da);
    pub const MUTED: Color = hex(0x9b98a9);
    pub const GOLD: Color = hex(0xd9b25f);
    pub const GOLD_HI: Color = hex(0xf6dc94);
    pub const GOLD_LO: Color = hex(0x7d6331);
    pub const TEAL: Color = hex(0x3ee6c8);
    pub const BLUE: Color = hex(0x4ea2ff);
    pub const RED: Color = hex(0xff5a5a);
    pub const GREEN: Color = hex(0x4fdc7c);
    pub const DANGER: Color = hex(0xff6b6b);
}

/// Écran affiché.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Home,
    Lobby,
    Draft,
    Loading,
    /// En partie : pas d'écran, le HUD prend le relais.
    Game,
    End,
}

/// `Router` du client web : fin > partie > chargement > draft > lobby > accueil.
pub fn route(state: &GameState) -> Screen {
    let phase = state.lobby.as_ref().map(|l| l.phase);
    if state.result.is_some() {
        Screen::End
    } else if state.game.is_some() {
        Screen::Game
    } else if matches!(phase, Some(RoomPhase::Loading | RoomPhase::Game))
        || (state.loading_ends_at.is_some() && state.draft.is_some())
    {
        Screen::Loading
    } else if state.draft.is_some() && phase != Some(RoomPhase::Lobby) {
        Screen::Draft
    } else if state.lobby.is_some() {
        Screen::Lobby
    } else {
        Screen::Home
    }
}

fn follow_state(
    state: Res<GameState>,
    current: Res<State<Screen>>,
    mut next: ResMut<NextState<Screen>>,
) {
    let screen = route(&state);
    if screen != *current.get() {
        next.set(screen);
    }
}

/// Couleurs d'un bouton selon son état (survol, désactivé).
#[derive(Component, Clone, Copy)]
pub struct ButtonLook {
    pub normal: Color,
    pub hover: Color,
    pub disabled: Color,
}

impl ButtonLook {
    pub fn new(normal: Color, hover: Color) -> Self {
        Self {
            normal,
            hover,
            disabled: normal.with_alpha(normal.alpha() * 0.45),
        }
    }
}

/// Bouton interactif : nœud, couleurs, survol. L'action se branche par un observateur `Activate`.
pub fn button(look: ButtonLook, node: Node) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        node,
        BackgroundColor(look.normal),
        look,
    )
}

/// Active ou désactive un bouton (`InteractionDisabled`).
pub fn set_enabled(commands: &mut Commands, entity: Entity, enabled: bool) {
    if enabled {
        commands.entity(entity).remove::<InteractionDisabled>();
    } else {
        commands.entity(entity).insert(InteractionDisabled);
    }
}

fn paint_buttons(
    mut buttons: Query<(
        &ButtonLook,
        &Hovered,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
    )>,
) {
    for (look, hovered, disabled, mut bg) in &mut buttons {
        let c = if disabled {
            look.disabled
        } else if hovered.0 {
            look.hover
        } else {
            look.normal
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

const CINZEL: &[u8] = include_bytes!("screens/fonts/Cinzel.ttf");
const INTER: &[u8] = include_bytes!("screens/fonts/Inter.ttf");

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        let forced = std::env::var("LOV_AUTOPLAY").is_ok_and(|v| v != "0");
        app.insert_resource(Autoplay(forced))
            .init_state::<Screen>()
            .add_systems(Update, (follow_state, paint_buttons))
            .add_plugins(screens::ScreensPlugin);
    }

    /// Polices chargées avant le premier `OnEnter` (l'accueil s'affiche dès le démarrage).
    fn finish(&self, app: &mut App) {
        let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
        let display = fonts.add(Font::from_bytes(CINZEL.to_vec()));
        let text = fonts.add(Font::from_bytes(INTER.to_vec()));
        app.insert_resource(UiFonts { display, text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::protocol::{DraftState, GameOver, LobbyState};
    use lov_shared::types::Team;

    fn lobby(phase: RoomPhase) -> LobbyState {
        LobbyState {
            code: "ABCDE".into(),
            phase,
            players: vec![],
            dev_mode: false,
        }
    }

    fn draft() -> DraftState {
        DraftState {
            turns: vec![],
            current_turn: 0,
            turn_ends_at: 0.0,
            picks: vec![],
            finalize_ends_at: None,
        }
    }

    #[test]
    fn routage_dans_l_ordre_du_client_web() {
        let mut s = GameState::default();
        assert_eq!(route(&s), Screen::Home);
        s.lobby = Some(lobby(RoomPhase::Lobby));
        assert_eq!(route(&s), Screen::Lobby);
        // un draft encore présent après un retour au salon n'affiche pas le draft
        s.draft = Some(draft());
        assert_eq!(route(&s), Screen::Lobby);
        s.lobby = Some(lobby(RoomPhase::Draft));
        assert_eq!(route(&s), Screen::Draft);
        s.loading_ends_at = Some(1.0);
        assert_eq!(route(&s), Screen::Loading);
        s.loading_ends_at = None;
        s.lobby = Some(lobby(RoomPhase::Game));
        assert_eq!(route(&s), Screen::Loading);
        s.result = Some(GameOver {
            winner: Team::Blue,
            duration: 60.0,
            stats: vec![],
        });
        assert_eq!(route(&s), Screen::End);
    }

    #[test]
    fn couleurs_hexadecimales() {
        assert_eq!(theme::css("#ff0000"), theme::hex(0xff0000));
        assert_eq!(theme::css("pas une couleur"), Color::WHITE);
    }
}
