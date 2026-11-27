//! FX procéduraux de satire parlementaire de Mélenchon. Aucun asset externe.
//!
//! | sort | événements |
//! |------|------------|
//! | P | buff `melenchon_p` : petit éclat |
//! | Q | projectile `melenchon_q` : un micro file vers la cible, feuilles qui volent à l'impact |
//! | W | zone `melenchon_w_telegraph` puis `melenchon_w` : envolée de feuilles |
//! | E | zone `melenchon_e_telegraph` (un hologramme du tribun apparaît) puis dash et zone `melenchon_e` |
//! | R | abilityCast (télégraphie), zone `melenchon_r` : ondes tricolores, feuilles, pancarte |
//!
//! Portage de `packages/client/src/fx/champions/melenchon.ts`. La pancarte « LA RÉPUBLIQUE, C'EST MOI ! »,
//! un sprite dessiné dans un canvas sur le web, est ici un panneau de l'interface placé à la projection
//! de son ancre (comme les chiffres flottants).

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent, TeamOrNeutral};

use crate::fx::core::palette::{FxColors, linear};
use crate::fx::core::runtime::{Effect, Fx};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, dash_trail, flash, shockwave, telegraph, timed,
};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};
use crate::render::camera::MainCamera;
use crate::render::champions::melenchon::spawn_melenchon;
use crate::render::views::EntityView;
use crate::ui::UiFonts;

fn blue() -> Vec3 {
    linear("#2865ff")
}

fn white() -> Vec3 {
    linear("#fff8e6")
}

fn red() -> Vec3 {
    linear("#f33450")
}

fn colors() -> [Vec3; 3] {
    [blue(), white(), red()]
}

fn rgba(c: Vec3, a: f32) -> Color {
    Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, a))
}

// ---------------------------------------------------------------------------------- feuilles

/// Maillage partagé des feuilles.
#[derive(Resource)]
struct PaperMesh(Handle<Mesh>);

impl FromWorld for PaperMesh {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource_mut::<Assets<Mesh>>()
                .add(Rectangle::new(0.22, 0.3)),
        )
    }
}

/// Envolée de feuilles autour de `at`.
fn papers(fx: &mut Fx, at: Vec3, count: usize) {
    let mesh = fx.world().get_resource_or_init::<PaperMesh>().0.clone();
    let c = linear("#fff6df");
    let material = fx
        .world()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: rgba(c, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
    let root = fx
        .world()
        .spawn((
            Name::new("fx-melenchon-papers"),
            Transform::from_translation(at),
            Visibility::Inherited,
        ))
        .id();
    let pages: Vec<(Entity, Vec3)> = (0..count)
        .map(|i| {
            let i = i as f32;
            let rot = Vec3::new(i * 0.8, i * 0.7, i);
            let e = fx
                .world()
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, rot.x, rot.y, rot.z)),
                    ChildOf(root),
                ))
                .id();
            (e, rot)
        })
        .collect();
    let mat2 = material.clone();
    let mut pages = pages;
    timed(
        fx,
        0.9,
        move |fx, t, _| {
            let world = fx.world();
            if let Some(mut m) = world
                .resource_mut::<Assets<StandardMaterial>>()
                .get_mut(&material)
            {
                m.base_color = rgba(c, 1.0 - t);
            }
            for (i, (e, rot)) in pages.iter_mut().enumerate() {
                let a = i as f32 * 2.399;
                rot.x += 0.025;
                rot.z += 0.035;
                if let Some(mut tf) = world.get_mut::<Transform>(*e) {
                    tf.translation = Vec3::new(
                        a.cos() * t * 1.6,
                        0.6 + (t * std::f32::consts::PI).sin() * 1.6,
                        a.sin() * t * 1.6,
                    );
                    tf.rotation = Quat::from_euler(EulerRot::XYZ, rot.x, rot.y, rot.z);
                }
            }
        },
        move |fx| {
            let world = fx.world();
            if let Ok(e) = world.get_entity_mut(root) {
                e.despawn();
            }
            world
                .resource_mut::<Assets<StandardMaterial>>()
                .remove(&mat2);
        },
    );
}

