//! Boutique (P) : onglets (conseillés selon la classe, améliorations de l'inventaire, tous, catégories),
//! recherche tolérante aux accents, filtres par statistique, « Abordables », tri, grille d'objets groupée
//! (prix avec composants déduits, manque d'or, possédés, achat rapide), fiche détaillée (statistiques,
//! passif, actif, recette, améliorations, aperçu au survol), inventaire pour vendre et annulation.
//!
//! Portage de `packages/client/src/ui/hud/Shop.tsx`. Clic : détails ; clic droit ou double-clic :
//! acheter ; flèches : naviguer ; Entrée : acheter la sélection. Achats à la fontaine (ou mort)
//! seulement, consultation ailleurs.
//!
//! Chaque zone (onglets, outils, grille, fiche, inventaire) n'est reconstruite que quand ce qu'elle montre
//! change ; ce qui suit l'or ou la sélection (couleurs, « manque », bouton d'achat) est mis à jour en
//! place, pour que le survol et le double-clic tiennent.

use std::sync::LazyLock;

use bevy::input_focus::InputFocus;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::{EditableText, Strikethrough, StrikethroughColor};
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{ScrollArea, TextInput};
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::{ITEM_LIST, item};
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{ChampionClass, ItemCategory, ItemDef};

use super::{HUD_Z, label, set_if, set_shown, set_text};
use crate::input::PlayerInput;
use crate::net::{Connection, GameState};
use crate::ui::icons::draw::{IconCache, category_colors};
use crate::ui::icons::paths::ITEM_GLYPH;
use crate::ui::logic::format::{format_stat, stat_entries, stat_label};
use crate::ui::logic::shop::{
    CATEGORY_ORDER, ShopSort, ShopVerdict, builds_into, can_buy, category_label, filter_items,
    quote_item, sort_items, upgrades_for,
};
use crate::ui::theme::{DANGER, GOLD, GOLD_HI, MUTED, TEAL, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, ButtonLook, Screen, UiFonts, button};

/// Emplacements de l'inventaire principal (la balise est à part).
const MAIN_SLOTS: usize = 6;
/// Largeur d'une carte et écart de la grille (px), pour compter les colonnes.
const CARD_W: f32 = 158.0;
const GAP: f32 = 8.0;
const GRID_PAD: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Reco,
    Up,
    All,
    Cat(ItemCategory),
}

fn tab_label(t: Tab) -> &'static str {
    match t {
        Tab::Reco => "Conseillés",
        Tab::Up => "Améliorations",
        Tab::All => "Tous",
        Tab::Cat(c) => category_label(c),
    }
}

fn sort_label(s: ShopSort) -> &'static str {
    match s {
        ShopSort::Category => "Par catégorie",
        ShopSort::PriceAsc => "Prix croissant",
        ShopSort::PriceDesc => "Prix décroissant",
        ShopSort::Name => "Nom (A → Z)",
    }
}

fn next_sort(s: ShopSort) -> ShopSort {
    match s {
        ShopSort::Category => ShopSort::PriceAsc,
        ShopSort::PriceAsc => ShopSort::PriceDesc,
        ShopSort::PriceDesc => ShopSort::Name,
        ShopSort::Name => ShopSort::Category,
    }
}

/// Filtres par statistique, combinables : l'objet doit avoir chacune des familles cochées.
const STAT_FILTERS: [(&str, &[&str]); 13] = [
    ("Attaque", &["ad"]),
    ("Vit. d'attaque", &["attackSpeed"]),
    ("Critique", &["critChance"]),
    ("Létalité", &["armorPen"]),
    ("Vol de vie", &["lifesteal"]),
    ("Puissance", &["ap"]),
    ("Pén. magique", &["magicPen"]),
    ("Mana", &["maxMana", "manaRegen"]),
    ("Accélération", &["abilityHaste"]),
    ("PV", &["maxHp", "hpRegen"]),
    ("Armure", &["armor"]),
    ("Rés. magique", &["mr"]),
    ("Vitesse", &["moveSpeed"]),
];

/// Libellés courts des statistiques sur les cartes.
fn short_stat(key: &str) -> &str {
    match key {
        "ad" => "AD",
        "ap" => "AP",
        "maxHp" => "PV",
        "maxMana" => "mana",
        "armor" => "armure",
        "mr" => "RM",
        "attackSpeed" => "VA",
        "critChance" => "crit",
        "moveSpeed" => "vitesse",
        "abilityHaste" => "accél.",
        "lifesteal" => "vol de vie",
        "armorPen" => "létalité",
        "magicPen" => "pén. mag.",
        "hpRegen" => "régén. PV",
        "manaRegen" => "régén. mana",
        k => stat_label(k),
    }
}

/// `mask` : bits des familles de [`STAT_FILTERS`] cochées (`matchesStatFilters`).
fn matches_stats(it: &ItemDef, mask: u16) -> bool {
    if mask == 0 {
        return true;
    }
    let stats = stat_entries(&it.stats);
    STAT_FILTERS.iter().enumerate().all(|(i, (_, keys))| {
        mask & (1 << i) == 0
            || keys
                .iter()
                .any(|k| stats.iter().any(|(s, v)| s == k && *v != 0.0))
    })
}

/// Stats recherchées par classe (objets conseillés).
fn class_stats(c: ChampionClass) -> &'static [&'static str] {
    match c {
        ChampionClass::Tank => &["armor", "mr", "maxHp", "hpRegen"],
        ChampionClass::Fighter => &["ad", "maxHp", "abilityHaste", "armor", "lifesteal"],
        ChampionClass::Mage => &["ap", "magicPen", "maxMana", "abilityHaste"],
        ChampionClass::Marksman => &["ad", "critChance", "attackSpeed", "lifesteal"],
        ChampionClass::Assassin => &["ad", "armorPen", "abilityHaste"],
        ChampionClass::Support => &["abilityHaste", "maxMana", "manaRegen", "maxHp", "ap"],
    }
}

/// Objets achetables (`shop: false` = donné par un objectif), copiés une fois pour toutes.
static SHOP: LazyLock<Vec<ItemDef>> = LazyLock::new(|| {
    ITEM_LIST
        .iter()
        .filter(|i| i.shop != Some(false))
        .cloned()
        .collect()
});

fn shop_list() -> &'static [ItemDef] {
    &SHOP
}

fn category_rank(c: ItemCategory) -> usize {
    CATEGORY_ORDER.iter().position(|x| *x == c).unwrap_or(9)
}

/// Objets conseillés pour une classe (`recommended`).
fn recommended(class: Option<ChampionClass>) -> Vec<&'static ItemDef> {
    let Some(class) = class else { return vec![] };
    let wanted = class_stats(class);
    let score = |i: &ItemDef| {
        stat_entries(&i.stats)
            .iter()
            .filter(|(k, _)| wanted.contains(k))
            .count()
    };
    let mut v: Vec<&ItemDef> = shop_list()
        .iter()
        .filter(|i| {
            i.category != ItemCategory::Trinket
                && (score(i) > 0
                    || matches!(i.category, ItemCategory::Consumable | ItemCategory::Boots))
        })
        .collect();
    v.sort_by(|a, b| score(b).cmp(&score(a)).then(b.cost.total_cmp(&a.cost)));
    v.truncate(20);
    v.sort_by(|a, b| {
        category_rank(a.category)
            .cmp(&category_rank(b.category))
            .then(a.cost.total_cmp(&b.cost))
    });
    v
}

/// Ce que l'interface lit du joueur pour la boutique, sans copie.
struct Wallet<'a> {
    gold: f64,
    can_shop: bool,
    items: &'a [Option<String>],
    class: Option<ChampionClass>,
}

fn wallet(state: &GameState) -> Wallet<'_> {
    let st = state.self_state.as_ref();
    let e = state.my_entity.and_then(|id| state.entities().get(&id));
    Wallet {
        gold: st.map_or(0.0, |s| s.gold),
        can_shop: st.is_some_and(|s| s.can_shop) || e.is_some_and(|e| e.dead == Some(true)),
        items: e
            .and_then(|e| e.champ.as_ref())
            .map_or(&[], |c| c.items.as_slice()),
        class: e
            .and_then(|e| e.subtype.as_deref())
            .and_then(get_champion)
            .map(|c| c.class),
    }
}

