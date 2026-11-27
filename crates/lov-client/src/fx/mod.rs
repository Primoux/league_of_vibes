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
pub mod champions;
// Le pool et les easings servent aux effets des areas fx-effects et champion ; jusque-là, seuls les tests
// les appellent.
#[allow(dead_code)]
pub mod core;
pub mod defaults;
pub mod demo;
#[allow(dead_code)]
pub mod dispatcher;
// Primitives et auras : boîte à outils des FX de sorts (areas fx-effects et champion), dont une partie
// n'est appelée que par les FX des champions et les tests.
#[allow(dead_code)]
pub mod effects;
#[allow(dead_code)]
pub mod keys;
#[cfg(feature = "spelllab")]
pub mod lab;
pub mod particles;
pub mod post;
#[allow(dead_code)]
pub mod registry;
pub mod shaders;
pub mod summoners;
// Indicateurs de visée : l'API manuelle (`show_ability`…) sert aux démos et aux tests ; la partie les
// pilote elle-même depuis l'état de l'input.
#[allow(dead_code)]
pub mod telegraph;
// Effets du monde : `register_auto_attack_fx` et les couleurs d'équipe servent aux FX des champions.
#[allow(dead_code)]
pub mod world;

pub use bus::FxBus;
pub use core::runtime::{CameraShake, FxRuntime};
pub use dispatcher::FxDispatcher;
pub use registry::FxRegistry;

use crate::net::{ServerEvent, now_ms};
use lov_shared::types::EntityId;
use post::{FxQuality, PostFxSettings};

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
            telegraph::TelegraphPlugin,
            world::WorldFxPlugin,
            champions::ChampionFxPlugin,
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
        app.add_systems(Update, reset_between_games.before(collect_events));
        app.world_mut()
            .get_resource_or_init::<FxRegistry>()
            .defaults = defaults::default_fx();
        summoners::register(app);
        if std::env::var("LOV_FX_QUALITY").as_deref() == Ok("low") {
            app.add_systems(Startup, |world: &mut World| {
                set_fx_quality(world, FxQuality::Low)
            });
        }
    }
}

/// Qualité des FX (`setFxQuality`) : basse = sans post-traitement et moitié moins de particules.
pub fn set_fx_quality(world: &mut World, q: FxQuality) {
    if let Some(mut post) = world.get_resource_mut::<PostFxSettings>() {
        post.quality = q;
    }
    if let Some(mut ps) = world.get_resource_mut::<particles::ParticleSystem>() {
        ps.density = match q {
            FxQuality::High => 1.0,
            FxQuality::Low => 0.5,
        };
    }
}

/// Remet les FX à zéro quand on entre dans une partie ou qu'on la quitte : effets, auras, minuteur,
/// projectiles en vol et statuts de la partie précédente disparaissent.
fn reset_between_games(world: &mut World, mut last: Local<Option<(String, EntityId, f64)>>) {
    let game = world
        .get_resource::<crate::net::GameState>()
        .and_then(|s| s.game.as_ref());
    // `server_time` change à chaque `GameStart` : deux parties de suite avec le même joueur et la même
    // entité se distinguent
    let same = match (&*last, game) {
        (Some((you, id, t)), Some(g)) => *you == g.you && *id == g.entity_id && *t == g.server_time,
        (None, None) => true,
        _ => false,
    };
    if same {
        return;
    }
    *last = game.map(|g| (g.you.clone(), g.entity_id, g.server_time));
    FxRuntime::reset(world);
    world.resource_mut::<FxDispatcher>().clear();
    if let Some(mut s) = world.get_resource_mut::<world::status::ActiveStatuses>() {
        s.clear();
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

    struct Ended(std::sync::Arc<AtomicU32>);
    impl Effect for Ended {
        fn update(&mut self, _: &mut Fx, _: f32) -> bool {
            true
        }
        fn end(&mut self, _: &mut Fx) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn game_start(server_time: f64) -> lov_shared::protocol::GameStart {
        lov_shared::protocol::GameStart {
            you: "p1".into(),
            team: lov_shared::types::Team::Blue,
            entity_id: 7,
            players: Vec::new(),
            server_time,
        }
    }

    /// Deux parties de suite dans la même session : même joueur, même entité, mais un `GameStart` neuf.
    #[test]
    fn deux_parties_de_suite_remettent_les_fx_a_zero_deux_fois() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::net::GameState>()
            .init_resource::<FxRuntime>()
            .init_resource::<FxDispatcher>()
            .add_systems(Update, reset_between_games);
        let resets = std::sync::Arc::new(AtomicU32::new(0));
        let start = |app: &mut App, t: f64| {
            Fx::new(app.world_mut()).add(Ended(resets.clone()));
            app.world_mut().resource_mut::<crate::net::GameState>().game = Some(game_start(t));
            app.update();
        };
        start(&mut app, 1000.0);
        assert_eq!(resets.load(Ordering::Relaxed), 1);
        // même partie, image suivante : rien ne bouge
        app.update();
        assert_eq!(resets.load(Ordering::Relaxed), 1);
        assert_eq!(app.world().resource::<FxRuntime>().active_effects(), 0);
        start(&mut app, 250_000.0);
        assert_eq!(resets.load(Ordering::Relaxed), 2, "nouveau `GameStart`");
    }
}