// ---------------------------------------------------------------------------------- pancarte

/// Taille de la pancarte (monde), comme le sprite du web.
const SIGN_W: f32 = 5.4;
const SIGN_H: f32 = 1.35;

struct Proclamation {
    root: Entity,
    stripes: [Entity; 3],
    lines: [Entity; 2],
    at: Vec3,
    t: f32,
}

impl Effect for Proclamation {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt / 2.0;
        let t = self.t.min(1.0);
        let opacity = (t * 10.0).min((1.0 - t) * 5.0).clamp(0.0, 1.0);
        let anchor = self.at + Vec3::Y * (3.4 + t * 0.45);
        let world = fx.world();
        let placed = {
            let mut q = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
            q.iter(world).next().and_then(|(camera, tf)| {
                let at = camera.world_to_viewport(tf, anchor).ok()?;
                let side = camera
                    .world_to_viewport(tf, anchor + tf.right() * 1.0)
                    .ok()?;
                Some((at, (side - at).length()))
            })
        };
        if let Some((at, ppu)) = placed {
            let (w, h) = (SIGN_W * ppu, SIGN_H * ppu);
            if let Some(mut node) = world.get_mut::<Node>(self.root) {
                node.left = Val::Px(at.x - w / 2.0);
                node.top = Val::Px(at.y - h / 2.0);
                node.width = Val::Px(w);
                node.height = Val::Px(h);
                node.padding = UiRect::top(Val::Px(h * 0.04));
            }
            if let Some(mut bg) = world.get_mut::<BackgroundColor>(self.root) {
                bg.0 = Color::srgba(17.0 / 255.0, 28.0 / 255.0, 50.0 / 255.0, opacity);
            }
            for (i, e) in self.stripes.iter().enumerate() {
                if let Some(mut bg) = world.get_mut::<BackgroundColor>(*e) {
                    bg.0 = rgba(colors()[i], opacity);
                }
                if let Some(mut node) = world.get_mut::<Node>(*e) {
                    node.height = Val::Px(h * 7.0 / 256.0);
                }
            }
            for e in self.lines {
                if let Some(mut font) = world.get_mut::<TextFont>(e) {
                    font.font_size = (h * 60.0 / 256.0).into();
                }
                if let Some(mut c) = world.get_mut::<TextColor>(e) {
                    c.0 = Color::srgba(1.0, 246.0 / 255.0, 229.0 / 255.0, opacity);
                }
            }
            if let Some(mut v) = world.get_mut::<Visibility>(self.root) {
                v.set_if_neq(Visibility::Inherited);
            }
        } else if let Some(mut v) = world.get_mut::<Visibility>(self.root) {
            v.set_if_neq(Visibility::Hidden);
        }
        self.t < 1.0
    }

    fn end(&mut self, fx: &mut Fx) {
        if let Ok(e) = fx.world().get_entity_mut(self.root) {
            e.despawn();
        }
    }
}

