//! Brouillard de guerre visuel : portage de `packages/client/src/render/FogOfWar.ts`.
//!
//! Une texture de visibilité basse résolution (192², blanc = visible) couvre la carte. Elle est
//! recalculée environ 12 fois par seconde à partir des entités alliées (rayons du `PLAN` §9) : la vision
//! révélée apparaît aussitôt et s'estompe en douceur quand elle est perdue (×0,7 par mise à jour).
//! Le sol, la végétation, les falaises et l'eau la lisent pour assombrir et désaturer les zones non
//! visibles : le code est commun, dans `fog.wesl`, importé par chaque shader du décor (`super::fog`) ;
//! la minicarte en superpose une version prête à afficher ([`FogOfWar::minimap`]).
//!
//! Le serveur ne transmet pas les ennemis invisibles : le brouillard est purement visuel. Hors partie,
//! la texture est blanche (brouillard désactivé, `uFowDark = 1` du web).

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use lov_shared::constants::MAP_SIZE;
use lov_shared::types::{EntityKind, EntitySnapshot, TeamOrNeutral};

use crate::net::GameState;

/// Côté de la texture de visibilité (texels).
pub const RES: usize = 192;
/// Texels par unité de jeu.
const K: f64 = RES as f64 / MAP_SIZE;
/// Période de mise à jour (s) : ≈ 12 Hz.
const PERIOD: f32 = 1.0 / 12.0;
/// Part de la vision gardée à chaque mise à jour (le web repeint du noir à 30 % d'opacité).
const KEEP: f32 = 0.7;
/// Bord doux du pinceau : opaque jusqu'à 82 % du rayon, puis dégradé jusqu'au bord.
const SOFT_FROM: f64 = 0.82;
/// Opacité du voile de la minicarte sur une zone non visible. Le web peint `globalAlpha = 0.62` en
/// `multiply` sur un canvas mélangé en sRGB ; l'UI Bevy mélange en linéaire, où le même noir paraît bien
/// plus clair : 0,88 retrouve à l'œil l'assombrissement du web.
const MINIMAP_VEIL: f32 = 0.88;
/// Module de shader commun du brouillard, importé par les shaders du décor.
const SHADER: &str = "embedded://lov_client/render/map/fog.wesl";

/// Garde chargé `fog.wesl` : un module WESL n'est résolu à l'import que s'il est chargé.
#[derive(Resource)]
pub struct FogShader {
    _handle: Handle<Shader>,
}

impl FromWorld for FogShader {
    fn from_world(world: &mut World) -> Self {
        Self {
            _handle: world.resource::<AssetServer>().load(SHADER),
        }
    }
}

/// Rayon de vision (unités de jeu) d'une entité alliée, 0 si elle ne voit rien : `visionRadius` du web.
pub fn vision_radius(kind: EntityKind) -> f64 {
    match kind {
        EntityKind::Champion | EntityKind::Tower => 1350.0,
        EntityKind::Minion => 1100.0,
        EntityKind::Inhibitor | EntityKind::Nexus => 1000.0,
        EntityKind::Ward => 900.0,
        _ => 0.0,
    }
}

/// Visibilité (0..1) de chaque texel, rangée ligne par ligne : la ligne `j` couvre `y ∈ [j, j+1) / K`.
#[derive(Debug, Clone)]
pub struct FogVis(pub Vec<f32>);

impl Default for FogVis {
    fn default() -> Self {
        Self(vec![1.0; RES * RES])
    }
}

impl FogVis {
    /// Visibilité au point de jeu `(x, y)` (texel le plus proche).
    #[cfg(test)]
    pub fn at(&self, x: f64, y: f64) -> f32 {
        let i = ((x * K) as usize).min(RES - 1);
        let j = ((y * K) as usize).min(RES - 1);
        self.0[j * RES + i]
    }

    /// Estompe l'ancienne vision.
    pub fn decay(&mut self) {
        for v in &mut self.0 {
            *v *= KEEP;
        }
    }

