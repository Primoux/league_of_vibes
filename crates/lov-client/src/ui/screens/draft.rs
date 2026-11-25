//! Sélection des champions : portage de `packages/client/src/ui/screens/Draft.tsx`.
//!
//! Panneaux des deux équipes (choix verrouillés, survols des alliés, joueur dont c'est le tour), tour en
//! cours et minuteur, fiche du champion sélectionné (rôles, portée, difficulté, barres de stats, sorts,
//! histoire), galerie filtrable par rôle (clic : sélection, double-clic : verrouillage), puis choix des
//! sorts d'invocateur une fois verrouillé. L'écran est reconstruit quand le draft ou la sélection
//! locale change ; le minuteur est rafraîchi à chaque image.
//!
//! Le champion sélectionné tourne sur sa scène 3D (`stage`), derrière sa fiche ; il joue son ultime quand
//! le joueur le verrouille. La scène vit avec l'écran : la reconstruire ne recrée que son image.
//!
//! Écarts : les portraits arrivent avec les icônes ; pas d'infobulles (le sort survolé de la fiche
//! s'affiche en dessous, comme au clic sur le web).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lov_shared::constants::DRAFT_TURN_SECONDS;
use lov_shared::data::champions::{CHAMPIONS, get_champion};
use lov_shared::data::summoners::{SUMMONER_IDS, summoner};
use lov_shared::protocol::{ClientMsg, DraftPick, DraftState};
use lov_shared::schema::{ChampionDef, Targeting};
use lov_shared::types::{AbilitySlot, Role, SummonerId, Team};

use super::stage::{ChampionStage, StageView};
use super::{screen_root, team_color, team_name};
use crate::net::{Connection, GameState};
use crate::ui::logic::format::{class_label, role_label};
use crate::ui::theme::{GOLD, GOLD_HI, MUTED, PANEL, TEAL, TEXT, css, hexa};
use crate::ui::{ButtonLook, Screen, UiFonts, button, remaining};

/// Durée de la finalisation (choix des sorts d'invocateur), en secondes.
const FINALIZE_SECONDS: f64 = 15.0;
/// Deux clics sur la même vignette en moins de 0,35 s : verrouillage.
const DOUBLE_CLICK: f32 = 0.35;

/// Sorts de la fiche : passif puis QWER.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KitSlot {
    P,
    Ability(AbilitySlot),
}

const KIT: [KitSlot; 5] = [
    KitSlot::P,
    KitSlot::Ability(AbilitySlot::Q),
    KitSlot::Ability(AbilitySlot::W),
    KitSlot::Ability(AbilitySlot::E),
    KitSlot::Ability(AbilitySlot::R),
];

/// Sélection locale (les `useState` du web).
#[derive(Resource, Clone, PartialEq)]
pub struct DraftUi {
    pub selected: Option<String>,
    pub role: Option<Role>,
    pub focus: KitSlot,
    pub summoner_slot: usize,
    last_click: Option<(String, f32)>,
    stage: Option<Entity>,
    /// Incrémenté au verrouillage : le champion joue son ultime.
    pulse: u32,
    /// Choix du joueur verrouillé à l'image précédente (`None` : pas encore observé).
    was_locked: Option<bool>,
}

impl Default for DraftUi {
    fn default() -> Self {
        Self {
            selected: None,
            role: None,
            focus: KitSlot::P,
            summoner_slot: 0,
            last_click: None,
            stage: None,
            pulse: 0,
            was_locked: None,
        }
    }
}

/// Ce que l'écran déduit du draft pour le joueur `me` (testé).
#[derive(Debug, PartialEq)]
pub struct DraftView {
    pub finalizing: bool,
    pub my_turn: bool,
    pub my_team: Team,
    pub turn_team: Option<Team>,
    pub phase_text: &'static str,
}

pub fn draft_view(draft: &DraftState, me: Option<&str>) -> DraftView {
    let my_pick = draft
        .picks
        .iter()
        .find(|p| Some(p.player_id.as_str()) == me);
    let my_team = my_pick.map_or(Team::Blue, |p| p.team);
    let finalizing = draft.current_turn as usize >= draft.turns.len();
    let turn: &[String] = draft
        .turns
        .get(draft.current_turn as usize)
        .map_or(&[], Vec::as_slice);
    let my_turn = !finalizing
        && me.is_some_and(|m| turn.iter().any(|p| p == m))
        && !my_pick.is_some_and(|p| p.locked);
    let turn_team = draft
        .picks
        .iter()
        .find(|p| turn.contains(&p.player_id))
        .map(|p| p.team);
    let phase_text = if finalizing {
        "Prépare-toi au combat"
    } else if my_turn {
        "À toi de choisir !"
    } else if turn_team == Some(my_team) {
        "Ton équipe choisit…"
    } else {
        "L’équipe adverse choisit…"
    };
    DraftView {
        finalizing,
        my_turn,
        my_team,
        turn_team,
        phase_text,
    }
}

