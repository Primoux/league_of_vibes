//! Indicateurs de visée et de portée au sol, comme `fxIndicators` du client web.
//!
//! Portage de `packages/client/src/fx/telegraph/indicators.ts`, avec la part de
//! `render/Indicators.ts` (`computeAim`) et de `render/fxIndicatorsAdapter.ts` qui choisit quoi montrer :
//! en partie, l'état de ciblage de l'input ([`PlayerInput`]) et la position du joueur suffisent à dessiner
//! l'indicateur, sans que l'input ait à appeler les FX. Une démo peut piloter les indicateurs à la main
//! ([`FxIndicators::manual`]).
//!
//! Les deux décals sont créés une fois et réutilisés.

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::data::summoners::summoner;
use lov_shared::schema::{AbilityDef, SummonerDef};
use lov_shared::types::Vec2;

use super::aim::{AimIndicator, AimShape, aim_for, aim_for_summoner};
use crate::fx::core::runtime::Fx;
use crate::fx::shaders::{DecalMaterial, DecalMesh, DecalShape, GroundDecal};
use crate::input::{PlayerInput, Targeting};
use crate::net::GameState;
use crate::render::RENDER_SCALE;

const RANGE_COLOR: Vec3 = Vec3::new(0.55, 0.8, 1.2);
const AIM_COLOR: Vec3 = Vec3::new(0.7, 0.9, 1.4);
/// Portée de pose d'une balise (`ward` de `computeAim`).
const WARD_RANGE: f64 = 600.0;
/// Marge ajoutée à la portée d'attaque affichée (rayon de collision moyen).
const AA_MARGIN: f64 = 65.0;

/// Ce que montrent les indicateurs (coordonnées de jeu).
#[derive(Resource, Debug, Default)]
pub struct FxIndicators {
    /// Cercle de portée : centre et rayon.
    pub range: Option<(Vec2, f64)>,
    pub aim: Option<AimShape>,
    /// Piloté à la main (démo) : la partie ne le remplace pas.
    pub manual: bool,
    decals: Option<[(Entity, Handle<DecalMaterial>, GroundDecal); 2]>,
}

impl FxIndicators {
    pub fn show_range(&mut self, center: Vec2, radius: f64) {
        self.range = Some((center, radius));
    }

    pub fn hide_range(&mut self) {
        self.range = None;
    }

    pub fn show_aim(&mut self, shape: Option<AimShape>) {
        self.aim = shape;
    }

    pub fn show(&mut self, ind: AimIndicator, caster: Vec2) {
        self.range = ind.range.map(|r| (caster, r));
        self.aim = ind.aim;
    }

    /// Indicateur complet d'un sort de champion (portée + forme selon le ciblage).
    pub fn show_ability(&mut self, def: &AbilityDef, caster: Vec2, cursor: Vec2) {
        self.show(aim_for(def, caster, cursor), caster);
    }

    pub fn show_summoner(&mut self, def: &SummonerDef, caster: Vec2, cursor: Vec2) {
        self.show(aim_for_summoner(def, caster, cursor), caster);
    }

    pub fn hide(&mut self) {
        self.range = None;
        self.aim = None;
    }
}

/// Indicateur de la partie : sort en cours de visée, sinon portée d'attaque (attaque-déplacement, survol de
/// son propre champion). Logique de `computeAim` et de l'adaptateur du web.
pub fn game_indicator(
    state: &GameState,
    input: &PlayerInput,
) -> (Option<(Vec2, f64)>, Option<AimShape>) {
    let (Some(me), Some(st)) = (state.my_entity, state.self_state.as_ref()) else {
        return (None, None);
    };
    let Some(pose) = state.pose(me) else {
        return (None, None);
    };
    let origin = Vec2 {
        x: pose.x,
        y: pose.y,
    };
    let cursor = input.cursor.unwrap_or(origin);
    let aa_range = st.stats.attack_range + AA_MARGIN;
    let champion = state
        .entities()
        .get(&me)
        .and_then(|e| e.subtype.as_deref())
        .and_then(get_champion);
    match input.targeting {
        Some(Targeting::Ability(slot)) => match champion {
            Some(c) => {
                let ind = aim_for(c.abilities.get(slot), origin, cursor);
                (ind.range.map(|r| (origin, r)), ind.aim)
            }
            None => (None, None),
        },
        Some(Targeting::Summoner(slot)) => {
            let ind = aim_for_summoner(summoner(*st.summoners.get(slot)), origin, cursor);
            (ind.range.map(|r| (origin, r)), ind.aim)
        }
        Some(Targeting::AttackMove) => (Some((origin, aa_range)), None),
        Some(Targeting::Ward) => (Some((origin, WARD_RANGE)), None),
        None if input.hover == Some(me) => (Some((origin, aa_range)), None),
        None => (None, None),
    }
}