    /// Ajoute un disque de vision à bord doux (mélange additif, `lighter` du web), borné à 1.
    pub fn stamp(&mut self, x: f64, y: f64, radius: f64) {
        if radius <= 0.0 {
            return;
        }
        let (cx, cy, r) = (x * K, y * K, radius * K);
        let lo = |c: f64| (c - r).floor().max(0.0) as usize;
        let hi = |c: f64| ((c + r).ceil().max(0.0) as usize).min(RES);
        for j in lo(cy)..hi(cy) {
            let dy = j as f64 + 0.5 - cy;
            for i in lo(cx)..hi(cx) {
                let dx = i as f64 + 0.5 - cx;
                let t = dx.hypot(dy) / r;
                if t >= 1.0 {
                    continue;
                }
                let a = if t <= SOFT_FROM {
                    1.0
                } else {
                    (1.0 - t) / (1.0 - SOFT_FROM)
                };
                let v = &mut self.0[j * RES + i];
                *v = (*v + a as f32).min(1.0);
            }
        }
    }

    /// Une mise à jour complète : estompe, puis la vision des entités alliées vivantes.
    pub fn update<'a>(
        &mut self,
        my_team: TeamOrNeutral,
        sources: impl IntoIterator<Item = (&'a EntitySnapshot, Option<(f64, f64)>)>,
    ) {
        self.decay();
        for (e, pos) in sources {
            if e.team != my_team || e.dead == Some(true) {
                continue;
            }
            let (x, y) = pos.unwrap_or((e.x, e.y));
            self.stamp(x, y, vision_radius(e.kind));
        }
    }
}

/// Textures du brouillard, partagées par les matériaux du décor et la minicarte.
#[derive(Resource, Debug)]
pub struct FogOfWar {
    /// Visibilité (R8, blanc = visible), lue par les shaders du décor.
    pub texture: Handle<Image>,
    /// Voile de la minicarte (noir, opacité `MINIMAP_VEIL × (1 − visibilité)`), ligne 0 en haut de la carte.
    pub minimap: Handle<Image>,
    pub vis: FogVis,
    acc: f32,
    /// Brouillard actif (en partie) : sinon la texture est blanche.
    enabled: bool,
}

impl FromWorld for FogOfWar {
    fn from_world(world: &mut World) -> Self {
        let size = Extent3d {
            width: RES as u32,
            height: RES as u32,
            depth_or_array_layers: 1,
        };
        let usage = RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD;
        let mut texture = Image::new_fill(
            size,
            TextureDimension::D2,
            &[255],
            TextureFormat::R8Unorm,
            usage,
        );
        texture.sampler = ImageSampler::linear();
        let mut minimap = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            usage,
        );
        minimap.sampler = ImageSampler::linear();
        let mut images = world.resource_mut::<Assets<Image>>();
        Self {
            texture: images.add(texture),
            minimap: images.add(minimap),
            vis: FogVis::default(),
            acc: 0.0,
            enabled: false,
        }
    }
}

