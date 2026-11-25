//! Client natif de League of Vibes.
//!
//! Portage de `packages/client/src` (rendu, FX, interface, audio, entrées, réseau). Il parle le même
//! protocole JSON v2 que le client web, donc au serveur Node tant que `lov-server` n'est pas porté.
//!
//! PROTOTYPE de validation : carte, entités en formes simples, déplacement au clic droit, compteur de FPS.
//! Lancement : `SKIP_DRAFT=1 DEV_LOBBY=1 npm run dev:server`, puis `cargo run -p lov-client --release`.
//! `LOV_URL` change l'adresse du serveur (défaut `ws://localhost:3000/ws`).
//! `LOV_FXLAB=1` lance la démo des FX sans serveur.
//! `LOV_VIEWLAB=1` lance la démo des vues d'entités (tous les modèles, toutes les animations) sans serveur.
//! `LOV_UILAB=<écran>` affiche un écran de l'interface avec des données fictives, sans serveur.
//! `LOV_STRESS=<n>` lance une scène de charge de n unités sans serveur (mesure de FPS).
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture 4 s après l'entrée en jeu, pour vérifier le rendu
//! sans regarder la fenêtre (CI, IA).

use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

mod fx;
mod input;
mod net;
mod render;
mod ui;

fn main() {
    let stress = render::stress::units_from_env();
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "League of Vibes".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }),
        FpsOverlayPlugin::default(),
        render::perf::PerfPlugin,
        render::map::MapPlugin,
        render::entities::EntitiesPlugin,
        render::camera::CameraPlugin,
        input::InputPlugin,
        fx::FxPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.06)))
    .add_systems(Startup, spawn_status)
    .add_systems(Update, (update_status, screenshot_once));
    // `LOV_STRESS=<n>` : scène de charge hors ligne, sans connexion au serveur.
    match stress {
        Some(units) => app.add_plugins((net::OfflinePlugin, render::stress::StressPlugin(units))),
        None if render::viewlab::enabled() => {
            app.add_plugins((net::OfflinePlugin, render::viewlab::ViewLabPlugin))
        }
        None if ui::lab::requested().is_some() => {
            app.add_plugins((net::OfflinePlugin, ui::UiPlugin, ui::lab::UiLabPlugin))
        }
        None if fx::demo::enabled() => {
            app.add_plugins((net::OfflinePlugin, fx::demo::FxDemoPlugin))
        }
        None => app.add_plugins((net::NetPlugin, ui::UiPlugin)),
    };
    app.run();
}

#[derive(Component)]
struct StatusText;

fn spawn_status(mut commands: Commands) {
    commands.spawn((
        StatusText,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            bottom: Val::Px(10.0),
            ..default()
        },
    ));
}

fn update_status(
    state: Res<net::GameState>,
    rig: Res<render::camera::CameraRig>,
    screen: Option<Res<State<ui::Screen>>>,
    mut text: Single<(&mut Text, &mut Visibility), With<StatusText>>,
) {
    // Ligne de debug masquée sous les écrans (accueil, salon…).
    let (text, visibility) = &mut *text;
    **visibility = match screen.as_deref().map(State::get) {
        Some(s) if *s != ui::Screen::Game => Visibility::Hidden,
        _ => Visibility::Inherited,
    };
    // La police par défaut de Bevy n'a que l'ASCII : pas d'accents ici tant que l'area ui-core
    // n'a pas apporté une police du jeu.
    text.0 = format!(
        "{} | {} unites | {} snapshots | vue {}\n\
         Clic droit : bouger/attaquer | QWER : sorts | DF : invocateur | A : attaque-deplacement\n\
         Espace : centrer | Y : fixer la vue | F11 : plein ecran | V : vsync | Esc : reglages",
        state.status,
        state.entities().len(),
        state.snapshots,
        if rig.locked { "fixe" } else { "libre" },
    );
}

fn screenshot_once(
    mut commands: Commands,
    state: Res<net::GameState>,
    time: Res<Time>,
    mut in_game_for: Local<f32>,
    mut done: Local<bool>,
) {
    if *done || state.my_entity.is_none() {
        return;
    }
    *in_game_for += time.delta_secs();
    if *in_game_for >= 4.0 {
        *done = true;
        if let Ok(path) = std::env::var("LOV_SCREENSHOT") {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    }
}