/// Champion verrouillé par un joueur, s'il y en a un.
fn taken_by<'a>(draft: &'a DraftState, id: &str) -> Option<&'a DraftPick> {
    draft
        .picks
        .iter()
        .find(|p| p.locked && p.champion_id.as_deref() == Some(id))
}

/// Sorts d'invocateur après avoir choisi `id` pour l'emplacement `slot` : s'il est déjà dans l'autre
/// emplacement, les deux s'échangent (`SummonerPicker`).
pub fn choose_summoner(cur: [SummonerId; 2], slot: usize, id: SummonerId) -> [SummonerId; 2] {
    let mut next = cur;
    let other = 1 - slot;
    if next[other] == id {
        next[other] = next[slot];
    }
    next[slot] = id;
    next
}

/// Barres de stats de la fiche (0..1) : dégâts, robustesse, portée, mobilité (`STAT_BARS`).
pub fn stat_bars(c: &ChampionDef) -> [(&'static str, f64); 4] {
    let s = &c.base_stats;
    let pl = &c.per_level;
    let dashes = [
        AbilitySlot::Q,
        AbilitySlot::W,
        AbilitySlot::E,
        AbilitySlot::R,
    ]
    .iter()
    .filter(|slot| {
        matches!(
            c.abilities.get(**slot).targeting,
            Targeting::Dash | Targeting::TargetedDash
        )
    })
    .count() as f64;
    let bar = |v: f64, max: f64| (v / max).clamp(0.08, 1.0);
    [
        ("Dégâts", bar(s.ad + pl.ad.unwrap_or(0.0) * 10.0, 120.0)),
        (
            "Robustesse",
            bar(
                s.max_hp / 10.0 + s.armor + pl.max_hp.unwrap_or(0.0) / 3.0,
                160.0,
            ),
        ),
        ("Portée", bar(s.attack_range, 650.0)),
        ("Mobilité", bar(s.move_speed - 300.0 + dashes * 25.0, 100.0)),
    ]
}

#[derive(Component, Clone)]
enum Act {
    Filter(Option<Role>),
    Tile(String),
    Lock,
    Focus(KitSlot),
    SummonerSlot(usize),
    Summoner(SummonerId),
}

#[derive(Component)]
struct DraftRoot;
#[derive(Component)]
struct TimerText;
#[derive(Component)]
struct TimerBar;

/// Ce qui a été affiché : draft, noms des joueurs, joueur local, sélection.
type Signature = (
    DraftState,
    Vec<(String, String)>,
    Option<String>,
    DraftUiKey,
);

#[derive(Resource, Default)]
struct Shown(Option<Signature>);

/// Partie de `DraftUi` qui change l'affichage.
#[derive(Clone, PartialEq)]
struct DraftUiKey(Option<String>, Option<Role>, KitSlot, usize);

fn rebuild(
    mut commands: Commands,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    mut ui: ResMut<DraftUi>,
    mut shown: ResMut<Shown>,
    roots: Query<Entity, With<DraftRoot>>,
) {
    let Some(draft) = state.draft.clone() else {
        return;
    };
    let me = state.player_id.clone();
    // le champion verrouillé devient la sélection
    if let Some(p) = draft
        .picks
        .iter()
        .find(|p| Some(&p.player_id) == me.as_ref())
        && p.locked
        && p.champion_id.is_some()
        && ui.selected != p.champion_id
    {
        ui.selected.clone_from(&p.champion_id);
    }
    let names: Vec<(String, String)> = state
        .lobby
        .iter()
        .flat_map(|l| l.players.iter().map(|p| (p.id.clone(), p.name.clone())))
        .collect();
    let key = DraftUiKey(ui.selected.clone(), ui.role, ui.focus, ui.summoner_slot);
    let sig = (draft, names, me, key);
    if roots.iter().next().is_some() && shown.0.as_ref() == Some(&sig) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    let (draft, names, me, _) = &sig;
    spawn(&mut commands, &fonts, draft, names, me.as_deref(), &ui);
    shown.0 = Some(sig);
}

fn name_of<'a>(names: &'a [(String, String)], id: &str) -> &'a str {
    names
        .iter()
        .find(|(p, _)| p == id)
        .map_or("Invocateur", |(_, n)| n.as_str())
}