fn verdict(it: &ItemDef, w: &Wallet) -> ShopVerdict {
    can_buy(it, w.items, w.gold, w.can_shop, &ITEM_LIST, MAIN_SLOTS)
}

fn price(it: &ItemDef, inv: &[Option<String>]) -> f64 {
    quote_item(&it.id, inv, &ITEM_LIST).price
}

fn verdict_text(v: ShopVerdict) -> &'static str {
    match v {
        ShopVerdict::Ok => "Acheter",
        ShopVerdict::Gold => "Or insuffisant",
        ShopVerdict::Full => "Inventaire plein",
        ShopVerdict::Unique => "Déjà possédé",
        ShopVerdict::Closed => "Retourne à la fontaine",
    }
}

// ---------------------------------------------------------------------------------- liste

/// Un groupe de la grille : « Pour ton inventaire », une catégorie, ou la liste seule (libellé vide).
#[derive(Clone, Debug, PartialEq)]
struct Group {
    label: &'static str,
    up: bool,
    items: Vec<&'static ItemDef>,
}

/// Ce que montre la grille pour ces réglages : onglet effectif, groupes, et ordre de navigation.
struct Listing {
    tab: Tab,
    groups: Vec<Group>,
    flat: Vec<&'static ItemDef>,
}

/// Groupes de la grille (`groups` de `ShopPanel`).
fn listing(f: &Filters, w: &Wallet) -> Listing {
    let upgrades = upgrades_for(w.items, shop_list(), &ITEM_LIST);
    // l'onglet Améliorations vide retombe sur Conseillés
    let tab = if f.tab == Tab::Up && upgrades.is_empty() {
        Tab::Reco
    } else {
        f.tab
    };
    let searching = !f.query.trim().is_empty();
    let keep =
        |i: &&ItemDef| matches_stats(i, f.stats) && (!f.affordable || price(i, w.items) <= w.gold);
    let base: Vec<&'static ItemDef> = if searching {
        filter_items(shop_list(), &f.query, None)
    } else {
        match tab {
            Tab::Reco => recommended(w.class),
            Tab::Up => upgrades.clone(),
            Tab::All => filter_items(shop_list(), "", None),
            Tab::Cat(c) => filter_items(shop_list(), "", Some(c)),
        }
    };
    let filtered: Vec<&'static ItemDef> = base.into_iter().filter(keep).collect();
    let by_price = |i: &ItemDef| price(i, w.items);
    let list = if f.sort == ShopSort::Category {
        filtered
    } else {
        sort_items(&filtered, f.sort, by_price)
    };
    let mut groups = Vec::new();
    if !searching && tab == Tab::Reco {
        let mut ups: Vec<&'static ItemDef> = upgrades.into_iter().filter(keep).take(6).collect();
        if f.sort != ShopSort::Category {
            ups = sort_items(&ups, f.sort, by_price);
        }
        if !ups.is_empty() {
            groups.push(Group {
                label: "Pour ton inventaire",
                up: true,
                items: ups,
            });
        }
    }
    let by_category = f.sort == ShopSort::Category && tab != Tab::Up;
    if by_category && (searching || matches!(tab, Tab::All | Tab::Reco)) {
        for c in CATEGORY_ORDER {
            let items: Vec<_> = list.iter().copied().filter(|i| i.category == c).collect();
            if !items.is_empty() {
                groups.push(Group {
                    label: category_label(c),
                    up: false,
                    items,
                });
            }
        }
    } else if !list.is_empty() {
        groups.push(Group {
            label: "",
            up: false,
            items: list,
        });
    }
    // ordre de navigation : un objet présent dans deux groupes compte à sa première place
    let mut flat: Vec<&'static ItemDef> = Vec::new();
    for g in &groups {
        for it in &g.items {
            if !flat.iter().any(|x| x.id == it.id) {
                flat.push(it);
            }
        }
    }
    Listing { tab, groups, flat }
}

/// Objets de chaque onglet (`count`).
fn tab_count(t: Tab, w: &Wallet) -> usize {
    match t {
        Tab::Reco => recommended(w.class).len(),
        Tab::Up => upgrades_for(w.items, shop_list(), &ITEM_LIST).len(),
        Tab::All => shop_list().len(),
        Tab::Cat(c) => shop_list().iter().filter(|i| i.category == c).count(),
    }
}

// ---------------------------------------------------------------------------------- état

/// Réglages de la liste : gardés d'une ouverture à l'autre (`prefs` du web), recherche comprise.
#[derive(Clone, Debug, PartialEq)]
struct Filters {
    tab: Tab,
    query: String,
    stats: u16,
    sort: ShopSort,
    affordable: bool,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            tab: Tab::Reco,
            query: String::new(),
            stats: 0,
            sort: ShopSort::Category,
            affordable: false,
        }
    }
}

impl Filters {
    fn active(&self) -> bool {
        self.stats != 0 || self.affordable || !self.query.trim().is_empty()
    }
}

/// Zones à reconstruire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Dirty {
    nav: bool,
    grid: bool,
    detail: bool,
    inventory: bool,
}

impl Dirty {
    const ALL: Self = Self {
        nav: true,
        grid: true,
        detail: true,
        inventory: true,
    };
}

/// État de la boutique.
#[derive(Resource, Default)]
pub struct ShopUi {
    filters: Filters,
    /// Objet épinglé (clic, flèches).
    selected: Option<String>,
    /// Objet survolé dans la grille : aperçu dans la fiche.
    hover: Option<String>,
    sell_slot: Option<usize>,
    /// Ce que montre la grille.
    groups: Vec<Group>,
    flat: Vec<&'static ItemDef>,
    tab_shown: Option<Tab>,
    /// Copies de ce qui a servi à construire les zones.
    inv: Vec<Option<String>>,
    class: Option<ChampionClass>,
    can_shop: bool,
    gold: i64,
    detail_of: Option<(String, bool)>,
    /// Les réglages ont changé : liste à recalculer.
    stale: bool,
    dirty: Dirty,
    /// Dernière mise à jour en place (or, fontaine, objet épinglé) ; `None` après une reconstruction.
    live: Option<(i64, bool, Option<&'static str>)>,
    /// Carte à faire défiler jusqu'à la vue, remise de la grille en haut.
    scroll_to: Option<&'static str>,
    scroll_top: bool,
}

impl ShopUi {
    /// Objet épinglé, sinon le premier de la liste.
    fn pinned(&self) -> Option<&'static ItemDef> {
        self.selected
            .as_deref()
            .and_then(item)
            .or_else(|| self.flat.first().copied())
    }

    /// Ce que montre la fiche : l'objet survolé, sinon l'épinglé ; et s'il s'agit d'un aperçu.
    fn shown(&self) -> Option<(&'static ItemDef, bool)> {
        let pinned = self.pinned();
        match self.hover.as_deref().and_then(item) {
            Some(h) => Some((h, pinned.is_none_or(|p| p.id != h.id))),
            None => pinned.map(|p| (p, false)),
        }
    }

    fn refilter(&mut self) {
        self.stale = true;
    }

    /// Flèches : déplace la sélection de `step` cartes dans l'ordre de la grille.
    fn navigate(&mut self, step: i32) {
        if self.flat.is_empty() {
            return;
        }
        let cur = self
            .pinned()
            .and_then(|p| self.flat.iter().position(|x| x.id == p.id));
        let last = self.flat.len() as i32 - 1;
        let next = cur.map_or(0, |c| (c as i32 + step).clamp(0, last)) as usize;
        let id = self.flat[next].id.as_str();
        self.selected = Some(id.to_owned());
        self.hover = None;
        self.scroll_to = Some(id);
    }
}

// ---------------------------------------------------------------------------------- composants

/// Fenêtre de la boutique.
#[derive(Component)]
struct ShopWindow;

#[derive(Component)]
struct Nav;

#[derive(Component)]
struct Tools;

#[derive(Component)]
struct Grid;

#[derive(Component)]
struct Detail;

#[derive(Component)]
struct Inventory;

#[derive(Component)]
struct WalletText;

#[derive(Component)]
struct ClosedHint;

#[derive(Component)]
struct Search;

