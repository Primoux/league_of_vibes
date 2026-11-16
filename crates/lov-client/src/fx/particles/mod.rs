//! Particules GPU : atlas procédural, styles standard, lots additif et normal.
//!
//! Portage de `packages/client/src/fx/particles/`.

use bevy::prelude::*;

use crate::fx::core::rng::Rng;

pub mod atlas;
pub mod styles;
pub mod system;

pub use styles::StdStyles;
pub use system::{EmitOptions, EmitShape, ParticleMaterial, ParticleSystem};

/// Capacité du lot additif (le lot normal en a le quart), comme le client web.
pub const CAPACITY: usize = 16384;

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "particles.wesl");
        let mut ps = ParticleSystem::new(CAPACITY, Rng::default());
        let styles = StdStyles::create(&mut ps);
        app.add_plugins(MaterialPlugin::<ParticleMaterial>::default())
            .insert_resource(ps)
            .insert_resource(styles)
            .add_systems(Startup, system::spawn_particles)
            .add_systems(PostUpdate, system::update_particles);
    }
}
