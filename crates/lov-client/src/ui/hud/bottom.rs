//! Barre du bas : statistiques, portrait (niveau, expérience, points à dépenser), sorts et sorts
//! d'invocateur (recharges, rangs, coût, amélioration), barres de PV et de ressource, inventaire (objets,
//! balise, or), effets actifs et rappel.
//!
//! Portage de `packages/client/src/ui/hud/BottomBar.tsx`.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lov_shared::constants::RECALL_SECONDS;
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::item;
use lov_shared::data::summoners::summoner;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{ChampionDef, ItemCategory, ItemDef, Resource};
use lov_shared::types::{AbilitySlot, EntitySnapshot, SelfState, Stats, SummonerSlot};

use super::{HUD_Z, hud_root, label, set_bg, set_shown, set_text, set_val, str_key};
use crate::input::{PlayerInput, Targeting};
use crate::net::{Connection, GameState};
use crate::ui::icons::draw::IconCache;
use crate::ui::icons::paths::ITEM_GLYPH;
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::format::{buff_initials, format_cooldown, format_int, is_debuff};
use crate::ui::logic::game::{can_rank_up, cooldown_fraction, hasted_cooldown, rank_value};
use crate::ui::theme::{GOLD, GOLD_HI, GOLD_LO, MUTED, PANEL, TEAL, TEXT, hex, hexa};
use crate::ui::{BlocksWorld, ButtonLook, Screen, UiFonts, button};

const SLOTS: [AbilitySlot; 4] = [
    AbilitySlot::Q,
    AbilitySlot::W,
    AbilitySlot::E,
    AbilitySlot::R,
];
const SUMS: [SummonerSlot; 2] = [SummonerSlot::D, SummonerSlot::F];
const ITEM_KEYS: [&str; 6] = ["1", "2", "3", "5", "6", "7"];
const MAX_BUFFS: usize = 12;
const MAX_PIPS: usize = 6;

const ICON: f32 = 56.0;
const SUM_ICON: f32 = 42.0;
const PASSIVE: f32 = 38.0;
const ITEM: f32 = 40.0;
const PORTRAIT: f32 = 92.0;

const MANA: Color = hex(0x4f8dff);
const ENERGY: Color = hex(0xf2d14c);
const HP: Color = hex(0x3ecf6a);
const SHIELD: Color = hex(0xdfe6ef);
const XP: Color = hex(0xb072ff);

/// Emplacement dont on affiche une recharge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Passive,
    Ability(usize),
    Summoner(usize),
    Item(usize),
    Trinket,
}

/// Texte dynamique.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(Src)]
enum T {
    Hp,
    HpRegen,
    Res,
    ResRegen,
    Level,
    Name,
    Points,
    Stat(usize),
    Cd(Slot),
    Cost(usize),
    Charges(Slot),
    Gold,
    Recall,
    BuffInit(usize),
    BuffStacks(usize),
    BuffTime(usize),
}

/// Largeur (ou hauteur) dynamique.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum F {
    Hp,
    HpTrail,
    Shield,
    Res,
    Xp,
    Recall,
}

/// Image dynamique, avec la clé de ce qu'elle montre (pour ne la changer qu'au besoin).
#[derive(Component, Clone, Debug, PartialEq)]
struct Img {
    kind: Slot,
    sweep: bool,
    shown: String,
    /// Cran de balayage affiché (sur 64), pour ne changer l'image qu'au besoin.
    step: Option<u32>,
}

impl Img {
    fn icon(kind: Slot) -> Self {
        Self {
            kind,
            sweep: false,
            shown: String::new(),
            step: None,
        }
    }

    fn sweep(kind: Slot) -> Self {
        Self {
            kind,
            sweep: true,
            shown: String::new(),
            step: None,
        }
    }
}

/// Visibilité dynamique.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum V {
    RankUp(usize),
    /// Pastille de rang `j` du sort `i`, montrée jusqu'au rang max du sort.
    Pip(usize, usize),
    Points,
    ResBar,
    Shield,
    Cd(Slot),
    Buff(usize),
    Recall,
    Bar,
    Charges(Slot),
}

/// Fond dynamique.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum C {
    Pip(usize, usize),
    Buff(usize),
}

/// Cadre d'un sort (couleur selon l'état).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Frame(Slot);

/// Action d'un bouton de la barre.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum Act {
    RankUp(AbilitySlot),
    UseItem(usize),
    Trinket,
    Shop,
}

/// Le joueur, son état privé et sa fiche de champion.
struct Me<'a> {
    e: &'a EntitySnapshot,
    st: &'a SelfState,
    champ: &'static ChampionDef,
}

fn me(state: &GameState) -> Option<Me<'_>> {
    let st = state.self_state.as_ref()?;
    let e = state.entities().get(&state.my_entity?)?;
    let champ = get_champion(e.subtype.as_deref()?)?;
    Some(Me { e, st, champ })
}

fn cooldown(st: &SelfState, s: Slot) -> (f64, f64) {
    let cd = &st.cooldowns;
    match s {
        Slot::Passive => (0.0, 0.0),
        Slot::Ability(i) => (
            [cd.q, cd.w, cd.e, cd.r][i],
            0.0, // total calculé par l'appelant
        ),
        Slot::Summoner(i) => {
            let id = *st.summoners.get(SUMS[i]);
            ([cd.d, cd.f][i], summoner(id).cooldown)
        }
        Slot::Item(i) => {
            let rem = st
                .item_cooldowns
                .as_ref()
                .and_then(|v| v.get(i).copied().flatten())
                .unwrap_or(0.0);
            (rem, 0.0)
        }
        Slot::Trinket => (
            st.ward_charges
                .as_ref()
                .filter(|w| w.charges == 0)
                .map_or(0.0, |w| w.cooldown_remaining),
            0.0,
        ),
    }
}