/// Recalcule l'indicateur de la partie à chaque image.
pub fn drive_indicators(
    state: Res<GameState>,
    input: Option<Res<PlayerInput>>,
    mut ind: ResMut<FxIndicators>,
) {
    if ind.manual {
        return;
    }
    let (range, aim) = match input {
        Some(input) => game_indicator(&state, &input),
        None => (None, None),
    };
    if ind.range != range {
        ind.range = range;
    }
    if ind.aim != aim {
        ind.aim = aim;
    }
}

fn world(p: Vec2) -> Vec3 {
    Fx::world_pos(p, 0.0)
}

/// Pose les deux décals selon l'état de [`FxIndicators`].
pub fn apply_indicators(
    mut commands: Commands,
    mut ind: ResMut<FxIndicators>,
    mesh: Option<Res<DecalMesh>>,
    mut materials: ResMut<Assets<DecalMaterial>>,
    mut q: Query<(&mut Transform, &mut Visibility)>,
) {
    let Some(mesh) = mesh else { return };
    let decals = ind.decals.get_or_insert_with(|| {
        let make = |commands: &mut Commands,
                    materials: &mut Assets<DecalMaterial>,
                    color,
                    fill,
                    edge,
                    opacity| {
            let mut m = DecalMaterial::default();
            m.set_color(color, color);
            m.u.fill = fill;
            m.u.edge = edge;
            m.u.opacity = opacity;
            let h = materials.add(m);
            let g = GroundDecal::default();
            let e = commands
                .spawn((
                    mesh.bundle(g, h.clone(), Transform::default()),
                    Visibility::Hidden,
                ))
                .id();
            (e, h, g)
        };
        [
            make(&mut commands, &mut materials, RANGE_COLOR, 0.0, 0.05, 0.55),
            make(&mut commands, &mut materials, AIM_COLOR, 0.22, 0.06, 0.8),
        ]
    });
    let [range_decal, aim_decal] = decals.clone();
    let range_tf = ind.range.and_then(|(c, r)| {
        let mut m = materials.get_mut(&range_decal.1)?;
        Some(range_decal.2.circle(
            &mut m,
            world(c),
            r as f32 * RENDER_SCALE,
            DecalShape::Circle,
        ))
    });
    let aim_tf = ind.aim.and_then(|shape| {
        let mut m = materials.get_mut(&aim_decal.1)?;
        let g = aim_decal.2;
        Some(match shape {
            AimShape::Circle { center, radius } => g.circle(
                &mut m,
                world(center),
                radius as f32 * RENDER_SCALE,
                DecalShape::Circle,
            ),
            AimShape::Line { from, to, width } => {
                g.rect(&mut m, world(from), world(to), width as f32 * RENDER_SCALE)
            }
            AimShape::Cone {
                from,
                to,
                half_angle,
            } => {
                let (a, b) = (world(from), world(to));
                g.cone(&mut m, a, b, a.distance(b), half_angle as f32)
            }
        })
    });
    for ((e, _, _), tf) in [(range_decal, range_tf), (aim_decal, aim_tf)] {
        // pas encore créée (commandes en attente) : l'image suivante
        let Ok((mut cur, mut vis)) = q.get_mut(e) else {
            continue;
        };
        match tf {
            Some(tf) => {
                *cur = tf;
                vis.set_if_neq(Visibility::Inherited);
            }
            None => {
                vis.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use lov_shared::types::AbilitySlot;

    use super::*;

    #[test]
    fn rien_hors_partie() {
        let state = GameState::default();
        let input = PlayerInput::default();
        assert_eq!(game_indicator(&state, &input), (None, None));
    }

    #[test]
    fn api_manuelle() {
        let mut ind = FxIndicators::default();
        let def = get_champion("ilyra").map(|c| c.abilities.get(AbilitySlot::W));
        let o = Vec2 { x: 0.0, y: 0.0 };
        if let Some(def) = def {
            ind.show_ability(def, o, Vec2 { x: 100.0, y: 0.0 });
        }
        assert!(ind.range.is_some() && ind.aim.is_some());
        ind.hide();
        assert_eq!((ind.range, ind.aim), (None, None));
    }
}
