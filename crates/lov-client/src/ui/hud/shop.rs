//! Boutique (P) : onglets (conseillés selon la classe, améliorations de l'inventaire, tous, catégories),
//! recherche tolérante aux accents, grille d'objets (prix avec composants déduits, manque d'or,
//! possédés), fiche détaillée (statistiques, passif, actif, recette, améliorations), inventaire pour
//! vendre et annulation du dernier achat.
//!
//! Portage de `packages/client/src/ui/hud/Shop.tsx`. Clic : détails ; clic droit ou double-clic :
//! acheter ; achats à la fontaine (ou mort) seulement, consultation ailleurs.

use bevy::input_focus::InputFocus;
use bevy::picking::events::PointerClick;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::{ScrollArea, TextInput};
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::{ITEM_LIST, item};
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{ChampionClass, ItemCategory, ItemDef};

use super::{HUD_Z, label};
use crate::input::PlayerInput;
use crate::net::{Connection, GameState};
use crate::ui::icons::draw::{IconCache, category_colors};
use crate::ui::logic::format::{format_stat, stat_entries, stat_label};
use crate::ui::logic::shop::{
    CATEGORY_ORDER, ShopVerdict, builds_into, can_buy, category_label, filter_items, quote_item,
    upgrades_for,
};
use crate::ui::theme::{DANGER, GOLD, GOLD_HI, MUTED, TEAL, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, ButtonLook, Screen, UiFonts, button};

/// Emplacements de l'inventaire principal (la balise est à part).
const MAIN_SLOTS: usize = 6;

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

/// Objets achetables (`shop: false` = donné par un objectif).
fn shop_list() -> Vec<&'static ItemDef> {
    ITEM_LIST.iter().filter(|i| i.shop != Some(false)).collect()
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
        .into_iter()
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

/// État de la boutique.
#[derive(Resource)]
pub struct ShopUi {
    tab: Tab,
    selected: Option<String>,
    sell_slot: Option<usize>,
    query: String,
    /// Clé de ce qui est affiché (onglet, recherche, inventaire, or, sélection) : redessin si elle change.
    shown: String,
    built: bool,
}

impl Default for ShopUi {
    fn default() -> Self {
        Self {
            tab: Tab::Reco,
            selected: None,
            sell_slot: None,
            query: String::new(),
            shown: String::new(),
            built: false,
        }
    }
}

/// Ce que l'interface lit du joueur pour la boutique.
struct Wallet {
    gold: f64,
    can_shop: bool,
    items: Vec<Option<String>>,
    class: Option<ChampionClass>,
}

fn wallet(state: &GameState) -> Wallet {
    let st = state.self_state.as_ref();
    let e = state.my_entity.and_then(|id| state.entities().get(&id));
    Wallet {
        gold: st.map_or(0.0, |s| s.gold),
        can_shop: st.is_some_and(|s| s.can_shop) || e.is_some_and(|e| e.dead == Some(true)),
        items: e
            .and_then(|e| e.champ.as_ref())
            .map(|c| c.items.clone())
            .unwrap_or_default(),
        class: e
            .and_then(|e| e.subtype.as_deref())
            .and_then(get_champion)
            .map(|c| c.class),
    }
}

