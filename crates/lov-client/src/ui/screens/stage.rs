//! Scène 3D vivante d'un champion : portage de `packages/client/src/ui/stage/ChampionStage.tsx` et de
//! l'éclairage de `studio.ts`.
//!
//! Le modèle animé du champion sur un piédestal lumineux à ses couleurs, particules ascendantes, rotation
//! lente (glisser pour tourner). Utilisée par l'accueil et la draft. `pulse` qui change = le champion joue
//! son ultime (verrouillage, changement de vedette).
//!
//! La scène est rendue dans une image par sa propre caméra, sur un calque (`RenderLayers`) à elle : rien
//! du jeu n'y apparaît et elle n'apparaît pas dans le jeu. L'entité `ChampionStage` porte la scène et vit
//! le temps de son écran ; un nœud d'interface `StageView` affiche son image, que l'écran peut
//! reconstruire sans recréer la scène.
//!
//! Écart : Mélenchon a sa propre vue, pilotée par le snapshot du serveur ; sur la scène il garde sa pose
//! de repos et ne joue pas son ultime.

use std::f32::consts::{PI, TAU};

use bevy::app::{HierarchyPropagatePlugin, Propagate};
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use lov_shared::data::champions::get_champion;
use lov_shared::types::{EntityKind, EntitySnapshot, TeamOrNeutral};

use crate::fx::post::PostFxCamera;
use crate::render::champions::model::{ModelColors, build_model, spawn_model};
use crate::render::champions::view::ChampionView;
use crate::render::champions::{ChampionAssets, melenchon, model_for};

const PARTICLES: usize = 90;
/// Identifiant des vues hors partie (aucune entité du serveur ne le porte).
const STAGE_ID: u32 = u32::MAX;
/// Premier calque des scènes (les suivants sont pris à tour de rôle).
const FIRST_LAYER: usize = 8;
const LAYERS: usize = 16;
/// Lux d'une unité d'intensité three.js : le soleil de la carte (2,75) vaut 9 000 lx côté Bevy.
const LUX: f32 = 3300.0;

/// Scène d'un champion. `champion` et `pulse` sont pilotés par l'écran.
#[derive(Component)]
pub struct ChampionStage {
    pub champion: Option<String>,
    /// Incrémenter pour déclencher une animation d'éclat (ultime).
    pub pulse: u32,
    /// Rotation automatique (rad/s).
    pub spin: f32,
    /// Élan donné en glissant, amorti à chaque image.
    pub drag: f32,
    pub image: Handle<Image>,
    layer: usize,
}

/// Nœud d'interface qui affiche l'image d'une scène (et la fait tourner quand on le glisse).
#[derive(Component)]
pub struct StageView(pub Entity);

/// État de la scène : entités et animation.
#[derive(Component)]
struct Scene {
    camera: Entity,
    root: Entity,
    rim: [Entity; 2],
    ring: Handle<StandardMaterial>,
    ring2: Entity,
    accents: [Handle<StandardMaterial>; 4],
    particles: Vec<(Entity, f32)>,
    model: Option<Model>,
    shown: Option<String>,
    pulse: u32,
    angle: f32,
    appear: f32,
    /// Hauteur du modèle, pour cadrer la caméra.
    height: f32,
    rng: u32,
}

struct Model {
    entity: Entity,
    view: Option<ChampionView>,
    snap: EntitySnapshot,
}

/// Prochain calque à attribuer.
#[derive(Resource, Default)]
struct NextLayer(usize);

