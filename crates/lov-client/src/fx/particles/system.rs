//! Particules GPU.
//!
//! Principe : chaque particule est un quad dont les attributs (position initiale, vitesse, naissance, durée
//! de vie, gravité, traînée, style) sont écrits UNE fois à l'émission dans un tampon circulaire. Le vertex
//! shader calcule la trajectoire analytiquement à partir de l'âge et lit couleur/alpha/taille dans une
//! texture de courbes. Aucun travail CPU par particule et par frame.
//!
//! Deux lots : additif (lumières, étincelles, magie) et alpha classique (fumée, poussière, débris).
//!
//! Portage de `packages/client/src/fx/particles/ParticleSystem.ts`. Écarts avec le client web :
//! - les attributs vivent dans des tampons de stockage (`ShaderBuffer`) lus par numéro de sommet, au lieu
//!   d'attributs instanciés ;
//! - chaque lot est découpé en blocs de `CHUNK` emplacements (un tampon, un matériau, une entité par bloc).
//!   Bevy renvoie un tampon entier quand il change : seuls les blocs touchés par les plages écrites (suivies
//!   par `RingAllocator`) repartent au GPU, et un bloc sans particule vivante est masqué, donc le vertex
//!   shader ne traite que les blocs utiles (le client web dessine la capacité entière du lot) ;
//! - l'horloge du shader est `globals.time` de Bevy (`ParticleSystem::time` la suit) : aucun matériau n'est
//!   modifié à chaque frame. Quand elle reboucle (toutes les heures), les particules en vol disparaissent.

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use super::atlas::{self, Tex};
use crate::fx::core::curves::{self, CURVE_RES, ColorStop, Curve, presets};
use crate::fx::core::ring::RingAllocator;
use crate::fx::core::rng::{Range, Rng};
use crate::fx::render_order;

const MAX_STYLES: usize = 128;
/// Nombre de `vec4` par particule dans le tampon (voir `Particle` dans `particles.wesl`).
const VEC4S: usize = 5;
const FLOATS: usize = VEC4S * 4;
/// Emplacements par bloc de rendu (voir l'en-tête).
pub const CHUNK: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Add,
    Normal,
}

pub struct ParticleStyleDef {
    pub texture: Tex,
    /// Dégradé de couleur sur la vie (linéaire, HDR possible). Défaut : blanc (la teinte d'émission colore).
    pub color: &'static [ColorStop],
    pub alpha: &'static Curve,
    /// Multiplicateur de taille sur la vie.
    pub size: &'static Curve,
    pub blend: Blend,
}

impl ParticleStyleDef {
    pub const WHITE: &'static [ColorStop] = &[[0.0, 1.0, 1.0, 1.0]];