fn verdict(it: &ItemDef, w: &Wallet) -> ShopVerdict {
    can_buy(it, &w.items, w.gold, w.can_shop, &ITEM_LIST, MAIN_SLOTS)
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

// ---------------------------------------------------------------------------------- composants

/// Fenêtre de la boutique.
#[derive(Component)]
struct ShopWindow;

#[derive(Component)]
struct Nav;

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

/// Action d'un élément cliquable de la boutique.
#[derive(Component, Clone, Debug, PartialEq)]
enum Act {
    Tab(Tab),
    Item(String),
    Buy(String),
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

fn coin(icons: &mut IconCache, images: &mut Assets<Image>, size: f32) -> impl Bundle {
    (
        Node {
            width: px(size),
            height: px(size),
            ..default()
        },
        ImageNode {
            image: icons.glyph(images, "gold"),
            color: GOLD_HI,
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn spawn(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut ui: ResMut<ShopUi>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let f = &*fonts;
    ui.built = false;
    ui.shown.clear();
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
                    width: px(980),
                    height: px(640),
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
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(6)),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(hexa(0xffffff, 0.15)),
                        BackgroundColor(hexa(0x000000, 0.35)),
                    ))
                    .with_children(|s| {
                        let mut text = EditableText::new("");
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
                        label(
                            f,
                            "Hors de la fontaine — consultation seule",
                            12.0,
                            600,
                            DANGER,
                        ),
                        ClosedHint,
                    ));
                    h.spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(6),
                        ..default()
                    })
                    .with_child((label(f, "0", 18.0, 800, GOLD_HI), WalletText));
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
                    .with_child((
                        Node {
                            width: px(14),
                            height: px(14),
                            ..default()
                        },
                        ImageNode {
                            image: icons.glyph(&mut images, "close"),
                            color: TEXT,
                            ..default()
                        },
                        Pickable::IGNORE,
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
                    body.spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            align_content: AlignContent::FlexStart,
                            column_gap: px(8),
                            row_gap: px(8),
                            padding: UiRect::all(px(10)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollArea,
                        Grid,
                    ));
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

fn items_for(tab: Tab, query: &str, w: &Wallet) -> Vec<&'static ItemDef> {
    let list = shop_list();
    let owned: Vec<ItemDef> = list.iter().map(|i| (*i).clone()).collect();
    if !query.trim().is_empty() {
        let found: Vec<String> = filter_items(&owned, query, None)
            .iter()
            .map(|i| i.id.clone())
            .collect();
        return found.iter().filter_map(|id| item(id)).collect();
    }
    match tab {
        Tab::Reco => {
            let mut v: Vec<&ItemDef> = upgrades_for(&w.items, &owned, &ITEM_LIST)
                .iter()
                .take(6)
                .filter_map(|i| item(&i.id))
                .collect();
            for r in recommended(w.class) {
                if !v.iter().any(|x| x.id == r.id) {
                    v.push(r);
                }
            }
            v
        }
        Tab::Up => upgrades_for(&w.items, &owned, &ITEM_LIST)
            .iter()
            .filter_map(|i| item(&i.id))
            .collect(),
        Tab::All => filter_items(&owned, "", None)
            .iter()
            .filter_map(|i| item(&i.id))
            .collect(),
        Tab::Cat(c) => filter_items(&owned, "", Some(c))
            .iter()
            .filter_map(|i| item(&i.id))
            .collect(),
    }
}

fn tab_count(t: Tab, w: &Wallet) -> usize {
    items_for(t, "", w).len()
}

fn draw_nav(p: &mut ChildSpawnerCommands, f: &UiFonts, ui: &ShopUi, w: &Wallet) {
    let tabs = [Tab::Reco, Tab::Up, Tab::All]
        .into_iter()
        .chain(CATEGORY_ORDER.iter().map(|c| Tab::Cat(*c)));
    for t in tabs {
        let n = tab_count(t, w);
        if n == 0 {
            continue;
        }
        let on = ui.query.trim().is_empty() && ui.tab == t;
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
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            Act::Tab(t),
        ))
        .with_children(|b| {
            b.spawn(label(
                f,
                tab_label(t),
                13.0,
                if on { 800 } else { 600 },
                if on { GOLD_HI } else { TEXT },
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
        "Clic droit / double-clic : acheter",
        "Entrée : acheter la sélection",
    ] {
        p.spawn(label(f, line, 10.0, 500, MUTED));
    }
}

fn draw_grid(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    list: &[&'static ItemDef],
    ui: &ShopUi,
    w: &Wallet,
) {
    if list.is_empty() {
        p.spawn(label(f, "Aucun objet ne correspond.", 14.0, 500, MUTED));
        return;
    }
    let mut last_cat = None;
    let grouped = ui.query.trim().is_empty() && matches!(ui.tab, Tab::All);
    for it in list {
        if grouped && last_cat != Some(it.category) {
            last_cat = Some(it.category);
            p.spawn((
                Node {
                    width: percent(100),
                    margin: UiRect::top(px(4)),
                    ..default()
                },
                children![label(f, category_label(it.category), 12.0, 800, GOLD)],
            ));
        }
        let q = quote_item(&it.id, &w.items, &ITEM_LIST);
        let v = verdict(it, w);
        let owned = w
            .items
            .iter()
            .filter(|x| x.as_deref() == Some(it.id.as_str()))
            .count();
        let sel = ui.selected.as_deref() == Some(it.id.as_str());
        let (c1, _) = category_colors(it.category);
        let missing = (q.price - w.gold).ceil();
        p.spawn((
            button(
                ButtonLook::new(
                    if sel {
                        hexa(0xd9b25f, 0.18)
                    } else {
                        hexa(0xffffff, 0.04)
                    },
                    hexa(0xffffff, 0.1),
                ),
                Node {
                    width: px(158),
                    padding: UiRect::all(px(6)),
                    column_gap: px(8),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            BorderColor::all(if sel {
                GOLD_HI
            } else {
                hex(c1).with_alpha(0.45)
            }),
            Act::Item(it.id.clone()),
        ))
        .with_children(|card| {
            card.spawn(icon_node(icons, images, it, 44.0));
            card.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    flex_shrink: 1.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|body| {
                body.spawn(label(
                    f,
                    it.name.clone(),
                    12.0,
                    700,
                    if v == ShopVerdict::Ok { TEXT } else { MUTED },
                ));
                body.spawn((
                    Node {
                        column_gap: px(4),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_children(|price| {
                    price.spawn(coin(icons, images, 11.0));
                    price.spawn(label(
                        f,
                        format!("{}", q.price.round()),
                        12.0,
                        800,
                        if missing > 0.0 {
                            hex(0xc9a35a)
                        } else {
                            GOLD_HI
                        },
                    ));
                    if owned > 0 {
                        price.spawn(label(f, format!("×{owned}"), 11.0, 800, TEAL));
                    }
                });
                if missing > 0.0 {
                    body.spawn(label(
                        f,
                        format!("manque {missing}"),
                        10.0,
                        600,
                        hexa(0xff8a7a, 0.85),
                    ));
                }
            });
        });
    }
}

fn recipe_row(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    ids: &[String],
    w: &Wallet,
) {
    p.spawn(Node {
        column_gap: px(6),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    })
    .with_children(|row| {
        for id in ids {
            let Some(c) = item(id) else { continue };
            let owned = w.items.iter().any(|x| x.as_deref() == Some(id.as_str()));
            row.spawn((
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
                Act::Item(id.clone()),
            ))
            .with_children(|n| {
                n.spawn(icon_node(icons, images, c, 34.0));
                n.spawn(label(f, format!("{}", c.cost.round()), 10.0, 700, GOLD_HI));
            });
        }
    });
}

fn draw_detail(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    it: &'static ItemDef,
    w: &Wallet,
) {
    let q = quote_item(&it.id, &w.items, &ITEM_LIST);
    let v = verdict(it, w);
    let missing = (q.price - w.gold).ceil().max(0.0);
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
    let ok = v == ShopVerdict::Ok;
    let title = match v {
        ShopVerdict::Gold => format!("Il manque {missing} or"),
        v => verdict_text(v).to_string(),
    };
    let price = if q.price < it.cost {
        format!(
            "{} or · composants déduits ({})",
            q.price.round(),
            it.cost.round()
        )
    } else {
        format!("{} or", q.price.round())
    };
    p.spawn((
        button(
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
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(10), px(8)),
                border_radius: BorderRadius::all(px(6)),
                row_gap: px(2),
                ..default()
            },
        ),
        Act::Buy(it.id.clone()),
    ))
    .with_children(|b| {
        b.spawn(label(
            f,
            title,
            15.0,
            800,
            if ok { hex(0x1a1206) } else { MUTED },
        ));
        b.spawn(label(
            f,
            price,
            11.0,
            600,
            if ok { hex(0x2a1d08) } else { MUTED },
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
            for (k, val) in stats {
                l.spawn(Node {
                    column_gap: px(6),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(label(
                        f,
                        format_stat(k, val, true),
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
            b.spawn(label(f, head, 12.0, 800, color));
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
    if it.passive.is_none() && it.active.is_none() && stat_entries(&it.stats).is_empty() {
        fx(p, String::new(), &it.description, TEXT);
    }
    if !it.builds_from.is_empty() {
        p.spawn(label(f, "Recette", 11.0, 800, MUTED));
        recipe_row(p, f, icons, images, &it.builds_from, w);
    }
    let into: Vec<String> = {
        let list: Vec<ItemDef> = shop_list().into_iter().cloned().collect();
        builds_into(&it.id, &list)
            .iter()
            .map(|i| i.id.clone())
            .collect()
    };
    if !into.is_empty() {
        p.spawn(label(f, "Améliorable en", 11.0, 800, MUTED));
        recipe_row(p, f, icons, images, &into, w);
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
        quote_item(&it.id, &w.items, &ITEM_LIST).consumed_slots
    });
    for i in 0..7 {
        let id = w.items.get(i).cloned().flatten();
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
            if let Some(d) = id.as_deref().and_then(item) {
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
    });
    p.spawn((
        button(
            ButtonLook::new(hexa(0xffffff, 0.08), hexa(0xffffff, 0.16)),
            Node {
                padding: UiRect::axes(px(12), px(7)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
        ),
        Act::Undo,
    ))
    .with_child(label(f, "Annuler", 13.0, 700, TEXT));
    let sell = ui
        .sell_slot
        .filter(|s| *s != 6)
        .and_then(|s| w.items.get(s).cloned().flatten())
        .and_then(|id| item(&id));
    let text = sell.map_or("Vendre".to_string(), |d| {
        format!("Vendre {} · +{}", d.name, (d.cost * 0.7).floor())
    });
    p.spawn((
        button(
            ButtonLook::new(
                if sell.is_some() {
                    hexa(0xb23a3a, 0.9)
                } else {
                    hexa(0xffffff, 0.05)
                },
                hexa(0xd04848, 0.95),
            ),
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
        if sell.is_some() { TEXT } else { MUTED },
    ));
}

/// Zones redessinées de la fenêtre : entité et rôle.
type Zones = (Entity, Has<Nav>, Has<Grid>, Has<Detail>, Has<Inventory>);

/// Redessine les zones dont le contenu a changé.
#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
fn redraw(
    mut commands: Commands,
    state: Res<GameState>,
    input: Res<PlayerInput>,
    fonts: Res<UiFonts>,
    mut ui: ResMut<ShopUi>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    mut window: Query<&mut Visibility, With<ShopWindow>>,
    zones: Query<Zones>,
    search: Query<&EditableText, With<Search>>,
    mut wallet_text: Query<&mut Text, With<WalletText>>,
    mut closed: Query<&mut Visibility, (With<ClosedHint>, Without<ShopWindow>)>,
) {
    for mut v in &mut window {
        v.set_if_neq(if input.shop_open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    if !input.shop_open {
        return;
    }
    if let Some(t) = search.iter().next() {
        let q = t.value().to_string();
        if q != ui.query {
            ui.query = q;
        }
    }
    let w = wallet(&state);
    for mut t in &mut wallet_text {
        let s = format!("{}", w.gold.floor());
        if t.0 != s {
            t.0 = s;
        }
    }
    for mut v in &mut closed {
        v.set_if_neq(if w.can_shop {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let key = format!(
        "{:?}|{}|{:?}|{}|{:?}|{:?}|{}",
        ui.tab,
        ui.query,
        w.items,
        w.gold.floor(),
        ui.selected,
        ui.sell_slot,
        w.can_shop
    );
    if ui.built && key == ui.shown {
        return;
    }
    ui.built = true;
    ui.shown = key;
    let list = items_for(ui.tab, &ui.query, &w);
    if ui.selected.as_deref().and_then(item).is_none() {
        ui.selected = list.first().map(|i| i.id.clone());
    }
    let shown = ui.selected.as_deref().and_then(item);
    let f = &*fonts;
    let icons = &mut *icons;
    let images = &mut *images;
    for (e, nav, grid, detail, inv) in &zones {
        if !(nav || grid || detail || inv) {
            continue;
        }
        let mut ec = commands.entity(e);
        ec.despawn_related::<Children>();
        ec.with_children(|p| {
            if nav {
                draw_nav(p, f, &ui, &w);
            } else if grid {
                draw_grid(p, f, icons, images, &list, &ui, &w);
            } else if detail {
                if let Some(it) = shown {
                    draw_detail(p, f, icons, images, it, &w);
                }
            } else {
                draw_inventory(p, f, icons, images, &ui, &w, shown);
            }
        });
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

fn on_click(
    ev: On<PointerClick>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    state: Res<GameState>,
    mut ui: ResMut<ShopUi>,
    mut input: ResMut<PlayerInput>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    let secondary = ev.button == PointerButton::Secondary;
    if ev.button != PointerButton::Primary && !secondary {
        return;
    }
    let w = wallet(&state);
    match act {
        Act::Tab(t) if !secondary => {
            ui.tab = *t;
            ui.selected = None;
        }
        Act::Item(id) => {
            if secondary || ev.count >= 2 {
                buy(&conn, &state, id);
            }
            ui.selected = Some(id.clone());
        }
        Act::Buy(id) if !secondary => buy(&conn, &state, id),
        Act::Slot(i) => {
            let id = w.items.get(*i).cloned().flatten();
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
                if let Some(id) = id.filter(|id| item(id).is_some_and(|d| d.shop != Some(false))) {
                    ui.selected = Some(id);
                }
            }
        }
        Act::Sell if !secondary => {
            if let Some(s) = ui
                .sell_slot
                .filter(|s| *s != 6 && w.items.get(*s).cloned().flatten().is_some())
            {
                conn.send(ClientMsg::Sell { slot: s as i32 });
                ui.sell_slot = None;
            }
        }
        Act::Undo if !secondary => conn.send(ClientMsg::UndoShop),
        Act::Close if !secondary => input.shop_open = false,
        _ => {}
    }
}

/// Entrée : acheter la sélection (hors saisie de recherche… qui l'achète aussi, comme le web).
fn enter_buys(
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<PlayerInput>,
    ui: Res<ShopUi>,
    conn: Res<Connection>,
    state: Res<GameState>,
) {
    if input.shop_open
        && keys.just_pressed(KeyCode::Enter)
        && let Some(id) = &ui.selected
    {
        buy(&conn, &state, id);
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

fn reset(mut ui: ResMut<ShopUi>) {
    *ui = ShopUi::default();
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopUi>()
            .add_systems(OnEnter(Screen::Game), (reset, spawn).chain())
            .add_systems(
                Update,
                (focus_search, redraw, enter_buys).run_if(in_state(Screen::Game)),
            )
            .add_observer(on_click);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let w = Wallet {
            gold: 500.0,
            can_shop: true,
            items: vec![None; 7],
            class: Some(ChampionClass::Fighter),
        };
        assert!(tab_count(Tab::All, &w) > 10);
        assert_eq!(
            tab_count(Tab::Up, &w),
            0,
            "rien à améliorer d'un inventaire vide"
        );
    }
}