fn spawn(
    commands: &mut Commands,
    f: &UiFonts,
    draft: &DraftState,
    names: &[(String, String)],
    me: Option<&str>,
    ui: &DraftUi,
) {
    let view = draft_view(draft, me);
    let my_pick = draft
        .picks
        .iter()
        .find(|p| Some(p.player_id.as_str()) == me);
    let champ = ui.selected.as_deref().and_then(get_champion);
    let accent = champ.map_or(TEAL, |c| css(&c.palette.primary));
    let turn: Vec<String> = if view.finalizing {
        vec![]
    } else {
        draft
            .turns
            .get(draft.current_turn as usize)
            .cloned()
            .unwrap_or_default()
    };
    commands
        .spawn((
            DraftRoot,
            screen_root(
                Screen::Draft,
                Node {
                    column_gap: px(20),
                    padding: UiRect::all(px(18)),
                    ..default()
                },
            ),
        ))
        .with_children(|root| {
            team_panel(root, f, Team::Blue, draft, view.my_team, &turn, me, names);
            // ---------------------------------------------------------------- centre
            root.spawn(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(14),
                ..default()
            })
            .with_children(|center| {
                center
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(20),
                        ..default()
                    })
                    .with_children(|head| {
                        head.spawn(Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            row_gap: px(4),
                            ..default()
                        })
                        .with_children(|phase| {
                            let small = if view.finalizing {
                                "FINALISATION".to_string()
                            } else {
                                format!(
                                    "PHASE DE SÉLECTION · TOUR {}/{}",
                                    draft.current_turn + 1,
                                    draft.turns.len()
                                )
                            };
                            phase.spawn((Text::new(small), f.text(12.0, 700), TextColor(GOLD)));
                            let c = if view.my_turn { GOLD_HI } else { TEXT };
                            phase.spawn((
                                Text::new(view.phase_text),
                                f.display(34.0, 900),
                                TextColor(c),
                            ));
                            phase
                                .spawn(Node {
                                    column_gap: px(6),
                                    ..default()
                                })
                                .with_children(|track| {
                                    for (i, t) in draft.turns.iter().enumerate() {
                                        let team = draft
                                            .picks
                                            .iter()
                                            .find(|p| t.contains(&p.player_id))
                                            .map_or(Team::Blue, |p| p.team);
                                        let current = draft.current_turn as usize;
                                        let alpha = if i < current {
                                            0.9
                                        } else if i == current {
                                            1.0
                                        } else {
                                            0.2
                                        };
                                        track.spawn((
                                            Node {
                                                width: px(14.0 * t.len() as f32 + 8.0),
                                                height: px(if i == current { 8 } else { 6 }),
                                                border_radius: BorderRadius::all(px(3)),
                                                ..default()
                                            },
                                            BackgroundColor(team_color(team).with_alpha(alpha)),
                                        ));
                                    }
                                });
                        });
                        // minuteur
                        head.spawn((
                            Node {
                                width: px(84),
                                height: px(84),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(4)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BorderColor::all(GOLD),
                            BackgroundColor(PANEL),
                        ))
                        .with_children(|ring| {
                            ring.spawn((
                                TimerText,
                                Text::new(""),
                                f.display(30.0, 900),
                                TextColor(TEXT),
                            ));
                            ring.spawn((
                                Node {
                                    width: px(44),
                                    height: px(3),
                                    border_radius: BorderRadius::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(hexa(0xffffff, 0.1)),
                            ))
                            .with_child((
                                TimerBar,
                                Node {
                                    width: percent(100),
                                    height: percent(100),
                                    border_radius: BorderRadius::all(px(2)),
                                    ..default()
                                },
                                BackgroundColor(GOLD),
                            ));
                        });
                    });
                // fiche du champion
                center
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::all(px(20)),
                            row_gap: px(10),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(12)),
                            ..default()
                        },
                        BackgroundGradient::from(RadialGradient {
                            stops: vec![
                                ColorStop::new(accent.with_alpha(0.22), percent(0)),
                                ColorStop::new(accent.with_alpha(0.0), percent(75)),
                            ],
                            shape: RadialGradientShape::FarthestSide,
                            ..default()
                        }),
                        BorderColor::all(hexa(0xffffff, 0.06)),
                    ))
                    .with_children(|stage| match champ {
                        Some(c) => {
                            if let Some(e) = ui.stage {
                                stage.spawn((
                                    StageView(e),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        right: px(0),
                                        top: px(0),
                                        bottom: px(0),
                                        width: percent(62),
                                        ..default()
                                    },
                                ));
                            }
                            champion_info(stage, f, c, ui.focus);
                        }
                        None => {
                            stage.spawn((
                                Text::new("Choisis un champion dans la galerie"),
                                f.text(18.0, 600),
                                TextColor(MUTED),
                                Node {
                                    align_self: AlignSelf::Center,
                                    ..default()
                                },
                            ));
                        }
                    });
                // dock : filtres, galerie, verrouillage
                center
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        ..default()
                    })
                    .with_children(|dock| {
                        dock.spawn(Node {
                            column_gap: px(8),
                            ..default()
                        })
                        .with_children(|chips| {
                            let roles = [
                                None,
                                Some(Role::Top),
                                Some(Role::Jungle),
                                Some(Role::Mid),
                                Some(Role::Bot),
                            ];
                            for r in roles {
                                let on = ui.role == r;
                                chips
                                    .spawn((
                                        button(
                                            ButtonLook::new(
                                                if on { hexa(0xd9b25f, 0.25) } else { PANEL },
                                                hexa(0xd9b25f, 0.3),
                                            ),
                                            Node {
                                                padding: UiRect::axes(px(14), px(6)),
                                                border: UiRect::all(px(1)),
                                                border_radius: BorderRadius::all(px(14)),
                                                ..default()
                                            },
                                        ),
                                        BorderColor::all(if on {
                                            GOLD
                                        } else {
                                            hexa(0xd9b25f, 0.25)
                                        }),
                                        Act::Filter(r),
                                    ))
                                    .with_child((
                                        Text::new(r.map_or("Tous", role_label)),
                                        f.text(13.0, 700),
                                        TextColor(TEXT),
                                    ));
                            }
                        });
                        dock.spawn(Node {
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(8),
                            row_gap: px(8),
                            ..default()
                        })
                        .with_children(|gallery| {
                            for c in CHAMPIONS
                                .iter()
                                .filter(|c| ui.role.is_none_or(|r| c.roles.contains(&r)))
                            {
                                gallery_tile(gallery, f, c, draft, view.my_team, me, names, ui);
                            }
                        });
                        if view.finalizing || my_pick.is_some_and(|p| p.locked) {
                            if let Some(p) = my_pick {
                                summoner_picker(dock, f, p.summoners, ui.summoner_slot);
                            }
                        } else {
                            let free = ui
                                .selected
                                .as_deref()
                                .is_some_and(|id| taken_by(draft, id).is_none());
                            let sub = if !view.my_turn {
                                "Ce n’est pas encore ton tour".to_string()
                            } else if let Some(c) = champ {
                                format!("{} entre dans la Faille", c.name)
                            } else {
                                "Choisis un champion".to_string()
                            };
                            let mut lock = dock.spawn((
                                button(
                                    ButtonLook::new(GOLD, GOLD_HI),
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::axes(px(20), px(10)),
                                        border_radius: BorderRadius::all(px(10)),
                                        ..default()
                                    },
                                ),
                                Act::Lock,
                            ));
                            lock.with_children(|b| {
                                let label = if view.my_turn {
                                    "VERROUILLER"
                                } else {
                                    "PATIENCE…"
                                };
                                b.spawn((
                                    Text::new(label),
                                    f.display(24.0, 900),
                                    TextColor(Color::srgb_u8(0x1a, 0x10, 0x02)),
                                ));
                                b.spawn((
                                    Text::new(sub),
                                    f.text(12.0, 700),
                                    TextColor(hexa(0x281802, 0.8)),
                                ));
                            });
                            if !(view.my_turn && free) {
                                lock.insert(bevy::ui::InteractionDisabled);
                            }
                        }
                    });
            });
            team_panel(root, f, Team::Red, draft, view.my_team, &turn, me, names);
        });
}

