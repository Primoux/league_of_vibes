//! Barres de vie 3D au-dessus des unités.
//!
//! Portage de `packages/client/src/render/HealthBars.ts` :
//! - taille fixe en pixels : projection de l'ancre 3D puis décalage écran dans le vertex shader
//!   (`healthbars.wesl`). PV, bouclier, mana (champions), graduations tous les 100 PV ;
//! - couleurs : joueur local vert-jaune, alliés bleu, ennemis rouge, neutres orange ;
//! - champions : étiquette niveau + nom, mise à jour seulement quand elle change.
//!
//! Écarts : le client web dessine toutes les barres en un seul draw call (quads instanciés) ; ici chaque
//! barre est un quad enfant de sa vue avec son propre matériau, réécrit seulement quand ses valeurs
//! changent. L'étiquette est un nœud d'interface placé à l'ancre projetée plutôt qu'un sprite de canvas ;
//! la police par défaut de Bevy n'a que l'ASCII, les noms accentués attendent la police du jeu (ui-core).

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use lov_shared::types::{EntityId, EntityKind, EntitySnapshot, Team, TeamOrNeutral};

use super::camera::MainCamera;
use super::models::kit::hex;

const SHADER: &str = "embedded://lov_client/render/healthbars.wesl";

/// Hauteur de l'ancre au-dessus du sol (monde), largeur et hauteur de la barre (px), selon le type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarLayout {
    pub h: f32,
    pub w: f32,
    pub hgt: f32,
}

const fn layout(h: f32, w: f32, hgt: f32) -> Option<BarLayout> {
    Some(BarLayout { h, w, hgt })
}

/// `barLayout` : `None` = pas de barre (projectiles, zones).
pub fn bar_layout(e: &EntitySnapshot) -> Option<BarLayout> {
    let sub = e.subtype.as_deref();
    match e.kind {
        EntityKind::Champion => layout(2.5, 104.0, 12.0),
        EntityKind::Minion => match sub {
            Some("super" | "cannon") => layout(1.5, 62.0, 5.0),
            _ => layout(1.2, 48.0, 5.0),
        },
        EntityKind::Tower => layout(6.2, 130.0, 9.0),
        EntityKind::Inhibitor => layout(2.8, 120.0, 8.0),
        EntityKind::Nexus => layout(4.6, 150.0, 10.0),
        EntityKind::Monster => match sub {
            Some("dragon" | "elder_dragon" | "leviathan" | "herald") => layout(4.2, 150.0, 10.0),
            Some("blue_buff" | "red_buff") => layout(2.4, 90.0, 7.0),
            _ => layout(1.5, 60.0, 6.0),
        },
        EntityKind::Ward => layout(0.9, 34.0, 4.0),
        _ => None,
    }
}

const COLOR_SELF: u32 = 0x9be03a;
const COLOR_ALLY: u32 = 0x3a8fff;
const COLOR_ENEMY: u32 = 0xe8433a;
const COLOR_NEUTRAL: u32 = 0xe89a2a;

/// Couleur de la barre selon le point de vue du joueur local.
pub fn bar_color(team: TeamOrNeutral, my_team: Option<Team>, is_self: bool) -> u32 {
    let mine = match my_team {
        Some(Team::Blue) => Some(TeamOrNeutral::Blue),
        Some(Team::Red) => Some(TeamOrNeutral::Red),
        None => None,
    };
    if is_self {
        COLOR_SELF
    } else if team == TeamOrNeutral::Neutral {
        COLOR_NEUTRAL
    } else if Some(team) == mine {
        COLOR_ALLY
    } else {
        COLOR_ENEMY
    }
}

/// Uniforms du shader (`Bar` dans `healthbars.wesl`).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct BarUniform {
    pub color: Vec4,
    /// PV, bouclier, mana (-1 = pas de mana), PV max (0 = pas de graduations).
    pub bars: Vec4,
    /// Largeur, hauteur (px physiques).
    pub size: Vec4,
}

