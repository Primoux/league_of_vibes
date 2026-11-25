//! Logique de la boutique côté interface : portage de `packages/client/src/ui/logic/shop.ts`. La boutique
//! qui fait foi reste celle du serveur.

use std::cmp::Ordering;

use lov_shared::schema::{ItemCategory, ItemDef};

use super::format::{stat_entries, stat_label};

/// Objet du catalogue `items` par id.
fn find<'a>(items: &'a [ItemDef], id: &str) -> Option<&'a ItemDef> {
    items.iter().find(|i| i.id == id)
}

/// Prix et composants consommés d'un achat.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeQuote {
    /// Prix à payer après déduction des composants possédés.
    pub price: f64,
    /// Emplacements d'inventaire des composants consommés, triés.
    pub consumed_slots: Vec<usize>,
}

/// Prix d'un objet en tenant compte des composants possédés, récursivement : un composant possédé est
/// déduit à son coût total, sinon on cherche ses propres composants.
pub fn quote_item(item_id: &str, inventory: &[Option<String>], items: &[ItemDef]) -> RecipeQuote {
    let Some(item) = find(items, item_id) else {
        return RecipeQuote {
            price: f64::INFINITY,
            consumed_slots: vec![],
        };
    };
    let mut used: Vec<usize> = Vec::new();
    let mut discount = 0.0;
    fn walk(
        id: &str,
        depth: u32,
        inventory: &[Option<String>],
        items: &[ItemDef],
        used: &mut Vec<usize>,
        discount: &mut f64,
    ) {
        let Some(def) = find(items, id) else {
            return;
        };
        if depth > 8 {
            return;
        }
        for comp in &def.builds_from {
            let slot = inventory
                .iter()
                .enumerate()
                .position(|(i, it)| it.as_deref() == Some(comp.as_str()) && !used.contains(&i));
            if let Some(slot) = slot {
                used.push(slot);
                *discount += find(items, comp).map_or(0.0, |c| c.cost);
            } else {
                walk(comp, depth + 1, inventory, items, used, discount);
            }
        }
    }
    walk(item_id, 0, inventory, items, &mut used, &mut discount);
    used.sort_unstable();
    RecipeQuote {
        price: (item.cost - discount).max(0.0),
        consumed_slots: used,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopVerdict {
    Ok,
    Gold,
    Full,
    Unique,
    Closed,
}

/// Peut-on acheter `item` ? (fontaine, or, place dans les `slots` premiers emplacements, unicité).
pub fn can_buy(
    item: &ItemDef,
    inventory: &[Option<String>],
    gold: f64,
    can_shop: bool,
    items: &[ItemDef],
    slots: usize,
) -> ShopVerdict {
    if !can_shop {
        return ShopVerdict::Closed;
    }
    let q = quote_item(&item.id, inventory, items);
    if q.price > gold {
        return ShopVerdict::Gold;
    }
    let owned = |id: &str| inventory.iter().any(|x| x.as_deref() == Some(id));
    if item.unique == Some(true) && owned(&item.id) {
        return ShopVerdict::Unique;
    }
    if item.category == ItemCategory::Trinket {
        return ShopVerdict::Ok;
    }
    let main = &inventory[..slots.min(inventory.len())];
    let used = main.iter().filter(|x| x.is_some()).count();
    let consumed_in_main = q.consumed_slots.iter().filter(|s| **s < slots).count();
    let stacks = item.max_stacks.is_some_and(|m| m > 0)
        && main.iter().any(|x| x.as_deref() == Some(item.id.as_str()));
    if !stacks && used - consumed_in_main.min(used) >= slots {
        return ShopVerdict::Full;
    }
    ShopVerdict::Ok
}

/// Objets construits à partir de `item_id` (« Utilisé dans »).
pub fn builds_into<'a>(item_id: &str, list: &'a [ItemDef]) -> Vec<&'a ItemDef> {
    list.iter()
        .filter(|i| i.builds_from.iter().any(|c| c == item_id))
        .collect()
}

pub const CATEGORY_ORDER: [ItemCategory; 7] = [
    ItemCategory::Starter,
    ItemCategory::Consumable,
    ItemCategory::Boots,
    ItemCategory::Basic,
    ItemCategory::Epic,
    ItemCategory::Legendary,
    ItemCategory::Trinket,
];

pub fn category_label(c: ItemCategory) -> &'static str {
    match c {
        ItemCategory::Starter => "Départ",
        ItemCategory::Consumable => "Consommables",
        ItemCategory::Boots => "Bottes",
        ItemCategory::Basic => "Basiques",
        ItemCategory::Epic => "Épiques",
        ItemCategory::Legendary => "Légendaires",
        ItemCategory::Trinket => "Babioles",
    }
}

fn category_rank(c: ItemCategory) -> usize {
    CATEGORY_ORDER.iter().position(|x| *x == c).unwrap_or(9)
}