#[allow(clippy::too_many_arguments)] // reprend les props du composant `TeamPanel`
fn team_panel(
    parent: &mut ChildSpawnerCommands,
    f: &UiFonts,
    team: Team,
    draft: &DraftState,
    my_team: Team,
    turn: &[String],
    me: Option<&str>,
    names: &[(String, String)],
) {
    let ally = team == my_team;
    parent
        .spawn(Node {
            width: px(250),
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        })
        .with_children(|panel| {
            panel.spawn((
                Text::new(team_name(team).to_uppercase()),
                f.display(17.0, 800),
                TextColor(team_color(team)),
            ));
            for p in draft.picks.iter().filter(|p| p.team == team) {
                let show = if p.locked {
                    p.champion_id.as_deref()
                } else if ally {
                    p.hover_champion_id.as_deref()
                } else {
                    None
                };
                let c = show.and_then(get_champion);
                let active = turn.contains(&p.player_id) && !p.locked;
                let mine = Some(p.player_id.as_str()) == me;
                let color = c.map_or(hexa(0x333344, 1.0), |c| css(&c.palette.primary));
                let border = if active {
                    GOLD_HI
                } else if mine {
                    GOLD
                } else {
                    hexa(0xffffff, 0.08)
                };
                panel
                    .spawn((
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: px(10),
                            padding: UiRect::all(px(8)),
                            border: UiRect::all(px(if active { 2 } else { 1 })),
                            border_radius: BorderRadius::all(px(8)),
                            ..default()
                        },
                        BackgroundColor(team_color(team).with_alpha(if p.locked {
                            0.18
                        } else {
                            0.07
                        })),
                        BorderColor::all(border),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                width: px(52),
                                height: px(52),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                            BackgroundColor(color.with_alpha(if p.locked { 1.0 } else { 0.45 })),
                        ))
                        .with_child((
                            Text::new(c.map_or("?".into(), |c| {
                                c.name.chars().take(1).collect::<String>()
                            })),
                            f.display(26.0, 900),
                            TextColor(Color::WHITE),
                        ));
                        row.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            ..default()
                        })
                        .with_children(|info| {
                            info.spawn((
                                Text::new(name_of(names, &p.player_id)),
                                f.text(12.0, 600),
                                TextColor(MUTED),
                            ));
                            let label = c.map_or(if active { "Choisit…" } else { "—" }, |c| {
                                c.name.as_str()
                            });
                            info.spawn((Text::new(label), f.text(16.0, 800), TextColor(TEXT)));
                            if let Some(c) = c {
                                info.spawn((
                                    Text::new(class_label(c.class)),
                                    f.text(11.0, 500),
                                    TextColor(MUTED),
                                ));
                            }
                        });
                        if ally {
                            let sums = format!(
                                "{}\n{}",
                                summoner(p.summoners[0]).name,
                                summoner(p.summoners[1]).name
                            );
                            row.spawn((Text::new(sums), f.text(10.0, 600), TextColor(GOLD)));
                        }
                    });
            }
        });
}

