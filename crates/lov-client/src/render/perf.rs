//! Mesure des performances : une ligne de log toutes les 5 s avec FPS moyens et temps d'image, que les
//! testeurs peuvent recopier depuis la console.

use bevy::prelude::*;

use crate::net::GameState;

const WINDOW_SECS: f32 = 5.0;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Window5s>()
            .add_systems(Update, log_perf);
    }
}

#[derive(Resource, Default)]
struct Window5s {
    elapsed: f32,
    frames: u32,
    worst: f32,
}

fn log_perf(time: Res<Time<Real>>, state: Res<GameState>, mut w: ResMut<Window5s>) {
    let dt = time.delta_secs();
    w.elapsed += dt;
    w.frames += 1;
    w.worst = w.worst.max(dt);
    if w.elapsed >= WINDOW_SECS {
        let avg = w.elapsed / w.frames as f32;
        info!(
            "perf : {:.1} FPS moyens | image {:.1} ms en moyenne, {:.1} ms au pire | {} unites",
            1.0 / avg,
            avg * 1000.0,
            w.worst * 1000.0,
            state.entities.len()
        );
        *w = Window5s::default();
    }
}
