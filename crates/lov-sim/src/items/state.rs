//! État économique d'une entité (`entity.data.economy`) : port de `packages/server/src/items/state.ts`.
//!
//! Lu et écrit par le système économie (`systems/economy.rs`) et les effets d'objets (`items/handlers.rs`).
//!
//! Différences avec le TypeScript :
//! - les tables `charges` et `paidPrice` (`Record<number, number>` indexé par emplacement) deviennent
//!   des tableaux d'`Option` de la taille de l'inventaire ;
//! - `-Infinity` devient `f64::NEG_INFINITY`.

use std::collections::BTreeMap;

use lov_shared::constants::INVENTORY_SLOTS;
use lov_shared::types::Team;

use crate::world::Entity;

/// Nombre d'emplacements d'inventaire (hors trinket).
pub const SLOTS: usize = INVENTORY_SLOTS as usize;

/// Emplacement du trinket dans les commandes et l'historique de boutique.
pub const TRINKET_SLOT: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopActionKind {
    Buy,
    Sell,
}

/// Composant consommé par une recette.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsumedItem {
    pub slot: usize,
    pub item_id: String,
    pub paid_price: f64,
}

/// Action de boutique annulable (pile `shop_history`, vidée quand on quitte la fontaine).
#[derive(Debug, Clone, PartialEq)]
pub struct ShopAction {
    pub kind: ShopActionKind,
    pub item_id: String,
    /// Emplacement d'inventaire (0..5) ; [`TRINKET_SLOT`] = trinket.
    pub slot: i32,
    /// Or dépensé (achat) ou remboursé (vente).
    pub gold: f64,
    /// Prix total payé pour l'objet (remboursement à 70 % à la vente).
    pub paid_price: f64,
    /// Composants consommés par la recette (achat uniquement).
    pub consumed: Vec<ConsumedItem>,
    /// Charges du consommable avant l'action.
    pub charges: Option<u32>,
}

/// Rappel en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recall {
    pub remaining: f64,
    pub total: f64,
}

/// Soin sur la durée (potion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealOverTime {
    pub total: f64,
    pub remaining: f64,
    pub duration: f64,
}

/// Demande d'invocation du Colosse, lue par les objectifs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColossusCall {
    pub team: Team,
    pub x: f64,
    pub y: f64,
}

/// `data.heraldEye` : œil du Héraut, posé par les objectifs sur le champion qui l'a tué et utilisable
/// comme trinket (commande `useItem` sur l'emplacement -1) jusqu'à `expires_at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeraldEye {
    pub expires_at: f64,
}

/// `data.leviathanBuff` : buff d'équipe du Léviathan, posé par les objectifs ; l'économie y lit la
/// durée du rappel renforcé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeviathanBuff {
    pub expires_at: f64,
    /// Durée du rappel renforcé (s) ; `None` = valeur par défaut de l'économie.
    pub recall_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EconomyState {
    pub kill_streak: u32,
    pub death_streak: u32,
    /// Prochain temps de jeu où l'or passif est versé (0 = pas encore initialisé).
    pub next_passive_gold_at: f64,
    pub recall: Option<Recall>,
    pub can_shop: bool,
    pub shop_history: Vec<ShopAction>,
    /// Id du trinket équipé.
    pub trinket: Option<String>,
    pub trinket_charges: u32,
    /// Temps de jeu de recharge de la prochaine charge de trinket.
    pub trinket_recharge_at: f64,
    /// Charges par emplacement de consommable.
    pub charges: [Option<u32>; SLOTS],
    /// Prix total payé par emplacement (vente à 70 %, annulation).
    pub paid_price: [Option<f64>; SLOTS],
    /// Recharges des actifs : id d'actif -> temps de jeu de fin.
    pub item_cooldowns: BTreeMap<String, f64>,
    pub hot: Option<HealOverTime>,
    pub stase_until: f64,
    pub grievous_until: f64,
    pub spellblade_ready: bool,
    /// L'auto-attaque en cours a consommé le sort-lame : ses dégâts magiques partent après l'impact.
    pub spellblade_hit: bool,
    /// Temps de jeu à partir duquel un sort peut réarmer le sort-lame (délai de 1,5 s).
    pub spellblade_ready_at: f64,
    pub rafale_until: f64,
    pub voile_shield_at: f64,
    pub colossus_call: Option<ColossusCall>,
}

impl Default for EconomyState {
    fn default() -> Self {
        Self {
            kill_streak: 0,
            death_streak: 0,
            next_passive_gold_at: 0.0,
            recall: None,
            can_shop: false,
            shop_history: Vec::new(),
            trinket: None,
            trinket_charges: 0,
            trinket_recharge_at: 0.0,
            charges: [None; SLOTS],
            paid_price: [None; SLOTS],
            item_cooldowns: BTreeMap::new(),
            hot: None,
            stase_until: f64::NEG_INFINITY,
            grievous_until: f64::NEG_INFINITY,
            spellblade_ready: false,
            spellblade_hit: false,
            spellblade_ready_at: 0.0,
            rafale_until: f64::NEG_INFINITY,
            voile_shield_at: 0.0,
            colossus_call: None,
        }
    }
}

impl EconomyState {
    /// Vide l'emplacement `slot` de ses charges et de son prix payé.
    pub fn clear_slot(&mut self, slot: usize) {
        if slot < SLOTS {
            self.charges[slot] = None;
            self.paid_price[slot] = None;
        }
    }
}

/// `econ(e)` : état économique de l'entité, créé à la demande.
pub fn econ(e: &mut Entity) -> &mut EconomyState {
    e.data.get_or_insert_with(EconomyState::default)
}

/// Lecture seule : `None` si l'entité n'a jamais eu d'état économique.
pub fn econ_ref(e: &Entity) -> Option<&EconomyState> {
    e.data.get::<EconomyState>()
}

/// Convertit un emplacement de commande en index d'inventaire (`None` hors de `0..SLOTS`).
pub fn inventory_index(slot: i32) -> Option<usize> {
    usize::try_from(slot).ok().filter(|&s| s < SLOTS)
}