/// Valeurs d'une barre (`push` du client web).
pub fn bar_uniform(e: &EntitySnapshot, l: BarLayout, color: u32, scale: f32) -> BarUniform {
    let max_hp = e.max_hp.max(1.0);
    let shield = e.shield.unwrap_or(0.0);
    let total = max_hp.max(e.hp + shield);
    let champion = e.kind == EntityKind::Champion;
    let max_mana = e.max_mana.unwrap_or(0.0);
    let mana = if champion && max_mana > 0.0 {
        e.mana.unwrap_or(0.0) / max_mana
    } else {
        -1.0
    };
    BarUniform {
        color: LinearRgba::from(hex(color)).to_vec4(),
        bars: Vec4::new(
            (e.hp / total) as f32,
            (shield / total) as f32,
            mana as f32,
            if champion { total as f32 } else { 0.0 },
        ),
        size: Vec4::new(l.w * scale, l.hgt * scale, 0.0, 0.0),
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct BarMaterial {
    #[uniform(0)]
    pub u: BarUniform,
}

impl Material for BarMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    /// Dessinées après tout le reste (`renderOrder = 1000`), comme les FX (voir `fx::render_order`).
    fn depth_bias(&self) -> f32 {
        1000.0 * 1.0e5
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        // `depthTest: false, depthWrite: false` : toujours au-dessus des modèles.
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_compare = Some(CompareFunction::Always);
            ds.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

/// Quad unité dans le plan xy (`PlaneGeometry(1, 1)`).
#[derive(Resource)]
struct BarQuad(Handle<Mesh>);

impl FromWorld for BarQuad {
    fn from_world(world: &mut World) -> Self {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
                [-0.5, 0.5, 0.0],
            ],
        )
        .with_inserted_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]));
        Self(world.resource_mut::<Assets<Mesh>>().add(mesh))
    }
}

/// Étiquette d'un champion : pastille de niveau et nom.
struct Label {
    root: Entity,
    level_text: Entity,
    name_text: Entity,
    level: u32,
    name: String,
}

/// Barre d'une entité.
struct BarSlot {
    view: Entity,
    bar: Entity,
    material: Handle<BarMaterial>,
    last: BarUniform,
    label: Option<Label>,
}

#[derive(Resource, Default)]
pub struct HealthBars(HashMap<EntityId, BarSlot>);

pub struct HealthBarsPlugin;

impl Plugin for HealthBarsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "healthbars.wesl");
        app.add_plugins(MaterialPlugin::<BarMaterial>::default())
            .init_resource::<BarQuad>()
            .init_resource::<HealthBars>();
    }
}

/// Taille de l'étiquette (le canvas de 256 × 64 est affiché à 192 × 48 px) et position de ses éléments.
const LABEL_W: f32 = 192.0;
const LABEL_H: f32 = 48.0;
const K: f32 = LABEL_W / 256.0;

fn spawn_label(commands: &mut Commands) -> Label {
    let level_text = commands
        .spawn((
            Text::new("1"),
            TextFont::from_font_size(26.0 * K),
            TextColor(hex(0xf0e6d2)),
        ))
        .id();
    // pastille de niveau : disque de rayon 18 centré en (22, 40) sur le canvas, liseré doré
    let badge = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((22.0 - 18.0) * K),
                top: Val::Px((40.0 - 18.0) * K),
                width: Val::Px(36.0 * K),
                height: Val::Px(36.0 * K),
                border: UiRect::all(Val::Px(2.0 * K)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(10.0 / 255.0, 10.0 / 255.0, 16.0 / 255.0, 0.85)),
            BorderColor::all(hex(0xc8aa6e)),
        ))
        .add_child(level_text)
        .id();
    let name_text = commands
        .spawn((
            Text::new(""),
            TextFont::from_font_size(26.0 * K),
            TextColor(hex(0xf0e6d2)),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::srgba(0.0, 0.0, 0.0, 0.8),
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(46.0 * K),
                top: Val::Px((40.0 - 15.0) * K),
                ..default()
            },
        ))
        .id();
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(LABEL_W),
                height: Val::Px(LABEL_H),
                ..default()
            },
            Visibility::Hidden,
        ))
        .add_children(&[badge, name_text])
        .id();
    Label {
        root,
        level_text,
        name_text,
        level: 0,
        name: String::new(),
    }
}