/// Carte de la grille et ses parties suivies en place.
#[derive(Component, Clone, Copy)]
struct Card(&'static ItemDef);

#[derive(Component, Clone, Copy)]
struct CardName(&'static ItemDef);

#[derive(Component, Clone, Copy)]
struct CardPrice(&'static ItemDef);

#[derive(Component, Clone, Copy)]
struct CardMissing(&'static ItemDef, Option<i64>);

#[derive(Component, Clone, Copy)]
struct CardQuick(&'static ItemDef);

/// Bouton d'achat de la fiche, son titre, sa ligne de prix et sa jauge d'or.
#[derive(Component, Clone, Copy)]
struct BuyButton(&'static ItemDef);

#[derive(Component, Clone, Copy)]
struct BuyTitle(&'static ItemDef, Option<i64>);

#[derive(Component, Clone, Copy)]
struct BuySub;

#[derive(Component, Clone, Copy)]
struct BuyProgress(&'static ItemDef);

/// Action d'un élément cliquable de la boutique.
#[derive(Component, Clone, Debug, PartialEq)]
enum Act {
    Tab(Tab),
    Item(&'static str),
    Buy(&'static str),
    Stat(usize),
    Affordable,
    Sort,
    ResetFilters,
    Slot(usize),
    Sell,
    Undo,
    Close,
}

fn icon_node(
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    it: &ItemDef,
    size: f32,
) -> impl Bundle {
    (
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        ImageNode::new(icons.item(images, it)),
        Pickable::IGNORE,
    )
}

fn glyph(
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    name: &str,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Node {
            width: px(size),
            height: px(size),
            flex_shrink: 0.0,
            ..default()
        },
        ImageNode {
            image: icons.glyph(images, name),
            color,
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn coin(icons: &mut IconCache, images: &mut Assets<Image>, size: f32) -> impl Bundle {
    glyph(icons, images, "gold", size, GOLD_HI)
}

/// Glyphe d'un onglet (`TAB_GLYPH`).
fn tab_icon(icons: &mut IconCache, images: &mut Assets<Image>, t: Tab) -> Handle<Image> {
    let name = match t {
        Tab::Reco | Tab::Cat(ItemCategory::Legendary) => "crown",
        Tab::Up => "plus",
        Tab::All => "nexus",
        Tab::Cat(ItemCategory::Starter) => "sword",
        Tab::Cat(ItemCategory::Boots) => "bolt",
        Tab::Cat(ItemCategory::Basic) => "inhibitor",
        Tab::Cat(ItemCategory::Epic) => "dragon",
        Tab::Cat(ItemCategory::Trinket) => "gold",
        Tab::Cat(ItemCategory::Consumable) => {
            let d = ITEM_GLYPH
                .iter()
                .find(|(k, _)| *k == "flask")
                .map_or("", |(_, d)| *d);
            return icons.path(images, "flask", d);
        }
    };
    icons.glyph(images, name)
}

fn spawn(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut ui: ResMut<ShopUi>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let f = &*fonts;
    ui.dirty = Dirty::ALL;
    ui.stale = true;
    ui.live = None;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(HUD_Z + 2),
            Pickable::IGNORE,
            Visibility::Hidden,
            ShopWindow,
            DespawnOnExit(Screen::Game),
        ))
        .with_children(|back| {
            back.spawn((
                Node {
                    width: px(1000),
                    height: px(660),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(10)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.6)),
                BackgroundColor(hexa(0x080a16, 0.97)),
                BlocksWorld,
            ))
            .with_children(|win| {
                // en-tête : titre, recherche, état, or, fermer
                win.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(14),
                        padding: UiRect::axes(px(16), px(10)),
                        border: UiRect::bottom(px(1)),
                        ..default()
                    },
                    BorderColor::all(hexa(0xd9b25f, 0.25)),
                ))
                .with_children(|h| {
                    h.spawn(glyph(&mut icons, &mut images, "gold", 20.0, GOLD_HI));
                    h.spawn((
                        Text::new("Boutique"),
                        f.display(22.0, 800),
                        TextColor(GOLD_HI),
                    ));
                    h.spawn((
                        Node {
                            flex_grow: 1.0,
                            height: px(32),
                            padding: UiRect::horizontal(px(10)),
                            column_gap: px(8),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(6)),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(hexa(0xffffff, 0.15)),
                        BackgroundColor(hexa(0x000000, 0.35)),
                    ))
                    .with_children(|s| {
                        s.spawn(glyph(&mut icons, &mut images, "search", 15.0, MUTED));
                        let mut text = EditableText::new(&ui.filters.query);
                        text.max_characters = Some(40);
                        s.spawn((
                            Search,
                            TextInput,
                            text,
                            f.text(14.0, 500),
                            TextColor(TEXT),
                            Node {
                                flex_grow: 1.0,
                                height: px(20),
                                ..default()
                            },
                        ));
                    });
                    h.spawn((
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: px(5),
                            ..default()
                        },
                        ClosedHint,
                    ))
                    .with_children(|c| {
                        c.spawn(glyph(&mut icons, &mut images, "lock", 13.0, DANGER));
                        c.spawn(label(
                            f,
                            "Hors de la fontaine — consultation seule",
                            12.0,
                            600,
                            DANGER,
                        ));
                    });
                    h.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(6),
                        ..default()
                    })
                    .with_children(|wl| {
                        wl.spawn(coin(&mut icons, &mut images, 16.0));
                        wl.spawn((label(f, "0", 18.0, 800, GOLD_HI), WalletText));
                    });
                    h.spawn((
                        button(
                            ButtonLook::new(hexa(0xffffff, 0.06), hexa(0xffffff, 0.16)),
                            Node {
                                width: px(30),
                                height: px(30),
                                border_radius: BorderRadius::all(px(6)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ),
                        Act::Close,
                    ))
                    .with_child(glyph(
                        &mut icons,
                        &mut images,
                        "close",
                        14.0,
                        TEXT,
                    ));
                });
                win.spawn(Node {
                    flex_grow: 1.0,
                    min_height: px(0),
                    ..default()
                })
                .with_children(|body| {
                    body.spawn((
                        Node {
                            width: px(196),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2),
                            padding: UiRect::all(px(8)),
                            border: UiRect::right(px(1)),
                            ..default()
                        },
                        BorderColor::all(hexa(0xffffff, 0.08)),
                        Nav,
                    ));
                    body.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: px(0),
                        flex_direction: FlexDirection::Column,
                        min_height: px(0),
                        ..default()
                    })
                    .with_children(|mid| {
                        mid.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(6),
                                padding: UiRect::new(px(10), px(10), px(8), px(4)),
                                ..default()
                            },
                            Tools,
                        ));
                        mid.spawn((
                            Node {
                                flex_grow: 1.0,
                                min_height: px(0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                align_content: AlignContent::FlexStart,
                                column_gap: px(GAP),
                                row_gap: px(GAP),
                                padding: UiRect::all(px(GRID_PAD)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollArea,
                            ScrollPosition::default(),
                            Grid,
                        ));
                    });
                    body.spawn((
                        Node {
                            width: px(280),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(8),
                            padding: UiRect::all(px(14)),
                            border: UiRect::left(px(1)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        BorderColor::all(hexa(0xffffff, 0.08)),
                        BackgroundColor(hexa(0x000000, 0.2)),
                        ScrollArea,
                        Detail,
                    ));
                });
                win.spawn((
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(10),
                        padding: UiRect::axes(px(16), px(10)),
                        border: UiRect::top(px(1)),
                        ..default()
                    },
                    BorderColor::all(hexa(0xd9b25f, 0.25)),
                    Inventory,
                ));
            });
        });
}

// ---------------------------------------------------------------------------------- dessin

fn chip(on: bool) -> ButtonLook {
    ButtonLook::new(
        if on {
            hexa(0xd9b25f, 0.28)
        } else {
            hexa(0xffffff, 0.05)
        },
        hexa(0xffffff, 0.12),
    )
}

fn chip_node() -> Node {
    Node {
        padding: UiRect::axes(px(8), px(3)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(10)),
        align_items: AlignItems::Center,
        column_gap: px(4),
        ..default()
    }
}

fn draw_nav(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    ui: &ShopUi,
    w: &Wallet,
) {
    let searching = !ui.filters.query.trim().is_empty();
    let tabs = [Tab::Reco, Tab::Up, Tab::All]
        .into_iter()
        .chain(CATEGORY_ORDER.iter().map(|c| Tab::Cat(*c)));
    let mut first = true;
    for t in tabs {
        let n = tab_count(t, w);
        if n == 0 {
            continue;
        }
        if !first && (t == Tab::All || t == Tab::Cat(CATEGORY_ORDER[0])) {
            p.spawn((
                Node {
                    height: px(1),
                    margin: UiRect::vertical(px(4)),
                    ..default()
                },
                BackgroundColor(hexa(0xffffff, 0.08)),
            ));
        }
        first = false;
        let on = !searching && ui.tab_shown == Some(t);
        let color = if on {
            GOLD_HI
        } else if t == Tab::Up {
            TEAL
        } else {
            TEXT
        };
        p.spawn((
            button(
                ButtonLook::new(
                    if on {
                        hexa(0xd9b25f, 0.22)
                    } else {
                        Color::NONE
                    },
                    hexa(0xffffff, 0.08),
                ),
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    border_radius: BorderRadius::all(px(6)),
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            Act::Tab(t),
        ))
        .with_children(|b| {
            b.spawn((
                Node {
                    width: px(15),
                    height: px(15),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode {
                    image: tab_icon(icons, images, t),
                    color,
                    ..default()
                },
                Pickable::IGNORE,
            ));
            b.spawn((
                label(f, tab_label(t), 13.0, if on { 800 } else { 600 }, color),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            b.spawn(label(f, n.to_string(), 11.0, 600, MUTED));
        });
    }
    p.spawn(Node {
        height: px(10),
        ..default()
    });
    for line in [
        "Clic : détails",
        "Double-clic / clic droit : acheter",
        "←↑↓→ : naviguer · Entrée : acheter",
    ] {
        p.spawn(label(f, line, 10.0, 500, MUTED));
    }
}

fn draw_tools(p: &mut ChildSpawnerCommands, f: &UiFonts, ui: &ShopUi) {
    let fl = &ui.filters;
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(4),
        row_gap: px(4),
        ..default()
    })
    .with_children(|row| {
        for (i, (name, _)) in STAT_FILTERS.iter().enumerate() {
            let on = fl.stats & (1 << i) != 0;
            row.spawn((
                button(chip(on), chip_node()),
                BorderColor::all(if on { GOLD } else { hexa(0xffffff, 0.12) }),
                Act::Stat(i),
            ))
            .with_child(label(f, *name, 11.0, 600, if on { GOLD_HI } else { TEXT }));
        }
    });
    p.spawn(Node {
        column_gap: px(6),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            button(chip(fl.affordable), chip_node()),
            BorderColor::all(if fl.affordable {
                GOLD
            } else {
                hexa(0xffffff, 0.12)
            }),
            Act::Affordable,
        ))
        .with_child(label(
            f,
            "Abordables",
            11.0,
            700,
            if fl.affordable { GOLD_HI } else { TEXT },
        ));
        row.spawn((
            button(chip(false), chip_node()),
            BorderColor::all(hexa(0xffffff, 0.12)),
            Act::Sort,
        ))
        .with_child(label(
            f,
            format!("Tri : {}", sort_label(fl.sort)),
            11.0,
            600,
            TEXT,
        ));
        if !fl.query.trim().is_empty() {
            row.spawn(label(
                f,
                format!("Résultats pour « {} » · {}", fl.query.trim(), ui.flat.len()),
                11.0,
                600,
                MUTED,
            ));
        }
        if fl.active() {
            row.spawn((
                button(chip(false), chip_node()),
                BorderColor::all(hexa(0xff8a7a, 0.4)),
                Act::ResetFilters,
            ))
            .with_child(label(f, "× Réinitialiser", 11.0, 600, TEXT));
        }
    });
}

/// Couleurs d'une carte selon l'or, la fontaine et la sélection (appliquées à la construction, puis
/// en place).
struct CardLook {
    normal: Color,
    border: Color,
    name: Color,
    price: Color,
    /// Or manquant, arrondi.
    missing: Option<i64>,
    quick: bool,
}

fn card_look(it: &ItemDef, w: &Wallet, pinned: bool) -> CardLook {
    let v = verdict(it, w);
    let missing = (price(it, w.items) - w.gold).ceil();
    let (c1, _) = category_colors(it.category);
    CardLook {
        normal: if pinned {
            hexa(0xd9b25f, 0.18)
        } else {
            hexa(0xffffff, 0.04)
        },
        border: if pinned {
            GOLD_HI
        } else {
            hex(c1).with_alpha(0.45)
        },
        name: if v == ShopVerdict::Ok { TEXT } else { MUTED },
        price: if missing > 0.0 {
            hex(0xc9a35a)
        } else {
            GOLD_HI
        },
        missing: (missing > 0.0).then_some(missing as i64),
        quick: v == ShopVerdict::Ok,
    }
}

fn draw_card(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    it: &'static ItemDef,
    w: &Wallet,
    pinned: bool,
) {
    let q = quote_item(&it.id, w.items, &ITEM_LIST);
    let look = card_look(it, w, pinned);
    let owned = w
        .items
        .iter()
        .filter(|x| x.as_deref() == Some(it.id.as_str()))
        .count();
    let stats = stat_entries(&it.stats);
    let fx = it
        .active
        .as_ref()
        .map(|a| &a.name)
        .or(it.passive.as_ref().map(|p| &p.name));
    p.spawn((
        button(
            ButtonLook::new(look.normal, hexa(0xffffff, 0.1)),
            Node {
                width: px(CARD_W),
                padding: UiRect::new(px(6), px(20), px(6), px(6)), // à droite : le bouton d'achat rapide
                column_gap: px(8),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                align_items: AlignItems::Center,
                ..default()
            },
        ),
        BorderColor::all(look.border),
        Act::Item(it.id.as_str()),
        Card(it),
    ))
    .with_children(|card| {
        card.spawn(Node {
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|ic| {
            ic.spawn(icon_node(icons, images, it, 44.0));
            if owned > 0 {
                ic.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(-3),
                        bottom: px(-3),
                        padding: UiRect::axes(px(3), px(0)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BackgroundColor(hexa(0x0f3a33, 0.95)),
                    Pickable::IGNORE,
                ))
                .with_child(label(
                    f,
                    if owned > 1 {
                        format!("×{owned}")
                    } else {
                        "✓".into()
                    },
                    10.0,
                    800,
                    TEAL,
                ));
            }
        });
        card.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                flex_shrink: 1.0,
                flex_grow: 1.0,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|body| {
            body.spawn((
                label(f, it.name.clone(), 12.0, 700, look.name),
                CardName(it),
            ));
            // trois puces de stats, sinon le nom de l'effet
            body.spawn((
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(4),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|chips| {
                for (k, v) in stats.iter().take(3) {
                    chips.spawn(label(
                        f,
                        format!("{} {}", format_stat(k, *v, true), short_stat(k)),
                        9.5,
                        600,
                        hex(0x9ff0c2),
                    ));
                }
                if stats.is_empty()
                    && let Some(name) = fx
                {
                    chips.spawn(label(f, name.clone(), 9.5, 600, hex(0xffd16b)));
                }
            });
            body.spawn((
                Node {
                    column_gap: px(4),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|row| {
                row.spawn(coin(icons, images, 11.0));
                row.spawn((
                    label(f, format!("{}", q.price.round()), 12.0, 800, look.price),
                    CardPrice(it),
                ));
                if q.price < it.cost {
                    row.spawn((
                        label(f, format!("{}", it.cost.round()), 10.0, 600, MUTED),
                        Strikethrough,
                        StrikethroughColor(MUTED),
                    ));
                }
                row.spawn((
                    label(
                        f,
                        look.missing
                            .map_or(String::new(), |m| format!("manque {m}")),
                        10.0,
                        600,
                        hexa(0xff8a7a, 0.85),
                    ),
                    CardMissing(it, look.missing),
                ));
            });
        });
        // achat rapide
        card.spawn((
            button(
                ButtonLook::new(hexa(0xc99a3c, 0.85), hex(0xe8b955)),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(4),
                    top: px(4),
                    width: px(18),
                    height: px(18),
                    border_radius: BorderRadius::all(px(4)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            Act::Buy(it.id.as_str()),
            CardQuick(it),
            if look.quick {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ))
        .with_child(glyph(icons, images, "plus", 10.0, hex(0x1a1206)));
    });
}

fn draw_grid(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    ui: &ShopUi,
    w: &Wallet,
) {
    if ui.flat.is_empty() {
        p.spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(8),
            margin: UiRect::top(px(40)),
            ..default()
        })
        .with_children(|e| {
            e.spawn(glyph(icons, images, "search", 28.0, MUTED));
            e.spawn(label(
                f,
                if ui.filters.affordable {
                    "Aucun objet ne correspond à ton or actuel."
                } else {
                    "Aucun objet ne correspond."
                },
                14.0,
                500,
                MUTED,
            ));
            if ui.filters.active() {
                e.spawn((
                    button(
                        ButtonLook::new(hexa(0xffffff, 0.08), hexa(0xffffff, 0.16)),
                        Node {
                            padding: UiRect::axes(px(12), px(6)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                    ),
                    Act::ResetFilters,
                ))
                .with_child(label(
                    f,
                    "Réinitialiser les filtres",
                    12.0,
                    700,
                    TEXT,
                ));
            }
        });
        return;
    }
    let pinned = ui.pinned().map(|p| p.id.as_str());
    for g in &ui.groups {
        if !g.label.is_empty() {
            p.spawn(Node {
                width: percent(100),
                margin: UiRect::top(px(4)),
                column_gap: px(6),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                h.spawn(label(f, g.label, 12.0, 800, if g.up { TEAL } else { GOLD }));
                h.spawn(label(f, g.items.len().to_string(), 10.0, 600, MUTED));
            });
        }
        for it in &g.items {
            draw_card(p, f, icons, images, it, w, pinned == Some(it.id.as_str()));
        }
    }
}

/// Arbre de recette (`RecipeTree`) : l'objet, puis ses composants en retrait, récursivement.
fn recipe_tree(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    it: &'static ItemDef,
    w: &Wallet,
    root: bool,
) {
    let owned = !root && w.items.iter().any(|x| x.as_deref() == Some(it.id.as_str()));
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(4),
        ..default()
    })
    .with_children(|col| {
        col.spawn((
            button(
                ButtonLook::new(Color::NONE, hexa(0xffffff, 0.08)),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(px(2)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
            ),
            BorderColor::all(if owned { TEAL } else { Color::NONE }),
            Act::Item(it.id.as_str()),
        ))
        .with_children(|n| {
            n.spawn(icon_node(icons, images, it, if root { 40.0 } else { 32.0 }));
            n.spawn(label(f, format!("{}", it.cost.round()), 10.0, 700, GOLD_HI));
        });
        if !it.builds_from.is_empty() {
            col.spawn(Node {
                column_gap: px(4),
                padding: UiRect::top(px(2)),
                border: UiRect::top(px(1)),
                ..default()
            })
            .insert(BorderColor::all(hexa(0xffffff, 0.12)))
            .with_children(|kids| {
                for c in it.builds_from.iter().filter_map(|c| item(c)) {
                    recipe_tree(kids, f, icons, images, c, w, false);
                }
            });
        }
    });
}

/// Couleurs et texte du bouton d'achat de la fiche selon l'or et la fontaine.
fn buy_look(it: &ItemDef, w: &Wallet) -> (bool, Option<i64>, String, f32) {
    let v = verdict(it, w);
    let q = price(it, w.items);
    let missing = (q - w.gold).ceil().max(0.0) as i64;
    let title = match v {
        ShopVerdict::Gold => format!("Il manque {missing} or"),
        v => verdict_text(v).to_string(),
    };
    let progress = if q > 0.0 {
        (w.gold / q).min(1.0) as f32
    } else {
        1.0
    };
    (
        v == ShopVerdict::Ok,
        (v == ShopVerdict::Gold).then_some(missing),
        title,
        progress,
    )
}

fn buy_colors(ok: bool) -> (ButtonLook, Color) {
    (
        ButtonLook::new(
            if ok {
                hex(0xc99a3c)
            } else {
                hexa(0xffffff, 0.08)
            },
            if ok {
                hex(0xe8b955)
            } else {
                hexa(0xffffff, 0.08)
            },
        ),
        if ok { hex(0x1a1206) } else { MUTED },
    )
}

fn draw_detail(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    it: &'static ItemDef,
    preview: bool,
    w: &Wallet,
) {
    let q = quote_item(&it.id, w.items, &ITEM_LIST);
    let (ok, missing, title, progress) = buy_look(it, w);
    if preview {
        p.spawn(label(f, "Aperçu · clic pour épingler", 10.0, 700, TEAL));
    }
    p.spawn(Node {
        align_items: AlignItems::Center,
        column_gap: px(12),
        ..default()
    })
    .with_children(|hero| {
        hero.spawn(icon_node(icons, images, it, 64.0));
        hero.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            flex_shrink: 1.0,
            ..default()
        })
        .with_children(|t| {
            t.spawn((
                Text::new(it.name.clone()),
                f.display(17.0, 800),
                TextColor(GOLD_HI),
            ));
            t.spawn(label(f, category_label(it.category), 11.0, 600, MUTED));
        });
    });
    let (look, ink) = buy_colors(ok);
    p.spawn((
        button(
            look,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(10), px(8)),
                border_radius: BorderRadius::all(px(6)),
                row_gap: px(2),
                overflow: Overflow::clip(),
                ..default()
            },
        ),
        Act::Buy(it.id.as_str()),
        BuyButton(it),
    ))
    .with_children(|b| {
        b.spawn((label(f, title, 15.0, 800, ink), BuyTitle(it, missing)));
        b.spawn((
            Node {
                column_gap: px(4),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            if q.price < it.cost {
                row.spawn((
                    label(f, format!("{}", it.cost.round()), 11.0, 600, ink),
                    Strikethrough,
                    StrikethroughColor(ink),
                    BuySub,
                ));
                row.spawn((
                    label(
                        f,
                        format!("{} or · composants déduits", q.price.round()),
                        11.0,
                        600,
                        ink,
                    ),
                    BuySub,
                ));
            } else {
                row.spawn((
                    label(f, format!("{} or", q.price.round()), 11.0, 600, ink),
                    BuySub,
                ));
            }
        });
        // jauge de l'or déjà réuni, quand il en manque
        b.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                bottom: px(0),
                height: px(3),
                width: percent(progress * 100.0),
                ..default()
            },
            BackgroundColor(hexa(0xe8b955, 0.8)),
            BuyProgress(it),
            if missing.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ));
    });
    let stats = stat_entries(&it.stats);
    if !stats.is_empty() {
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        })
        .with_children(|l| {
            for (k, val) in &stats {
                l.spawn(Node {
                    column_gap: px(6),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(label(
                        f,
                        format_stat(k, *val, true),
                        12.0,
                        800,
                        hex(0x9ff0c2),
                    ));
                    r.spawn(label(f, stat_label(k).to_string(), 12.0, 500, TEXT));
                });
            }
        });
    }
    let fx = |p: &mut ChildSpawnerCommands, head: String, desc: &str, color: Color| {
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        })
        .with_children(|b| {
            if !head.is_empty() {
                b.spawn(label(f, head, 12.0, 800, color));
            }
            b.spawn((
                Text::new(desc.to_string()),
                f.text(11.5, 500),
                TextColor(hexa(0xece7da, 0.85)),
                Node {
                    max_width: px(250),
                    ..default()
                },
            ));
        });
    };
    if let Some(pas) = &it.passive {
        fx(
            p,
            format!("Passif · {}", pas.name),
            &pas.description,
            hex(0xffd16b),
        );
    }
    if let Some(act) = &it.active {
        let head = if act.cooldown > 0.0 {
            format!("Actif · {} ({} s)", act.name, act.cooldown)
        } else {
            format!("Actif · {}", act.name)
        };
        fx(p, head, &act.description, hex(0x8cc8ff));
    }
    if it.passive.is_none() && it.active.is_none() && stats.is_empty() {
        fx(p, String::new(), &it.description, TEXT);
    }
    if !it.builds_from.is_empty() {
        p.spawn(label(f, "Recette", 11.0, 800, MUTED));
        recipe_tree(p, f, icons, images, it, w, true);
    }
    let into = builds_into(&it.id, shop_list());
    if !into.is_empty() {
        p.spawn(label(f, "Améliorable en", 11.0, 800, MUTED));
        p.spawn(Node {
            column_gap: px(6),
            row_gap: px(6),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            for i in into {
                row.spawn((
                    button(
                        ButtonLook::new(Color::NONE, hexa(0xffffff, 0.08)),
                        Node {
                            padding: UiRect::all(px(2)),
                            border_radius: BorderRadius::all(px(5)),
                            ..default()
                        },
                    ),
                    Act::Item(i.id.as_str()),
                ))
                .with_child(icon_node(icons, images, i, 34.0));
            }
        });
    }
}