fn proclamation(fx: &mut Fx, at: Vec3) {
    let font = fx
        .world()
        .get_resource::<UiFonts>()
        .map(|f| f.display(24.0, 700));
    let world = fx.world();
    let root = world
        .spawn((
            Name::new("fx-melenchon-pancarte"),
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(-4),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .id();
    let band = world
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let stripes = [0, 1, 2].map(|_| {
        world
            .spawn((
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(band),
            ))
            .id()
    });
    let lines = ["LA RÉPUBLIQUE,", "C’EST MOI !"].map(|text| {
        world
            .spawn((
                Text::new(text),
                font.clone()
                    .unwrap_or_else(|| TextFont::from_font_size(24.0)),
                TextColor(Color::NONE),
                Node {
                    margin: UiRect::top(Val::Percent(1.0)),
                    ..default()
                },
                ChildOf(root),
            ))
            .id()
    });
    fx.add(Proclamation {
        root,
        stripes,
        lines,
        at,
        t: 0.0,
    });
}

// ---------------------------------------------------------------------------------- Q

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let world = fx.world();
    let (handle_mesh, head_mesh) = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Cylinder::new(0.045, 0.35).mesh().resolution(8)),
            meshes.add(Sphere::new(0.12).mesh().uv(8, 6)),
        )
    };
    let (handle_mat, head_mat) = {
        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        (
            mats.add(StandardMaterial {
                base_color: Color::srgb_u8(0xce, 0xd8, 0xdf),
                metallic: 0.5,
                perceptual_roughness: 0.4,
                ..default()
            }),
            mats.add(StandardMaterial {
                base_color: Color::srgb_u8(0x26, 0x36, 0x4a),
                perceptual_roughness: 0.9,
                ..default()
            }),
        )
    };
    let from = Fx::world_pos(*from, 0.0);
    let mic = world
        .spawn((
            Name::new("fx-melenchon-micro"),
            Transform::from_translation(from.with_y(1.3)),
            Visibility::Inherited,
        ))
        .id();
    world.spawn((
        Mesh3d(handle_mesh),
        MeshMaterial3d(handle_mat),
        Transform::IDENTITY,
        ChildOf(mic),
    ));
    world.spawn((
        Mesh3d(head_mesh),
        MeshMaterial3d(head_mat),
        Transform::from_xyz(0.0, 0.22, 0.0),
        ChildOf(mic),
    ));
    let spark = fx.styles().spark;
    let stop = ProjectileFx::launch(
        fx,
        from,
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: red(),
            core: Some(white()),
            size: 0.38,
            height: 1.3,
            ribbon_width: 0.14,
            trail_style: Some(spark),
            trail_density: 5.0,
            on_step: Some(Box::new(move |fx, pos, dir, _| {
                if let Some(mut tf) = fx.world().get_mut::<Transform>(mic) {
                    tf.translation = pos;
                    tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::X));
                }
            })),
            on_end: Some(Box::new(move |fx, _| {
                if let Ok(e) = fx.world().get_entity_mut(mic) {
                    e.despawn();
                }
            })),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        papers(fx, Fx::world_pos(*pos, 0.0), 6);
        flash(fx, Fx::world_pos(*pos, 1.0), 1.1, red(), 0.25);
    }
}

// ---------------------------------------------------------------------------------- W, E

fn w_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    {
        telegraph(
            fx,
            Fx::world_pos(*pos, 0.0),
            Fx::scale(*radius as f32),
            *duration as f32,
            red(),
            TelegraphOptions {
                fill: Some(0.1),
                ..default()
            },
        );
    }
}

fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        let at = Fx::world_pos(*pos, 0.0);
        papers(fx, at, 14);
        shockwave(fx, at, Fx::scale(*radius as f32), 0.55, red(), 0.12);
    }
}

fn e_dash(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let GameEvent::Dash {
        source, from, to, ..
    } = e
    {
        dash_trail(
            fx,
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            0.08,
            c.accent,
            false,
        );
    }
}

fn e_area(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        shockwave(
            fx,
            Fx::world_pos(*pos, 0.0),
            Fx::scale(*radius as f32),
            0.5,
            c.accent,
            0.12,
        );
    }
}

/// Hologramme du tribun qui harangue à l'endroit visé pendant le délai.
fn spawn_hologram(fx: &mut Fx, source: EntityId, at: Vec3) -> Entity {
    let facing = fx
        .world()
        .get_resource::<crate::net::GameState>()
        .and_then(|s| s.pose(source))
        .map_or(0.0, |p| p.facing as f32);
    let world = fx.world();
    let root = world
        .spawn((
            Name::new("fx-melenchon-hologramme"),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::Inherited,
        ))
        .id();
    let view = world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
        world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
            let mut commands = world.commands();
            spawn_melenchon(
                &mut commands,
                &mut meshes,
                &mut materials,
                root,
                source,
                TeamOrNeutral::Blue,
                true,
            )
        })
    });
    world.flush();
    // la caricature est animée par le rendu (`animate_melenchon`) : il lui faut l'animation demandée
    world.entity_mut(root).insert((
        EntityView {
            anim: "cast_Q".into(),
            dead: false,
            drake: None,
        },
        view,
    ));
    root
}