/// Sans accents ni majuscules (`normalize('NFD')` puis retrait des diacritiques).
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let base = match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
                    'a'
                }
                'ç' | 'Ç' => 'c',
                'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
                'ñ' | 'Ñ' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
                'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
                'ý' | 'ÿ' | 'Ý' => 'y',
                c => c,
            };
            base.to_lowercase()
        })
        .collect()
}

/// Comparaison de noms à la française (sans accents, puis exacte).
fn name_cmp(a: &str, b: &str) -> Ordering {
    normalize(a).cmp(&normalize(b)).then_with(|| a.cmp(b))
}

fn by_category(a: &ItemDef, b: &ItemDef) -> Ordering {
    category_rank(a.category)
        .cmp(&category_rank(b.category))
        .then_with(|| a.cost.total_cmp(&b.cost))
        .then_with(|| name_cmp(&a.name, &b.name))
}

/// Texte indexé : nom, description, passif, actif et libellés des stats (« armure », « vol de vie »…).
fn search_text(i: &ItemDef) -> String {
    let stats: Vec<&str> = stat_entries(&i.stats)
        .into_iter()
        .map(|(k, _)| stat_label(k))
        .collect();
    normalize(&format!(
        "{} {} {} {} {} {}",
        i.name,
        i.description,
        i.passive.as_ref().map_or("", |p| p.name.as_str()),
        i.passive.as_ref().map_or("", |p| p.description.as_str()),
        i.active.as_ref().map_or("", |a| a.name.as_str()),
        stats.join(" ")
    ))
}

/// Recherche tolérante aux accents et à la casse, triée par catégorie puis prix puis nom.
pub fn filter_items<'a>(
    list: &'a [ItemDef],
    query: &str,
    category: Option<ItemCategory>,
) -> Vec<&'a ItemDef> {
    let q = normalize(query.trim());
    let mut out: Vec<&ItemDef> = list
        .iter()
        .filter(|i| category.is_none_or(|c| i.category == c))
        .filter(|i| q.is_empty() || search_text(i).contains(&q))
        .collect();
    out.sort_by(|a, b| by_category(a, b));
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShopSort {
    #[default]
    Category,
    PriceAsc,
    PriceDesc,
    Name,
}

/// Trie selon le mode ; `price` est le prix réellement payé (après déduction des composants).
pub fn sort_items<'a>(
    list: &[&'a ItemDef],
    mode: ShopSort,
    price: impl Fn(&ItemDef) -> f64,
) -> Vec<&'a ItemDef> {
    let mut out = list.to_vec();
    match mode {
        ShopSort::PriceAsc => out.sort_by(|a, b| {
            price(a)
                .total_cmp(&price(b))
                .then_with(|| name_cmp(&a.name, &b.name))
        }),
        ShopSort::PriceDesc => out.sort_by(|a, b| {
            price(b)
                .total_cmp(&price(a))
                .then_with(|| name_cmp(&a.name, &b.name))
        }),
        ShopSort::Name => out.sort_by(|a, b| name_cmp(&a.name, &b.name)),
        ShopSort::Category => out.sort_by(|a, b| by_category(a, b)),
    }
    out
}

/// Objets qui réutilisent un composant de l'inventaire (prix réduit), du moins cher au plus cher.
pub fn upgrades_for<'a>(
    inventory: &[Option<String>],
    list: &'a [ItemDef],
    items: &[ItemDef],
) -> Vec<&'a ItemDef> {
    if inventory.iter().all(Option::is_none) {
        return vec![];
    }
    let owned = |id: &str| inventory.iter().any(|x| x.as_deref() == Some(id));
    let mut out: Vec<(&ItemDef, f64)> = list
        .iter()
        .filter(|i| !i.builds_from.is_empty() && !(i.unique == Some(true) && owned(&i.id)))
        .map(|i| (i, quote_item(&i.id, inventory, items).price))
        .filter(|(i, p)| *p < i.cost)
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out.into_iter().map(|(i, _)| i).collect()
}

/// Valeur d'un inventaire (or des objets, au tableau des scores).
pub fn inventory_value(inventory: &[Option<String>], items: &[ItemDef]) -> f64 {
    inventory
        .iter()
        .flatten()
        .map(|id| find(items, id).map_or(0.0, |i| i.cost))
        .sum()
}

