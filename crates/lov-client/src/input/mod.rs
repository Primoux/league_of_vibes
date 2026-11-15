//! Entrées : ordres de déplacement au clic droit, et ce que le navigateur ne permettait pas —
//! curseur confiné à la fenêtre, plein écran, vsync au choix.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, MonitorSelection, PresentMode, WindowMode};
use lov_shared::protocol::ClientMsg;

use crate::net::Connection;
use crate::render::map::MAP_SIZE;
use crate::render::{to_game, to_shared};

/// Clic droit maintenu : un ordre toutes les 100 ms, comme le client web.
const HOLD_REPEAT_SECS: f32 = 0.1;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, confine_cursor)
            .add_systems(Update, (move_orders, window_shortcuts));
    }
}

fn confine_cursor(mut cursor: Single<&mut CursorOptions>) {
    cursor.grab_mode = CursorGrabMode::Confined;
}

fn move_orders(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    conn: Res<Connection>,
    time: Res<Time>,
    mut since_last: Local<f32>,
) {
    *since_last += time.delta_secs();
    let pressed = buttons.just_pressed(MouseButton::Right);
    let held = buttons.pressed(MouseButton::Right) && *since_last >= HOLD_REPEAT_SECS;
    if !pressed && !held {
        return;
    }
    let (camera, camera_transform) = *camera;
    if let Some(cursor) = window.cursor_position()
        && let Ok(ray) = camera.viewport_to_world(camera_transform, cursor)
        && let Some(point) = ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
    {
        let pos = to_game(point).clamp(Vec2::ZERO, Vec2::splat(MAP_SIZE));
        conn.send(ClientMsg::Move {
            pos: to_shared(pos),
        });
        *since_last = 0.0;
    }
}

fn window_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut Window>,
    mut cursor: Single<&mut CursorOptions>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
    if keys.just_pressed(KeyCode::KeyV) {
        window.present_mode = match window.present_mode {
            PresentMode::AutoVsync => PresentMode::AutoNoVsync,
            _ => PresentMode::AutoVsync,
        };
    }
    // Échap libère la souris (pour sortir de la fenêtre), un second appui la confine à nouveau.
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = match cursor.grab_mode {
            CursorGrabMode::None => CursorGrabMode::Confined,
            _ => CursorGrabMode::None,
        };
    }
    if keys.pressed(KeyCode::ControlLeft) && keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}
