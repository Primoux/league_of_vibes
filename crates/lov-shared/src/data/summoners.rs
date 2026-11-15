//! Sorts d'invocateur (port de `data/summoners.ts`). Effets implémentés dans `lov-sim`
//! (système summoners).

use crate::schema::{SummonerDef, Targeting};
use crate::types::SummonerId;
use std::collections::BTreeMap;
use std::sync::LazyLock;

fn params<const N: usize>(p: [(&str, f64); N]) -> BTreeMap<String, f64> {
    p.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// `SUMMONERS` : dans l'ordre du TypeScript.
pub static SUMMONERS: LazyLock<Vec<SummonerDef>> = LazyLock::new(|| {
    vec![
        SummonerDef {
            id: SummonerId::Flash,
            name: "Saut éclair".into(),
            description: "Se téléporte sur une courte distance (400).".into(),
            cooldown: 300.0,
            params: None,
            targeting: Targeting::Ground,
            range: 400.0,
            fx_key: "sum_flash".into(),
        },
        SummonerDef {
            id: SummonerId::Ignite,
            name: "Embrasement".into(),
            description: "Brûle un champion ennemi : 70 à 410 dégâts bruts selon le niveau sur 5 s, soins réduits de 40 %.".into(),
            cooldown: 180.0,
            params: Some(params([("duration", 5.0), ("baseDamage", 50.0), ("damagePerLevel", 20.0), ("grievous", 0.4)])),
            targeting: Targeting::Targeted,
            range: 600.0,
            fx_key: "sum_ignite".into(),
        },
        SummonerDef {
            id: SummonerId::Heal,
            name: "Soins".into(),
            description: "Rend 95 à 350 PV selon le niveau au lanceur et à l’allié le plus blessé à proximité ; +30 % de vitesse pendant 1 s.".into(),
            cooldown: 240.0,
            params: Some(params([("baseHeal", 80.0), ("healPerLevel", 15.0), ("speed", 0.3), ("speedDuration", 1.0)])),
            targeting: Targeting::SelfCast,
            range: 850.0,
            fx_key: "sum_heal".into(),
        },
        SummonerDef {
            id: SummonerId::Barrier,
            name: "Barrière".into(),
            description: "Octroie un bouclier de 125 à 465 PV selon le niveau pendant 2,5 s.".into(),
            cooldown: 180.0,
            params: Some(params([("baseShield", 105.0), ("shieldPerLevel", 20.0), ("duration", 2.5)])),
            targeting: Targeting::SelfCast,
            range: 0.0,
            fx_key: "sum_barrier".into(),
        },
        SummonerDef {
            id: SummonerId::Exhaust,
            name: "Fatigue".into(),
            description: "Ralentit un champion ennemi de 30 % et réduit ses dégâts de 40 % pendant 3 s.".into(),
            cooldown: 210.0,
            params: Some(params([("duration", 3.0), ("slow", 0.3), ("damageReduction", 0.4)])),
            targeting: Targeting::Targeted,
            range: 650.0,
            fx_key: "sum_exhaust".into(),
        },
        SummonerDef {
            id: SummonerId::Teleport,
            name: "Téléportation".into(),
            description: "Après 4 s de canalisation, se téléporte sur une tour, un sbire ou une balise alliée.".into(),
            cooldown: 360.0,
            params: Some(params([("channelDuration", 4.0)])),
            targeting: Targeting::Targeted,
            range: 99999.0,
            fx_key: "sum_teleport".into(),
        },
        SummonerDef {
            id: SummonerId::Smite,
            name: "Châtiment".into(),
            description: "Inflige 600 dégâts bruts à un monstre ou un sbire. Requis pour jouer jungle (bonus d’or/xp des camps).".into(),
            cooldown: 90.0,
            params: Some(params([("damage", 600.0), ("healMaxHp", 0.1)])),
            targeting: Targeting::Targeted,
            range: 500.0,
            fx_key: "sum_smite".into(),
        },
        SummonerDef {
            id: SummonerId::Ghost,
            name: "Fantôme".into(),
            description: "Ignore les collisions et gagne 24 à 40 % de vitesse de déplacement selon le niveau pendant 10 s.".into(),
            cooldown: 210.0,
            params: Some(params([("duration", 10.0), ("speedMin", 0.24), ("speedMax", 0.4)])),
            targeting: Targeting::SelfCast,
            range: 0.0,
            fx_key: "sum_ghost".into(),
        },
        SummonerDef {
            id: SummonerId::Cleanse,
            name: "Purge".into(),
            description: "Retire tous les contrôles (sauf projections et suppressions) et réduit les suivants de 65 % pendant 3 s.".into(),
            cooldown: 210.0,
            params: Some(params([("duration", 3.0), ("tenacity", 0.65)])),
            targeting: Targeting::SelfCast,
            range: 0.0,
            fx_key: "sum_cleanse".into(),
        },
    ]
});

/// `SUMMONERS[id]`.
pub fn summoner(id: SummonerId) -> &'static SummonerDef {
    // Les neuf sorts sont déclarés ci-dessus : la recherche ne peut pas échouer (test dédié).
    SUMMONERS
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| unreachable!("sort d'invocateur absent de SUMMONERS"))
}

/// `SUMMONER_IDS` : `Object.keys(SUMMONERS)`.
pub const SUMMONER_IDS: [SummonerId; 9] = [
    SummonerId::Flash,
    SummonerId::Ignite,
    SummonerId::Heal,
    SummonerId::Barrier,
    SummonerId::Exhaust,
    SummonerId::Teleport,
    SummonerId::Smite,
    SummonerId::Ghost,
    SummonerId::Cleanse,
];

pub const DEFAULT_SUMMONERS: [SummonerId; 2] = [SummonerId::Flash, SummonerId::Ignite];