/// Portage de la partie « boutique » de `packages/client/test/ui-logic.test.ts`.
#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::data::items::ITEM_LIST;
    use lov_shared::types::PartialStats;

    fn def(id: &str, category: ItemCategory, cost: f64, from: &[&str], unique: bool) -> ItemDef {
        ItemDef {
            id: id.into(),
            name: id
                .to_uppercase()
                .chars()
                .take(1)
                .chain(id.chars().skip(1))
                .collect(),
            description: String::new(),
            category,
            cost,
            builds_from: from.iter().map(|s| (*s).to_string()).collect(),
            stats: PartialStats::default(),
            passive: None,
            active: None,
            unique: unique.then_some(true),
            max_stacks: None,
            shop: None,
            icon: "sword".into(),
        }
    }

    fn fake() -> Vec<ItemDef> {
        vec![
            def("a", ItemCategory::Basic, 400.0, &[], false),
            def("b", ItemCategory::Basic, 300.0, &[], false),
            def("mid", ItemCategory::Epic, 1100.0, &["a", "b"], false),
            def(
                "top",
                ItemCategory::Legendary,
                3000.0,
                &["mid", "a", "a"],
                true,
            ),
        ]
    }

    fn inv(ids: &[&str]) -> Vec<Option<String>> {
        ids.iter()
            .map(|s| (!s.is_empty()).then(|| (*s).to_string()))
            .collect()
    }

    fn ids(list: &[&ItemDef]) -> Vec<String> {
        list.iter().map(|i| i.id.clone()).collect()
    }

    #[test]
    fn deduit_les_composants_possedes_recursivement() {
        let items = fake();
        assert_eq!(quote_item("mid", &[], &items).price, 1100.0);
        assert_eq!(
            quote_item("mid", &inv(&["a", ""]), &items),
            RecipeQuote {
                price: 700.0,
                consumed_slots: vec![0]
            }
        );
        let q = quote_item("top", &inv(&["b", "a", "a"]), &items);
        assert_eq!(q.price, 3000.0 - 300.0 - 400.0 - 400.0);
        assert_eq!(q.consumed_slots, vec![0, 1, 2]);
        assert_eq!(
            quote_item("top", &inv(&["mid", "a"]), &items).price,
            3000.0 - 1100.0 - 400.0
        );
    }

    #[test]
    fn refuse_hors_fontaine_sans_or_plein_unique() {
        let items = fake();
        let (b, mid, top) = (&items[1], &items[2], &items[3]);
        let six = inv(&["a", "a", "a", "a", "a", "a"]);
        assert_eq!(
            can_buy(top, &[], 5000.0, false, &items, 6),
            ShopVerdict::Closed
        );
        assert_eq!(can_buy(top, &[], 100.0, true, &items, 6), ShopVerdict::Gold);
        assert_eq!(can_buy(b, &six, 5000.0, true, &items, 6), ShopVerdict::Full);
        // consomme un « a »
        assert_eq!(can_buy(mid, &six, 5000.0, true, &items, 6), ShopVerdict::Ok);
        assert_eq!(
            can_buy(top, &inv(&["top"]), 5000.0, true, &items, 6),
            ShopVerdict::Unique
        );
    }

    #[test]
    fn filtre_sans_accents_et_trie_par_categorie_puis_prix() {
        assert!(ids(&filter_items(&ITEM_LIST, "epee", None)).contains(&"long_sword".to_string()));
        let basics = filter_items(&ITEM_LIST, "", Some(ItemCategory::Basic));
        assert!(basics.iter().all(|i| i.category == ItemCategory::Basic));
        for w in basics.windows(2) {
            assert!(w[1].cost >= w[0].cost);
        }
    }

    #[test]
    fn recherche_aussi_par_libelle_de_statistique() {
        let armor = filter_items(&ITEM_LIST, "armure", None);
        assert!(!armor.is_empty());
        assert!(armor.iter().any(|i| i.stats.armor.unwrap_or(0.0) > 0.0));
    }

    #[test]
    fn trie_par_prix_apres_deduction_et_par_nom() {
        let items = fake();
        let list: Vec<&ItemDef> = items.iter().collect();
        let cost = |i: &ItemDef| i.cost;
        assert_eq!(
            ids(&sort_items(&list, ShopSort::PriceAsc, cost)),
            ["b", "a", "mid", "top"]
        );
        assert_eq!(
            ids(&sort_items(&list, ShopSort::PriceDesc, cost)),
            ["top", "mid", "a", "b"]
        );
        assert_eq!(
            ids(&sort_items(&list, ShopSort::Name, cost)),
            ["a", "b", "mid", "top"]
        );
        // avec « mid » possédé, top coûte 1900 et a 400 : le tri suit le prix payé
        let owned = inv(&["mid"]);
        let price = |i: &ItemDef| quote_item(&i.id, &owned, &items).price;
        assert_eq!(
            ids(&sort_items(
                &[&items[3], &items[0]],
                ShopSort::PriceDesc,
                price
            )),
            ["top", "a"]
        );
        assert_eq!(price(&items[3]), 1900.0);
    }

    #[test]
    fn propose_les_ameliorations_de_la_moins_chere_a_la_plus_chere() {
        let items = fake();
        assert!(upgrades_for(&[], &items, &items).is_empty());
        assert_eq!(
            ids(&upgrades_for(&inv(&["a"]), &items, &items)),
            ["mid", "top"]
        );
        assert!(upgrades_for(&inv(&["top", "mid"]), &items, &items).is_empty());
        assert_eq!(inventory_value(&inv(&["a", "", "b"]), &items), 700.0);
        assert_eq!(ids(&builds_into("a", &items)), ["mid", "top"]);
    }
}