/// Recharge restante et fraction (0..1) d'un emplacement.
fn cd_frac(m: &Me, s: Slot) -> (f64, f64) {
    let (rem, total) = cooldown(m.st, s);
    let total = match s {
        Slot::Ability(i) => {
            let a = m.champ.abilities.get(SLOTS[i]);
            let rank = *m.st.ability_levels.get(SLOTS[i]);
            if rank > 0 {
                hasted_cooldown(rank_value(&a.cooldown, rank), m.st.stats.ability_haste)
            } else {
                0.0
            }
        }
        Slot::Item(i) => item_at(m, i)
            .and_then(|d| d.active.as_ref())
            .map_or(0.0, |a| a.cooldown),
        _ => total,
    };
    (rem, cooldown_fraction(rem, total.max(rem)))
}

/// Coût du sort au rang actuel.
fn cost(m: &Me, i: usize) -> f64 {
    let rank = *m.st.ability_levels.get(SLOTS[i]);
    if rank > 0 {
        rank_value(&m.champ.abilities.get(SLOTS[i]).cost, rank)
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AbState {
    Locked,
    Cd,
    NoMana,
    Ready,
}

fn ab_state(m: &Me, i: usize) -> AbState {
    let rank = *m.st.ability_levels.get(SLOTS[i]);
    let (cd, _) = cd_frac(m, Slot::Ability(i));
    let no_mana =
        m.champ.resource != Resource::None && m.e.mana.unwrap_or(f64::INFINITY) < cost(m, i);
    if rank == 0 {
        AbState::Locked
    } else if cd > 0.0 {
        AbState::Cd
    } else if no_mana {
        AbState::NoMana
    } else {
        AbState::Ready
    }
}

fn items<'a>(m: &'a Me<'a>) -> &'a [Option<String>] {
    m.e.champ.as_ref().map_or(&[], |c| c.items.as_slice())
}

/// Identifiant de l'objet de l'emplacement `i`, sans copie.
fn item_id<'a>(m: &'a Me<'a>, i: usize) -> Option<&'a str> {
    items(m).get(i).and_then(|o| o.as_deref())
}

fn item_at(m: &Me, i: usize) -> Option<&'static ItemDef> {
    item_id(m, i).and_then(item)
}

/// Balise : l'emplacement 7, sinon un objet de catégorie balise.
fn trinket(m: &Me) -> Option<&'static ItemDef> {
    item_at(m, 6).or_else(|| {
        items(m)
            .iter()
            .flatten()
            .filter_map(|id| item(id))
            .find(|d| d.category == ItemCategory::Trinket)
    })
}

/// Statistiques permanentes : clé, glyphe, couleur, format.
const STAT_ROWS: [(&str, &str, u32); 10] = [
    ("ad", "sword", 0xffaa5c),
    ("ap", "wand", 0xc48bff),
    ("armor", "shield", 0xf1d37f),
    ("mr", "cloak", 0x7fc6ff),
    ("attackSpeed", "dagger", 0xffd16b),
    ("abilityHaste", "hourglass", 0x9fe7ff),
    ("critChance", "blade", 0xff7a6b),
    ("moveSpeed", "boots", 0x9ff0c2),
    ("armorPen", "scythe", 0xff9a5c),
    ("magicPen", "flask", 0xd2a8ff),
];

fn stat_value(s: &Stats, key: &str) -> f64 {
    match key {
        "ad" => s.ad,
        "ap" => s.ap,
        "armor" => s.armor,
        "mr" => s.mr,
        "attackSpeed" => s.attack_speed,
        "abilityHaste" => s.ability_haste,
        "critChance" => s.crit_chance,
        "moveSpeed" => s.move_speed,
        "armorPen" => s.armor_pen,
        _ => s.magic_pen,
    }
}

fn stat_text(s: &Stats, key: &str) -> String {
    match key {
        "ad" => format!("{}", s.ad.round()),
        "ap" => format!("{}", s.ap.round()),
        "armor" => format!("{}", s.armor.round()),
        "mr" => format!("{}", s.mr.round()),
        "attackSpeed" => format!("{:.2}", s.attack_speed).replace('.', ","),
        "abilityHaste" => format!("{}", s.ability_haste.round()),
        "critChance" => format!("{}%", (s.crit_chance * 100.0).round()),
        "moveSpeed" => format!("{}", s.move_speed.round()),
        "armorPen" => format!("{}", s.armor_pen.round()),
        _ => format!("{}", s.magic_pen.round()),
    }
}