/// Recopie la visibilité dans la texture des shaders.
fn upload_texture(vis: &FogVis, texture: &mut Image) {
    if let Some(data) = texture.data.as_mut() {
        for (d, v) in data.iter_mut().zip(&vis.0) {
            *d = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Recopie la visibilité dans le voile de la minicarte.
fn upload_minimap(vis: &FogVis, minimap: &mut Image) {
    if let Some(data) = minimap.data.as_mut() {
        for (row, px) in data.as_chunks_mut::<{ RES * 4 }>().0.iter_mut().enumerate() {
            // la minicarte a le nord (y = MAP_SIZE) en haut
            let src = &vis.0[(RES - 1 - row) * RES..][..RES];
            for (p, v) in px.as_chunks_mut::<4>().0.iter_mut().zip(src) {
                p[3] = (MINIMAP_VEIL * (1.0 - v.clamp(0.0, 1.0)) * 255.0).round() as u8;
            }
        }
    }
}

/// Recalcule la visibilité (≈ 12 Hz) depuis les entités alliées, aux positions affichées.
pub fn update_fog(
    time: Res<Time>,
    state: Res<GameState>,
    mut fog: ResMut<FogOfWar>,
    mut images: ResMut<Assets<Image>>,
) {
    let my_team = state.game.as_ref().map(|g| match g.team {
        lov_shared::types::Team::Blue => TeamOrNeutral::Blue,
        lov_shared::types::Team::Red => TeamOrNeutral::Red,
    });
    let fog = &mut *fog;
    match my_team {
        None => {
            // hors partie : brouillard désactivé, une seule fois
            if !fog.enabled {
                return;
            }
            fog.enabled = false;
            fog.vis = FogVis::default();
        }
        Some(team) => {
            fog.acc += time.delta_secs();
            if fog.enabled && fog.acc < PERIOD {
                return;
            }
            if !fog.enabled {
                // entrée en partie : on part du noir
                fog.enabled = true;
                fog.vis.0.fill(0.0);
            }
            fog.acc = 0.0;
            let sources = state
                .entities()
                .iter()
                .map(|(id, e)| (e, state.pose(*id).map(|p| (p.x, p.y))));
            fog.vis.update(team, sources);
        }
    }
    // en place : pas de copie des images, seulement leur renvoi au GPU
    if let Some(mut texture) = images.get_mut(&fog.texture) {
        upload_texture(&fog.vis, &mut texture);
    }
    if let Some(mut minimap) = images.get_mut(&fog.minimap) {
        upload_minimap(&fog.vis, &mut minimap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::EntitySnapshot;

    fn ent(kind: EntityKind, team: TeamOrNeutral, x: f64, y: f64) -> EntitySnapshot {
        EntitySnapshot {
            id: 1,
            kind,
            team,
            x,
            y,
            facing: 0.0,
            hp: 100.0,
            max_hp: 100.0,
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
    fn un_allie_revele_son_rayon_et_pas_au_dela() {
        let mut vis = FogVis(vec![0.0; RES * RES]);
        let me = ent(EntityKind::Champion, TeamOrNeutral::Blue, 3000.0, 3000.0);
        vis.update(TeamOrNeutral::Blue, [(&me, None)]);
        assert_eq!(vis.at(3000.0, 3000.0), 1.0);
        assert_eq!(vis.at(3000.0 + 1000.0, 3000.0), 1.0, "dans le rayon");
        assert_eq!(
            vis.at(3000.0 + 1500.0, 3000.0),
            0.0,
            "hors du rayon de 1350"
        );
        let edge = vis.at(3000.0 + 1290.0, 3000.0);
        assert!(edge > 0.0 && edge < 1.0, "bord doux : {edge}");
    }

    #[test]
    fn ni_ennemi_ni_mort_ni_monstre_ne_revelent() {
        let mut vis = FogVis(vec![0.0; RES * RES]);
        let foe = ent(EntityKind::Champion, TeamOrNeutral::Red, 3000.0, 3000.0);
        let mut dead = ent(EntityKind::Champion, TeamOrNeutral::Blue, 6000.0, 6000.0);
        dead.dead = Some(true);
        let camp = ent(EntityKind::Monster, TeamOrNeutral::Neutral, 9000.0, 9000.0);
        vis.update(
            TeamOrNeutral::Blue,
            [(&foe, None), (&dead, None), (&camp, None)],
        );
        assert!(vis.0.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn la_vision_perdue_s_estompe_en_une_demi_seconde() {
        let mut vis = FogVis(vec![0.0; RES * RES]);
        let ward = ent(EntityKind::Ward, TeamOrNeutral::Blue, 6000.0, 6000.0);
        vis.update(TeamOrNeutral::Blue, [(&ward, None)]);
        assert_eq!(vis.at(6000.0, 6000.0), 1.0);
        // position affichée prioritaire sur celle du snapshot
        vis.update(TeamOrNeutral::Blue, [(&ward, Some((2000.0, 2000.0)))]);
        assert_eq!(vis.at(2000.0, 2000.0), 1.0);
        assert!((vis.at(6000.0, 6000.0) - 0.7).abs() < 1e-6);
        for _ in 0..5 {
            vis.update(TeamOrNeutral::Blue, std::iter::empty());
        }
        // ≈ 0,5 s plus tard (6 mises à jour à 12 Hz), il reste moins de 15 %
        assert!(vis.at(6000.0, 6000.0) < 0.15);
    }

    #[test]
    fn les_textures_suivent_la_visibilite() {
        let mut vis = FogVis(vec![0.0; RES * RES]);
        vis.stamp(1000.0, 1000.0, 900.0); // en bas à gauche de la carte
        let size = Extent3d {
            width: RES as u32,
            height: RES as u32,
            depth_or_array_layers: 1,
        };
        let usage = RenderAssetUsages::MAIN_WORLD;
        let mut tex = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0],
            TextureFormat::R8Unorm,
            usage,
        );
        let mut mini = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            usage,
        );
        upload_texture(&vis, &mut tex);
        upload_minimap(&vis, &mut mini);
        let i = (1000.0 * K) as usize;
        let t = tex.data.as_ref().expect("données");
        assert_eq!(t[i * RES + i], 255);
        assert_eq!(t[(RES - 1) * RES + RES - 1], 0);
        // minicarte : ligne 0 en haut (nord) ; le coin sud-ouest est visible, le nord-est voilé
        let m = mini.data.as_ref().expect("données");
        let row = RES - 1 - i;
        assert_eq!(m[(row * RES + i) * 4 + 3], 0);
        let veil = (MINIMAP_VEIL * 255.0).round() as u8;
        assert_eq!(m[3], veil, "coin nord-ouest voilé");
        assert_eq!(m[(RES - 1) * 4 + 3], veil, "coin nord-est voilé");
    }

    /// `fog.wesl` divise la position monde par `MAP_WORLD` : la carte fait 12 000 unités de jeu à 0,01.
    #[test]
    fn la_taille_de_carte_du_shader_suit_les_constantes() {
        let src = include_str!("fog.wesl");
        let line = src
            .lines()
            .find(|l| l.starts_with("const MAP_WORLD: f32 = "))
            .expect("MAP_WORLD dans fog.wesl");
        let value: f64 = line["const MAP_WORLD: f32 = ".len()..]
            .trim_end_matches(';')
            .parse()
            .expect("nombre");
        assert!((MAP_SIZE * f64::from(crate::render::RENDER_SCALE) - value).abs() < 1e-3);
    }

    /// Les shaders du décor lisent tous le code commun, aucun ne garde sa copie.
    #[test]
    fn les_shaders_du_decor_importent_le_brouillard() {
        for (name, src) in [
            ("terrain", include_str!("terrain.wesl")),
            ("foliage", include_str!("foliage.wesl")),
            ("cliff", include_str!("cliff.wesl")),
            ("water", include_str!("water.wesl")),
        ] {
            assert!(src.contains("import super::fog::"), "{name}.wesl");
            assert!(!src.contains("fn apply_fog"), "{name}.wesl copie apply_fog");
            assert!(!src.contains("120.0"), "{name}.wesl code la carte en dur");
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<GameState>()
            .init_resource::<FogOfWar>()
            .add_systems(Update, update_fog);
        app
    }

    fn texel(app: &App, x: f64, y: f64) -> u8 {
        let fog = app.world().resource::<FogOfWar>();
        let tex = app.world().resource::<Assets<Image>>();
        let data = tex.get(&fog.texture).and_then(|t| t.data.as_ref());
        let i = ((x * K) as usize).min(RES - 1);
        let j = ((y * K) as usize).min(RES - 1);
        data.expect("texture du brouillard")[j * RES + i]
    }

    #[test]
    fn entree_en_partie_au_noir_puis_sortie_au_blanc() {
        use lov_shared::protocol::GameStart;
        use lov_shared::types::Team;
        let mut app = app();
        app.update();
        assert_eq!(
            texel(&app, 6000.0, 6000.0),
            255,
            "hors partie : tout visible"
        );

        let mut me = ent(EntityKind::Champion, TeamOrNeutral::Blue, 2000.0, 2000.0);
        me.id = 7;
        {
            let mut state = app.world_mut().resource_mut::<GameState>();
            state.game = Some(GameStart {
                you: "p".into(),
                team: Team::Blue,
                entity_id: 7,
                players: Vec::new(),
                server_time: 0.0,
            });
            state.buffer.entities.insert(7, me);
        }
        app.update();
        assert_eq!(texel(&app, 2000.0, 2000.0), 255, "autour du champion");
        assert_eq!(
            texel(&app, 9000.0, 9000.0),
            0,
            "entrée en partie : le reste au noir"
        );
        let veil = {
            let fog = app.world().resource::<FogOfWar>();
            let images = app.world().resource::<Assets<Image>>();
            images
                .get(&fog.minimap)
                .and_then(|m| m.data.as_ref())
                .expect("voile")[3]
        };
        assert_eq!(
            veil,
            (MINIMAP_VEIL * 255.0).round() as u8,
            "minicarte voilée"
        );

        app.world_mut().resource_mut::<GameState>().game = None;
        app.update();
        assert_eq!(
            texel(&app, 9000.0, 9000.0),
            255,
            "sortie de partie : tout visible"
        );
        let fog = app.world().resource::<FogOfWar>();
        assert!(!fog.enabled);
        assert!(fog.vis.0.iter().all(|&v| v == 1.0));
    }
}