#[allow(clippy::too_many_arguments)] // une vignette dépend de tout le draft
fn gallery_tile(
    parent: &mut ChildSpawnerCommands,
    f: &UiFonts,
    c: &ChampionDef,
    draft: &DraftState,
    my_team: Team,
    me: Option<&str>,
    names: &[(String, String)],
    ui: &DraftUi,
) {
    let taken = taken_by(draft, &c.id);
    let mine = taken.is_some_and(|p| Some(p.player_id.as_str()) == me);
    let blocked = taken.is_some() && !mine;
    let hover = draft.picks.iter().find(|p| {
        p.team == my_team
            && !p.locked
            && p.hover_champion_id.as_deref() == Some(c.id.as_str())
            && Some(p.player_id.as_str()) != me
    });
    let color = css(&c.palette.primary);
    let selected = ui.selected.as_deref() == Some(c.id.as_str());
    let border = if selected {
        GOLD_HI
    } else if let Some(t) = taken {
        team_color(t.team)
    } else {
        hexa(0xffffff, 0.12)
    };
    let mut tile = parent.spawn((
        button(
            ButtonLook::new(
                color.with_alpha(if blocked { 0.15 } else { 0.4 }),
                color.with_alpha(0.7),
            ),
            Node {
                width: px(78),
                height: px(92),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::End,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(px(4)),
                border: UiRect::all(px(if selected { 3 } else { 1 })),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
        ),
        BorderColor::all(border),
        Act::Tile(c.id.clone()),
    ));
    tile.with_children(|t| {
        let initial: String = c.name.chars().take(1).collect();
        let tint = if blocked {
            hexa(0xffffff, 0.3)
        } else {
            Color::WHITE
        };
        t.spawn((
            Text::new(initial),
            f.display(34.0, 900),
            TextColor(tint),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ));
        t.spawn((
            Text::new(c.name.clone()),
            f.text(11.0, 700),
            TextColor(tint),
        ));
        if blocked {
            t.spawn((
                Text::new("PRIS"),
                f.text(9.0, 800),
                TextColor(GOLD),
                BackgroundColor(hexa(0x05060f, 0.85)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(3),
                    padding: UiRect::axes(px(4), px(1)),
                    ..default()
                },
            ));
        }
        if let Some(h) = hover {
            t.spawn((
                Text::new(name_of(names, &h.player_id).to_string()),
                f.text(9.0, 700),
                TextColor(TEAL),
                BackgroundColor(hexa(0x05060f, 0.85)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(3),
                    padding: UiRect::axes(px(4), px(1)),
                    ..default()
                },
            ));
        }
    });
    if blocked {
        tile.insert(bevy::ui::InteractionDisabled);
    }
}

fn champion_info(parent: &mut ChildSpawnerCommands, f: &UiFonts, c: &ChampionDef, focus: KitSlot) {
    parent.spawn((
        Text::new(class_label(c.class).to_uppercase()),
        f.text(13.0, 700),
        TextColor(GOLD),
    ));
    parent.spawn((
        Text::new(c.name.clone()),
        f.display(48.0, 900),
        TextColor(TEXT),
    ));
    parent.spawn((
        Text::new(c.title.clone()),
        f.text(16.0, 500),
        TextColor(MUTED),
    ));
    let mut tags: Vec<String> = c.roles.iter().map(|r| role_label(*r).to_string()).collect();
    tags.push(if c.ranged { "Distance" } else { "Mêlée" }.into());
    let diff = usize::from(c.difficulty.min(3));
    tags.push(format!("{}{}", "◆".repeat(diff), "◇".repeat(3 - diff)));
    parent.spawn((
        Text::new(tags.join("   ·   ")),
        f.text(13.0, 600),
        TextColor(GOLD_HI),
    ));
    parent
        .spawn(Node {
            column_gap: px(18),
            ..default()
        })
        .with_children(|bars| {
            for (label, v) in stat_bars(c) {
                bars.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    ..default()
                })
                .with_children(|b| {
                    b.spawn((Text::new(label), f.text(11.0, 600), TextColor(MUTED)));
                    b.spawn((
                        Node {
                            width: px(110),
                            height: px(6),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(hexa(0xffffff, 0.08)),
                    ))
                    .with_child((
                        Node {
                            width: percent(v * 100.0),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(css(&c.palette.primary)),
                    ));
                });
            }
        });
    parent
        .spawn(Node {
            column_gap: px(8),
            ..default()
        })
        .with_children(|kit| {
            for slot in KIT {
                let key = match slot {
                    KitSlot::P => "•",
                    KitSlot::Ability(s) => slot_key(s),
                };
                let on = focus == slot;
                kit.spawn((
                    button(
                        ButtonLook::new(
                            if on { hexa(0xd9b25f, 0.25) } else { PANEL },
                            hexa(0xd9b25f, 0.3),
                        ),
                        Node {
                            width: px(42),
                            height: px(42),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                    ),
                    BorderColor::all(if on { GOLD_HI } else { hexa(0xd9b25f, 0.3) }),
                    Act::Focus(slot),
                ))
                .with_child((
                    Text::new(key),
                    f.display(18.0, 800),
                    TextColor(TEXT),
                ));
            }
        });
    let a = match focus {
        KitSlot::P => &c.passive,
        KitSlot::Ability(s) => c.abilities.get(s),
    };
    parent.spawn((
        Text::new(a.name.clone()),
        f.text(16.0, 800),
        TextColor(GOLD_HI),
    ));
    let mut meta = Vec::new();
    if a.cooldown.first().is_some_and(|v| *v > 0.0) {
        meta.push(format!("Récupération {}", ranks(&a.cooldown, " s")));
    }
    if a.cost.iter().any(|v| *v > 0.0) {
        meta.push(format!("Coût {}", ranks(&a.cost, "")));
    }
    if !meta.is_empty() {
        parent.spawn((
            Text::new(meta.join("   ·   ")),
            f.text(12.0, 600),
            TextColor(MUTED),
        ));
    }
    parent.spawn((
        Text::new(a.description.clone()),
        f.text(14.0, 500),
        TextColor(TEXT),
        Node {
            max_width: px(720),
            ..default()
        },
    ));
    parent.spawn((
        Text::new(c.lore.clone()),
        f.text(12.0, 400),
        TextColor(hexa(0xece7da, 0.55)),
        Node {
            max_width: px(720),
            ..default()
        },
    ));
}

fn slot_key(s: AbilitySlot) -> &'static str {
    match s {
        AbilitySlot::Q => "Q",
        AbilitySlot::W => "W",
        AbilitySlot::E => "E",
        AbilitySlot::R => "R",
    }
}

/// Valeurs par rang : « 70 » si toutes égales, sinon « 6 / 5,5 / 5 » (`formatRanks`).
pub fn ranks(values: &[f64], unit: &str) -> String {
    let fmt = |v: f64| {
        let r = (v * 100.0).round() / 100.0;
        format!("{r}").replace('.', ",")
    };
    if values.windows(2).all(|w| w[0] == w[1]) {
        values
            .first()
            .map_or(String::new(), |v| format!("{}{unit}", fmt(*v)))
    } else {
        format!(
            "{}{unit}",
            values
                .iter()
                .map(|v| fmt(*v))
                .collect::<Vec<_>>()
                .join(" / ")
        )
    }
}

fn summoner_picker(
    parent: &mut ChildSpawnerCommands,
    f: &UiFonts,
    cur: [SummonerId; 2],
    slot: usize,
) {
    parent
        .spawn(Node {
            column_gap: px(16),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            for (i, key) in ["D", "F"].into_iter().enumerate() {
                let on = slot == i;
                row.spawn((
                    button(
                        ButtonLook::new(
                            if on { hexa(0xd9b25f, 0.25) } else { PANEL },
                            hexa(0xd9b25f, 0.3),
                        ),
                        Node {
                            width: px(110),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(px(8)),
                            border: UiRect::all(px(2)),
                            border_radius: BorderRadius::all(px(8)),
                            ..default()
                        },
                    ),
                    BorderColor::all(if on { GOLD_HI } else { hexa(0xd9b25f, 0.3) }),
                    Act::SummonerSlot(i),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(key), f.display(16.0, 800), TextColor(GOLD)));
                    b.spawn((
                        Text::new(summoner(cur[i]).name.clone()),
                        f.text(13.0, 700),
                        TextColor(TEXT),
                    ));
                });
            }
            row.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6),
                row_gap: px(6),
                max_width: px(620),
                ..default()
            })
            .with_children(|opts| {
                for id in SUMMONER_IDS {
                    let used = cur.contains(&id);
                    opts.spawn((
                        button(
                            ButtonLook::new(
                                if used { hexa(0xd9b25f, 0.18) } else { PANEL },
                                hexa(0xd9b25f, 0.3),
                            ),
                            Node {
                                padding: UiRect::axes(px(10), px(6)),
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(6)),
                                ..default()
                            },
                        ),
                        BorderColor::all(hexa(0xd9b25f, if used { 0.6 } else { 0.2 })),
                        Act::Summoner(id),
                    ))
                    .with_child((
                        Text::new(summoner(id).name.clone()),
                        f.text(12.0, 600),
                        TextColor(TEXT),
                    ));
                }
            });
        });
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    state: Res<GameState>,
    time: Res<Time>,
    mut ui: ResMut<DraftUi>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    let Some(draft) = &state.draft else { return };
    let me = state.player_id.as_deref();
    let view = draft_view(draft, me);
    match act.clone() {
        Act::Filter(r) => ui.role = r,
        Act::Focus(s) => ui.focus = s,
        Act::SummonerSlot(s) => ui.summoner_slot = s,
        Act::Summoner(id) => {
            let Some(p) = draft
                .picks
                .iter()
                .find(|p| Some(p.player_id.as_str()) == me)
            else {
                return;
            };
            let slot = ui.summoner_slot;
            conn.send(ClientMsg::DraftSummoners {
                summoners: choose_summoner(p.summoners, slot, id),
            });
            ui.summoner_slot = 1 - slot;
        }
        Act::Tile(id) => {
            if taken_by(draft, &id).is_some_and(|p| Some(p.player_id.as_str()) != me) {
                return;
            }
            let now = time.elapsed_secs();
            let double = ui
                .last_click
                .as_ref()
                .is_some_and(|(last, at)| *last == id && now - at < DOUBLE_CLICK);
            ui.last_click = Some((id.clone(), now));
            ui.selected = Some(id.clone());
            ui.focus = KitSlot::P;
            if view.my_turn {
                conn.send(ClientMsg::DraftHover {
                    champion_id: id.clone(),
                });
                if double && taken_by(draft, &id).is_none() {
                    conn.send(ClientMsg::DraftLock { champion_id: id });
                }
            }
        }
        Act::Lock => {
            if let Some(id) = ui.selected.clone()
                && view.my_turn
                && taken_by(draft, &id).is_none()
            {
                conn.send(ClientMsg::DraftLock { champion_id: id });
            }
        }
    }
}

