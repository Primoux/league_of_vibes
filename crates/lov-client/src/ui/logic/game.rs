//! Règles d'affichage du HUD : portage de `packages/client/src/ui/logic/game.ts`.

use lov_shared::constants::ULT_LEVELS;
use lov_shared::types::{AbilitySlot, Role, SummonerId, Team};

/// Le joueur peut-il monter ce sort maintenant ? R à 6/11/16 ; QWE au plus ⌈niveau / 2⌉, rang 5 max.
pub fn can_rank_up(slot: AbilitySlot, level: u32, rank: u32, skill_points: u32) -> bool {
    if skill_points == 0 {
        return false;
    }
    if slot == AbilitySlot::R {
        let unlocked = ULT_LEVELS.iter().filter(|l| level >= **l).count() as u32;
        return rank < 3 && unlocked > rank;
    }
    rank < 5 && rank < level.div_ceil(2)
}

/// Ligne du tableau des scores.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreRow {
    pub player_id: String,
    pub name: String,
    pub team: Team,
    pub champion_id: String,
    pub role: Option<Role>,
    pub level: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub items: Vec<Option<String>>,
    pub summoners: [SummonerId; 2],
    pub dead: bool,
    pub respawn_in: Option<f64>,
    pub is_me: bool,
}

const ROLE_ORDER: [Role; 4] = [Role::Top, Role::Jungle, Role::Mid, Role::Bot];

/// Tri par rôle (haut, jungle, milieu, bas ; sans rôle en dernier) puis par nom.
pub fn sort_score_rows(rows: &mut [ScoreRow]) {
    let rank = |r: &ScoreRow| {
        r.role
            .and_then(|role| ROLE_ORDER.iter().position(|x| *x == role))
            .unwrap_or(9)
    };
    rows.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.cmp(&b.name)));
}

/// Fraction 0..1 d'un temps de recharge, bornée.
pub fn cooldown_fraction(remaining: f64, total: f64) -> f64 {
    if remaining <= 0.0 || total <= 0.0 {
        0.0
    } else {
        (remaining / total).min(1.0)
    }
}

/// Valeur d'un sort à un rang donné (rang 1 = premier élément), bornée.
pub fn rank_value(values: &[f64], rank: u32) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let i = (rank.max(1) as usize - 1).min(values.len() - 1);
    values[i]
}

/// Recharge effective avec accélération : cd × 100 / (100 + accélération).
pub fn hasted_cooldown(base: f64, haste: f64) -> f64 {
    base * 100.0 / (100.0 + haste.max(0.0))
}

/// Portage de la partie « règles HUD » de `packages/client/test/ui-logic.test.ts`.
#[cfg(test)]
mod tests {
    use super::*;
    use AbilitySlot::{Q, R, W};

    #[test]
    fn limite_les_rangs_selon_le_niveau() {
        assert!(can_rank_up(Q, 1, 0, 1));
        assert!(!can_rank_up(Q, 2, 1, 1));
        assert!(can_rank_up(Q, 3, 1, 1));
        assert!(!can_rank_up(R, 5, 0, 1));
        assert!(can_rank_up(R, 6, 0, 1));
        assert!(!can_rank_up(R, 10, 1, 1));
        assert!(can_rank_up(R, 11, 1, 1));
        assert!(!can_rank_up(Q, 18, 5, 1));
        assert!(!can_rank_up(W, 9, 2, 0));
    }

    #[test]
    fn applique_l_acceleration() {
        assert_eq!(hasted_cooldown(10.0, 100.0), 5.0);
        assert_eq!(cooldown_fraction(5.0, 10.0), 0.5);
        assert_eq!(cooldown_fraction(-1.0, 10.0), 0.0);
        assert_eq!(rank_value(&[10.0, 20.0, 30.0], 0), 10.0);
        assert_eq!(rank_value(&[10.0, 20.0, 30.0], 9), 30.0);
    }

    #[test]
    fn trie_le_tableau_par_role() {
        let row = |name: &str, role: Option<Role>| ScoreRow {
            player_id: name.into(),
            name: name.into(),
            team: Team::Blue,
            champion_id: "ilyra".into(),
            role,
            level: 1,
            kills: 0,
            deaths: 0,
            assists: 0,
            cs: 0,
            items: vec![],
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            dead: false,
            respawn_in: None,
            is_me: false,
        };
        let mut rows = vec![
            row("d", Some(Role::Bot)),
            row("a", Some(Role::Mid)),
            row("z", None),
            row("c", Some(Role::Top)),
        ];
        sort_score_rows(&mut rows);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["c", "a", "d", "z"]);
    }
}