fn decimal(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

// ---------------------------------------------------------------------------------- construction

fn slot_frame(size: f32, ult: bool) -> Node {
    Node {
        width: px(size),
        height: px(size),
        border: UiRect::all(px(if ult { 2.0 } else { 1.5 })),
        border_radius: BorderRadius::all(px(6)),
        overflow: Overflow::clip(),
        ..default()
    }
}

fn overlay() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn kbd(f: &UiFonts, key: &str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(2),
            bottom: px(1),
            padding: UiRect::axes(px(3), px(0)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(hexa(0x05060c, 0.8)),
        Pickable::IGNORE,
        children![label(f, key, 10.0, 700, GOLD_HI)],
    )
}

fn spawn_ability(p: &mut ChildSpawnerCommands, f: &UiFonts, i: usize) {
    let slot = SLOTS[i];
    let ult = slot == AbilitySlot::R;
    let s = Slot::Ability(i);
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(4),
        ..default()
    })
    .with_children(|col| {
        // bouton d'amélioration, au-dessus de l'icône
        col.spawn((
            button(
                ButtonLook::new(TEAL, hex(0x8ff7e2)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(-20),
                    width: px(20),
                    height: px(16),
                    border_radius: BorderRadius::all(px(4)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            Act::RankUp(slot),
            V::RankUp(i),
            // il dépasse du cadre de la barre : il bloque le monde lui-même
            BlocksWorld,
            Visibility::Hidden,
            children![label(f, "+", 13.0, 900, hex(0x032019))],
        ));
        col.spawn((
            slot_frame(ICON, ult),
            BorderColor::all(if ult { GOLD } else { GOLD_LO }),
            Frame(s),
            BackgroundColor(hex(0x05060c)),
        ))
        .with_children(|icon| {
            icon.spawn((
                overlay(),
                ImageNode::default(),
                Img::icon(s),
                Pickable::IGNORE,
            ));
            icon.spawn((
                overlay(),
                ImageNode::default(),
                Img::sweep(s),
                V::Cd(s),
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
            icon.spawn((overlay(), V::Cd(s), Visibility::Hidden, Pickable::IGNORE))
                .with_child((label(f, "", 18.0, 800, Color::WHITE), T::Cd(s)));
            icon.spawn(kbd(f, &format!("{slot:?}")));
            icon.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(3),
                    bottom: px(1),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_child((label(f, "", 10.0, 800, hex(0x9cc4ff)), T::Cost(i)));
        });
        // pastilles de rang
        col.spawn((
            Node {
                column_gap: px(3),
                height: px(5),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|pips| {
            // rang max du sort, connu une fois le champion reçu : les pastilles en trop sont cachées
            for j in 0..MAX_PIPS {
                pips.spawn((
                    Node {
                        width: px(if ult { 14.0 } else { 8.0 }),
                        height: px(5),
                        border_radius: BorderRadius::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(hexa(0xffffff, 0.12)),
                    C::Pip(i, j),
                    V::Pip(i, j),
                    Visibility::Hidden,
                ));
            }
        });
    });
}

/// Passif, avant Q : icône seule, sans recharge ni rang.
fn spawn_passive(p: &mut ChildSpawnerCommands) {
    p.spawn((
        Node {
            margin: UiRect::bottom(px(9)),
            ..slot_frame(PASSIVE, false)
        },
        BorderColor::all(GOLD_LO),
        BackgroundColor(hex(0x05060c)),
    ))
    .with_child((
        overlay(),
        ImageNode::default(),
        Img::icon(Slot::Passive),
        Pickable::IGNORE,
    ));
}

fn spawn_summoner(p: &mut ChildSpawnerCommands, f: &UiFonts, i: usize) {
    let s = Slot::Summoner(i);
    p.spawn((
        Node {
            margin: UiRect::bottom(px(9)),
            ..slot_frame(SUM_ICON, false)
        },
        BorderColor::all(GOLD_LO),
        BackgroundColor(hex(0x05060c)),
    ))
    .with_children(|icon| {
        icon.spawn((
            overlay(),
            ImageNode::default(),
            Img::icon(s),
            Pickable::IGNORE,
        ));
        icon.spawn((
            overlay(),
            ImageNode::default(),
            Img::sweep(s),
            V::Cd(s),
            Visibility::Hidden,
            Pickable::IGNORE,
        ));
        icon.spawn((overlay(), V::Cd(s), Visibility::Hidden, Pickable::IGNORE))
            .with_child((label(f, "", 14.0, 800, Color::WHITE), T::Cd(s)));
        icon.spawn(kbd(f, ["D", "F"][i]));
    });
}

fn bar(p: &mut ChildSpawnerCommands, f: &UiFonts, hp: bool) {
    let (fill, text, regen) = if hp {
        (F::Hp, T::Hp, T::HpRegen)
    } else {
        (F::Res, T::Res, T::ResRegen)
    };
    let mut e = p.spawn((
        Node {
            width: percent(100),
            height: px(if hp { 21.0 } else { 17.0 }),
            border_radius: BorderRadius::all(px(2)),
            overflow: Overflow::clip(),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(hexa(0x000000, 0.75)),
    ));
    if !hp {
        e.insert(V::ResBar);
    }
    e.with_children(|b| {
        let abs = |left: f32| Node {
            position_type: PositionType::Absolute,
            left: percent(left),
            top: px(0),
            bottom: px(0),
            width: percent(100),
            ..default()
        };
        if hp {
            b.spawn((abs(0.0), BackgroundColor(hexa(0xffd9a0, 0.55)), F::HpTrail));
        }
        b.spawn((
            abs(0.0),
            BackgroundGradient::from(LinearGradient {
                angle: 0.0,
                stops: vec![
                    ColorStop::auto(Color::WHITE.with_alpha(0.25)),
                    ColorStop::auto(Color::NONE),
                ],
                ..default()
            }),
            BackgroundColor(if hp { HP } else { MANA }),
            fill,
        ));
        if hp {
            b.spawn((
                abs(100.0),
                BackgroundColor(SHIELD),
                F::Shield,
                V::Shield,
                Visibility::Hidden,
            ));
        }
        b.spawn((label(f, "", if hp { 13.0 } else { 11.0 }, 700, TEXT), text));
        b.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(5),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_child((label(f, "", 10.0, 600, hexa(0xffffff, 0.7)), regen));
    });
}

fn spawn_portrait(p: &mut ChildSpawnerCommands, f: &UiFonts) {
    p.spawn(Node {
        width: px(PORTRAIT),
        height: px(PORTRAIT),
        margin: UiRect::top(px(-26)),
        flex_shrink: 0.0,
        ..default()
    })
    .with_children(|pt| {
        pt.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(6),
                top: px(6),
                width: px(PORTRAIT - 12.0),
                height: px(PORTRAIT - 12.0),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(GOLD),
        ))
        .with_child((
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            PortraitSlot,
        ));
        // jauge d'expérience verticale, à gauche
        pt.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(8),
                bottom: px(8),
                width: px(4),
                border_radius: BorderRadius::all(px(2)),
                flex_direction: FlexDirection::ColumnReverse,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(hexa(0x000000, 0.8)),
        ))
        .with_child((
            Node {
                width: percent(100),
                height: percent(0),
                ..default()
            },
            BackgroundColor(XP),
            F::Xp,
        ));
        // niveau et nom
        pt.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(-4),
                left: px(0),
                right: px(0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_child((
            Node {
                min_width: px(28),
                height: px(24),
                padding: UiRect::horizontal(px(6)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: px(5),
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(hex(0x10142a)),
            children![
                (label(f, "1", 13.0, 800, GOLD_HI), T::Level),
                (label(f, "", 10.0, 600, MUTED), T::Name),
            ],
        ));
        // points de compétence à dépenser
        pt.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                right: px(-2),
                padding: UiRect::axes(px(5), px(1)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(TEAL),
            V::Points,
            Visibility::Hidden,
            children![(label(f, "", 11.0, 800, hex(0x032019)), T::Points)],
        ));
    });
}

/// Nœud qui reçoit le portrait du champion dès qu'il est connu.
#[derive(Component)]
struct PortraitSlot;

fn spawn_stats(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
) {
    p.spawn(Node {
        display: Display::Grid,
        grid_template_columns: RepeatedGridTrack::px(2, 58.0),
        row_gap: px(2),
        column_gap: px(6),
        align_self: AlignSelf::Center,
        ..default()
    })
    .with_children(|g| {
        for (i, (_, glyph, color)) in STAT_ROWS.iter().enumerate() {
            let d = ITEM_GLYPH
                .iter()
                .find(|(k, _)| k == glyph)
                .map_or("", |(_, d)| *d);
            let image = icons.path(images, glyph, d);
            g.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(4),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: px(13),
                        height: px(13),
                        ..default()
                    },
                    ImageNode {
                        image,
                        color: hex(*color),
                        ..default()
                    },
                ));
                row.spawn((label(f, "0", 11.0, 700, TEXT), T::Stat(i)));
            });
        }
    });
}

fn spawn_item_slot(p: &mut ChildSpawnerCommands, f: &UiFonts, s: Slot, key: &str) {
    let act = match s {
        Slot::Item(i) => Act::UseItem(i),
        _ => Act::Trinket,
    };
    p.spawn((
        button(
            ButtonLook::new(hexa(0x05060c, 0.9), hexa(0x1a1f35, 0.95)),
            Node {
                width: px(ITEM),
                height: px(ITEM),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                overflow: Overflow::clip(),
                ..default()
            },
        ),
        BorderColor::all(hexa(0x7d6331, 0.8)),
        act,
    ))
    .with_children(|sl| {
        sl.spawn((
            overlay(),
            ImageNode::default(),
            Img::icon(s),
            Pickable::IGNORE,
        ));
        // la balise n'a pas de balayage (sa recharge n'a pas de durée totale connue) : le chiffre seul
        if s != Slot::Trinket {
            sl.spawn((
                overlay(),
                ImageNode::default(),
                Img::sweep(s),
                V::Cd(s),
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        }
        sl.spawn((overlay(), V::Cd(s), Visibility::Hidden, Pickable::IGNORE))
            .with_child((label(f, "", 13.0, 800, Color::WHITE), T::Cd(s)));
        sl.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(2),
                top: px(0),
                ..default()
            },
            V::Charges(s),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_child((label(f, "", 11.0, 800, Color::WHITE), T::Charges(s)));
        sl.spawn(kbd(f, key));
    });
}

fn spawn_inventory(
    p: &mut ChildSpawnerCommands,
    f: &UiFonts,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
) {
    p.spawn(Node {
        column_gap: px(6),
        align_items: AlignItems::FlexEnd,
        ..default()
    })
    .with_children(|inv| {
        inv.spawn(Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::px(3, ITEM),
            row_gap: px(4),
            column_gap: px(4),
            ..default()
        })
        .with_children(|g| {
            for (i, key) in ITEM_KEYS.iter().enumerate() {
                spawn_item_slot(g, f, Slot::Item(i), key);
            }
        });
        inv.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|side| {
            spawn_item_slot(side, f, Slot::Trinket, "4");
            let coin = icons.glyph(images, "gold");
            side.spawn((
                button(
                    ButtonLook::new(hexa(0x1a1406, 0.95), hexa(0x3a2c0c, 0.95)),
                    Node {
                        min_width: px(ITEM + 18.0),
                        height: px(ITEM),
                        padding: UiRect::horizontal(px(6)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        column_gap: px(4),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ),
                BorderColor::all(GOLD_LO),
                Act::Shop,
            ))
            .with_children(|b| {
                b.spawn((
                    Node {
                        width: px(15),
                        height: px(15),
                        ..default()
                    },
                    ImageNode {
                        image: coin,
                        color: GOLD_HI,
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                b.spawn((label(f, "0", 14.0, 800, GOLD_HI), T::Gold));
            });
        });
    });
}

fn spawn_buffs(p: &mut ChildSpawnerCommands, f: &UiFonts) {
    p.spawn((
        Node {
            column_gap: px(4),
            margin: UiRect::bottom(px(6)),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|row| {
        for i in 0..MAX_BUFFS {
            row.spawn((
                Node {
                    width: px(30),
                    height: px(30),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(hexa(0xffffff, 0.25)),
                BackgroundColor(hexa(0x1d3a24, 0.9)),
                C::Buff(i),
                V::Buff(i),
                Visibility::Hidden,
            ))
            .with_children(|b| {
                b.spawn((label(f, "", 11.0, 800, TEXT), T::BuffInit(i)));
                b.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(1),
                        top: px(-2),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_child((label(f, "", 9.0, 800, GOLD_HI), T::BuffStacks(i)));
                b.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(-13),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_child((label(f, "", 9.0, 700, MUTED), T::BuffTime(i)));
            });
        }
    });
}

fn spawn_recall(p: &mut ChildSpawnerCommands, f: &UiFonts) {
    p.spawn((
        Node {
            width: px(260),
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: UiRect::axes(px(10), px(4)),
            margin: UiRect::bottom(px(8)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BackgroundColor(PANEL),
        V::Recall,
        Visibility::Hidden,
        Pickable::IGNORE,
    ))
    .with_children(|r| {
        r.spawn(label(f, "Rappel", 12.0, 700, hex(0x9fd8ff)));
        r.spawn((
            Node {
                flex_grow: 1.0,
                height: px(6),
                border_radius: BorderRadius::all(px(3)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(hexa(0x000000, 0.6)),
        ))
        .with_child((
            Node {
                width: percent(0),
                height: percent(100),
                ..default()
            },
            BackgroundColor(hex(0x6fc3ff)),
            F::Recall,
        ));
        r.spawn((label(f, "", 12.0, 800, TEXT), T::Recall));
    });
}

fn spawn(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let f = &*fonts;
    commands.spawn(hud_root()).with_children(|root| {
        root.spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            V::Bar,
            Visibility::Hidden,
        ))
        .with_children(|col| {
            spawn_buffs(col, f);
            spawn_recall(col, f);
            col.spawn((
                Node {
                    align_items: AlignItems::FlexEnd,
                    column_gap: px(12),
                    padding: UiRect::new(px(8), px(14), px(9), px(9)),
                    border: UiRect::top(px(2)),
                    border_radius: BorderRadius::top(px(10)),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.7)),
                BackgroundGradient::from(LinearGradient {
                    angle: 0.0,
                    stops: vec![
                        ColorStop::auto(hexa(0x161a33, 0.94)),
                        ColorStop::auto(hexa(0x070912, 0.97)),
                    ],
                    ..default()
                }),
                BlocksWorld,
                GlobalZIndex(HUD_Z),
            ))
            .with_children(|frame| {
                spawn_stats(frame, f, &mut icons, &mut images);
                spawn_portrait(frame, f);
                frame
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(7),
                        ..default()
                    })
                    .with_children(|core| {
                        core.spawn(Node {
                            align_items: AlignItems::FlexEnd,
                            column_gap: px(6),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_passive(row);
                            for i in 0..4 {
                                spawn_ability(row, f, i);
                            }
                            row.spawn(Node {
                                width: px(6),
                                ..default()
                            });
                            for i in 0..2 {
                                spawn_summoner(row, f, i);
                            }
                        });
                        core.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            ..default()
                        })
                        .with_children(|bars| {
                            bar(bars, f, true);
                            bar(bars, f, false);
                        });
                    });
                spawn_inventory(frame, f, &mut icons, &mut images);
            });
        });
    });
}

// ---------------------------------------------------------------------------------- mise à jour

/// Dernière valeur source d'un texte : il n'est reformaté que quand elle change.
#[derive(Component, Default)]
struct Src(Option<[u64; 3]>);

/// Valeurs dont dépend le texte `t` (au plus trois).
fn text_src(t: T, m: &Me, shield: f64) -> [f64; 3] {
    let st = m.st;
    let buff = |i: usize| m.e.buffs.as_deref().and_then(|b| b.get(i));
    match t {
        T::Hp => [m.e.hp.round(), m.e.max_hp.round(), shield.round()],
        T::HpRegen => [st.stats.hp_regen, 0.0, 0.0],
        T::Res => [
            m.e.mana.unwrap_or(0.0).round(),
            m.e.max_mana.unwrap_or(0.0).round(),
            0.0,
        ],
        T::ResRegen => [st.stats.mana_regen, 0.0, 0.0],
        T::Level => [f64::from(st.level), 0.0, 0.0],
        T::Name => [str_key(&m.champ.id), 0.0, 0.0],
        T::Points => [f64::from(st.skill_points), 0.0, 0.0],
        T::Stat(i) => [stat_value(&st.stats, STAT_ROWS[i].0), 0.0, 0.0],
        // au dixième : ce que montre `format_cooldown` sous la seconde
        T::Cd(s) => [(cd_frac(m, s).0 * 10.0).ceil(), 0.0, 0.0],
        T::Cost(i) => [cost(m, i), 0.0, 0.0],
        T::Charges(s) => [charges(m, s).unwrap_or(-1.0), 0.0, 0.0],
        T::Gold => [st.gold.floor(), 0.0, 0.0],
        T::Recall => [st.recalling.map_or(-1.0, |r| (r * 10.0).round()), 0.0, 0.0],
        T::BuffInit(i) => [buff(i).map_or(0.0, |b| str_key(&b.id)), 0.0, 0.0],
        T::BuffStacks(i) => [
            buff(i).and_then(|b| b.stacks).map_or(0.0, f64::from),
            0.0,
            0.0,
        ],
        T::BuffTime(i) => [buff(i).map_or(-1.0, |b| b.remaining.ceil()), 0.0, 0.0],
    }
}

/// Charges affichées d'un objet ou de la balise.
fn charges(m: &Me, s: Slot) -> Option<f64> {
    match s {
        Slot::Item(i) => {
            m.st.item_charges
                .as_ref()
                .and_then(|v| v.get(i).copied().flatten())
                .filter(|c| *c > 1.0)
        }
        Slot::Trinket => m.st.ward_charges.as_ref().map(|w| f64::from(w.charges)),
        _ => None,
    }
}

fn text(t: T, m: &Me, shield: f64, out: &mut String) {
    use std::fmt::Write;
    let st = m.st;
    let buff = |i: usize| m.e.buffs.as_deref().and_then(|b| b.get(i));
    out.clear();
    // `write!` dans une `String` ne peut pas échouer
    let _ = match t {
        T::Hp if shield > 0.0 => write!(
            out,
            "{} / {} (+{})",
            format_int(m.e.hp),
            format_int(m.e.max_hp),
            format_int(shield)
        ),
        T::Hp => write!(out, "{} / {}", format_int(m.e.hp), format_int(m.e.max_hp)),
        T::HpRegen => write!(out, "+{}", decimal(st.stats.hp_regen / 5.0)),
        T::Res => write!(
            out,
            "{} / {}",
            format_int(m.e.mana.unwrap_or(0.0)),
            format_int(m.e.max_mana.unwrap_or(0.0))
        ),
        T::ResRegen => write!(out, "+{}", decimal(st.stats.mana_regen / 5.0)),
        T::Level => write!(out, "{}", st.level),
        T::Name => write!(out, "{}", m.champ.name),
        T::Points => write!(out, "+{}", st.skill_points),
        T::Stat(i) => write!(out, "{}", stat_text(&st.stats, STAT_ROWS[i].0)),
        T::Cd(s) => write!(out, "{}", format_cooldown(cd_frac(m, s).0)),
        T::Cost(i) => {
            let c = cost(m, i);
            if c > 0.0 {
                write!(out, "{}", c.round())
            } else {
                Ok(())
            }
        }
        T::Charges(s) => match charges(m, s) {
            Some(c) => write!(out, "{}", c.round()),
            None => Ok(()),
        },
        T::Gold => write!(out, "{}", st.gold.floor()),
        T::Recall => match st.recalling {
            Some(r) => write!(out, "{}", decimal(r)),
            None => Ok(()),
        },
        T::BuffInit(i) => match buff(i) {
            Some(b) => write!(out, "{}", buff_initials(&b.id)),
            None => Ok(()),
        },
        T::BuffStacks(i) => match buff(i).and_then(|b| b.stacks).filter(|s| *s > 1) {
            Some(n) => write!(out, "{n}"),
            None => Ok(()),
        },
        T::BuffTime(i) => match buff(i) {
            Some(b) => write!(out, "{}", b.remaining.ceil()),
            None => Ok(()),
        },
    };
}

fn shield_of(m: &Me) -> f64 {
    if m.e.dead == Some(true) {
        0.0
    } else {
        m.e.shield.unwrap_or(0.0)
    }
}

fn update_texts(
    state: Res<GameState>,
    mut q: Query<(&T, &mut Src, &mut Text)>,
    mut buf: Local<String>,
) {
    let Some(m) = me(&state) else { return };
    let shield = shield_of(&m);
    for (t, mut src, mut txt) in &mut q {
        let key = text_src(*t, &m, shield).map(f64::to_bits);
        if src.0 == Some(key) {
            continue;
        }
        src.0 = Some(key);
        text(*t, &m, shield, &mut buf);
        set_text(&mut txt, &buf);
    }
}

fn update_fills(
    state: Res<GameState>,
    time: Res<Time>,
    mut q: Query<(&F, &mut Node)>,
    mut trail: Local<f32>,
) {
    let Some(m) = me(&state) else { return };
    let max_hp = m.e.max_hp.max(1.0);
    let shield = shield_of(&m);
    let denom = max_hp.max(m.e.hp + shield);
    let hp = (m.e.hp / denom * 100.0) as f32;
    let sh = (shield / denom * 100.0) as f32;
    // la traînée rattrape les PV perdus en ~0,5 s, puis s'arrête dessus
    *trail = if *trail < hp || (*trail - hp).abs() < 0.05 {
        hp
    } else {
        *trail + (hp - *trail) * (time.delta_secs() * 4.0).min(1.0)
    };
    let res = match m.e.max_mana {
        Some(mx) if mx > 0.0 => (m.e.mana.unwrap_or(0.0) / mx * 100.0) as f32,
        _ => 0.0,
    };
    let xp = if m.st.xp_to_next > 0.0 {
        (m.st.xp / m.st.xp_to_next).min(1.0) as f32 * 100.0
    } else {
        100.0
    };
    let recall = m.st.recalling.map_or(0.0, |r| {
        let total = RECALL_SECONDS.max(r);
        ((1.0 - r / total) * 100.0) as f32
    });
    for (f, mut node) in &mut q {
        match f {
            F::Hp => set_val(&mut node, |n| &mut n.width, percent(hp)),
            F::HpTrail => set_val(&mut node, |n| &mut n.width, percent(*trail)),
            F::Shield => {
                set_val(&mut node, |n| &mut n.left, percent(hp));
                set_val(&mut node, |n| &mut n.width, percent(sh));
            }
            F::Res => set_val(&mut node, |n| &mut n.width, percent(res)),
            F::Xp => set_val(&mut node, |n| &mut n.height, percent(xp)),
            F::Recall => set_val(&mut node, |n| &mut n.width, percent(recall)),
        }
    }
}

/// Ce que montre une icône (clé sans copie) et sa teinte.
fn icon_of<'a>(m: &'a Me<'a>, kind: Slot) -> (&'a str, Color) {
    match kind {
        Slot::Passive => (m.champ.passive.id.as_str(), Color::WHITE),
        Slot::Ability(i) => {
            let tint = match ab_state(m, i) {
                AbState::Locked => Color::srgb(0.3, 0.3, 0.33),
                AbState::Cd => Color::srgb(0.62, 0.62, 0.66),
                AbState::NoMana => Color::srgb(0.45, 0.55, 1.0),
                AbState::Ready => Color::WHITE,
            };
            (m.champ.abilities.get(SLOTS[i]).id.as_str(), tint)
        }
        Slot::Summoner(i) => (
            summoner(*m.st.summoners.get(SUMS[i])).name.as_str(),
            Color::WHITE,
        ),
        Slot::Item(i) => (item_id(m, i).unwrap_or(""), Color::WHITE),
        Slot::Trinket => (trinket(m).map_or("", |d| d.id.as_str()), Color::WHITE),
    }
}

fn update_images(
    state: Res<GameState>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(&mut Img, &mut ImageNode, &mut Visibility)>,
) {
    let Some(m) = me(&state) else { return };
    for (mut img, mut node, mut vis) in &mut q {
        if img.sweep {
            let (cd, frac) = cd_frac(&m, img.kind);
            // 64 crans, comme le cache des balayages
            let step = (frac.clamp(0.0, 1.0) * 64.0).round() as u32;
            if cd > 0.0 && img.step != Some(step) {
                node.image = icons.sweep(&mut images, step as f32 / 64.0);
                img.step = Some(step);
            }
            set_shown(&mut vis, cd > 0.0);
            continue;
        }
        let (key, tint) = icon_of(&m, img.kind);
        if node.color != tint {
            node.color = tint;
        }
        set_shown(&mut vis, !key.is_empty());
        if img.shown == key {
            continue;
        }
        let handle = match img.kind {
            Slot::Passive => Some(icons.ability(&mut images, &m.champ.passive, Some(m.champ))),
            Slot::Ability(i) => {
                Some(icons.ability(&mut images, m.champ.abilities.get(SLOTS[i]), Some(m.champ)))
            }
            Slot::Summoner(i) => Some(icons.summoner(&mut images, *m.st.summoners.get(SUMS[i]))),
            Slot::Item(_) | Slot::Trinket => item(key).map(|d| icons.item(&mut images, d)),
        };
        if let Some(h) = handle {
            node.image = h;
        }
        key.clone_into(&mut img.shown);
    }
}

fn update_shown(state: Res<GameState>, mut q: Query<(&V, &mut Visibility), Without<Img>>) {
    let m = me(&state);
    for (v, mut vis) in &mut q {
        let Some(m) = &m else {
            set_shown(&mut vis, false);
            continue;
        };
        let on = match *v {
            V::Bar => true,
            V::RankUp(i) => can_rank_up(
                SLOTS[i],
                m.st.level,
                *m.st.ability_levels.get(SLOTS[i]),
                m.st.skill_points,
            ),
            V::Pip(i, j) => (j as u32) < m.champ.abilities.get(SLOTS[i]).max_rank,
            V::Points => m.st.skill_points > 0,
            V::ResBar => m.champ.resource != Resource::None,
            V::Shield => shield_of(m) > 0.0,
            V::Cd(s) => cd_frac(m, s).0 > 0.0,
            V::Buff(i) => m.e.buffs.as_ref().is_some_and(|b| b.len() > i),
            V::Recall => m.st.recalling.is_some_and(|r| r > 0.0),
            V::Charges(s) => charges(m, s).is_some(),
        };
        set_shown(&mut vis, on);
    }
}

fn update_colors(
    state: Res<GameState>,
    mut bgs: Query<(&C, &mut BackgroundColor)>,
    mut frames: Query<(&Frame, &mut BorderColor)>,
) {
    let Some(m) = me(&state) else { return };
    for (c, mut bg) in &mut bgs {
        let color = match *c {
            C::Pip(i, j) => {
                if (j as u32) < *m.st.ability_levels.get(SLOTS[i]) {
                    GOLD_HI
                } else {
                    hexa(0xffffff, 0.12)
                }
            }
            C::Buff(i) => match m.e.buffs.as_ref().and_then(|b| b.get(i)) {
                Some(b) if is_debuff(&b.id) => hexa(0x4a1820, 0.92),
                _ => hexa(0x163a26, 0.92),
            },
        };
        set_bg(&mut bg, color);
    }
    let resource_color = if m.champ.resource == Resource::Energy {
        ENERGY
    } else {
        MANA
    };
    for (Frame(s), mut border) in &mut frames {
        let Slot::Ability(i) = *s else { continue };
        let c = match ab_state(&m, i) {
            AbState::NoMana => resource_color,
            AbState::Ready if SLOTS[i] == AbilitySlot::R => GOLD,
            AbState::Ready => GOLD_LO,
            _ => hexa(0x3a3f55, 1.0),
        };
        if border.top != c {
            *border = BorderColor::all(c);
        }
    }
}

/// Portrait posé dès que le champion est connu ; couleur de la barre de ressource.
fn update_portrait(
    mut commands: Commands,
    state: Res<GameState>,
    slots: Query<(Entity, Option<&ChampionPortrait>), With<PortraitSlot>>,
    mut res: Query<(&F, &mut BackgroundColor), Without<C>>,
) {
    let Some(m) = me(&state) else { return };
    let dead = m.e.dead == Some(true);
    for (e, p) in &slots {
        let same = p.is_some_and(|p| {
            p.id == m.champ.id && p.kind == PortraitKind::Square && p.grayscale == dead
        });
        if !same {
            commands
                .entity(e)
                .insert(ChampionPortrait::new(m.champ.id.clone(), PortraitKind::Square).gray(dead));
        }
    }
    let color = if m.champ.resource == Resource::Energy {
        ENERGY
    } else {
        MANA
    };
    for (f, mut bg) in &mut res {
        if *f == F::Res {
            set_bg(&mut bg, color);
        }
    }
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    state: Res<GameState>,
    mut input: ResMut<PlayerInput>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    match *act {
        Act::RankUp(slot) => conn.send(ClientMsg::LevelAbility { slot }),
        Act::UseItem(i) => {
            let active = state
                .entities()
                .get(&state.my_entity.unwrap_or_default())
                .and_then(|e| e.champ.as_ref())
                .and_then(|c| c.items.get(i).cloned().flatten())
                .and_then(|id| item(&id))
                .is_some_and(|d| d.active.is_some());
            if active {
                conn.send(ClientMsg::UseItem {
                    slot: i as i32,
                    target_pos: input.cursor,
                });
            }
        }
        Act::Trinket => {
            let Some(m) = me(&state) else { return };
            match trinket(&m) {
                Some(d) if d.active.is_some() && d.id != "stealth_ward" => {
                    conn.send(ClientMsg::UseItem {
                        slot: -1,
                        target_pos: input.cursor,
                    })
                }
                Some(_) => input.targeting = Some(Targeting::Ward),
                None => {}
            }
        }
        Act::Shop => input.shop_open = !input.shop_open,
    }
}

pub struct BottomBarPlugin;

impl Plugin for BottomBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Game), spawn)
            .add_systems(
                Update,
                (
                    update_texts,
                    update_fills,
                    update_images,
                    update_shown,
                    update_colors,
                    update_portrait,
                )
                    .run_if(in_state(Screen::Game)),
            )
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::types::{
        AbilityMap, ChampSnapshot, Cooldowns, EntityKind, SummonerId, SummonerMap, TeamOrNeutral,
        WardCharges,
    };

    /// Grondak niveau 6 (Q 2, W 1, E 0, R 1), 200 de mana.
    fn grondak() -> (EntitySnapshot, SelfState) {
        let champ = get_champion("grondak").unwrap();
        let e = EntitySnapshot {
            id: 1,
            kind: EntityKind::Champion,
            team: TeamOrNeutral::Blue,
            x: 0.0,
            y: 0.0,
            facing: 0.0,
            hp: 500.0,
            max_hp: 1000.0,
            mana: Some(200.0),
            max_mana: Some(400.0),
            shield: None,
            dead: None,
            subtype: Some("grondak".into()),
            owner_id: None,
            level: Some(6),
            anim: None,
            buffs: None,
            champ: Some(ChampSnapshot {
                player_id: "p".into(),
                name: "P".into(),
                kills: 0,
                deaths: 0,
                assists: 0,
                cs: 0,
                items: vec![
                    Some("lame_des_rafales".into()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("stealth_ward".into()),
                ],
                respawn_in: None,
            }),
        };
        let st = SelfState {
            entity_id: 1,
            gold: 500.0,
            xp: 0.0,
            xp_to_next: 100.0,
            level: 6,
            skill_points: 0,
            ability_levels: AbilityMap {
                q: 2,
                w: 1,
                e: 0,
                r: 1,
            },
            cooldowns: Cooldowns {
                q: 4.25,
                w: 0.0,
                e: 0.0,
                r: 0.0,
                d: 150.0,
                f: 0.0,
            },
            summoners: SummonerMap {
                d: SummonerId::Flash,
                f: SummonerId::Ignite,
            },
            stats: champ.base_stats.clone(),
            recalling: None,
            can_shop: false,
            ward_charges: Some(WardCharges {
                charges: 0,
                cooldown_remaining: 30.0,
            }),
            item_charges: None,
            item_cooldowns: Some(vec![Some(15.0)]),
        };
        (e, st)
    }

    fn me_of<'a>(e: &'a EntitySnapshot, st: &'a SelfState) -> Me<'a> {
        Me {
            e,
            st,
            champ: get_champion("grondak").unwrap(),
        }
    }

    #[test]
    fn etat_et_cout_des_sorts() {
        let (mut e, mut st) = grondak();
        {
            let m = me_of(&e, &st);
            // Q rang 2 : coût 55, en recharge
            assert_eq!(cost(&m, 0), 55.0);
            assert_eq!(ab_state(&m, 0), AbState::Cd);
            // W rang 1, prêt ; E non appris : verrouillé et sans coût
            assert_eq!(ab_state(&m, 1), AbState::Ready);
            assert_eq!(cost(&m, 2), 0.0);
            assert_eq!(ab_state(&m, 2), AbState::Locked);
            assert_eq!(ab_state(&m, 3), AbState::Ready);
        }
        // 50 de mana : le R (100) manque de mana, le W (60) aussi
        e.mana = Some(50.0);
        st.cooldowns.q = 0.0;
        let m = me_of(&e, &st);
        assert_eq!(ab_state(&m, 3), AbState::NoMana);
        assert_eq!(ab_state(&m, 1), AbState::NoMana);
        assert_eq!(ab_state(&m, 0), AbState::NoMana, "Q rang 2 coûte 55");
        // 60 de mana : Q (55) prêt, W (60) prêt, R (100) toujours pas
        e.mana = Some(60.0);
        let m = me_of(&e, &st);
        assert_eq!(ab_state(&m, 0), AbState::Ready);
        assert_eq!(ab_state(&m, 1), AbState::Ready);
        assert_eq!(ab_state(&m, 3), AbState::NoMana);
    }

    #[test]
    fn fraction_des_recharges() {
        let (e, st) = grondak();
        let m = me_of(&e, &st);
        // Q rang 2 : 8,5 s, il en reste 4,25 → moitié
        let (rem, f) = cd_frac(&m, Slot::Ability(0));
        assert_eq!(rem, 4.25);
        assert!((f - 0.5).abs() < 1e-9, "{f}");
        // Flash : recharge totale du sort d'invocateur
        let (rem, f) = cd_frac(&m, Slot::Summoner(0));
        assert_eq!(rem, 150.0);
        let total = summoner(SummonerId::Flash).cooldown;
        assert!((f - 150.0 / total).abs() < 1e-9);
        // objet actif : 15 s sur 60
        let (rem, f) = cd_frac(&m, Slot::Item(0));
        assert_eq!(rem, 15.0);
        assert!((f - 0.25).abs() < 1e-9);
        // emplacement vide, passif : rien
        assert_eq!(cd_frac(&m, Slot::Item(1)).0, 0.0);
        assert_eq!(cd_frac(&m, Slot::Passive).0, 0.0);
        // balise sans charge : sa recharge
        assert_eq!(cd_frac(&m, Slot::Trinket).0, 30.0);
    }

    #[test]
    fn balise_a_l_emplacement_7_sinon_dans_l_inventaire() {
        let (mut e, st) = grondak();
        assert_eq!(
            trinket(&me_of(&e, &st)).map(|d| d.id.as_str()),
            Some("stealth_ward")
        );
        let items = &mut e.champ.as_mut().unwrap().items;
        items[6] = None;
        items[3] = Some("stealth_ward".into());
        assert_eq!(
            trinket(&me_of(&e, &st)).map(|d| d.id.as_str()),
            Some("stealth_ward")
        );
        e.champ.as_mut().unwrap().items[3] = None;
        assert!(trinket(&me_of(&e, &st)).is_none());
    }

    #[test]
    fn un_texte_n_est_reformate_que_si_sa_source_change() {
        let mut app = App::new();
        let (e, st) = grondak();
        let mut state = GameState::default();
        state.my_entity = Some(1);
        state.buffer.entities.insert(1, e);
        state.self_state = Some(st);
        app.insert_resource(state);
        app.add_systems(Update, update_texts);
        let gold = app.world_mut().spawn((T::Gold, Text::default())).id();
        let cd = app
            .world_mut()
            .spawn((T::Cd(Slot::Ability(0)), Text::default()))
            .id();
        app.update();
        let text = |app: &App, e: Entity| app.world().get::<Text>(e).unwrap().0.clone();
        assert_eq!(text(&app, gold), "500");
        assert_eq!(text(&app, cd), "5");
        // même source : le texte n'est pas réécrit (pas de changement signalé)
        let tick = app.world().read_change_tick();
        app.update();
        assert!(
            !app.world()
                .entity(gold)
                .get_ref::<Text>()
                .unwrap()
                .last_changed()
                .is_newer_than(tick, app.world().read_change_tick())
        );
        // l'or change : le texte suit
        app.world_mut()
            .resource_mut::<GameState>()
            .self_state
            .as_mut()
            .unwrap()
            .gold = 612.7;
        app.update();
        assert_eq!(text(&app, gold), "612");
    }

    #[test]
    fn formats_des_statistiques() {
        let mut s = Stats {
            max_hp: 0.0,
            hp_regen: 0.0,
            max_mana: 0.0,
            mana_regen: 0.0,
            ad: 64.4,
            ap: 0.0,
            armor: 30.0,
            mr: 30.0,
            attack_speed: 0.658,
            attack_range: 125.0,
            move_speed: 345.0,
            crit_chance: 0.25,
            ability_haste: 0.0,
            lifesteal: 0.0,
            armor_pen: 0.0,
            magic_pen: 0.0,
        };
        assert_eq!(stat_text(&s, "ad"), "64");
        assert_eq!(stat_text(&s, "attackSpeed"), "0,66");
        assert_eq!(stat_text(&s, "critChance"), "25%");
        s.ad = 100.5;
        assert_eq!(stat_text(&s, "ad"), "101");
        assert_eq!(decimal(1.24), "1,2");
    }
}
