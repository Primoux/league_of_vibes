//! Forme d'indicateur de visée d'un sort, calculée à partir de ses données (logique pure, testée).
//! Coordonnées de JEU, comme l'input.
//!
//! Portage de `packages/client/src/fx/telegraph/aim.ts`.

use lov_shared::schema::{AbilityDef, AbilityParam, SummonerDef, Targeting};
use lov_shared::types::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AimShape {
    Line {
        from: Vec2,
        to: Vec2,
        width: f64,
    },
    Cone {
        from: Vec2,
        to: Vec2,
        half_angle: f64,
    },
    Circle {
        center: Vec2,
        radius: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimIndicator {
    /// Cercle de portée autour du lanceur (`None` = aucun).
    pub range: Option<f64>,
    pub aim: Option<AimShape>,
}

/// Point `cursor` ramené à `max_dist` de `from`.
pub fn clamp_to_range(from: Vec2, cursor: Vec2, max_dist: f64) -> Vec2 {
    let (dx, dy) = (cursor.x - from.x, cursor.y - from.y);
    let d = dx.hypot(dy);
    if d <= max_dist || d == 0.0 {
        return cursor;
    }
    Vec2 {
        x: from.x + dx / d * max_dist,
        y: from.y + dy / d * max_dist,
    }
}

/// Point à exactement `dist` de `from` dans la direction du curseur (skillshots : portée fixe).
pub fn towards(from: Vec2, cursor: Vec2, dist: f64) -> Vec2 {
    let (dx, dy) = (cursor.x - from.x, cursor.y - from.y);
    let d = dx.hypot(dy);
    let d = if d == 0.0 { 1.0 } else { d };
    Vec2 {
        x: from.x + dx / d * dist,
        y: from.y + dy / d * dist,
    }
}

/// Indicateur d'un sort de champion selon son ciblage.
pub fn aim_for(def: &AbilityDef, caster: Vec2, cursor: Vec2) -> AimIndicator {
    let range = def.range;
    match def.targeting {
        Targeting::Skillshot | Targeting::Global => {
            // les ultimes globaux affichent une ligne longue mais pas un cercle de 12 000 unités
            let len = range.min(3000.0);
            AimIndicator {
                range: (range <= 3000.0).then_some(range),
                aim: Some(AimShape::Line {
                    from: caster,
                    to: towards(caster, cursor, len),
                    width: def.width.unwrap_or(80.0),
                }),
            }
        }
        Targeting::Cone => {
            let deg = match def.params.as_ref().and_then(|p| p.get("coneAngle")) {
                Some(AbilityParam::One(v)) => *v,
                _ => 50.0,
            };
            AimIndicator {
                range: Some(range),
                aim: Some(AimShape::Cone {
                    from: caster,
                    to: towards(caster, cursor, range),
                    half_angle: (deg / 2.0).to_radians(),
                }),
            }
        }
        Targeting::Ground => AimIndicator {
            range: Some(range),
            aim: Some(AimShape::Circle {
                center: clamp_to_range(caster, cursor, range),
                radius: def.radius.unwrap_or(100.0),
            }),
        },
        Targeting::Dash => {
            let to = clamp_to_range(caster, cursor, range);
            let aim = match def.radius {
                Some(r) if r > 0.0 => AimShape::Circle {
                    center: to,
                    radius: r,
                },
                _ => AimShape::Line {
                    from: caster,
                    to,
                    width: def.width.unwrap_or(100.0),
                },
            };
            AimIndicator {
                range: Some(range),
                aim: Some(aim),
            }
        }
        Targeting::Targeted | Targeting::TargetedDash => AimIndicator {
            range: Some(range),
            aim: None,
        },
        Targeting::SelfCast => AimIndicator {
            range: None,
            aim: def
                .radius
                .filter(|r| *r > 0.0)
                .map(|radius| AimShape::Circle {
                    center: caster,
                    radius,
                }),
        },
    }
}

/// Indicateur d'un sort d'invocateur.
pub fn aim_for_summoner(def: &SummonerDef, caster: Vec2, cursor: Vec2) -> AimIndicator {
    match def.targeting {
        Targeting::Ground => AimIndicator {
            range: Some(def.range),
            aim: Some(AimShape::Circle {
                center: clamp_to_range(caster, cursor, def.range),
                radius: 60.0,
            }),
        },
        Targeting::Targeted if def.range < 5000.0 => AimIndicator {
            range: Some(def.range),
            aim: None,
        },
        _ => AimIndicator {
            range: None,
            aim: None,
        },
    }
}

/// Port de `packages/client/test/fx/aim.test.ts`.
#[cfg(test)]
mod tests {
    use lov_shared::data::champions::get_champion;
    use lov_shared::types::AbilitySlot;

    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }

    fn ability(champ: &str, slot: AbilitySlot) -> &'static AbilityDef {
        get_champion(champ)
            .map(|c| c.abilities.get(slot))
            .unwrap_or_else(|| panic!("{champ} inconnu"))
    }

    #[test]
    fn clamp_to_range_et_towards() {
        assert_eq!(
            clamp_to_range(v(0.0, 0.0), v(300.0, 400.0), 1000.0),
            v(300.0, 400.0)
        );
        assert_eq!(
            clamp_to_range(v(0.0, 0.0), v(300.0, 400.0), 250.0),
            v(150.0, 200.0)
        );
        assert_eq!(towards(v(0.0, 0.0), v(3.0, 4.0), 10.0), v(6.0, 8.0));
    }

    #[test]
    fn forme_selon_le_ciblage() {
        let caster = v(1000.0, 1000.0);
        let target = v(1600.0, 1000.0);
        let q = aim_for(ability("ilyra", AbilitySlot::Q), caster, target);
        assert_eq!(q.range, Some(950.0));
        let Some(AimShape::Line { to, .. }) = q.aim else {
            panic!("ligne attendue : {q:?}");
        };
        assert_eq!(to, v(1950.0, 1000.0));

        let w = aim_for(ability("ilyra", AbilitySlot::W), caster, v(5000.0, 1000.0));
        assert_eq!(
            w.aim,
            Some(AimShape::Circle {
                center: v(1850.0, 1000.0),
                radius: 280.0
            })
        );

        assert_eq!(
            aim_for(ability("ilyra", AbilitySlot::E), caster, target).aim,
            None
        );

        let cone = aim_for(ability("sable", AbilitySlot::W), caster, target);
        let Some(AimShape::Cone { half_angle, .. }) = cone.aim else {
            panic!("cône attendu : {cone:?}");
        };
        assert!((half_angle - 25f64.to_radians()).abs() < 1e-9);

        let global_ult = aim_for(ability("sable", AbilitySlot::R), caster, target);
        assert_eq!(global_ult.range, None, "pas de cercle de 12 000 u");
    }
}