fn e_telegraph(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AreaEffect {
        source,
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.0);
    let duration = *duration as f32;
    telegraph(
        fx,
        at,
        Fx::scale(*radius as f32),
        duration,
        c.accent,
        TelegraphOptions {
            fill: Some(0.08),
            ..default()
        },
    );
    let root = spawn_hologram(fx, *source, at);
    timed(
        fx,
        duration,
        |_, _, _| {},
        move |fx| {
            if let Ok(e) = fx.world().get_entity_mut(root) {
                e.despawn();
            }
        },
    );
}

// ---------------------------------------------------------------------------------- R, P

fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast { source, .. } = e
        && let Some(at) = fx.entity_pos(*source, 0.0)
    {
        telegraph(
            fx,
            at,
            Fx::scale(500.0),
            0.6,
            white(),
            TelegraphOptions {
                fill: Some(0.04),
                ..default()
            },
        );
    }
}

fn r_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect { pos, radius, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    for (i, color) in colors().into_iter().enumerate() {
        fx.after(i as f32 * 0.09, move |fx| {
            shockwave(fx, at, r, 0.8, color, 0.055)
        });
    }
    papers(fx, at, 18);
    proclamation(fx, at);
}

/// L'aura du modèle suit le buff réel ; la pancarte est réservée à l'impact validé par le serveur.
fn nothing(_: &mut Fx, _: &GameEvent, _: &FxColors) {}

fn p_buff(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let GameEvent::BuffApplied { target, .. } = e
        && let Some(pos) = fx.entity_pos(*target, 0.0)
    {
        flash(fx, pos, 0.7, c.secondary, 0.3);
    }
}

pub fn register(app: &mut App) {
    app.register_fx(
        "melenchon_q",
        FxHandlers {
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "melenchon_w_telegraph",
        FxHandlers {
            area: Some(w_telegraph),
            ..default()
        },
    )
    .register_fx(
        "melenchon_w",
        FxHandlers {
            area: Some(w_area),
            ..default()
        },
    )
    .register_fx(
        "melenchon_e",
        FxHandlers {
            dash: Some(e_dash),
            area: Some(e_area),
            ..default()
        },
    )
    .register_fx(
        "melenchon_e_telegraph",
        FxHandlers {
            area: Some(e_telegraph),
            ..default()
        },
    )
    .register_fx(
        "melenchon_r",
        FxHandlers {
            cast: Some(r_cast),
            area: Some(r_area),
            buff: Some(nothing),
            ..default()
        },
    )
    .register_fx(
        "melenchon_p",
        FxHandlers {
            buff: Some(p_buff),
            ..default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn feuilles_et_pancarte_disparaissent() {
        let mut w = fx_world();
        let mut fx = Fx::new(&mut w);
        papers(&mut fx, Vec3::ZERO, 6);
        proclamation(&mut fx, Vec3::ZERO);
        let named = |w: &mut World| w.query::<&Name>().iter(w).count();
        assert!(named(&mut w) >= 2);
        for _ in 0..25 {
            FxRuntime::update(&mut w, 0.1);
        }
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
        assert_eq!(named(&mut w), 0, "plus rien à l'écran");
    }

    #[test]
    fn l_hologramme_est_construit_puis_retire() {
        let mut w = fx_world();
        let root = spawn_hologram(&mut Fx::new(&mut w), 5, Vec3::ZERO);
        assert!(w.get::<EntityView>(root).is_some());
        let meshes = w.query::<&Mesh3d>().iter(&w).count();
        assert!(meshes > 10, "la caricature complète : {meshes} maillages");
        w.entity_mut(root).despawn();
        assert_eq!(w.query::<&Mesh3d>().iter(&w).count(), 0);
    }
}