fn tick_timer(
    state: Res<GameState>,
    mut texts: Query<(&mut Text, &mut TextColor), With<TimerText>>,
    mut bars: Query<(&mut Node, &mut BackgroundColor), With<TimerBar>>,
) {
    let Some(draft) = &state.draft else { return };
    let finalizing = draft.current_turn as usize >= draft.turns.len();
    let (ends_at, total) = if finalizing {
        (draft.finalize_ends_at, FINALIZE_SECONDS)
    } else {
        (Some(draft.turn_ends_at), DRAFT_TURN_SECONDS)
    };
    let left = remaining(&state, ends_at);
    let warn = left <= 5.0;
    let color = if warn { crate::ui::theme::DANGER } else { GOLD };
    for (mut text, mut c) in &mut texts {
        let v = format!("{}", left.ceil() as u64);
        if text.0 != v {
            text.0 = v;
        }
        c.0 = if warn { color } else { TEXT };
    }
    for (mut node, mut bg) in &mut bars {
        node.width = percent((left / total).clamp(0.0, 1.0) * 100.0);
        bg.0 = color;
    }
}

fn reset(mut commands: Commands, mut ui: ResMut<DraftUi>, mut images: ResMut<Assets<Image>>) {
    *ui = DraftUi::default();
    ui.stage = Some(ChampionStage::spawn(
        &mut commands,
        &mut images,
        Screen::Draft,
        0.25,
    ));
}