    /// Style additif blanc, alpha décroissant, taille constante (défauts du client web).
    pub const fn new(texture: Tex) -> Self {
        Self {
            texture,
            color: Self::WHITE,
            alpha: presets::FADE_OUT,
            size: presets::ONE,
            blend: Blend::Add,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticleStyle {
    pub index: usize,
    pub tex: Tex,
    pub blend: Blend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitShape {
    Point,
    Sphere,
    Disc,
    Ring,
    Cone,
    Line,
}

#[derive(Clone, Debug)]
pub struct EmitOptions {
    pub count: f32,
    /// Durée de vie (s).
    pub life: Range,
    /// Taille (unités monde, 1 = 100 unités de jeu).
    pub size: Range,
    /// Vitesse (unités monde / s).
    pub speed: Option<Range>,
    pub shape: EmitShape,
    /// Rayon de la forme d'émission (unités monde).
    pub radius: f32,
    /// Axe du cône (normalisé), défaut +Y.
    pub dir: Option<Vec3>,
    /// Demi-angle du cône (rad).
    pub angle: f32,
    /// Fin du segment pour `EmitShape::Line`.
    pub to: Option<Vec3>,
    /// Vitesse verticale ajoutée (unités/s).
    pub up: Option<Range>,
    /// Vitesse ajoutée à toutes les particules (ex. vitesse du projectile).
    pub inherit: Option<Vec3>,
    /// Gravité (unités/s², positive = vers le bas).
    pub gravity: f32,
    /// Traînée exponentielle (1/s).
    pub drag: f32,
    /// Vitesse de rotation (rad/s).
    pub spin: Option<Range>,
    /// > 0 : quad étiré dans le sens de la vitesse (étincelles).
    pub stretch: f32,
    /// Teinte multipliée à la couleur du style (linéaire).
    pub tint: Vec3,
    /// Multiplicateur de luminosité (> 1 = bloom).
    pub intensity: f32,
    /// Retard d'apparition (s) — utile pour étaler une salve.
    pub delay: Option<Range>,
}

impl Default for EmitOptions {
    fn default() -> Self {
        Self {
            count: 1.0,
            life: Range::Fixed(1.0),
            size: Range::Fixed(1.0),
            speed: None,
            shape: EmitShape::Sphere,
            radius: 0.0,
            dir: None,
            angle: 0.4,
            to: None,
            up: None,
            inherit: None,
            gravity: 0.0,
            drag: 0.0,
            spin: None,
            stretch: 0.0,
            tint: Vec3::ONE,
            intensity: 1.0,
            delay: None,
        }
    }
}

/// Bloc de rendu : `chunk` emplacements consécutifs du lot, avec leur tampon GPU et leur entité.
struct Chunk {
    /// Instant où la dernière particule écrite dans le bloc meurt : au-delà, le bloc est vide.
    end_time: f32,
    live: bool,
    /// À renvoyer au GPU.
    dirty: bool,
    buffer: Handle<ShaderBuffer>,
    entity: Option<Entity>,
}

/// Un lot de particules partageant un mode de mélange (données CPU, miroir des tampons GPU).
pub struct ParticleBatch {
    ring: RingAllocator,
    data: Vec<f32>,
    used: usize,
    /// Taille d'un bloc (`CHUNK`, ou la capacité si elle est plus petite).
    chunk: usize,
    chunks: Vec<Chunk>,
}

impl ParticleBatch {
    fn new(capacity: usize) -> Self {
        let mut data = vec![0.0; capacity * FLOATS];
        // Naissance très loin dans le futur : les emplacements jamais écrits restent invisibles.
        for p in data.as_chunks_mut::<FLOATS>().0 {
            p[3] = 1e9;
            p[7] = 1.0;
        }
        let chunk = CHUNK.min(capacity);
        assert!(
            capacity.is_multiple_of(chunk),
            "capacité de particules non multiple de {CHUNK}"
        );
        let chunks = (0..capacity / chunk)
            .map(|_| Chunk {
                end_time: -1.0,
                live: false,
                dirty: false,
                buffer: Handle::default(),
                entity: None,
            })
            .collect();
        Self {
            ring: RingAllocator::new(capacity),
            data,
            used: 0,
            chunk,
            chunks,
        }
    }

    /// Nombre de blocs dessinés (contenant au moins une particule vivante).
    pub fn live_chunks(&self) -> usize {
        self.chunks.iter().filter(|c| c.live).count()
    }

    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }

    /// Nombre d'emplacements occupés depuis le dernier lot vide.
    pub fn used(&self) -> usize {
        self.used
    }

    /// Écrit une particule.
    fn write(&mut self, p: [f32; FLOATS], birth: f32, life: f32) {
        let i = self.ring.next();
        if self.used < self.ring.capacity() {
            self.used += 1;
        }
        let c = &mut self.chunks[i / self.chunk];
        c.end_time = c.end_time.max(birth + life);
        c.live = true;
        self.data[i * FLOATS..(i + 1) * FLOATS].copy_from_slice(&p);
    }

    /// Marque à renvoyer les blocs touchés par les plages écrites et masque ceux dont tout est mort. Quand tout
    /// le lot est mort, il repart de zéro. `wrapped` : l'horloge a rebouclé, tout ce qui volait disparaît.
    fn flush(&mut self, time: f32, wrapped: bool) {
        let ranges = self.ring.flush();
        for k in 0..ranges {
            let (start, count) = (self.ring.range_start[k], self.ring.range_count[k]);
            for c in start / self.chunk..=(start + count - 1) / self.chunk {
                self.chunks[c].dirty = true;
            }
        }
        for c in &mut self.chunks {
            if c.live && (wrapped || time > c.end_time) {
                c.live = false;
            }
        }
        if self.used > 0 && self.chunks.iter().all(|c| !c.live) {
            self.used = 0;
            self.ring.reset();
        }
    }
}

/// Système de particules : styles, lots, horloge. Ressource unique, utilisée par tous les FX.
#[derive(Resource)]
pub struct ParticleSystem {
    add: ParticleBatch,
    normal: ParticleBatch,
    curve_data: Vec<f32>,
    curves_dirty: bool,
    curves: Handle<Image>,
    styles: Vec<ParticleStyle>,
    rng: Rng,
    /// Horloge des particules (s).
    pub time: f32,
    /// Nombre de particules émises depuis le début (stat de debug / budget).
    pub emitted: u64,
    /// Multiplicateur global de nombre de particules (qualité basse = 0,5).
    pub density: f32,
}

impl ParticleSystem {
    pub fn new(capacity: usize, rng: Rng) -> Self {
        Self {
            add: ParticleBatch::new(capacity),
            normal: ParticleBatch::new(capacity >> 2),
            curve_data: vec![0.0; CURVE_RES * 4 * MAX_STYLES * 2],
            curves_dirty: true,
            curves: Handle::default(),
            styles: Vec::new(),
            rng,
            time: 0.0,
            emitted: 0,
            density: 1.0,
        }
    }

    pub fn batch(&self, blend: Blend) -> &ParticleBatch {
        match blend {
            Blend::Add => &self.add,
            Blend::Normal => &self.normal,
        }
    }

    fn batch_mut(&mut self, blend: Blend) -> &mut ParticleBatch {
        match blend {
            Blend::Add => &mut self.add,
            Blend::Normal => &mut self.normal,
        }
    }

    pub fn define_style(&mut self, def: ParticleStyleDef) -> ParticleStyle {
        assert!(
            self.styles.len() < MAX_STYLES,
            "Trop de styles de particules"
        );
        let index = self.styles.len();
        curves::bake_style(
            def.color,
            def.alpha,
            def.size,
            &mut self.curve_data,
            index * 2,
        );
        self.curves_dirty = true;
        let style = ParticleStyle {
            index,
            tex: def.texture,
            blend: def.blend,
        };
        self.styles.push(style);
        style
    }

    /// Émet `o.count` particules autour de `origin` (coordonnées monde). L'ordre des tirages aléatoires est
    /// celui du client web.
    pub fn emit(&mut self, style: ParticleStyle, origin: Vec3, o: &EmitOptions) {
        let count = ((o.count * self.density).round() as usize).max(1);
        let tint = o.tint * o.intensity;
        let code = (style.index + style.tex as usize * 256) as f32;
        let cone = Quat::from_rotation_arc(Vec3::Y, o.dir.unwrap_or(Vec3::Y));
        let time = self.time;
        for _ in 0..count {
            let rng = &mut self.rng;
            let mut p = origin;
            let speed = o.speed.map_or(0.0, |s| rng.pick(s));
            let mut v;
            match o.shape {
                EmitShape::Point | EmitShape::Sphere => {
                    v = random_unit(rng);
                    let r = if o.shape == EmitShape::Sphere {
                        o.radius * rng.next_f32().cbrt()
                    } else {
                        0.0
                    };
                    p += v * r;
                    v *= speed;
                }
                EmitShape::Disc | EmitShape::Ring => {
                    let a = rng.next_f32() * TAU;
                    let r = if o.shape == EmitShape::Ring {
                        o.radius
                    } else {
                        o.radius * rng.next_f32().sqrt()
                    };
                    let (s, c) = a.sin_cos();
                    p.x += c * r;
                    p.z += s * r;
                    v = Vec3::new(c * speed, 0.0, s * speed);
                }
                EmitShape::Cone => {
                    let ang = o.angle * rng.next_f32().sqrt();
                    let az = rng.next_f32() * TAU;
                    v = cone
                        * Vec3::new(ang.sin() * az.cos(), ang.cos(), ang.sin() * az.sin())
                        * speed;
                    if o.radius > 0.0 {
                        p.x += (rng.next_f32() - 0.5) * o.radius;
                        p.z += (rng.next_f32() - 0.5) * o.radius;
                    }
                }
                EmitShape::Line => {
                    let k = rng.next_f32();
                    p += (o.to.unwrap_or(origin) - origin) * k;
                    v = random_unit(rng) * speed;
                    if o.radius > 0.0 {
                        p.x += (rng.next_f32() - 0.5) * o.radius;
                        p.z += (rng.next_f32() - 0.5) * o.radius;
                    }
                }
            }
            if let Some(up) = o.up {
                v.y += rng.pick(up);
            }
            if let Some(inherit) = o.inherit {
                v += inherit;
            }
            let birth = time + o.delay.map_or(0.0, |d| rng.pick(d));
            let life = rng.pick(o.life);
            let size = rng.pick(o.size);
            let spin = o.spin.map_or(0.0, |s| rng.pick(s));
            let rot = rng.next_f32() * TAU;
            #[rustfmt::skip]
            let data = [
                p.x, p.y, p.z, birth,
                v.x, v.y, v.z, life,
                size, spin, o.gravity, o.drag,
                code, o.stretch, rot, 0.0,
                tint.x, tint.y, tint.z, 0.0,
            ];
            self.batch_mut(style.blend).write(data, birth, life);
        }
        self.emitted += count as u64;
    }
}

fn random_unit(rng: &mut Rng) -> Vec3 {
    let z = rng.next_f32() * 2.0 - 1.0;
    let a = rng.next_f32() * TAU;
    let r = (1.0 - z * z).sqrt();
    Vec3::new(r * a.cos(), z, r * a.sin())
}

// ------------------------------------------------------------------------------------------------
// Rendu Bevy
// ------------------------------------------------------------------------------------------------

const SHADER: &str = "embedded://lov_client/fx/particles/particles.wesl";

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ParticleMaterial {
    /// x : —, y : 1 = additif.
    #[uniform(0)]
    params: Vec4,
    #[storage(1, read_only)]
    particles: Handle<ShaderBuffer>,
    #[texture(2, sample_type = "float", filterable = false)]
    curves: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    atlas: Handle<Image>,
    blend: Blend,
}

impl Material for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        match self.blend {
            Blend::Add => AlphaMode::Add,
            Blend::Normal => AlphaMode::Blend,
        }
    }

    /// Ordre de dessin du client web : lot normal (10) puis lot additif (11).
    fn depth_bias(&self) -> f32 {
        match self.blend {
            Blend::Add => render_order(11.0),
            Blend::Normal => render_order(10.0),
        }
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
        // Quads visibles des deux côtés (la rotation peut les retourner).
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Maillage fixe de `capacity` quads : coin en xy, numéro de particule en z.
fn quads(capacity: usize) -> Mesh {
    let mut positions = Vec::with_capacity(capacity * 4);
    let mut indices = Vec::with_capacity(capacity * 6);
    for i in 0..capacity {
        let z = i as f32;
        positions.extend([
            [-0.5, -0.5, z],
            [0.5, -0.5, z],
            [0.5, 0.5, z],
            [-0.5, 0.5, z],
        ]);
        let b = (i * 4) as u32;
        indices.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

fn curve_image(data: &[f32]) -> Image {
    let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
    let mut image = Image::new(
        Extent3d {
            width: CURVE_RES as u32,
            height: (MAX_STYLES * 2) as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba32Float,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn atlas_image() -> Image {
    let mut image = Image::new(
        Extent3d {
            width: atlas::SIZE as u32,
            height: atlas::SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        atlas::build_atlas(),
        // masque, pas une couleur : pas de conversion sRGB
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Pas de mipmaps : aux niveaux réduits, une cellule déborderait sur ses voisines.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

/// Crée les maillages, matériaux et textures (une fois les styles standard définis).
pub fn spawn_particles(
    mut commands: Commands,
    mut ps: ResMut<ParticleSystem>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ParticleMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut images: ResMut<Assets<Image>>,
) {
    let atlas = images.add(atlas_image());
    let curves = images.add(curve_image(&ps.curve_data));
    ps.curves = curves.clone();
    ps.curves_dirty = false;
    for blend in [Blend::Normal, Blend::Add] {
        let batch = ps.batch_mut(blend);
        let chunk = batch.chunk;
        // un maillage et un matériau par bloc, mais le maillage est le même pour tous les blocs du lot
        let mesh = meshes.add(quads(chunk));
        let name = match blend {
            Blend::Add => "fx-particles-add",
            Blend::Normal => "fx-particles-normal",
        };
        for (k, c) in batch.chunks.iter_mut().enumerate() {
            c.buffer = buffers.add(ShaderBuffer::new(
                batch.data[k * chunk * FLOATS..(k + 1) * chunk * FLOATS].to_vec(),
                RenderAssetUsages::default(),
            ));
            let material = materials.add(ParticleMaterial {
                params: Vec4::new(0.0, if blend == Blend::Add { 1.0 } else { 0.0 }, 0.0, 0.0),
                particles: c.buffer.clone(),
                curves: curves.clone(),
                atlas: atlas.clone(),
                blend,
            });
            let entity = commands
                .spawn((
                    Name::new(format!("{name}-{k}")),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    NoFrustumCulling,
                ))
                .id();
            c.entity = Some(entity);
        }
    }
}

/// Suit l'horloge du shader et envoie au GPU ce qui a changé (`ParticleSystem.update` du client web).
pub fn update_particles(
    time: Res<Time>,
    mut ps: ResMut<ParticleSystem>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut images: ResMut<Assets<Image>>,
    mut visibility: Query<&mut Visibility>,
) {
    let now = time.elapsed_secs_wrapped();
    let wrapped = now < ps.time;
    ps.time = now;
    if ps.curves_dirty {
        ps.curves_dirty = false;
        if let Some(mut image) = images.get_mut(&ps.curves) {
            *image = curve_image(&ps.curve_data);
        }
    }
    for blend in [Blend::Add, Blend::Normal] {
        let batch = ps.batch_mut(blend);
        batch.flush(now, wrapped);
        let chunk = batch.chunk;
        for (k, c) in batch.chunks.iter_mut().enumerate() {
            if c.dirty
                && let Some(mut buffer) = buffers.get_mut(&c.buffer)
            {
                c.dirty = false;
                buffer.clear();
                buffer.extend_from_slice(&batch.data[k * chunk * FLOATS..(k + 1) * chunk * FLOATS]);
            }
            if let Some(mut vis) = c.entity.and_then(|e| visibility.get_mut(e).ok()) {
                vis.set_if_neq(if c.live {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system() -> ParticleSystem {
        ParticleSystem::new(64, Rng::new(7))
    }

    #[test]
    fn emet_dans_le_bon_lot_avec_le_code_de_style() {
        let mut ps = system();
        ps.define_style(ParticleStyleDef::new(Tex::Glow));
        let smoke = ps.define_style(ParticleStyleDef {
            blend: Blend::Normal,
            ..ParticleStyleDef::new(Tex::Smoke)
        });
        ps.emit(
            smoke,
            Vec3::new(1.0, 2.0, 3.0),
            &EmitOptions {
                count: 3.0,
                shape: EmitShape::Point,
                ..default()
            },
        );
        assert_eq!(ps.batch(Blend::Normal).used(), 3);
        assert_eq!(ps.batch(Blend::Add).used(), 0);
        assert_eq!(ps.emitted, 3);
        let p = &ps.batch(Blend::Normal).data[..FLOATS];
        assert_eq!(p[..3], [1.0, 2.0, 3.0]);
        assert_eq!(p[12], (1 + Tex::Smoke as usize * 256) as f32);
    }

    #[test]
    fn densite_arrondit_et_garde_au_moins_une_particule() {
        let mut ps = system();
        let s = ps.define_style(ParticleStyleDef::new(Tex::Dot));
        ps.density = 0.5;
        ps.emit(
            s,
            Vec3::ZERO,
            &EmitOptions {
                count: 10.0,
                ..default()
            },
        );
        assert_eq!(ps.emitted, 5);
        ps.density = 0.01;
        ps.emit(
            s,
            Vec3::ZERO,
            &EmitOptions {
                count: 10.0,
                ..default()
            },
        );
        assert_eq!(ps.emitted, 6);
    }

    #[test]
    fn formes_disque_et_anneau_restent_dans_le_rayon() {
        let mut ps = system();
        let s = ps.define_style(ParticleStyleDef::new(Tex::Dot));
        for (shape, exact) in [(EmitShape::Ring, true), (EmitShape::Disc, false)] {
            ps.emit(
                s,
                Vec3::ZERO,
                &EmitOptions {
                    count: 20.0,
                    shape,
                    radius: 2.0,
                    speed: Some(1.0.into()),
                    ..default()
                },
            );
            for p in ps
                .batch(Blend::Add)
                .data
                .as_chunks::<FLOATS>()
                .0
                .iter()
                .take(20)
            {
                let r = Vec2::new(p[0], p[2]).length();
                assert!(if exact {
                    (r - 2.0).abs() < 1e-4
                } else {
                    r <= 2.0 + 1e-4
                });
                assert_eq!(p[1], 0.0);
                assert!((Vec2::new(p[4], p[6]).length() - 1.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn le_lot_se_vide_quand_tout_est_mort() {
        let mut ps = system();
        let s = ps.define_style(ParticleStyleDef::new(Tex::Dot));
        ps.emit(
            s,
            Vec3::ZERO,
            &EmitOptions {
                count: 4.0,
                life: 0.5.into(),
                ..default()
            },
        );
        let b = &mut ps.add;
        b.flush(0.1, false);
        assert!(b.chunks[0].dirty, "écritures à téléverser");
        b.chunks[0].dirty = false;
        b.flush(0.2, false);
        assert!(!b.chunks[0].dirty, "rien de neuf");
        assert_eq!(b.live_chunks(), 1);
        b.flush(0.6, false);
        assert!(!b.chunks[0].dirty);
        assert_eq!(b.live_chunks(), 0);
        assert_eq!(b.used(), 0);
    }

    #[test]
    fn seuls_les_blocs_ecrits_sont_renvoyes_et_dessines() {
        let mut ps = ParticleSystem::new(4 * CHUNK, Rng::new(3));
        let s = ps.define_style(ParticleStyleDef::new(Tex::Dot));
        let burst = |count: f32, life: f32| EmitOptions {
            count,
            life: life.into(),
            ..default()
        };
        // 1,5 bloc écrit : blocs 0 et 1
        ps.emit(s, Vec3::ZERO, &burst((CHUNK + CHUNK / 2) as f32, 1.0));
        let b = &mut ps.add;
        b.flush(0.0, false);
        let dirty: Vec<bool> = b.chunks.iter().map(|c| c.dirty).collect();
        assert_eq!(dirty, [true, true, false, false]);
        assert_eq!(b.live_chunks(), 2);
        b.chunks.iter_mut().for_each(|c| c.dirty = false);
        // le bloc 1 reçoit des particules qui vivent plus longtemps : le bloc 0 meurt seul
        ps.time = 0.5;
        ps.emit(s, Vec3::ZERO, &burst(10.0, 2.0));
        let b = &mut ps.add;
        b.flush(1.2, false);
        let live: Vec<bool> = b.chunks.iter().map(|c| c.live).collect();
        assert_eq!(live, [false, true, false, false]);
        assert!(b.chunks[1].dirty && !b.chunks[0].dirty);
        // tout est mort : le lot repart de zéro
        b.flush(3.0, false);
        assert_eq!((b.live_chunks(), b.used()), (0, 0));
        // l'horloge qui reboucle tue ce qui volait
        ps.emit(s, Vec3::ZERO, &burst(1.0, 5.0));
        ps.add.flush(0.1, true);
        assert_eq!(ps.add.live_chunks(), 0);
    }
}
