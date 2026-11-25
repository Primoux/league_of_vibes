//! Formatage pur, portage de `packages/client/src/ui/logic/format.ts` (ce qu'utilisent les écrans ; le
//! reste arrive avec le HUD).

use lov_shared::schema::ChampionClass;
use lov_shared::types::Role;

/// 125,4 → « 2:05 » ; 3725 → « 1:02:05 ».
pub fn format_clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    let (h, m, ss) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{ss:02}")
    } else {
        format!("{m}:{ss:02}")
    }
}

/// 1234 → « 1,2k » ; 999 → « 999 ».
pub fn format_gold_short(g: f64) -> String {
    if g.abs() < 1000.0 {
        return format!("{}", g.round());
    }
    format!("{:.1}k", g / 1000.0).replace('.', ",")
}

/// Entier arrondi, milliers séparés par une espace fine insécable (`toLocaleString('fr-FR')`).
pub fn format_int(n: f64) -> String {
    let v = n.round() as i64;
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

pub fn kda_ratio(k: u32, d: u32, a: u32) -> f64 {
    let (k, d, a) = (f64::from(k), f64::from(d), f64::from(a));
    if d == 0.0 { k + a } else { (k + a) / d }
}

pub fn format_kda_ratio(k: u32, d: u32, a: u32) -> String {
    if d == 0 {
        return if k + a > 0 {
            "Parfait".into()
        } else {
            "0,00".into()
        };
    }
    format!("{:.2}", kda_ratio(k, d, a)).replace('.', ",")
}

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

/// Score de MVP de l'écran de fin (`mvpScore` de `logic/game.ts`).
pub fn mvp_score(kills: u32, deaths: u32, assists: u32, cs: u32, damage: f64) -> f64 {
    f64::from(kills) * 3.0 + f64::from(assists) * 1.5 - f64::from(deaths) * 1.5
        + f64::from(cs) * 0.02
        + damage / 1500.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ui-logic.test.ts` › format › formate le chrono.
    #[test]
    fn formate_le_chrono() {
        assert_eq!(format_clock(0.0), "0:00");
        assert_eq!(format_clock(125.9), "2:05");
        assert_eq!(format_clock(3725.0), "1:02:05");
    }

    /// `ui-logic.test.ts` › format › formate or, KDA… (partie or et KDA).
    #[test]
    fn formate_or_et_kda() {
        assert_eq!(format_gold_short(950.0), "950");
        assert_eq!(format_gold_short(12345.0), "12,3k");
        assert_eq!(format_kda_ratio(3, 0, 2), "Parfait");
        assert_eq!(format_kda_ratio(4, 2, 2), "3,00");
        assert_eq!(format_kda_ratio(0, 0, 0), "0,00");
    }

    #[test]
    fn separe_les_milliers_a_la_francaise() {
        assert_eq!(format_int(999.4), "999");
        assert_eq!(format_int(12345.0), "12\u{202f}345");
        assert_eq!(format_int(1_234_567.0), "1\u{202f}234\u{202f}567");
    }

    #[test]
    fn score_de_mvp() {
        assert_eq!(mvp_score(2, 1, 4, 100, 3000.0), 6.0 + 6.0 - 1.5 + 2.0 + 2.0);
    }
}
