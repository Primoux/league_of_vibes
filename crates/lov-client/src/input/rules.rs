//! Règles pures de l'entrée : port de `packages/client/src/input/rules.ts` et de `keyLetter`
//! (`gameMode.ts`).

use bevy::input::keyboard::{Key, KeyCode};
use lov_shared::schema::Targeting;
use lov_shared::types::{EntityKind, EntitySnapshot, Team, TeamOrNeutral};

/// Peut-on auto-attaquer cette entité ? (ennemie ou neutre, vivante, d'un type ciblable)
pub fn is_attackable(e: &EntitySnapshot, my_team: Team) -> bool {
    let mine = match my_team {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    };
    e.dead != Some(true)
        && e.team != mine
        && matches!(
            e.kind,
            EntityKind::Champion
                | EntityKind::Minion
                | EntityKind::Tower
                | EntityKind::Inhibitor
                | EntityKind::Nexus
                | EntityKind::Monster
                | EntityKind::Ward
        )
}

/// Le sort exige-t-il une unité ciblée (clic sur une entité) ?
pub fn needs_target(t: Targeting) -> bool {
    matches!(t, Targeting::Targeted | Targeting::TargetedDash)
}

/// Action d'une touche de la rangée du haut (position physique, indépendante de la disposition) :
/// 1 2 3 5 6 7 = objets (emplacements 0 à 5), 4 = balise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigitAction {
    Item(i32),
    Ward,
}

pub fn digit_action(code: KeyCode) -> Option<DigitAction> {
    Some(match code {
        KeyCode::Digit1 => DigitAction::Item(0),
        KeyCode::Digit2 => DigitAction::Item(1),
        KeyCode::Digit3 => DigitAction::Item(2),
        KeyCode::Digit4 => DigitAction::Ward,
        KeyCode::Digit5 => DigitAction::Item(3),
        KeyCode::Digit6 => DigitAction::Item(4),
        KeyCode::Digit7 => DigitAction::Item(5),
        _ => return None,
    })
}

/// Lettre A-Z d'une touche : la touche logique d'abord (en AZERTY, le joueur attend les lettres
/// gravées), sinon la position physique.
pub fn key_letter(logical: &Key, physical: KeyCode) -> Option<char> {
    if let Key::Character(s) = logical {
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            let u = c.to_ascii_uppercase();
            if u.is_ascii_uppercase() {
                return Some(u);
            }
        }
    }
    let c = match physical {
        KeyCode::KeyA => 'A',
        KeyCode::KeyB => 'B',
        KeyCode::KeyC => 'C',
        KeyCode::KeyD => 'D',
        KeyCode::KeyE => 'E',
        KeyCode::KeyF => 'F',
        KeyCode::KeyG => 'G',
        KeyCode::KeyH => 'H',
        KeyCode::KeyI => 'I',
        KeyCode::KeyJ => 'J',
        KeyCode::KeyK => 'K',
        KeyCode::KeyL => 'L',
        KeyCode::KeyM => 'M',
        KeyCode::KeyN => 'N',
        KeyCode::KeyO => 'O',
        KeyCode::KeyP => 'P',
        KeyCode::KeyQ => 'Q',
        KeyCode::KeyR => 'R',
        KeyCode::KeyS => 'S',
        KeyCode::KeyT => 'T',
        KeyCode::KeyU => 'U',
        KeyCode::KeyV => 'V',
        KeyCode::KeyW => 'W',
        KeyCode::KeyX => 'X',
        KeyCode::KeyY => 'Y',
        KeyCode::KeyZ => 'Z',
        _ => return None,
    };
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::EntityKind;

    fn ent(kind: EntityKind, team: TeamOrNeutral, dead: bool) -> EntitySnapshot {
        EntitySnapshot {
            id: 1,
            kind,
            team,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 1.0,
            max_hp: 1.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: dead.then_some(true),
            subtype: None,
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    #[test]
    fn attaquable_ennemi_ou_neutre_vivant_d_un_type_ciblable() {
        let blue = Team::Blue;
        assert!(is_attackable(
            &ent(EntityKind::Champion, TeamOrNeutral::Red, false),
            blue
        ));
        assert!(is_attackable(
            &ent(EntityKind::Monster, TeamOrNeutral::Neutral, false),
            blue
        ));
        assert!(!is_attackable(
            &ent(EntityKind::Minion, TeamOrNeutral::Blue, false),
            blue
        ));
        assert!(!is_attackable(
            &ent(EntityKind::Minion, TeamOrNeutral::Red, true),
            blue
        ));
        assert!(!is_attackable(
            &ent(EntityKind::Projectile, TeamOrNeutral::Red, false),
            blue
        ));
        assert!(!is_attackable(
            &ent(EntityKind::Zone, TeamOrNeutral::Red, false),
            blue
        ));
    }

    #[test]
    fn seuls_les_sorts_cibles_attendent_une_unite() {
        assert!(needs_target(Targeting::Targeted));
        assert!(needs_target(Targeting::TargetedDash));
        assert!(!needs_target(Targeting::Skillshot));
        assert!(!needs_target(Targeting::Ground));
    }

    #[test]
    fn rangee_du_haut_objets_et_balise() {
        assert_eq!(digit_action(KeyCode::Digit1), Some(DigitAction::Item(0)));
        assert_eq!(digit_action(KeyCode::Digit4), Some(DigitAction::Ward));
        assert_eq!(digit_action(KeyCode::Digit7), Some(DigitAction::Item(5)));
        assert_eq!(digit_action(KeyCode::Digit8), None);
    }

    #[test]
    fn la_lettre_logique_prime_puis_la_position_physique() {
        // AZERTY : la touche physique Q porte un « a ».
        assert_eq!(
            key_letter(&Key::Character("a".into()), KeyCode::KeyQ),
            Some('A')
        );
        assert_eq!(
            key_letter(&Key::Character("Q".into()), KeyCode::KeyA),
            Some('Q')
        );
        // Caractère non latin : la position physique décide.
        assert_eq!(
            key_letter(&Key::Character("й".into()), KeyCode::KeyQ),
            Some('Q')
        );
        assert_eq!(key_letter(&Key::Enter, KeyCode::Enter), None);
    }
}
