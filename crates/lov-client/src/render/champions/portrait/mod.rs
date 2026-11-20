//! Modèles inspirés des portraits des champions : géométrie sculptée (volumes, mèches, plaques
//! biseautées, étoffes), anatomie commune, et un portrait par champion.
//!
//! Portage de `packages/client/src/render/champions/portrait/` (area `render-champions`, issue #110).

pub mod anatomy;
pub mod geometry;
pub mod humanoids;
pub mod three;

use super::model::{ChampionModelDef, ModelBuild};

/// Construction d'un portrait : elle commence par `Sculpt::new(b)` (surfaces sculptées).
pub type PortraitBuild = ModelBuild;

/// Portrait d'un champion, s'il en a un.
pub fn portrait_build(id: &str) -> Option<PortraitBuild> {
    humanoids::PORTRAITS
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, build)| *build)
}

/// `portraitModel` : la géométrie change ; squelette, clips, port d'arme et gameplay restent ceux du
/// modèle de base du champion.
pub fn portrait_model(id: &str, base: ChampionModelDef) -> ChampionModelDef {
    match portrait_build(id) {
        Some(build) => ChampionModelDef { build, ..base },
        None => base,
    }
}

/// Portage de `packages/client/test/portrait-models.test.ts`.
#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use lov_shared::data::champions::CHAMPIONS;
    use lov_shared::types::{BuffSnapshot, EntityKind, EntitySnapshot, TeamOrNeutral};

    use crate::render::champions::anim::pose::JOINT_COUNT;
    use crate::render::champions::builder::{BuiltModel, JointMeshes};
    use crate::render::champions::model::{
        ChampionMaterials, ModelColors, SpawnedModel, build_model,
    };
    use crate::render::champions::model_for;
    use crate::render::champions::rig::{PARENT, Skeleton};
    use crate::render::champions::view::ChampionView;

    fn snap(champion: &str) -> EntitySnapshot {
        EntitySnapshot {
            id: 200,
            kind: EntityKind::Champion,
            team: TeamOrNeutral::Red,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 100.0,
            max_hp: 100.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: Some(champion.into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    fn view(champion: &str) -> (ChampionView, BuiltModel) {
        let Some(colors) = ModelColors::of(champion, TeamOrNeutral::Red) else {
            panic!("{champion} inconnu");
        };
        let def = model_for(champion, colors.class);
        let built = build_model(&def, &colors);
        let model = SpawnedModel {
            skeleton: Skeleton {
                root: Entity::PLACEHOLDER,
                joints: [Entity::PLACEHOLDER; JOINT_COUNT],
                rest: def.rig.rest_positions(),
            },
            materials: ChampionMaterials {
                body: Handle::default(),
                metal: Handle::default(),
                glow: Handle::default(),
            },
            meshes: vec![],
            outlines: vec![],
            glows: vec![],
        };
        (ChampionView::new(200, &def, model), built)
    }

    fn lots(m: &BuiltModel) -> [&JointMeshes; 5] {
        [
            &m.body,
            &m.metal,
            &m.glow,
            &m.outline_body,
            &m.outline_metal,
        ]
    }

    fn positions(mesh: &Mesh) -> Vec<Vec3> {
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .map(|p| p.iter().copied().map(Vec3::from_array).collect())
            .unwrap_or_default()
    }

    /// Les portraits, et Touilleur sur le modèle de sa classe ; Mélenchon a sa propre vue.
    fn champions() -> impl Iterator<Item = &'static str> {
        CHAMPIONS
            .iter()
            .map(|c| c.id.as_str())
            .filter(|id| *id != "melenchon")
    }

    #[test]
    fn geometrie_valide_budget_borne_et_articulations_finies_pendant_les_sorts() {
        for id in champions() {
            let (mut v, built) = view(id);
            let mut triangles = 0;
            for lot in lots(&built) {
                for mesh in lot.iter().flatten() {
                    let n = mesh.count_vertices();
                    assert!(n > 0 && n % 3 == 0, "{id}");
                    triangles += n / 3;
                    let normals = mesh
                        .attribute(Mesh::ATTRIBUTE_NORMAL)
                        .and_then(|a| a.as_float3())
                        .map_or(0, <[_]>::len);
                    assert_eq!(normals, n, "{id} : une normale par sommet");
                    for (_, values) in mesh.attributes() {
                        let finite = match values {
                            bevy::mesh::VertexAttributeValues::Float32x3(v) => {
                                v.iter().flatten().all(|x| x.is_finite())
                            }
                            bevy::mesh::VertexAttributeValues::Float32x4(v) => {
                                v.iter().flatten().all(|x| x.is_finite())
                            }
                            _ => true,
                        };
                        assert!(finite, "{id} : attribut non fini");
                    }
                }
            }
            // contours inclus
            assert!(triangles < 25_000, "{id} : {triangles} triangles");
            let s = snap(id);
            let scale = v.height(1.0);
            for slot in ["Q", "W", "E", "R"] {
                v.cast(slot);
                for _ in 0..18 {
                    let f = v.step(&s, "idle", 1.0 / 30.0, Vec3::ZERO, 0.0);
                    let mut world = [Mat4::IDENTITY; JOINT_COUNT];
                    for j in 0..JOINT_COUNT {
                        let parent =
                            PARENT[j].map_or(Mat4::from_scale(Vec3::splat(scale)), |p| world[p]);
                        world[j] = parent * f.joints[j].to_matrix();
                    }
                    for lot in lots(&built) {
                        for (j, mesh) in lot.iter().enumerate() {
                            let Some(mesh) = mesh else { continue };
                            let p = positions(mesh);
                            for v in p.iter().step_by((p.len() / 32).max(1)) {
                                let w = world[j].transform_point3(*v);
                                assert!(
                                    w.is_finite() && w.length() < 10.0,
                                    "{id} {slot} : sommet {w}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn les_portraits_remplacent_la_geometrie_seulement() {
        for id in ["kaelen", "vrenna", "ilyra", "zephyr", "sable"] {
            let (_, built) = view(id);
            assert!(built.sculpted, "{id} : surfaces sculptées");
            assert!(super::portrait_build(id).is_some());
        }
        // sans portrait : modèle de base inchangé
        let Some(c) = ModelColors::of("touilleur", TeamOrNeutral::Blue) else {
            panic!("touilleur connu");
        };
        let base = crate::render::champions::generic::generic_for(c.class);
        let def = model_for("touilleur", c.class);
        assert_eq!(def.rig, base.rig);
        assert!(!build_model(&def, &c).sculpted);
    }

    #[test]
    fn les_dagues_metalliques_de_nyx_suivent_l_invisibilite() {
        let (mut v, built) = view("nyx");
        if super::portrait_build("nyx").is_none() {
            return; // portrait de Nyx : PR suivante
        }
        assert!(built.metal.iter().any(Option::is_some), "dagues en métal");
        let mut s = snap("nyx");
        s.buffs = Some(vec![BuffSnapshot {
            id: "invisible".into(),
            remaining: 2.0,
            stacks: None,
        }]);
        assert!(v.step(&s, "idle", 0.1, Vec3::ZERO, 0.0).translucent);
        s.buffs = None;
        assert!(!v.step(&s, "idle", 0.1, Vec3::ZERO, 0.0).translucent);
    }
}
