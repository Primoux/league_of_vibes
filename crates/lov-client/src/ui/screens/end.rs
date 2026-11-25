//! Fin de partie : portage de `packages/client/src/ui/screens/EndScreen.tsx`.
//!
//! Victoire ou défaite, durée, tableaux des deux équipes (KDA et ratio, CS, or, barre des dégâts aux
//! champions, objets), MVP, retour au salon ou départ de la salle.
//!
//! Écart : les objets sont listés par leur nom (les icônes d'objets arrivent avec le HUD, area `ui-hud`).

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::item;
use lov_shared::protocol::{ClientMsg, GameOver, PlayerStats};
use lov_shared::types::Team;

use super::rift::RiftBackground;
use super::{screen_root, team_color, team_name};
use crate::net::{Connection, GameState};
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::format::{
    format_clock, format_gold_short, format_int, format_kda_ratio, mvp_score,
};
use crate::ui::theme::{DANGER, GOLD, GOLD_HI, MUTED, PANEL, TEXT, hex, hexa};
use crate::ui::{ButtonLook, Screen, UiFonts, button};

/// Joueur MVP : meilleur `mvpScore` (le premier en cas d'égalité, comme le tri stable du web).
pub fn mvp(result: &GameOver) -> Option<&str> {
    let mut best: Option<(&PlayerStats, f64)> = None;
    for r in &result.stats {
        let s = mvp_score(r.kills, r.deaths, r.assists, r.cs, r.damage_to_champions);
        if best.is_none_or(|(_, b)| s > b) {
            best = Some((r, s));
        }
    }
    best.map(|(r, _)| r.player_id.as_str())
}

#[derive(Component, Clone, Copy)]
enum Act {
    Back,
    Leave,
}