/// Champion de la scène, éclat d'ultime au verrouillage (pas à l'arrivée sur un choix déjà verrouillé).
fn drive_stage(
    state: Res<GameState>,
    mut ui: ResMut<DraftUi>,
    mut stages: Query<&mut ChampionStage>,
) {
    let locked = state.draft.as_ref().is_some_and(|d| {
        d.picks.iter().any(|p| {
            Some(&p.player_id) == state.player_id.as_ref() && p.locked && p.champion_id.is_some()
        })
    });
    if ui.was_locked != Some(locked) {
        if ui.was_locked == Some(false) && locked {
            ui.pulse += 1;
        }
        ui.was_locked = Some(locked);
    }
    let Some(mut stage) = ui.stage.and_then(|e| stages.get_mut(e).ok()) else {
        return;
    };
    if stage.champion != ui.selected {
        stage.champion.clone_from(&ui.selected);
    }
    if stage.pulse != ui.pulse {
        stage.pulse = ui.pulse;
    }
}

pub struct DraftPlugin;

impl Plugin for DraftPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DraftUi>()
            .init_resource::<Shown>()
            .add_systems(OnEnter(Screen::Draft), reset)
            .add_systems(
                Update,
                (rebuild, tick_timer, drive_stage)
                    .chain()
                    .run_if(in_state(Screen::Draft)),
            )
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(id: &str, team: Team, locked: Option<&str>) -> DraftPick {
        DraftPick {
            player_id: id.into(),
            team,
            champion_id: locked.map(Into::into),
            hover_champion_id: None,
            locked: locked.is_some(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
        }
    }

    fn draft(current: u32) -> DraftState {
        DraftState {
            turns: vec![
                vec!["a".into()],
                vec!["b".into(), "c".into()],
                vec!["d".into()],
            ],
            current_turn: current,
            turn_ends_at: 0.0,
            picks: vec![
                pick("a", Team::Blue, None),
                pick("b", Team::Red, None),
                pick("c", Team::Red, None),
                pick("d", Team::Blue, None),
            ],
            finalize_ends_at: None,
        }
    }

    #[test]
    fn texte_de_phase_selon_le_tour() {
        assert_eq!(
            draft_view(&draft(0), Some("a")).phase_text,
            "À toi de choisir !"
        );
        assert_eq!(
            draft_view(&draft(0), Some("d")).phase_text,
            "Ton équipe choisit…"
        );
        assert_eq!(
            draft_view(&draft(1), Some("a")).phase_text,
            "L’équipe adverse choisit…"
        );
        let v = draft_view(&draft(3), Some("a"));
        assert!(v.finalizing && !v.my_turn);
        assert_eq!(v.phase_text, "Prépare-toi au combat");
        // verrouillé : ce n'est plus son tour
        let mut d = draft(0);
        d.picks[0].locked = true;
        assert!(!draft_view(&d, Some("a")).my_turn);
    }

    #[test]
    fn choisir_un_sort_deja_pris_echange_les_emplacements() {
        use SummonerId::{Flash, Heal, Ignite};
        assert_eq!(choose_summoner([Flash, Ignite], 0, Heal), [Heal, Ignite]);
        assert_eq!(choose_summoner([Flash, Ignite], 0, Ignite), [Ignite, Flash]);
        assert_eq!(choose_summoner([Flash, Ignite], 1, Flash), [Ignite, Flash]);
    }

    #[test]
    fn barres_de_stats_bornees() {
        for c in CHAMPIONS.iter() {
            for (_, v) in stat_bars(c) {
                assert!((0.08..=1.0).contains(&v), "{} : {v}", c.id);
            }
        }
    }

    #[test]
    fn valeurs_par_rang() {
        assert_eq!(ranks(&[70.0, 70.0, 70.0], ""), "70");
        assert_eq!(ranks(&[6.0, 5.5, 5.0], " s"), "6 / 5,5 / 5 s");
    }
}