impl ChampionStage {
    /// Fait apparaître une scène (sans champion) ; elle disparaît avec l'écran `screen`.
    pub fn spawn<S: States>(
        commands: &mut Commands,
        images: &mut Assets<Image>,
        screen: S,
        spin: f32,
    ) -> Entity {
        let image = images.add(Image::new_target_texture(
            16,
            16,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
        commands
            .spawn((
                ChampionStage {
                    champion: None,
                    pulse: 0,
                    spin,
                    drag: 0.0,
                    image,
                    layer: 0,
                },
                DespawnOnExit(screen),
                Transform::default(),
                Visibility::default(),
            ))
            .id()
    }
}

/// Pseudo-aléatoire de la scène (`Math.random` du web : pas besoin d'être reproductible).
fn random(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state >> 8) as f32 / (1u32 << 24) as f32
}

/// Position de départ d'une particule : anneau autour du piédestal, hauteur `y`.
fn particle_start(rng: &mut u32, y: f32) -> (Vec3, f32) {
    let a = random(rng) * TAU;
    let r = 0.25 + random(rng) * 0.6;
    let speed = 0.15 + random(rng) * 0.45;
    (Vec3::new(a.cos() * r, y, a.sin() * r), speed)
}

/// Anneau plat (ou arc de `arc` radians), à plat dans le plan xz, normale vers le haut.
fn ring_mesh(inner: f32, outer: f32, segments: u32, arc: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=segments {
        // `RingGeometry` couchée par `rotation.x = −π/2` : (cos a, sin a, 0) devient (cos a, 0, −sin a)
        let a = arc * i as f32 / segments as f32;
        for r in [inner, outer] {
            positions.push([a.cos() * r, 0.0, -a.sin() * r]);
            uvs.push([i as f32 / segments as f32, r / outer]);
        }
        if i < segments {
            let k = i * 2;
            indices.extend([k, k + 1, k + 3, k, k + 3, k + 2]);
        }
    }
    let n = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Halo radial blanc (`glowTexture`) : opaque au centre, 0,55 au quart, transparent au bord.
pub fn glow_texture() -> Image {
    const N: u32 = 128;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let d = Vec2::new(x as f32 + 0.5 - 64.0, y as f32 + 0.5 - 64.0).length() / 64.0;
            let a = if d < 0.25 {
                1.0 - d / 0.25 * 0.45
            } else {
                (0.55 * (1.0 - (d - 0.25) / 0.75)).max(0.0)
            };
            data.extend([255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Matériau additif non éclairé (`MeshBasicMaterial` en `AdditiveBlending`, sans écriture de profondeur).
fn additive(color: Color, alpha: f32, texture: Option<Handle<Image>>) -> StandardMaterial {
    StandardMaterial {
        base_color: color.with_alpha(alpha),
        base_color_texture: texture,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// Lumières de studio (`studioLights`) : ciel, clé chaude, appoint froid, deux contre-jours à la couleur
/// du champion. Renvoie les contre-jours.
pub fn studio_lights(parent: &mut ChildSpawnerCommands, rim: Color) -> [Entity; 2] {
    let light = |color: u32, intensity: f32| DirectionalLight {
        color: crate::ui::theme::hex(color),
        illuminance: intensity * LUX,
        ..default()
    };
    let at = |x: f32, y: f32, z: f32| Transform::from_xyz(x, y, z).looking_at(Vec3::ZERO, Vec3::Y);
    parent.spawn((light(0xfff0d8, 2.4), at(2.5, 3.5, 4.0)));
    parent.spawn((light(0x8fb4ff, 0.7), at(-3.0, 1.5, 2.0)));
    let mut rim_light = |intensity: f32, pos: Transform| {
        parent
            .spawn((
                DirectionalLight {
                    color: rim,
                    illuminance: intensity * LUX,
                    ..default()
                },
                pos,
            ))
            .id()
    };
    [
        rim_light(3.2, at(-2.5, 2.5, -3.5)),
        rim_light(2.0, at(3.0, 1.2, -3.0)),
    ]
}

/// `HemisphereLight(0xdfe6ff, 0x1a1020, 0.9)` approchée par une lumière ambiante (moyenne ciel/sol).
pub fn studio_ambient() -> AmbientLight {
    AmbientLight {
        color: Color::srgb(0.48, 0.47, 0.56),
        brightness: 0.9 * LUX / PI,
        ..default()
    }
}

/// État factice minimal pour animer une vue hors de la partie (`idleSnap`).
pub fn idle_snap(id: &str) -> EntitySnapshot {
    EntitySnapshot {
        id: STAGE_ID,
        kind: EntityKind::Champion,
        team: TeamOrNeutral::Neutral,
        x: 0.0,
        y: 0.0,
        facing: 0.0,
        hp: 1.0,
        max_hp: 1.0,
        mana: None,
        max_mana: None,
        shield: None,
        dead: None,
        subtype: Some(id.into()),
        owner_id: None,
        level: None,
        anim: Some("idle".into()),
        buffs: None,
        champ: None,
    }
}

/// Modèle posé à l'origine, en pose de repos, et sa hauteur (`makeChampionModel`).
pub fn spawn_champion(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    champs: &mut ChampionAssets,
    id: &str,
) -> Option<(Entity, Option<ChampionView>, f32)> {
    let colors = ModelColors::of(id, TeamOrNeutral::Neutral)?;
    let holder = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();
    if id == melenchon::ID {
        let m = melenchon::spawn_melenchon(
            commands,
            meshes,
            &mut champs.materials,
            holder,
            STAGE_ID,
            TeamOrNeutral::Neutral,
            false,
        );
        // pose de repos : sans snapshot du serveur, la vue n'est pas animée
        let _ = m;
        return Some((holder, None, melenchon::HEIGHT));
    }
    let def = model_for(id, colors.class);
    let built = build_model(&def, &colors);
    // hauteur au repos : sommet des maillages, articulation par articulation
    let rest = def.rig.rest_world();
    let mut top = 0.0f32;
    for lot in [&built.body, &built.metal, &built.glow] {
        for (j, mesh) in lot.iter().enumerate() {
            let Some(p) = mesh
                .as_ref()
                .and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION))
                .and_then(|a| a.as_float3())
            else {
                continue;
            };
            for v in p {
                top = top.max(rest[j].y + v[1]);
            }
        }
    }
    let outline = champs.outline.0.clone();
    let model = spawn_model(
        commands,
        meshes,
        &mut champs.materials,
        &outline,
        &def,
        &colors,
        built,
    );
    commands.entity(model.skeleton.root).insert(ChildOf(holder));
    let mut view = ChampionView::new(STAGE_ID, &def, model);
    view.play_anim("idle");
    Some((holder, Some(view), top * def.scale))
}

/// Construit la scène des nouvelles `ChampionStage` (`Stage` du web : piédestal, anneaux, halo,
/// particules, caméra).
fn setup(
    mut commands: Commands,
    mut stages: Query<(Entity, &mut ChampionStage), Without<Scene>>,
    mut next: ResMut<NextLayer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, mut stage) in &mut stages {
        stage.layer = FIRST_LAYER + next.0 % LAYERS;
        next.0 += 1;
        let tex = images.add(glow_texture());
        let white = Color::WHITE;
        let disc = materials.add(StandardMaterial {
            base_color: crate::ui::theme::hex(0x15131f),
            metallic: 0.6,
            perceptual_roughness: 0.35,
            ..default()
        });
        let ring = materials.add(additive(white, 1.0, None));
        let ring2 = materials.add(additive(white, 0.5, None));
        let floor = materials.add(additive(white, 0.55, Some(tex.clone())));
        let spark = materials.add(additive(white, 0.9, Some(tex)));
        let mut rng = (0x9e37_79b9 ^ (entity.to_bits() as u32).wrapping_mul(2_654_435_761)) | 1;
        let mut rim = [Entity::PLACEHOLDER; 2];
        let mut camera = Entity::PLACEHOLDER;
        let mut root = Entity::PLACEHOLDER;
        let mut ring2_e = Entity::PLACEHOLDER;
        let mut particles = Vec::with_capacity(PARTICLES);
        let spark_mesh = meshes.add(Rectangle::new(0.05, 0.05));
        commands
            .entity(entity)
            .insert(Propagate(RenderLayers::layer(stage.layer)))
            .with_children(|s| {
                camera = s
                    .spawn((
                        Camera3d::default(),
                        Camera {
                            order: -1 - stage.layer as isize,
                            clear_color: ClearColorConfig::Custom(Color::NONE),
                            ..default()
                        },
                        RenderTarget::Image(stage.image.clone().into()),
                        Projection::Perspective(PerspectiveProjection {
                            fov: 28f32.to_radians(),
                            near: 0.01,
                            far: 100.0,
                            ..default()
                        }),
                        PostFxCamera,
                        studio_ambient(),
                        Transform::from_xyz(0.0, 1.2, 6.0),
                    ))
                    .id();
                rim = studio_lights(s, white);
                root = s.spawn((Transform::default(), Visibility::default())).id();
                // piédestal : disque sombre, deux anneaux additifs, halo au sol
                s.spawn((
                    Mesh3d(meshes.add(ConicalFrustum {
                        radius_top: 0.62,
                        radius_bottom: 0.7,
                        height: 0.08,
                    })),
                    MeshMaterial3d(disc),
                    Transform::from_xyz(0.0, -0.04, 0.0),
                ));
                s.spawn((
                    Mesh3d(meshes.add(ring_mesh(0.66, 0.7, 96, TAU))),
                    MeshMaterial3d(ring.clone()),
                    Transform::from_xyz(0.0, 0.002, 0.0),
                ));
                ring2_e = s
                    .spawn((
                        Mesh3d(meshes.add(ring_mesh(0.82, 0.84, 96, PI * 1.6))),
                        MeshMaterial3d(ring2.clone()),
                        Transform::default(),
                    ))
                    .id();
                s.spawn((
                    Mesh3d(meshes.add(Plane3d::default().mesh().size(3.2, 3.2))),
                    MeshMaterial3d(floor.clone()),
                    Transform::from_xyz(0.0, -0.085, 0.0),
                ));
                // particules ascendantes
                for _ in 0..PARTICLES {
                    let y = random(&mut rng) * 2.2;
                    let (pos, speed) = particle_start(&mut rng, y);
                    let e = s
                        .spawn((
                            Mesh3d(spark_mesh.clone()),
                            MeshMaterial3d(spark.clone()),
                            Transform::from_translation(pos),
                        ))
                        .id();
                    particles.push((e, speed));
                }
            });
        commands.entity(entity).insert(Scene {
            camera,
            root,
            rim,
            ring: ring.clone(),
            ring2: ring2_e,
            accents: [ring, ring2, floor, spark],
            particles,
            model: None,
            shown: None,
            pulse: stage.pulse,
            angle: 0.4,
            appear: 1.0,
            height: 1.8,
            rng,
        });
    }
}

/// Change de champion : nouveau modèle, couleurs des contre-jours, anneaux, halo et particules.
fn switch_champion(
    mut commands: Commands,
    mut stages: Query<(&ChampionStage, &mut Scene)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut champs: ChampionAssets,
    mut lights: Query<&mut DirectionalLight>,
) {
    for (stage, mut scene) in &mut stages {
        if stage.champion == scene.shown {
            continue;
        }
        scene.shown.clone_from(&stage.champion);
        if let Some(m) = scene.model.take() {
            commands.entity(m.entity).despawn();
        }
        let Some(id) = stage.champion.as_deref() else {
            continue;
        };
        let Some(def) = get_champion(id) else {
            continue;
        };
        let Some((entity, view, height)) =
            spawn_champion(&mut commands, &mut meshes, &mut champs, id)
        else {
            continue;
        };
        commands.entity(entity).insert(ChildOf(scene.root));
        let primary = crate::ui::theme::css(&def.palette.primary);
        let accent = crate::ui::theme::css(&def.palette.accent);
        for (e, c) in scene.rim.iter().zip([primary, accent]) {
            if let Ok(mut l) = lights.get_mut(*e) {
                l.color = c;
            }
        }
        // anneau, arc, halo au sol, particules
        for (h, c) in scene.accents.iter().zip([primary, accent, primary, accent]) {
            if let Some(mut m) = champs.materials.get_mut(h) {
                let a = m.base_color.alpha();
                m.base_color = c.with_alpha(a);
            }
        }
        scene.model = Some(Model {
            entity,
            view,
            snap: idle_snap(id),
        });
        scene.height = height.max(0.3);
        scene.appear = 0.0;
    }
}

/// Une image de la scène : rotation, apparition, ultime, anneaux, particules, cadrage (`start` et `resize`).
#[allow(clippy::too_many_arguments)] // un système Bevy lit autant de requêtes que la scène a de parties
fn animate(
    time: Res<Time>,
    mut stages: Query<(&mut ChampionStage, &mut Scene)>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
    mut projections: Query<&mut Projection>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (mut stage, mut scene) in &mut stages {
        let scene = &mut *scene;
        scene.angle += (stage.spin + stage.drag) * dt;
        stage.drag *= 0.92;
        if let Some(m) = scene.model.as_mut()
            && let Some(view) = m.view.as_mut()
        {
            if stage.pulse != scene.pulse {
                view.cast("R");
            }
            let f = view.step(&m.snap, "idle", dt, Vec3::ZERO, 0.0);
            let model = &view.model;
            for (j, tr) in model.skeleton.joints.iter().zip(f.joints) {
                if let Ok(mut t) = transforms.get_mut(*j) {
                    *t = tr;
                }
            }
            if let Ok(mut tr) = transforms.get_mut(model.skeleton.root) {
                tr.rotation = Quat::from_rotation_y(f.model_yaw);
                tr.translation.y = f.model_lift;
            }
            for (e, (tr, on)) in model.extras.iter().zip(&f.extras) {
                if let Ok(mut t) = transforms.get_mut(*e) {
                    *t = *tr;
                }
                if let Ok(mut v) = visibility.get_mut(*e) {
                    v.set_if_neq(if *on {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    });
                }
            }
        }
        scene.pulse = stage.pulse;
        // apparition : montée + mise à l'échelle
        scene.appear = (scene.appear + dt * 2.2).min(1.0);
        let e = 1.0 - (1.0 - scene.appear).powi(3);
        if let Ok(mut tr) = transforms.get_mut(scene.root) {
            *tr = Transform::from_xyz(0.0, (1.0 - e) * -0.15, 0.0)
                .with_rotation(Quat::from_rotation_y(scene.angle))
                .with_scale(Vec3::splat(0.85 + 0.15 * e));
        }
        if let Ok(mut tr) = transforms.get_mut(scene.ring2) {
            tr.rotate_y(dt * 0.6);
        }
        if let Some(mut m) = materials.get_mut(&scene.ring) {
            m.base_color.set_alpha(0.65 + (t * 2.4).sin() * 0.25);
        }
        for i in 0..scene.particles.len() {
            let (e, speed) = scene.particles[i];
            let Ok(mut tr) = transforms.get_mut(e) else {
                continue;
            };
            tr.translation.y += speed * dt;
            if tr.translation.y > 2.2 {
                let (pos, speed) = particle_start(&mut scene.rng, 0.0);
                tr.translation = pos;
                scene.particles[i].1 = speed;
            }
        }
        // cadrage : tout le modèle, au moins le piédestal en largeur
        let aspect = images
            .get(&stage.image)
            .map_or(1.0, |i| i.width() as f32 / i.height().max(1) as f32);
        let h = scene.height;
        let fov = 28f32.to_radians();
        let visible = (h * 1.45).max(2.0 / aspect);
        let dist = visible / 2.0 / (fov / 2.0).tan();
        if let Ok(mut tr) = transforms.get_mut(scene.camera) {
            *tr = Transform::from_xyz(0.0, h * 0.62, dist)
                .looking_at(Vec3::new(0.0, h * 0.42, 0.0), Vec3::Y);
        }
        if let Ok(mut p) = projections.get_mut(scene.camera)
            && let Projection::Perspective(p) = &mut *p
        {
            p.aspect_ratio = aspect;
        }
    }
}

/// Taille de l'image = taille du nœud qui l'affiche (`ResizeObserver`).
fn fit_views(
    views: Query<(&StageView, &ComputedNode)>,
    stages: Query<&ChampionStage>,
    mut images: ResMut<Assets<Image>>,
) {
    for (view, node) in &views {
        let Ok(stage) = stages.get(view.0) else {
            continue;
        };
        let size = node.size().round().as_uvec2();
        if size.x < 2 || size.y < 2 {
            continue;
        }
        let same = images.get(&stage.image).is_some_and(|i| i.size() == size);
        if !same && let Some(mut image) = images.get_mut(&stage.image) {
            image.resize(Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            });
        }
    }
}

/// Affiche l'image de la scène sur les nouveaux `StageView`, et les rend glissables.
fn attach_views(
    mut commands: Commands,
    views: Query<(Entity, &StageView), Added<StageView>>,
    stages: Query<&ChampionStage>,
) {
    for (e, view) in &views {
        if let Ok(stage) = stages.get(view.0) {
            commands
                .entity(e)
                .insert(ImageNode::new(stage.image.clone()))
                .observe(drag_view);
        }
    }
}

/// Glisser fait tourner le champion : 60 rad/s par largeur de nœud parcourue.
fn drag_view(
    ev: On<PointerDrag>,
    views: Query<(&StageView, &ComputedNode)>,
    mut stages: Query<&mut ChampionStage>,
) {
    let Ok((view, node)) = views.get(ev.event_target()) else {
        return;
    };
    if let Ok(mut stage) = stages.get_mut(view.0) {
        let w = node.size().x * node.inverse_scale_factor();
        stage.drag = ev.delta.x / w.max(1.0) * 60.0;
    }
}

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<HierarchyPropagatePlugin<RenderLayers>>() {
            app.add_plugins(HierarchyPropagatePlugin::<RenderLayers>::new(PostUpdate));
        }
        app.init_resource::<NextLayer>().add_systems(
            Update,
            (setup, switch_champion, attach_views, fit_views, animate).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particules_autour_du_piedestal() {
        let mut rng = 12345;
        for _ in 0..500 {
            let (p, speed) = particle_start(&mut rng, 0.0);
            let r = Vec2::new(p.x, p.z).length();
            assert!((0.25..=0.85 + 1e-4).contains(&r), "{r}");
            assert!((0.15..=0.6).contains(&speed));
        }
    }

    #[test]
    fn anneau_et_arc() {
        let full = ring_mesh(0.66, 0.7, 96, TAU);
        assert_eq!(full.count_vertices(), 97 * 2);
        let arc = ring_mesh(0.82, 0.84, 96, PI * 1.6);
        let p = arc
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        // dernier point : angle 1,6π, couché dans le plan xz
        let last = p[p.len() - 1];
        assert!((last[0] - 0.84 * (PI * 1.6).cos()).abs() < 1e-5);
        assert!((last[2] + 0.84 * (PI * 1.6).sin()).abs() < 1e-5);
        assert_eq!(last[1], 0.0);
    }

    #[test]
    fn halo_radial() {
        let img = glow_texture();
        let data = img.data.unwrap_or_default();
        let alpha = |x: usize, y: usize| data[(y * 128 + x) * 4 + 3];
        assert!(alpha(64, 64) > 240);
        assert_eq!(alpha(0, 0), 0);
        assert!(alpha(64, 64) > alpha(64, 90));
    }
}
