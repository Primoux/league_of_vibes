//! Formatage pur, portage de `packages/client/src/ui/logic/format.ts`.

use lov_shared::data::buffs::buff;
use lov_shared::schema::{BuffKind, ChampionClass};
use lov_shared::types::{DragonElement, MonsterType, PartialStats, Role};

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

/// Temps de recharge affiché : « 12 », « 4 », « 0.8 » sous 1 s (point décimal, comme le web).
pub fn format_cooldown(seconds: f64) -> String {
    if seconds <= 0.0 {
        String::new()
    } else if seconds < 1.0 {
        format!("{seconds:.1}")
    } else {
        format!("{}", seconds.ceil())
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

pub fn dragon_label(e: DragonElement) -> &'static str {
    match e {
        DragonElement::Infernal => "Drake infernal",
        DragonElement::Ocean => "Drake océanique",
        DragonElement::Mountain => "Drake des montagnes",
        DragonElement::Cloud => "Drake des nuages",
        DragonElement::Hextech => "Drake électrique",
        DragonElement::Chemtech => "Drake toxique",
    }
}

/// Couleur `#rrggbb` d'un élément de drake.
pub fn dragon_color(e: DragonElement) -> u32 {
    match e {
        DragonElement::Infernal => 0xff6a2b,
        DragonElement::Ocean => 0x3fc4e8,
        DragonElement::Mountain => 0xc79a5b,
        DragonElement::Cloud => 0xd6e6ff,
        DragonElement::Hextech => 0x5ef0d0,
        DragonElement::Chemtech => 0x9fe03a,
    }
}

/// Adjectif de l'âme du drake (« Âme infernale »).
pub fn soul_label(e: DragonElement) -> &'static str {
    match e {
        DragonElement::Infernal => "infernale",
        DragonElement::Ocean => "océanique",
        DragonElement::Mountain => "des montagnes",
        DragonElement::Cloud => "des nuages",
        DragonElement::Hextech => "électrique",
        DragonElement::Chemtech => "toxique",
    }
}

/// Élément de drake d'après son id (`infernal`…).
pub fn dragon_element(id: &str) -> Option<DragonElement> {
    Some(match id {
        "infernal" => DragonElement::Infernal,
        "ocean" => DragonElement::Ocean,
        "mountain" => DragonElement::Mountain,
        "cloud" => DragonElement::Cloud,
        "hextech" => DragonElement::Hextech,
        "chemtech" => DragonElement::Chemtech,
        _ => return None,
    })
}

/// Nom d'un monstre (`MONSTER_LABEL`), s'il en a un.
pub fn monster_label(m: MonsterType) -> Option<&'static str> {
    Some(match m {
        MonsterType::Dragon => "Drake",
        MonsterType::ElderDragon => "Drake Ancien",
        MonsterType::Grub => "Larve",
        MonsterType::Herald => "Colosse du Rift",
        MonsterType::Leviathan => "Léviathan",
        MonsterType::BlueBuff => "Gardien d’azur",
        MonsterType::RedBuff => "Bête écarlate",
        MonsterType::Scuttle => "Crabe de rivière",
        MonsterType::Gromp => "Grenouille",
        MonsterType::Wolves => "Loups",
        MonsterType::Raptors => "Raptors",
        MonsterType::Krugs => "Golems de pierre",
        _ => return None,
    })
}

/// Statistiques, dans l'ordre de `Stats` : clé (celle du web), libellé.
pub const STATS: [(&str, &str); 16] = [
    ("maxHp", "Points de vie"),
    ("hpRegen", "Régén. PV (5 s)"),
    ("maxMana", "Mana"),
    ("manaRegen", "Régén. mana (5 s)"),
    ("ad", "Dégâts d'attaque"),
    ("ap", "Puissance"),
    ("armor", "Armure"),
    ("mr", "Résistance magique"),
    ("attackSpeed", "Vitesse d'attaque"),
    ("attackRange", "Portée"),
    ("moveSpeed", "Vitesse de déplacement"),
    ("critChance", "Chances de critique"),
    ("abilityHaste", "Accélération"),
    ("lifesteal", "Vol de vie"),
    ("armorPen", "Létalité"),
    ("magicPen", "Pénétration magique"),
];

/// Libellé d'une statistique (`STAT_LABEL`), la clé elle-même si elle est inconnue.
pub fn stat_label(key: &str) -> &str {
    STATS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(key, |(_, l)| l)
}

/// Statistiques présentes, dans l'ordre de `STATS`.
pub fn stat_entries(s: &PartialStats) -> Vec<(&'static str, f64)> {
    let v = [
        s.max_hp,
        s.hp_regen,
        s.max_mana,
        s.mana_regen,
        s.ad,
        s.ap,
        s.armor,
        s.mr,
        s.attack_speed,
        s.attack_range,
        s.move_speed,
        s.crit_chance,
        s.ability_haste,
        s.lifesteal,
        s.armor_pen,
        s.magic_pen,
    ];
    STATS
        .iter()
        .zip(v)
        .filter_map(|((k, _), v)| v.map(|v| (*k, v)))
        .collect()
}

