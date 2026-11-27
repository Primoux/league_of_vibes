//! FX : particules, shaders, post-traitement, runtime des effets de sorts.
//!
//! Portage de `packages/client/src/fx/` (area `fx-core` pour le socle). Suivi : issue #111.
//!
//! Chaîne des effets de sorts, comme `installFx` du client web :
//! `ServerEvent` (réseau) → [`FxBus`] → [`FxDispatcher`] → handlers du [`FxRegistry`] (par fxKey, sinon
//! génériques) → primitives du runtime ([`Fx`]) → particules, décals, rubans, sprites.

use bevy::prelude::*;

// Bus, clés, registre et dispatcher : API publique des FX, appelée par les areas fx-effects et champion
// (primitives, FX des sorts). Jusque-là, une partie n'est appelée que par les tests.
#[allow(dead_code)]
pub mod bus;
// Le pool et les easings servent aux effets des areas fx-effects et champion ; jusque-là, seuls les tests
// les appellent.
#[allow(dead_code)]
pub mod core;
pub mod demo;
#[allow(dead_code)]
pub mod dispatcher;
// Primitives et auras : boîte à outils des FX de sorts (areas fx-effects et champion), dont une partie
// n'est appelée que par les FX des champions et les tests.
#[allow(dead_code)]
pub mod effects;
#[allow(dead_code)]
pub mod keys;
pub mod particles;
pub mod post;
#[allow(dead_code)]
pub mod registry;
pub mod shaders;

pub use bus::FxBus;
pub use core::runtime::{CameraShake, FxRuntime};
pub use dispatcher::FxDispatcher;
pub use registry::FxRegistry;

use crate::net::{ServerEvent, now_ms};

/// Pas de temps maximal des FX (s) : une image très lente ne téléporte pas les effets.
const MAX_DT: f32 = 0.1;

/// Socle FX : à ajouter une fois au client.
pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            particles::ParticlesPlugin,
            shaders::ShadersPlugin,
            post::PostFxPlugin,
            effects::EffectsPlugin,
        ))
        .add_message::<ServerEvent>()
        .add_message::<CameraShake>()
        .init_resource::<FxRuntime>()
        .init_resource::<FxDispatcher>()
        .init_resource::<FxBus>()
        .init_resource::<core::runtime::SpriteAssets>()
        .add_systems(Update, (collect_events, run_fx).chain())
        .add_systems(
            PostUpdate,
            core::runtime::face_camera.before(bevy::transform::TransformSystems::Propagate),
        );
        app.world_mut().get_resource_or_init::<FxRegistry>();
    }
}

/// Événements du réseau → bus FX.
fn collect_events(mut events: MessageReader<ServerEvent>, mut bus: ResMut<FxBus>) {
    let now = now_ms();
    for ServerEvent(e) in events.read() {
        bus.dispatch(e.clone(), now);
    }
}

/// Livre les événements dus au dispatcher, puis avance les effets (`update` d'`installFx`).
fn run_fx(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs().min(MAX_DT);
    let ready = {
        let mut bus = world.resource_mut::<FxBus>();
        bus.update(now_ms());
        bus.take_ready()
    };
    for e in &ready {
        FxDispatcher::handle(world, e);
    }
    world.resource_mut::<FxBus>().recycle(ready);
    FxRuntime::update(world, dt);
}

/// Ordre de dessin d'un FX transparent (`renderOrder` du client web : décals 5, particules 10 et 11,
/// rubans 12), traduit en biais de tri pour `Material::depth_bias`. Bevy trie les objets transparents par
/// profondeur de vue + biais : un écart bien plus grand que la scène fait primer l'ordre sur la distance,
/// comme dans three.js, et laisse les objets transparents ordinaires (biais 0) dessinés avant les FX.
pub(crate) fn render_order(order: f32) -> f32 {
    order * 1.0e5
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use lov_shared::types::{GameEvent, Vec2};

    use super::core::palette::FxColors;
    use super::core::runtime::{Effect, Fx};
    use super::registry::{FxHandlers, RegisterFx};
    use super::*;

    static AREAS: AtomicU32 = AtomicU32::new(0);
    static TICKS: AtomicU32 = AtomicU32::new(0);
    static LATER: AtomicU32 = AtomicU32::new(0);

    struct Tick;
    impl Effect for Tick {
        fn update(&mut self, _: &mut Fx, _: f32) -> bool {
            TICKS.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    fn area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
        if matches!(e, GameEvent::AreaEffect { .. }) {
            AREAS.fetch_add(1, Ordering::Relaxed);
            fx.add(Tick);
            fx.after(0.0, |_| {
                LATER.fetch_add(1, Ordering::Relaxed);
            });
        }
    }

    /// `ServerEvent` → bus → dispatcher → handler → effet et minuteur, avec les vrais systèmes de
    /// `FxPlugin`, dans une application sans rendu.
    #[test]
    fn un_evenement_du_reseau_traverse_toute_la_chaine() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<ServerEvent>()
            .add_message::<CameraShake>()
            .init_resource::<FxRuntime>()
            .init_resource::<FxDispatcher>()
            .init_resource::<FxBus>()
            .add_systems(Update, (collect_events, run_fx).chain())
            .register_fx(
                "chaine_q",
                FxHandlers {
                    area: Some(area),
                    ..default()
                },
            );
        app.update();
        app.world_mut()
            .write_message(ServerEvent(GameEvent::AreaEffect {
                source: 1,
                fx_key: "chaine_q".into(),
                pos: Vec2 { x: 0.0, y: 0.0 },
                radius: 100.0,
                duration: 0.5,
            }));
        app.update();
        app.update();
        assert_eq!(AREAS.load(Ordering::Relaxed), 1);
        assert!(TICKS.load(Ordering::Relaxed) >= 1, "l'effet tourne");
        assert_eq!(LATER.load(Ordering::Relaxed), 1, "la tâche est partie");
        assert_eq!(app.world().resource::<FxDispatcher>().stats.custom, 1);
        assert_eq!(app.world().resource::<FxRuntime>().active_effects(), 1);
    }
}