fn draw_inventory(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    ui: &ShopUi,
    w: &Wallet,
    shown: Option<&ItemDef>,
) {
    p.spawn(label(f, "Inventaire", 12.0, 800, MUTED));
    let used = shown.map_or(vec![], |it| {
        quote_item(&it.id, w.items, &ITEM_LIST).consumed_slots
    });
    for i in 0..7 {
        let id = w.items.get(i).and_then(|x| x.as_deref());
        let sel = ui.sell_slot == Some(i);
        let border = if sel {
            DANGER
        } else if used.contains(&i) {
            TEAL
        } else {
            hexa(0xffffff, 0.15)
        };
        p.spawn((
            button(
                ButtonLook::new(hexa(0x000000, 0.4), hexa(0xffffff, 0.08)),
                Node {
                    width: px(44),
                    height: px(44),
                    border: UiRect::all(px(if sel || used.contains(&i) { 2.0 } else { 1.0 })),
                    border_radius: BorderRadius::all(px(5)),
                    margin: UiRect::left(px(if i == 6 { 10.0 } else { 0.0 })),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ),
            BorderColor::all(border),
            Act::Slot(i),
        ))
        .with_children(|s| {
            if let Some(d) = id.and_then(item) {
                s.spawn((
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    ImageNode::new(icons.item(images, d)),
                    Pickable::IGNORE,
                ));
            }
        });
    }
    p.spawn(Node {
        flex_grow: 1.0,
        ..default()
    })
    .with_child(label(
        f,
        if used.is_empty() {
            "Clic droit sur un objet de l'inventaire : vendre (70 %)"
        } else {
            "Les objets surlignés seront utilisés pour la recette"
        },
        10.5,
        500,
        MUTED,
    ));
    let enabled = |on: bool| {
        if on {
            ButtonLook::new(hexa(0xffffff, 0.08), hexa(0xffffff, 0.16))
        } else {
            ButtonLook::new(hexa(0xffffff, 0.03), hexa(0xffffff, 0.03))
        }
    };
    p.spawn((
        button(
            enabled(w.can_shop),
            Node {
                padding: UiRect::axes(px(12), px(7)),
                border_radius: BorderRadius::all(px(6)),
                column_gap: px(6),
                align_items: AlignItems::Center,
                ..default()
            },
        ),
        Act::Undo,
    ))
    .with_children(|b| {
        let c = if w.can_shop { TEXT } else { MUTED };
        b.spawn(glyph(icons, images, "undo", 14.0, c));
        b.spawn(label(f, "Annuler", 13.0, 700, c));
    });
    let sell = sell_item(ui, w);
    let can_sell = w.can_shop && sell.is_some();
    let text = sell.map_or("Vendre".to_string(), |d| {
        format!("Vendre {} · +{}", d.name, (d.cost * 0.7).floor())
    });
    p.spawn((
        button(
            if can_sell {
                ButtonLook::new(hexa(0xb23a3a, 0.9), hexa(0xd04848, 0.95))
            } else {
                enabled(false)
            },
            Node {
                padding: UiRect::axes(px(12), px(7)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
        ),
        Act::Sell,
    ))
    .with_child(label(
        f,
        text,
        13.0,
        700,
        if can_sell { TEXT } else { MUTED },
    ));
}

/// Objet de l'emplacement choisi pour la vente (pas la balise).
fn sell_item(ui: &ShopUi, w: &Wallet) -> Option<&'static ItemDef> {
    ui.sell_slot
        .filter(|s| *s != 6)
        .and_then(|s| w.items.get(s))
        .and_then(|x| x.as_deref())
        .and_then(item)
}

// ---------------------------------------------------------------------------------- mise à jour

/// Recherche, inventaire, or, fontaine : marque ce qui doit être recalculé ou reconstruit.
fn sync(
    state: Res<GameState>,
    input: Res<PlayerInput>,
    mut ui: ResMut<ShopUi>,
    search: Query<&EditableText, With<Search>>,
    mut window: Query<&mut Visibility, With<ShopWindow>>,
) {
    for mut v in &mut window {
        set_shown(&mut v, input.shop_open);
    }
    if !input.shop_open {
        return;
    }
    let ui = &mut *ui;
    if let Some(t) = search.iter().next()
        && t.value() != ui.filters.query.as_str()
    {
        // une nouvelle recherche repart de son premier résultat (`setSel(null)`)
        ui.filters.query = t.value().to_string();
        ui.selected = None;
        ui.refilter();
    }
    let w = wallet(&state);
    if ui.inv.as_slice() != w.items {
        ui.inv = w.items.to_vec();
        ui.refilter();
        ui.dirty = Dirty::ALL;
        ui.sell_slot = ui
            .sell_slot
            .filter(|s| w.items.get(*s).is_some_and(Option::is_some));
    }
    if ui.class != w.class {
        ui.class = w.class;
        ui.refilter();
        ui.dirty.nav = true;
    }
    if ui.can_shop != w.can_shop {
        ui.can_shop = w.can_shop;
        ui.dirty.inventory = true;
    }
    let gold = w.gold.floor() as i64;
    if ui.gold != gold {
        ui.gold = gold;
        // « Abordables » : la liste dépend de l'or
        if ui.filters.affordable {
            ui.refilter();
        }
    }
    if ui.stale {
        ui.stale = false;
        let l = listing(&ui.filters, &w);
        if ui.tab_shown != Some(l.tab) {
            ui.tab_shown = Some(l.tab);
            ui.dirty.nav = true;
        }
        // la grille n'est reconstruite que si elle change vraiment
        if ui.groups != l.groups {
            ui.dirty.grid = true;
        }
        ui.groups = l.groups;
        ui.flat = l.flat;
        ui.dirty.detail = true;
    }
    let shown = ui.shown().map(|(it, preview)| (it.id.as_str(), preview));
    if ui.detail_of.as_ref().map(|(id, p)| (id.as_str(), *p)) != shown {
        ui.detail_of = shown.map(|(id, p)| (id.to_owned(), p));
        ui.dirty.detail = true;
        // les emplacements consommés par la recette suivent la fiche
        ui.dirty.inventory = true;
    }
}

/// Zones reconstruites, chacune seulement quand ce qu'elle montre a changé.
type Zones = (
    Entity,
    Has<Nav>,
    Has<Tools>,
    Has<Grid>,
    Has<Detail>,
    Has<Inventory>,
);

#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn redraw(
    mut commands: Commands,
    state: Res<GameState>,
    input: Res<PlayerInput>,
    fonts: Res<UiFonts>,
    mut ui: ResMut<ShopUi>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    zones: Query<Zones>,
) {
    if !input.shop_open || ui.dirty == Dirty::default() {
        return;
    }
    let dirty = std::mem::take(&mut ui.dirty);
    let w = wallet(&state);
    let shown = ui.shown();
    let f = &*fonts;
    let icons = &mut *icons;
    let images = &mut *images;
    for (e, nav, tools, grid, detail, inv) in &zones {
        let redo = (nav && dirty.nav)
            || (tools && dirty.grid)
            || (grid && dirty.grid)
            || (detail && dirty.detail)
            || (inv && dirty.inventory);
        if !redo {
            continue;
        }
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|p| {
            if nav {
                draw_nav(p, f, icons, images, &ui, &w);
            } else if tools {
                draw_tools(p, f, &ui);
            } else if grid {
                draw_grid(p, f, icons, images, &ui, &w);
            } else if detail {
                if let Some((it, preview)) = shown {
                    draw_detail(p, f, icons, images, it, preview, &w);
                }
            } else {
                draw_inventory(p, f, icons, images, &ui, &w, shown.map(|s| s.0));
            }
        });
    }
    // les nouvelles cartes portent déjà leur état ; la prochaine mise à jour en place repart de zéro
    ui.live = None;
}