/// Valeur de stat lisible : « +25 % » / « +0,65 » / « +40 ».
pub fn format_stat(key: &str, value: f64, signed: bool) -> String {
    let sign = if signed && value > 0.0 { "+" } else { "" };
    match key {
        "critChance" | "lifesteal" => format!("{sign}{} %", (value * 100.0).round()),
        "attackSpeed" => format!("{sign}{value:.2}").replace('.', ","),
        _ => format!("{sign}{}", value.round()),
    }
}

/// Valeurs par rang : « 80 / 120 / 160 » (une seule si elles sont égales).
pub fn format_ranks(values: &[f64], unit: &str) -> String {
    let fmt = |v: f64| {
        if v.fract() == 0.0 {
            format!("{v}{unit}")
        } else {
            format!("{v:.1}{unit}").replace('.', ",")
        }
    };
    if values.iter().all(|v| Some(v) == values.first()) {
        fmt(values.first().copied().unwrap_or(0.0))
    } else {
        values
            .iter()
            .map(|v| fmt(*v))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

/// Noms des buffs hors registre (camps et objectifs).
fn extra_buff_name(id: &str) -> Option<&'static str> {
    Some(match id {
        "blue_buff" => "Gardien d’azur",
        "red_buff" => "Bête écarlate",
        "leviathan" | "leviathan_buff" => "Bénédiction du Léviathan",
        "elder" | "elder_buff" => "Drake Ancien",
        "recall" => "Rappel",
        _ => return None,
    })
}

/// Nom lisible d'un buff : registre partagé, table locale, drakes et âmes, puis id reformaté.
pub fn buff_name(id: &str) -> String {
    if let Some(n) = buff(id)
        .map(|b| b.name.as_str())
        .or_else(|| extra_buff_name(id))
    {
        return n.to_string();
    }
    for (prefix, soul) in [("dragon_", false), ("soul_", true)] {
        if let Some(e) = id.strip_prefix(prefix).and_then(dragon_element) {
            return if soul {
                format!("Âme {}", soul_label(e))
            } else {
                dragon_label(e).to_string()
            };
        }
    }
    let s = id.replace('_', " ");
    let mut c = s.chars();
    c.next()
        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

const DEBUFF_WORDS: [&str; 20] = [
    "stun", "root", "slow", "silence", "knock", "taunt", "suppress", "fear", "blind", "ground",
    "bleed", "burn", "ignite", "mark", "frost", "givre", "poison", "exhaust", "grievous", "anti",
];

/// Effet négatif ? Registre, sinon mots-clés dans l'id.
pub fn is_debuff(id: &str) -> bool {
    if let Some(b) = buff(id) {
        return b.kind != BuffKind::Buff;
    }
    let id = id.to_lowercase();
    DEBUFF_WORDS.iter().any(|w| id.contains(w))
}

/// Initiales de la pastille de buff.
pub fn buff_initials(id: &str) -> String {
    let name = buff_name(id);
    let words: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c == '’' || c == '\'')
        .filter(|w| w.chars().count() > 2)
        .collect();
    let s: String = if words.len() > 1 {
        words
            .iter()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect()
    } else {
        name.chars().take(2).collect()
    };
    s.to_uppercase()
}

/// Hash déterministe FNV-1a 32 bits (motifs stables) ; sur les unités UTF-16, comme `charCodeAt`.
pub fn hash_string(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for u in s.encode_utf16() {
        h ^= u32::from(u);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
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
    fn formate_les_cooldowns() {
        assert_eq!(format_cooldown(0.0), "");
        assert_eq!(format_cooldown(0.42), "0.4");
        assert_eq!(format_cooldown(4.1), "5");
    }

    #[test]
    fn formate_rangs_et_stats() {
        assert_eq!(format_ranks(&[70.0, 70.0, 70.0], ""), "70");
        assert_eq!(format_ranks(&[6.0, 5.5, 5.0], ""), "6 / 5,5 / 5");
        assert_eq!(format_stat("critChance", 0.25, true), "+25 %");
        assert_eq!(format_stat("attackSpeed", 0.65, false), "0,65");
        assert_eq!(format_stat("ad", 40.0, true), "+40");
        assert_eq!(stat_label("armor"), "Armure");
    }

    #[test]
    fn nomme_les_buffs() {
        assert_eq!(buff_name("stun"), "Étourdissement");
        assert!(is_debuff("stun"));
        assert!(!is_debuff("haste"));
        assert_eq!(buff_name("dragon_ocean"), "Drake océanique");
        assert_eq!(buff_name("soul_infernal"), "Âme infernale");
        assert_eq!(buff_name("blue_buff"), "Gardien d’azur");
        assert_eq!(buff_name("ilyra_frost"), "Ilyra frost");
        assert_eq!(buff_initials("blue_buff"), "GA");
        assert_eq!(buff_initials("stun"), "ÉT");
    }

    #[test]
    fn hash_fnv1a() {
        assert_eq!(hash_string(""), 0x811c_9dc5);
        assert_eq!(hash_string("a"), 0xe40c_292c);
    }

    #[test]
    fn score_de_mvp() {
        assert_eq!(mvp_score(2, 1, 4, 100, 3000.0), 6.0 + 6.0 - 1.5 + 2.0 + 2.0);
    }
}
