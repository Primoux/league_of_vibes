//! Choix des glyphes : portage de `packages/client/src/ui/icons/glyphs.ts` (sorts d'après leur nom, leur
//! description et leur ciblage ; objets d'après leur icône, leur id et leurs stats ; teinte et variation
//! déterministe des objets).

use lov_shared::schema::{AbilityDef, AbilityDefSlot, ItemDef, Targeting};

use super::paths::{ITEM_GLYPH, ITEM_TABLE, TARGET_GLYPH, THEME_GLYPH, THEMES};
use crate::ui::logic::format::hash_string;
use crate::ui::logic::shop::normalize;

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `text` contient-il l'alternative `alt` ? Un `\b` en tête ou en fin exige une limite de mot.
fn contains(text: &str, alt: &str) -> bool {
    let left = alt.starts_with("\\b");
    let right = alt.ends_with("\\b");
    let core = alt.trim_start_matches("\\b").trim_end_matches("\\b");
    let mut from = 0;
    while let Some(i) = text[from..].find(core) {
        let at = from + i;
        let end = at + core.len();
        let before = text[..at].chars().next_back();
        let after = text[end..].chars().next();
        let first = core.chars().next();
        let last = core.chars().next_back();
        let ok_left = !left || is_word(before) != is_word(first);
        let ok_right = !right || is_word(after) != is_word(last);
        if ok_left && ok_right {
            return true;
        }
        from = at + core.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Le texte correspond-il à l'une des alternatives ?
pub fn matches(text: &str, alts: &[&str]) -> bool {
    alts.iter().any(|a| contains(text, a))
}

fn lookup(table: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn targeting_key(t: Targeting) -> &'static str {
    match t {
        Targeting::Skillshot => "skillshot",
        Targeting::Targeted => "targeted",
        Targeting::Ground => "ground",
        Targeting::Cone => "cone",
        Targeting::SelfCast => "self",
        Targeting::Dash => "dash",
        Targeting::TargetedDash => "targetedDash",
        Targeting::Global => "global",
    }
}

/// Nom du glyphe d'un sort : thème trouvé dans le nom, puis la description, sinon d'après le ciblage.
pub fn ability_glyph_name(a: &AbilityDef) -> &'static str {
    for text in [normalize(&a.name), normalize(&a.description)] {
        if let Some((_, g)) = THEMES.iter().find(|(alts, _)| matches(&text, alts)) {
            return g;
        }
    }
    if a.slot == AbilityDefSlot::P {
        return "passive";
    }
    targeting_key(a.targeting)
}

/// Tracé du glyphe d'un sort.
pub fn ability_glyph(a: &AbilityDef) -> &'static str {
    let n = ability_glyph_name(a);
    lookup(&THEME_GLYPH, n)
        .or_else(|| lookup(&TARGET_GLYPH, n))
        .or_else(|| lookup(&TARGET_GLYPH, "self"))
        .unwrap_or_default()
}

/// Clé du glyphe d'un objet : icône et id, puis statistiques dominantes.
pub fn item_glyph_key(item: &ItemDef) -> &'static str {
    let k = format!("{} {}", item.icon, item.id).to_lowercase();
    if let Some((_, g)) = ITEM_TABLE.iter().find(|(alts, _)| matches(&k, alts)) {
        return g;
    }
    let s = &item.stats;
    let has = |v: Option<f64>| v.is_some_and(|v| v != 0.0);
    if has(s.crit_chance) {
        "cloak"
    } else if has(s.attack_speed) {
        "dagger"
    } else if has(s.ad) || has(s.armor_pen) {
        "sword"
    } else if has(s.ap) || has(s.magic_pen) {
        "wand"
    } else if has(s.armor) {
        "chest"
    } else if has(s.mr) {
        "cloak"
    } else if has(s.max_hp) {
        "heart"
    } else if has(s.max_mana) {
        "orb"
    } else {
        "gem"
    }
}

/// Tracé du glyphe d'un objet.
pub fn item_glyph(item: &ItemDef) -> &'static str {
    lookup(&ITEM_GLYPH, item_glyph_key(item)).unwrap_or_default()
}