/// Couleurs, « manque », achat rapide et bouton d'achat : mis à jour en place quand l'or, la fontaine
/// ou l'objet épinglé changent.
#[allow(clippy::type_complexity)] // requêtes disjointes sur les parties d'une carte
#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn live(
    state: Res<GameState>,
    input: Res<PlayerInput>,
    mut ui: ResMut<ShopUi>,
    mut cards: Query<(&Card, &mut ButtonLook, &mut BorderColor)>,
    mut names: Query<(&CardName, &mut TextColor), (Without<CardPrice>, Without<BuyTitle>)>,
    mut prices: Query<(&CardPrice, &mut TextColor), (Without<CardName>, Without<BuyTitle>)>,
    mut missing: Query<(&mut CardMissing, &mut Text), Without<BuyTitle>>,
    mut quick: Query<(&CardQuick, &mut Visibility), Without<BuyProgress>>,
    mut buy: Query<(&BuyButton, &mut ButtonLook), Without<Card>>,
    mut titles: Query<
        (&mut BuyTitle, &mut Text, &mut TextColor),
        (Without<CardName>, Without<CardPrice>, Without<BuySub>),
    >,
    mut subs: Query<
        (&mut TextColor, Option<&mut StrikethroughColor>),
        (With<BuySub>, Without<CardName>, Without<CardPrice>),
    >,
    mut progress: Query<(&BuyProgress, &mut Node, &mut Visibility), Without<CardQuick>>,
    mut wallet_text: Query<&mut Text, (With<WalletText>, Without<CardMissing>, Without<BuyTitle>)>,
    mut closed: Query<
        &mut Visibility,
        (With<ClosedHint>, Without<CardQuick>, Without<BuyProgress>),
    >,
) {
    if !input.shop_open {
        return;
    }
    let w = wallet(&state);
    let pinned = ui.pinned().map(|p| p.id.as_str());
    let key = (w.gold.floor() as i64, w.can_shop, pinned);
    if ui.live == Some(key) {
        return;
    }
    ui.live = Some(key);
    for mut t in &mut wallet_text {
        set_text(&mut t, &key.0.to_string());
    }
    for mut v in &mut closed {
        set_shown(&mut v, !w.can_shop);
    }
    for (Card(it), mut look, mut border) in &mut cards {
        let l = card_look(it, &w, pinned == Some(it.id.as_str()));
        set_if(&mut look, |b| &mut b.normal, l.normal);
        if border.top != l.border {
            *border = BorderColor::all(l.border);
        }
    }
    for (CardName(it), mut c) in &mut names {
        let ok = verdict(it, &w) == ShopVerdict::Ok;
        set_if(&mut c, |c| &mut c.0, if ok { TEXT } else { MUTED });
    }
    for (CardPrice(it), mut c) in &mut prices {
        let short = price(it, w.items) > w.gold;
        set_if(
            &mut c,
            |c| &mut c.0,
            if short { hex(0xc9a35a) } else { GOLD_HI },
        );
    }
    for (mut m, mut t) in &mut missing {
        let now = card_look(m.0, &w, false).missing;
        if m.1 != now {
            m.1 = now;
            set_text(
                &mut t,
                &now.map_or(String::new(), |n| format!("manque {n}")),
            );
        }
    }
    for (CardQuick(it), mut v) in &mut quick {
        set_shown(&mut v, verdict(it, &w) == ShopVerdict::Ok);
    }
    let mut ok_any = None;
    for (BuyButton(it), mut look) in &mut buy {
        let (ok, ..) = buy_look(it, &w);
        let (want, _) = buy_colors(ok);
        set_if(&mut look, |b| &mut b.normal, want.normal);
        set_if(&mut look, |b| &mut b.hover, want.hover);
        ok_any = Some(ok);
    }
    let ink = ok_any.map(|ok| buy_colors(ok).1);
    for (mut t, mut text, mut c) in &mut titles {
        let (_, missing, title, _) = buy_look(t.0, &w);
        if let Some(ink) = ink {
            set_if(&mut c, |c| &mut c.0, ink);
        }
        if t.1 != missing || text.0 != title {
            t.1 = missing;
            set_text(&mut text, &title);
        }
    }
    if let Some(ink) = ink {
        for (mut c, strike) in &mut subs {
            set_if(&mut c, |c| &mut c.0, ink);
            if let Some(mut s) = strike {
                set_if(&mut s, |s| &mut s.0, ink);
            }
        }
    }
    for (BuyProgress(it), mut node, mut v) in &mut progress {
        let (_, missing, _, p) = buy_look(it, &w);
        set_shown(&mut v, missing.is_some());
        set_if(&mut node, |n| &mut n.width, percent(p * 100.0));
    }
}