fn on_enter(mut commands: Commands, state: Res<GameState>, fonts: Res<UiFonts>) {
    let Some(result) = state.result.clone() else {
        return;
    };
    let f = &*fonts;
    let me = state.player_id.clone();
    let my_team = state
        .game
        .as_ref()
        .map(|g| g.team)
        .or_else(|| {
            result
                .stats
                .iter()
                .find(|r| Some(&r.player_id) == me.as_ref())
                .map(|r| r.team)
        })
        .unwrap_or(Team::Blue);
    let won = result.winner == my_team;
    let names: Vec<(String, String)> = state
        .lobby
        .iter()
        .flat_map(|l| l.players.iter().map(|p| (p.id.clone(), p.name.clone())))
        .chain(
            state
                .game
                .iter()
                .flat_map(|g| g.players.iter().map(|p| (p.id.clone(), p.name.clone()))),
        )
        .collect();
    let max_dmg = result
        .stats
        .iter()
        .map(|r| r.damage_to_champions)
        .fold(1.0, f64::max);
    let mvp_id = mvp(&result).map(str::to_owned);
    let accent = if won { GOLD_HI } else { DANGER };
    commands
        .spawn(screen_root(
            Screen::End,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(18),
                padding: UiRect::all(px(24)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(
                RiftBackground::new(if won { hex(0xf6dc94) } else { hex(0xff5a5a) }, 1.2).bundle(),
            );
            root.spawn((
                Text::new(if won { "VICTOIRE" } else { "DÉFAITE" }),
                f.display(96.0, 900),
                TextColor(accent),
            ));
            let sub = if won {
                "Le nexus ennemi s’est effondré."
            } else {
                "Votre nexus a été détruit."
            };
            root.spawn((
                Text::new(format!("{sub} · Durée {}", format_clock(result.duration))),
                f.text(16.0, 500),
                TextColor(MUTED),
            ));
            for team in [Team::Blue, Team::Red] {
                let rows: Vec<&PlayerStats> =
                    result.stats.iter().filter(|r| r.team == team).collect();
                let kills: u32 = rows.iter().map(|r| r.kills).sum();
                let gold: f64 = rows.iter().map(|r| r.gold).sum();
                let winner = result.winner == team;
                root.spawn((
                    Node {
                        width: px(1180),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        padding: UiRect::all(px(12)),
                        border: UiRect::all(px(if winner { 2 } else { 1 })),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(team_color(team).with_alpha(if winner { 0.9 } else { 0.35 })),
                ))
                .with_children(|table| {
                    table
                        .spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|h| {
                            let crown = if winner { "  ★" } else { "" };
                            h.spawn((
                                Text::new(format!("{}{crown}", team_name(team))),
                                f.display(20.0, 800),
                                TextColor(team_color(team)),
                            ));
                            h.spawn((
                                Text::new(format!(
                                    "{kills} éliminations · {} or",
                                    format_gold_short(gold)
                                )),
                                f.text(13.0, 600),
                                TextColor(MUTED),
                            ));
                        });
                    header(table, f);
                    for r in rows {
                        let name = names
                            .iter()
                            .find(|(id, _)| *id == r.player_id)
                            .map_or("Invocateur", |(_, n)| n.as_str());
                        row(
                            table,
                            f,
                            r,
                            name,
                            Some(&r.player_id) == me.as_ref(),
                            Some(&r.player_id) == mvp_id.as_ref(),
                            max_dmg,
                        );
                    }
                });
            }
            root.spawn(Node {
                column_gap: px(16),
                margin: UiRect::top(px(8)),
                ..default()
            })
            .with_children(|actions| {
                actions
                    .spawn((
                        button(
                            ButtonLook::new(hexa(0xffffff, 0.04), hexa(0xffffff, 0.1)),
                            Node {
                                padding: UiRect::axes(px(26), px(14)),
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::all(px(10)),
                                ..default()
                            },
                        ),
                        BorderColor::all(hexa(0xd9b25f, 0.35)),
                        Act::Leave,
                    ))
                    .with_child((
                        Text::new("Quitter la salle"),
                        f.text(17.0, 700),
                        TextColor(TEXT),
                    ));
                actions
                    .spawn((
                        button(
                            ButtonLook::new(GOLD, GOLD_HI),
                            Node {
                                padding: UiRect::axes(px(30), px(14)),
                                border_radius: BorderRadius::all(px(10)),
                                ..default()
                            },
                        ),
                        Act::Back,
                    ))
                    .with_child((
                        Text::new("Retour au salon"),
                        f.text(17.0, 800),
                        TextColor(Color::srgb_u8(0x1a, 0x10, 0x02)),
                    ));
            });
        });
}

/// Largeurs des colonnes : champion, K/D/A, CS, or, dégâts, objets.
const COLS: [f32; 6] = [250.0, 130.0, 60.0, 80.0, 250.0, 380.0];

fn header(table: &mut ChildSpawnerCommands, f: &UiFonts) {
    table
        .spawn(Node {
            column_gap: px(8),
            ..default()
        })
        .with_children(|h| {
            for (label, w) in [
                "Champion",
                "K / D / A",
                "CS",
                "Or",
                "Dégâts aux champions",
                "Objets",
            ]
            .into_iter()
            .zip(COLS)
            {
                h.spawn((
                    Text::new(label),
                    f.text(11.0, 700),
                    TextColor(GOLD),
                    Node {
                        width: px(w),
                        ..default()
                    },
                ));
            }
        });
}

fn row(
    table: &mut ChildSpawnerCommands,
    f: &UiFonts,
    r: &PlayerStats,
    name: &str,
    me: bool,
    is_mvp: bool,
    max_dmg: f64,
) {
    let champ = get_champion(&r.champion_id).map_or(r.champion_id.clone(), |c| c.name.clone());
    table
        .spawn((
            Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(0), px(4)),
                ..default()
            },
            BackgroundColor(if me { hexa(0xd9b25f, 0.1) } else { Color::NONE }),
        ))
        .with_children(|row| {
            row.spawn(Node {
                width: px(COLS[0]),
                column_gap: px(10),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(ChampionPortrait::new(&r.champion_id, PortraitKind::Round).sized(36.0));
                cell.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|who| {
                    let badge = if is_mvp { "   MVP" } else { "" };
                    who.spawn((
                        Text::new(format!("{name}{badge}")),
                        f.text(14.0, 800),
                        TextColor(if is_mvp { GOLD_HI } else { TEXT }),
                    ));
                    who.spawn((Text::new(champ), f.text(11.0, 500), TextColor(MUTED)));
                });
            });
            row.spawn(Node {
                width: px(COLS[1]),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|kda| {
                kda.spawn((
                    Text::new(format!("{} / {} / {}", r.kills, r.deaths, r.assists)),
                    f.text(14.0, 700),
                    TextColor(TEXT),
                ));
                kda.spawn((
                    Text::new(format_kda_ratio(r.kills, r.deaths, r.assists)),
                    f.text(11.0, 500),
                    TextColor(MUTED),
                ));
            });
            row.spawn((
                Text::new(r.cs.to_string()),
                f.text(14.0, 600),
                TextColor(TEXT),
                Node {
                    width: px(COLS[2]),
                    ..default()
                },
            ));
            row.spawn((
                Text::new(format_gold_short(r.gold)),
                f.text(14.0, 600),
                TextColor(GOLD),
                Node {
                    width: px(COLS[3]),
                    ..default()
                },
            ));
            row.spawn(Node {
                width: px(COLS[4]),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            })
            .with_children(|dmg| {
                dmg.spawn((
                    Text::new(format_int(r.damage_to_champions)),
                    f.text(12.0, 600),
                    TextColor(TEXT),
                ));
                dmg.spawn((
                    Node {
                        width: percent(100),
                        height: px(5),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(hexa(0xffffff, 0.08)),
                ))
                .with_child((
                    Node {
                        width: percent(r.damage_to_champions / max_dmg * 100.0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(hexa(0xff7a45, 1.0)),
                ));
            });
            let items: Vec<String> = r
                .items
                .iter()
                .take(7)
                .flatten()
                .map(|id| item(id).map_or(id.clone(), |i| i.name.clone()))
                .collect();
            row.spawn((
                Text::new(if items.is_empty() {
                    "—".into()
                } else {
                    items.join(" · ")
                }),
                f.text(11.0, 500),
                TextColor(MUTED),
                Node {
                    width: px(COLS[5]),
                    ..default()
                },
            ));
        });
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    mut state: ResMut<GameState>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    match act {
        Act::Back => conn.send(ClientMsg::ReturnToLobby),
        Act::Leave => {
            conn.send(ClientMsg::LeaveRoom);
            state.lobby = None;
        }
    }
    state.game = None;
    state.result = None;
    state.draft = None;
    state.my_entity = None;
}

pub struct EndPlugin;

impl Plugin for EndPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::End), on_enter)
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(id: &str, k: u32, d: u32, a: u32) -> PlayerStats {
        PlayerStats {
            player_id: id.into(),
            champion_id: "grondak".into(),
            team: Team::Blue,
            kills: k,
            deaths: d,
            assists: a,
            cs: 0,
            gold: 0.0,
            damage_to_champions: 0.0,
            items: vec![],
        }
    }

    #[test]
    fn le_mvp_a_le_meilleur_score() {
        let r = GameOver {
            winner: Team::Blue,
            duration: 600.0,
            stats: vec![
                stats("a", 1, 3, 2),
                stats("b", 5, 1, 4),
                stats("c", 5, 1, 4),
            ],
        };
        assert_eq!(mvp(&r), Some("b"));
        assert_eq!(mvp(&GameOver { stats: vec![], ..r }), None);
    }
}
