//! Registre des kits : port de `packages/server/src/abilities/registry.ts`.
//!
//! En TypeScript, `CHAMPION_ABILITIES` associe un id de champion à quatre fonctions. En Rust un kit est
//! une valeur statique, `Kit`, que le fichier `champions/<nom>.rs` expose ; le tableau des kits vit dans
//! `champions/mod.rs` (une ligne par champion). Comme un kit TypeScript réunit les sorts ET les bouts
//! de passif qui étaient codés en dur dans `passives.ts` / `engine.ts`, `Kit` porte aussi ces crochets :
//! chacun est optionnel et le moteur les appelle à l'endroit exact où le TS exécutait la branche du
//! champion.
//!
//! `register_kit` et `register_champion_def` ajoutent des kits et des fiches à l'exécution, pour les
//! tests du moteur (un champion de test) ; les champions du jeu passent par `champions/mod.rs` et
//! `lov-shared`. Ces tables sont des constantes du processus, jamais de l'état de partie.

use std::sync::RwLock;

use lov_shared::data::champions::get_champion;
use lov_shared::schema::ChampionDef;
use lov_shared::types::EntityId;

use crate::abilities::champions::KITS;
use crate::abilities::context::CastContext;
use crate::world::World;

/// `AbilityHandler` : exécute un sort. Fonction simple (pas de capture) : l'état par partie vit dans
/// le monde.
pub type AbilityHandler = fn(&mut World, &CastContext);

/// Règle de ciblage d'un sort ciblé (`resolveTarget` du TS codait ces cas par champion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRule {
    /// Défaut : un ennemi, unité (champion, sbire, monstre).
    Enemy,
    /// Un champion ennemi (`R` de Kaelen, Vrenna, Nyx, Touilleur).
    EnemyChampion,
    /// Un champion allié ; sans cible, le lanceur (`E` d'Ilyra, `W`/`E` de Solenne).
    AllyChampion,
    /// Toute unité ou balise, sauf le lanceur, alliée ou ennemie (`E` de Nyx).
    UnitOrWard,
}

/// Le kit d'un champion. Se construit en `const` : `Kit::new("nom", [q, w, e, r])` puis les
/// `with_*` utiles.
#[derive(Debug, Clone, Copy)]
pub struct Kit {
    pub champion_id: &'static str,
    /// Q, W, E, R.
    pub abilities: [AbilityHandler; 4],
    /// Règle de ciblage de chaque emplacement ciblé (Q, W, E, R).
    pub targeting: [TargetRule; 4],
    /// Appelé à chaque lancer validé, une fois les ressources dépensées, avec le nombre de lancers déjà
    /// faits (avant celui-ci) ; renvoie l'amplification du sort (1 par défaut). Le passif de Zephyr
    /// (sort renforcé tous les N lancers, mana rendu) y vit.
    pub on_cast: Option<fn(&mut World, EntityId, u32) -> f64>,
    /// Appelé une fois par monde, à l'`init` du système `abilities`, avant les crochets du moteur :
    /// c'est ici qu'un passif branche ses `world.hooks.*` et ses écouteurs d'événements.
    pub install: Option<fn(&mut World)>,
    /// Chaque tick, pour chaque champion VIVANT de ce kit (`update(world, champion, dt)`).
    pub update_champion: Option<fn(&mut World, EntityId, f64)>,
    /// Chaque tick, pour chaque entité vivante (cibles de saignements, de marques…), après
    /// `update_champion`.
    pub update_entity: Option<fn(&mut World, EntityId, f64)>,
    /// Chaque tick, pour chaque entité morte (nettoyage de ce que le kit a posé dessus).
    pub reset_dead: Option<fn(&mut World, EntityId)>,
}

impl Kit {
    pub const fn new(champion_id: &'static str, abilities: [AbilityHandler; 4]) -> Self {
        Self {
            champion_id,
            abilities,
            targeting: [TargetRule::Enemy; 4],
            on_cast: None,
            install: None,
            update_champion: None,
            update_entity: None,
            reset_dead: None,
        }
    }

    pub const fn with_targeting(self, targeting: [TargetRule; 4]) -> Self {
        Self { targeting, ..self }
    }

    pub const fn with_on_cast(self, f: fn(&mut World, EntityId, u32) -> f64) -> Self {
        Self {
            on_cast: Some(f),
            ..self
        }
    }

    pub const fn with_install(self, f: fn(&mut World)) -> Self {
        Self {
            install: Some(f),
            ..self
        }
    }

    pub const fn with_update_champion(self, f: fn(&mut World, EntityId, f64)) -> Self {
        Self {
            update_champion: Some(f),
            ..self
        }
    }

    pub const fn with_update_entity(self, f: fn(&mut World, EntityId, f64)) -> Self {
        Self {
            update_entity: Some(f),
            ..self
        }
    }

    pub const fn with_reset_dead(self, f: fn(&mut World, EntityId)) -> Self {
        Self {
            reset_dead: Some(f),
            ..self
        }
    }
}

static EXTRA_KITS: RwLock<Vec<&'static Kit>> = RwLock::new(Vec::new());
static EXTRA_DEFS: RwLock<Vec<&'static ChampionDef>> = RwLock::new(Vec::new());

/// Ajoute (ou remplace, à id égal) un kit à l'exécution. Le kit est conservé pour toute la vie du
/// processus. Réservé aux tests du moteur : à appeler avant `World::init`, qui installe les passifs.
pub fn register_kit(kit: Kit) -> &'static Kit {
    let leaked: &'static Kit = Box::leak(Box::new(kit));
    EXTRA_KITS
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .push(leaked);
    leaked
}

/// Ajoute une fiche de champion qui n'existe pas encore dans `lov-shared` (tests du moteur). Les fiches
/// de `lov-shared` priment toujours.
pub fn register_champion_def(def: ChampionDef) -> &'static ChampionDef {
    let leaked: &'static ChampionDef = Box::leak(Box::new(def));
    EXTRA_DEFS
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .push(leaked);
    leaked
}

/// Le kit d'un champion (le dernier enregistré à l'exécution prime sur la table statique).
pub fn kit_for(id: &str) -> Option<&'static Kit> {
    let extra = EXTRA_KITS
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .rev()
        .find(|k| k.champion_id == id)
        .copied();
    extra.or_else(|| KITS.iter().copied().find(|k| k.champion_id == id))
}

/// Vide `out` puis y met tous les kits, dans l'ordre de `KITS` puis des enregistrements. Un kit
/// remplacé à l'exécution n'apparaît qu'une fois, sous sa dernière version.
pub fn kits_into(out: &mut Vec<&'static Kit>) {
    out.clear();
    let extras = EXTRA_KITS.read().unwrap_or_else(|p| p.into_inner());
    for k in KITS.iter().copied() {
        if !extras.iter().any(|x| x.champion_id == k.champion_id) {
            out.push(k);
        }
    }
    for (i, k) in extras.iter().enumerate() {
        if !extras[i + 1..]
            .iter()
            .any(|x| x.champion_id == k.champion_id)
        {
            out.push(k);
        }
    }
}

/// Fiche d'un champion : `lov-shared` d'abord, puis les fiches enregistrées. `None` si inconnu
/// (le `getChampion` du TS lève une erreur).
pub fn champion_def(id: &str) -> Option<&'static ChampionDef> {
    get_champion(id).or_else(|| {
        EXTRA_DEFS
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .rev()
            .find(|d| d.id == id)
            .copied()
    })
}