/// Fait défiler la grille jusqu'à la carte choisie au clavier, ou en haut après un changement d'onglet.
fn scroll(
    mut ui: ResMut<ShopUi>,
    cards: Query<(&Card, &ComputedNode, &UiGlobalTransform)>,
    mut grid: Query<(&ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<Grid>>,
) {
    let Ok((gn, gt, mut pos)) = grid.single_mut() else {
        return;
    };
    if ui.scroll_top {
        ui.scroll_top = false;
        pos.0.y = 0.0;
    }
    let Some(id) = ui.scroll_to else { return };
    let Some((_, cn, ct)) = cards.iter().find(|(c, ..)| c.0.id == id) else {
        return;
    };
    ui.scroll_to = None;
    let k = gn.inverse_scale_factor();
    let top = (ct.translation.y - cn.size().y / 2.0 - (gt.translation.y - gn.size().y / 2.0)) * k;
    let (h, view) = (cn.size().y * k, gn.size().y * k);
    if top < GRID_PAD {
        pos.0.y = (pos.0.y + top - GRID_PAD).max(0.0);
    } else if top + h > view - GRID_PAD {
        pos.0.y += top + h - (view - GRID_PAD);
    }
}

fn buy(conn: &Connection, state: &GameState, id: &str) {
    let w = wallet(state);
    if let Some(it) = item(id)
        && verdict(it, &w) == ShopVerdict::Ok
    {
        conn.send(ClientMsg::Buy {
            item_id: id.to_owned(),
        });
    }
}

#[allow(clippy::too_many_arguments)] // un observateur Bevy : ses paramètres sont ses accès au monde
fn on_click(
    ev: On<PointerClick>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    state: Res<GameState>,
    mut ui: ResMut<ShopUi>,
    mut input: ResMut<PlayerInput>,
    mut search: Query<&mut EditableText, With<Search>>,
    mut focus: ResMut<InputFocus>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    let secondary = ev.button == PointerButton::Secondary;
    if ev.button != PointerButton::Primary && !secondary {
        return;
    }
    let w = wallet(&state);
    let ui = &mut *ui;
    let mut clear_search = || {
        for mut t in &mut search {
            t.clear();
        }
    };
    match act {
        Act::Tab(t) if !secondary => {
            ui.filters.tab = *t;
            ui.filters.query.clear();
            clear_search();
            ui.selected = None;
            ui.scroll_top = true;
            ui.refilter();
        }
        Act::Item(id) => {
            // la grille n'est pas reconstruite par la sélection : le second clic tombe sur la même carte
            if secondary || ev.count >= 2 {
                buy(&conn, &state, id);
            }
            ui.selected = Some((*id).to_owned());
            ui.hover = None;
        }
        Act::Buy(id) if !secondary => buy(&conn, &state, id),
        Act::Stat(i) if !secondary => {
            ui.filters.stats ^= 1 << i;
            ui.refilter();
        }
        Act::Affordable if !secondary => {
            ui.filters.affordable = !ui.filters.affordable;
            ui.refilter();
        }
        Act::Sort if !secondary => {
            ui.filters.sort = next_sort(ui.filters.sort);
            ui.refilter();
        }
        Act::ResetFilters if !secondary => {
            ui.filters.stats = 0;
            ui.filters.affordable = false;
            ui.filters.query.clear();
            clear_search();
            ui.selected = None;
            ui.refilter();
        }
        Act::Slot(i) => {
            let id = w.items.get(*i).and_then(|x| x.as_deref());
            if secondary {
                if id.is_some() && w.can_shop && *i != 6 {
                    conn.send(ClientMsg::Sell { slot: *i as i32 });
                    ui.sell_slot = None;
                }
            } else {
                ui.sell_slot = if ui.sell_slot == Some(*i) {
                    None
                } else {
                    Some(*i)
                };
                ui.dirty.inventory = true;
                if let Some(id) = id.filter(|id| item(id).is_some_and(|d| d.shop != Some(false))) {
                    ui.selected = Some(id.to_owned());
                }
            }
        }
        Act::Sell if !secondary => {
            if w.can_shop
                && let Some(s) = ui
                    .sell_slot
                    .filter(|s| *s != 6 && w.items.get(*s).is_some_and(Option::is_some))
            {
                conn.send(ClientMsg::Sell { slot: s as i32 });
                ui.sell_slot = None;
            }
        }
        Act::Undo if !secondary => {
            if w.can_shop {
                conn.send(ClientMsg::UndoShop);
            }
        }
        Act::Close if !secondary => input.shop_open = false,
        _ => {}
    }
    // un clic hors de la recherche lui rend les flèches et Entrée
    if !matches!(act, Act::Tab(_) | Act::ResetFilters)
        && search.iter().next().is_some()
        && focus.get().is_some()
    {
        focus.clear();
    }
}

/// Survol d'une carte de la grille : aperçu dans la fiche.
fn on_over(ev: On<PointerOver>, cards: Query<&Card>, mut ui: ResMut<ShopUi>) {
    if let Ok(Card(it)) = cards.get(ev.entity)
        && ui.hover.as_deref() != Some(it.id.as_str())
    {
        ui.hover = Some(it.id.clone());
    }
}

fn on_out(ev: On<PointerOut>, cards: Query<&Card>, mut ui: ResMut<ShopUi>) {
    if let Ok(Card(it)) = cards.get(ev.entity)
        && ui.hover.as_deref() == Some(it.id.as_str())
    {
        ui.hover = None;
    }
}

const ARROWS: [KeyCode; 4] = [
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
];

/// Flèches : naviguer dans la grille ; Entrée : acheter l'objet épinglé. Tant que la boutique est
/// ouverte, les flèches lui appartiennent : la caméra ne défile pas.
fn keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    input: Res<PlayerInput>,
    mut ui: ResMut<ShopUi>,
    conn: Res<Connection>,
    state: Res<GameState>,
    grid: Query<&ComputedNode, With<Grid>>,
) {
    if !input.shop_open {
        return;
    }
    let cols = grid.iter().next().map_or(1, |g| {
        let w = g.size().x * g.inverse_scale_factor() - 2.0 * GRID_PAD;
        ((w + GAP) / (CARD_W + GAP)).floor().max(1.0) as i32
    });
    for (k, step) in ARROWS.into_iter().zip([-1, 1, -cols, cols]) {
        if keys.just_pressed(k) {
            ui.navigate(step);
        }
        keys.reset(k);
    }
    if keys.just_pressed(KeyCode::Enter)
        && let Some(it) = ui.pinned()
    {
        buy(&conn, &state, &it.id);
    }
}