/// Coin supérieur gauche de l'étiquette : centrée sur l'ancre, son bas 15 % au-dessus (`center = (0.5,
/// -0.15)` du sprite).
pub fn label_origin(anchor: Vec2) -> Vec2 {
    Vec2::new(anchor.x - LABEL_W / 2.0, anchor.y - LABEL_H * 1.15)
}

/// Accès groupés de `update_bars`.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct BarWorld<'w, 's> {
    commands: Commands<'w, 's>,
    quad: Res<'w, BarQuad>,
    materials: ResMut<'w, Assets<BarMaterial>>,
    bars: ResMut<'w, HealthBars>,
    window: Single<'w, 's, &'static Window>,
    camera: Single<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    nodes: Query<'w, 's, (&'static mut Node, &'static mut Visibility), Without<Text>>,
    texts: Query<'w, 's, &'static mut Text>,
    transforms: Query<'w, 's, &'static Transform>,
}

/// Met à jour les barres de toutes les vues (`begin` / `push` / `end` du client web) : `views` associe
/// chaque entité du serveur à sa vue.
pub(super) fn update_bars<'a>(
    w: &mut BarWorld,
    entities: &HashMap<EntityId, EntitySnapshot>,
    views: impl Iterator<Item = (&'a EntityId, Entity)>,
    my_entity: Option<EntityId>,
    my_team: Option<Team>,
) {
    let scale = w.window.scale_factor();
    let (camera, cam_transform) = *w.camera;
    let mut seen: Vec<EntityId> = Vec::new();
    for (id, view) in views {
        let Some(e) = entities.get(id) else {
            continue;
        };
        let Some(l) = bar_layout(e) else {
            continue;
        };
        seen.push(*id);
        // vue reconstruite (drake) : la barre suit la nouvelle vue
        if w.bars.0.get(id).is_some_and(|s| s.view != view)
            && let Some(old) = w.bars.0.remove(id)
            && let Some(label) = old.label
        {
            w.commands.entity(label.root).despawn();
        }
        if !w.bars.0.contains_key(id) {
            let material = w.materials.add(BarMaterial {
                u: BarUniform::default(),
            });
            let bar = w
                .commands
                .spawn((
                    Mesh3d(w.quad.0.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(0.0, l.h, 0.0),
                    NoFrustumCulling,
                    ChildOf(view),
                ))
                .id();
            let label = (e.kind == EntityKind::Champion).then(|| spawn_label(&mut w.commands));
            w.bars.0.insert(
                *id,
                BarSlot {
                    view,
                    bar,
                    material,
                    last: BarUniform::default(),
                    label,
                },
            );
        }
        let Some(slot) = w.bars.0.get_mut(id) else {
            continue;
        };
        let dead = e.dead == Some(true);
        w.commands.entity(slot.bar).insert(if dead {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        let color = bar_color(e.team, my_team, my_entity == Some(*id));
        let u = bar_uniform(e, l, color, scale);
        if u != slot.last
            && let Some(mut m) = w.materials.get_mut(&slot.material)
        {
            m.u = u;
            slot.last = u;
        }
        let Some(label) = slot.label.as_mut() else {
            continue;
        };
        let anchor = w
            .transforms
            .get(view)
            .ok()
            .map(|t| t.translation + Vec3::Y * l.h)
            .and_then(|p| camera.world_to_viewport(cam_transform, p).ok());
        if let Ok((mut node, mut vis)) = w.nodes.get_mut(label.root) {
            match anchor.filter(|_| !dead) {
                Some(a) => {
                    let o = label_origin(a);
                    node.left = Val::Px(o.x);
                    node.top = Val::Px(o.y);
                    *vis = Visibility::Inherited;
                }
                None => *vis = Visibility::Hidden,
            }
        }
        // redessinée seulement si le niveau ou le nom change ; le texte d'une étiquette créée à cette
        // frame n'existe pas encore : la valeur n'est retenue qu'une fois écrite
        let level = e.level.unwrap_or(1);
        let name = e.champ.as_ref().map_or("", |c| c.name.as_str());
        if label.level != level
            && let Ok(mut t) = w.texts.get_mut(label.level_text)
        {
            t.0 = level.to_string();
            label.level = level;
        }
        if label.name != name
            && let Ok(mut t) = w.texts.get_mut(label.name_text)
        {
            t.0 = name.to_owned();
            label.name = name.to_owned();
        }
    }
    // entités disparues : la barre part avec la vue, l'étiquette est à part
    let commands = &mut w.commands;
    w.bars.0.retain(|id, slot| {
        let keep = seen.contains(id);
        if !keep && let Some(label) = &slot.label {
            commands.entity(label.root).despawn();
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(kind: EntityKind, hp: f64, max_hp: f64) -> EntitySnapshot {
        EntitySnapshot {
            id: 1,
            kind,
            team: TeamOrNeutral::Blue,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp,
            max_hp,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: None,
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    #[test]
    fn disposition_par_type() {
        let mut e = snap(EntityKind::Minion, 1.0, 1.0);
        assert_eq!(bar_layout(&e).map(|l| l.w), Some(48.0));
        e.subtype = Some("cannon".into());
        assert_eq!(bar_layout(&e).map(|l| l.w), Some(62.0));
        let mut m = snap(EntityKind::Monster, 1.0, 1.0);
        m.subtype = Some("herald".into());
        assert_eq!(bar_layout(&m).map(|l| l.h), Some(4.2));
        assert_eq!(bar_layout(&snap(EntityKind::Projectile, 1.0, 1.0)), None);
    }

    #[test]
    fn bouclier_et_mana() {
        let mut e = snap(EntityKind::Champion, 600.0, 1000.0);
        e.shield = Some(600.0);
        e.mana = Some(50.0);
        e.max_mana = Some(200.0);
        let l = bar_layout(&e).unwrap_or(BarLayout {
            h: 0.0,
            w: 0.0,
            hgt: 0.0,
        });
        let u = bar_uniform(&e, l, COLOR_SELF, 2.0);
        // PV + bouclier dépassent le max : la barre se rapporte au total
        assert!((u.bars.x - 0.5).abs() < 1e-6 && (u.bars.y - 0.5).abs() < 1e-6);
        assert!((u.bars.z - 0.25).abs() < 1e-6);
        assert_eq!(u.bars.w, 1200.0);
        assert_eq!((u.size.x, u.size.y), (208.0, 24.0));
        // pas de mana ni de graduations hors champions
        let u = bar_uniform(&snap(EntityKind::Minion, 10.0, 20.0), l, COLOR_ALLY, 1.0);
        assert_eq!((u.bars.z, u.bars.w), (-1.0, 0.0));
    }

    #[test]
    fn couleurs_selon_le_point_de_vue() {
        use TeamOrNeutral::*;
        assert_eq!(bar_color(Red, Some(Team::Blue), true), COLOR_SELF);
        assert_eq!(bar_color(Blue, Some(Team::Blue), false), COLOR_ALLY);
        assert_eq!(bar_color(Red, Some(Team::Blue), false), COLOR_ENEMY);
        assert_eq!(bar_color(Neutral, Some(Team::Blue), false), COLOR_NEUTRAL);
        assert_eq!(bar_color(Blue, None, false), COLOR_ENEMY);
    }

    #[test]
    fn etiquette_au_dessus_de_l_ancre() {
        let o = label_origin(Vec2::new(500.0, 300.0));
        assert_eq!(o.x, 500.0 - 96.0);
        assert!(
            o.y + LABEL_H < 300.0,
            "le bas de l'étiquette est au-dessus de l'ancre"
        );
    }
}
