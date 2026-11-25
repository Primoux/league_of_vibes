//! Formatage pur, portage de `packages/client/src/ui/logic/format.ts` (ce qu'utilisent les écrans ; le
//! reste arrive avec eux et avec le HUD).

use lov_shared::schema::ChampionClass;
use lov_shared::types::Role;

pub fn role_label(r: Role) -> &'static str {
    match r {
        Role::Top => "Haut",
        Role::Jungle => "Jungle",
        Role::Mid => "Milieu",
        Role::Bot => "Bas",
    }
}

pub fn class_label(c: ChampionClass) -> &'static str {
    match c {
        ChampionClass::Tank => "Tank",
        ChampionClass::Fighter => "Combattant",
        ChampionClass::Mage => "Mage",
        ChampionClass::Marksman => "Tireur",
        ChampionClass::Assassin => "Assassin",
        ChampionClass::Support => "Soutien",
    }
}