/// À l'ouverture, la recherche prend le focus (le web : `searchRef.focus()`).
fn focus_search(
    input: Res<PlayerInput>,
    mut was_open: Local<bool>,
    search: Query<Entity, With<Search>>,
    mut focus: ResMut<InputFocus>,
) {
    if input.shop_open && !*was_open {
        if let Some(e) = search.iter().next() {
            focus.set(e, bevy::input_focus::FocusCause::Navigated);
        }
    } else if !input.shop_open
        && *was_open
        && search.iter().next().is_some_and(|e| focus.get() == Some(e))
    {
        focus.clear();
    }
    *was_open = input.shop_open;
}

/// Nouvelle partie : on garde les réglages de la liste (`prefs`), le reste repart de zéro.
fn reset(mut ui: ResMut<ShopUi>) {
    let filters = std::mem::take(&mut ui.filters);
    *ui = ShopUi {
        filters,
        ..ShopUi::default()
    };
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopUi>()
            .add_systems(OnEnter(Screen::Game), (reset, spawn).chain())
            .add_systems(
                Update,
                (focus_search, keys, sync, redraw, live, scroll)
                    .chain()
                    .before(crate::render::camera::drive_camera)
                    .run_if(in_state(Screen::Game)),
            )
            .add_observer(on_click)
            .add_observer(on_over)
            .add_observer(on_out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet_with(items: &[Option<String>], gold: f64) -> Wallet<'_> {
        Wallet {
            gold,
            can_shop: true,
            items,
            class: Some(ChampionClass::Fighter),
        }
    }

    #[test]
    fn conseilles_par_classe_et_bornes() {
        let r = recommended(Some(ChampionClass::Mage));
        assert!(!r.is_empty() && r.len() <= 20);
        assert!(r.iter().all(|i| i.category != ItemCategory::Trinket));
        // triés par catégorie puis prix
        let ranks: Vec<usize> = r.iter().map(|i| category_rank(i.category)).collect();
        assert!(ranks.windows(2).all(|w| w[0] <= w[1]));
        assert!(recommended(None).is_empty());
    }

    #[test]
    fn la_boutique_exclut_les_objets_d_objectif() {
        assert!(shop_list().iter().all(|i| i.shop != Some(false)));
        let empty = vec![None; 7];
        let w = wallet_with(&empty, 500.0);
        assert!(tab_count(Tab::All, &w) > 10);
        assert_eq!(
            tab_count(Tab::Up, &w),
            0,
            "rien à améliorer d'un inventaire vide"
        );
    }

    #[test]
    fn ameliorations_vides_retombent_sur_conseilles() {
        let empty = vec![None; 7];
        let w = wallet_with(&empty, 500.0);
        let l = listing(
            &Filters {
                tab: Tab::Up,
                ..Filters::default()
            },
            &w,
        );
        assert_eq!(l.tab, Tab::Reco);
        assert!(!l.flat.is_empty());
        // groupées par catégorie, dans l'ordre des catégories
        let ranks: Vec<usize> = l
            .groups
            .iter()
            .map(|g| category_rank(g.items[0].category))
            .collect();
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn filtres_abordables_et_statistiques() {
        let empty = vec![None; 7];
        let w = wallet_with(&empty, 450.0);
        let all = Filters {
            tab: Tab::All,
            ..Filters::default()
        };
        let cheap = listing(
            &Filters {
                affordable: true,
                ..all.clone()
            },
            &w,
        );
        assert!(!cheap.flat.is_empty());
        assert!(cheap.flat.iter().all(|i| i.cost <= 450.0));
        assert!(cheap.flat.len() < listing(&all, &w).flat.len());
        // armure ET résistance magique
        let both = listing(
            &Filters {
                stats: (1 << 10) | (1 << 11),
                ..all.clone()
            },
            &w,
        );
        assert!(!both.flat.is_empty());
        for it in &both.flat {
            let s = stat_entries(&it.stats);
            assert!(s.iter().any(|(k, _)| *k == "armor"), "{}", it.id);
            assert!(s.iter().any(|(k, _)| *k == "mr"), "{}", it.id);
        }
    }

    #[test]
    fn tri_par_prix_sans_groupes() {
        let empty = vec![None; 7];
        let w = wallet_with(&empty, 0.0);
        let l = listing(
            &Filters {
                tab: Tab::All,
                sort: ShopSort::PriceDesc,
                ..Filters::default()
            },
            &w,
        );
        assert_eq!(l.groups.len(), 1);
        assert!(l.groups[0].label.is_empty());
        assert!(l.flat.windows(2).all(|p| p[0].cost >= p[1].cost));
    }

    #[test]
    fn une_recherche_vide_la_selection_et_les_fleches_naviguent() {
        let empty = vec![None; 7];
        let w = wallet_with(&empty, 0.0);
        let mut ui = ShopUi::default();
        let l = listing(
            &Filters {
                tab: Tab::All,
                ..Filters::default()
            },
            &w,
        );
        ui.flat = l.flat;
        assert_eq!(ui.pinned().map(|i| &i.id), ui.flat.first().map(|i| &i.id));
        ui.navigate(1);
        assert_eq!(ui.selected.as_deref(), Some(ui.flat[1].id.as_str()));
        assert_eq!(ui.scroll_to, Some(ui.flat[1].id.as_str()));
        ui.navigate(-5);
        assert_eq!(ui.selected.as_deref(), Some(ui.flat[0].id.as_str()));
        ui.navigate(10_000);
        assert_eq!(
            ui.selected.as_deref(),
            ui.flat.last().map(|i| i.id.as_str())
        );
        // un survol montre l'aperçu sans changer l'épinglé
        ui.hover = Some(ui.flat[0].id.clone());
        let (shown, preview) = ui.shown().unwrap();
        assert_eq!(shown.id, ui.flat[0].id);
        assert!(preview);
    }
}