/// Teinte du glyphe d'un objet, d'après sa statistique principale.
pub fn item_tint(item: &ItemDef) -> u32 {
    let s = &item.stats;
    let has = |v: Option<f64>| v.is_some_and(|v| v != 0.0);
    if has(s.ap) || has(s.magic_pen) {
        0xc48bff
    } else if has(s.crit_chance) || has(s.attack_speed) {
        0xffd16b
    } else if has(s.ad) || has(s.armor_pen) || has(s.lifesteal) {
        0xff9a5c
    } else if has(s.armor) {
        0xe8d08c
    } else if has(s.mr) {
        0x7fc6ff
    } else if has(s.max_hp) || has(s.hp_regen) {
        0xff6d7a
    } else if has(s.max_mana) || has(s.mana_regen) {
        0x6fa8ff
    } else if has(s.move_speed) {
        0x9ff0c2
    } else {
        0xf2f2f2
    }
}

const GEM_COLORS: [u32; 7] = [
    0xff5a6e, 0x59d8ff, 0x7dff9a, 0xffd166, 0xc79bff, 0xff9f43, 0xe9f1ff,
];

/// Variation visuelle déterministe d'un objet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemVariation {
    /// Décalage de teinte du fond (degrés, −25..25).
    pub hue: i32,
    /// Abscisse de la lumière du fond (%).
    pub light_x: u32,
    pub ring: u32,
    /// Inclinaison du glyphe (degrés, −12..12).
    pub tilt: i32,
    /// Abscisse de la gemme des objets légendaires.
    pub gem_x: u32,
    pub gem: u32,
}

pub fn item_variation(id: &str) -> ItemVariation {
    let h = hash_string(id);
    ItemVariation {
        hue: (h % 51) as i32 - 25,
        light_x: 25 + ((h >> 6) % 21),
        ring: (h >> 11) % 5,
        tilt: ((h >> 14) % 25) as i32 - 12,
        gem_x: if (h >> 19) % 2 == 1 { 7 } else { 33 },
        gem: GEM_COLORS[((h >> 21) % GEM_COLORS.len() as u32) as usize],
    }
}

/// Portage de `packages/client/test/ui-icons.test.ts`.
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use lov_shared::data::champions::CHAMPIONS;
    use lov_shared::data::items::ITEM_LIST;

    use super::*;

    #[test]
    fn limites_de_mot() {
        assert!(contains("gel des lacs", "gel\\b"));
        assert!(!contains("gelee", "gel\\b"));
        assert!(contains("l'aube se leve", "\\baube\\b"));
        assert!(!contains("aubergine", "\\baube\\b"));
        assert!(contains("boots_du_mont", "_mont\\b"));
        assert!(contains("tir rapide", "tir "));
    }

    #[test]
    fn glyphes_de_sorts_varies_par_champion() {
        for c in CHAMPIONS.iter() {
            let a = &c.abilities;
            let names: HashSet<&str> = [&a.q, &a.w, &a.e, &a.r]
                .into_iter()
                .map(ability_glyph_name)
                .collect();
            assert!(names.len() >= 3, "{} : {names:?}", c.id);
            assert!(!ability_glyph(&c.passive).is_empty());
        }
    }

    #[test]
    fn un_glyphe_defini_pour_chaque_objet() {
        for it in ITEM_LIST.iter() {
            assert!(!item_glyph(it).is_empty(), "{}", it.id);
        }
    }

    #[test]
    fn objets_varies_sur_tout_le_catalogue() {
        let glyphs: HashSet<&str> = ITEM_LIST.iter().map(item_glyph_key).collect();
        assert!(glyphs.len() >= 30, "{}", glyphs.len());
        // glyphe + teinte + variation : quasiment toutes distinctes
        let combos: HashSet<String> = ITEM_LIST
            .iter()
            .map(|i| {
                let v = item_variation(&i.id);
                format!(
                    "{}|{}|{}|{}",
                    item_glyph_key(i),
                    item_tint(i),
                    v.hue,
                    v.tilt
                )
            })
            .collect();
        assert!(combos.len() + 2 >= ITEM_LIST.len());
    }
}
