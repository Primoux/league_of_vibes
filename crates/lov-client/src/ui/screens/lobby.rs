//! Salon de partie : portage de `packages/client/src/ui/screens/Lobby.tsx`.
//!
//! Code d'invitation (cliquer copie), bannières des deux équipes (une place libre de l'autre équipe la
//! rejoint), rôle préféré, jauge des joueurs prêts, bouton Prêt et, pour l'hôte, Lancer. L'écran est
//! reconstruit à chaque changement de l'état du salon.

use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lov_shared::constants::TEAM_SIZE;
use lov_shared::protocol::{ClientMsg, LobbyPlayer, LobbyState};
use lov_shared::types::{Role, Team};

use super::rift::RiftBackground;
use super::{screen_root, team_color, team_name};
use crate::net::{Connection, GameState};
use crate::ui::logic::format::role_label;
use crate::ui::theme::{DANGER, GOLD, GOLD_HI, GREEN, MUTED, PANEL, TEAL, TEXT, hex, hexa};
use crate::ui::{ButtonLook, Screen, UiFonts, button};

const ROLES: [Role; 4] = [Role::Top, Role::Jungle, Role::Mid, Role::Bot];

fn role_hint(r: Role) -> &'static str {
    match r {
        Role::Top => "Duelliste solitaire",
        Role::Jungle => "Maître des camps",
        Role::Mid => "Cœur de la Faille",
        Role::Bot => "Tireur ou soutien",
    }
}

/// Ce que l'écran déduit de l'état du salon (testé).
#[derive(Debug, PartialEq)]
pub struct LobbyView {
    pub can_start: bool,
    pub status: String,
}

pub fn lobby_view(lobby: &LobbyState, me: Option<&str>) -> LobbyView {
    let total = lobby.players.len();
    let me = lobby.players.iter().find(|p| Some(p.id.as_str()) == me);
    let ready = lobby.players.iter().filter(|p| p.ready).count();
    let all_ready = total > 0 && lobby.players.iter().all(|p| p.ready || !p.connected);
    let full = total == (TEAM_SIZE * 2) as usize;
    let can_start = me.is_some_and(|p| p.is_host) && all_ready && (full || lobby.dev_mode);
    let status = if can_start {
        "Tout le monde est prêt !".into()
    } else if !full && !lobby.dev_mode {
        format!("En attente de joueurs… {total}/{}", TEAM_SIZE * 2)
    } else if !all_ready {
        format!("{ready}/{total} joueurs prêts")
    } else {
        "En attente de l’hôte".into()
    };
    LobbyView { can_start, status }
}

#[derive(Component, Clone)]
enum Act {
    Copy,
    Leave,
    Team(Team),
    Role(Role),
    Ready(bool),
    Start,
}

#[derive(Component)]
struct LobbyRoot;

/// Signature du dernier état affiché : l'écran est reconstruit quand elle change.
#[derive(Resource, Default)]
struct Shown(Option<(LobbyState, Option<String>, Option<String>, bool)>);

/// « Copié ! » affiché jusqu'à cet instant (s, horloge du jeu).
#[derive(Resource, Default)]
struct Copied(f32);

#[allow(clippy::too_many_arguments)] // un système Bevy lit autant de ressources que l'écran en affiche
fn rebuild(
    mut commands: Commands,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    time: Res<Time>,
    copied: Res<Copied>,
    mut shown: ResMut<Shown>,
    roots: Query<Entity, With<LobbyRoot>>,
) {
    let Some(lobby) = state.lobby.clone() else {
        return;
    };
    let is_copied = time.elapsed_secs() < copied.0;
    let sig = (
        lobby,
        state.player_id.clone(),
        state.error.clone(),
        is_copied,
    );
    if roots.iter().next().is_some() && shown.0.as_ref() == Some(&sig) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    let (lobby, me, error, is_copied) = &sig;
    spawn(
        &mut commands,
        &fonts,
        lobby,
        me.as_deref(),
        error.as_deref(),
        *is_copied,
    );
    shown.0 = Some(sig);
}

fn spawn(
    commands: &mut Commands,
    f: &UiFonts,
    lobby: &LobbyState,
    me: Option<&str>,
    error: Option<&str>,
    copied: bool,
) {
    let view = lobby_view(lobby, me);
    let self_player = lobby.players.iter().find(|p| Some(p.id.as_str()) == me);
    let ghost = ButtonLook::new(hexa(0xffffff, 0.04), hexa(0xffffff, 0.1));
    commands
        .spawn((
            LobbyRoot,
            screen_root(
                Screen::Lobby,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(vw(4), vh(3)),
                    row_gap: vh(3),
                    ..default()
                },
            ),
        ))
        .with_children(|root| {
            root.spawn(RiftBackground::new(hex(0x4ea2ff), 0.7).bundle());
            // ---------------------------------------------------------------- en-tête
            root.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(24),
                ..default()
            })
            .with_children(|head| {
                head.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|title| {
                    let badge = if lobby.dev_mode {
                        "   ENTRAÎNEMENT"
                    } else {
                        ""
                    };
                    title.spawn((
                        Text::new(format!("SALON DE PARTIE{badge}")),
                        f.text(13.0, 700),
                        TextColor(GOLD),
                    ));
                    title.spawn((
                        Text::new("La Faille t’attend"),
                        f.display(44.0, 900),
                        TextColor(TEXT),
                    ));
                });
                head.spawn((
                    button(
                        ButtonLook::new(PANEL, hexa(0x1a1d36, 0.95)),
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(28), px(8)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(8)),
                            ..default()
                        },
                    ),
                    BorderColor::all(GOLD),
                    Act::Copy,
                ))
                .with_children(|seal| {
                    seal.spawn((
                        Text::new("Code d’invitation"),
                        f.text(11.0, 600),
                        TextColor(MUTED),
                    ));
                    let spaced: String = lobby.code.chars().flat_map(|c| [c, ' ']).collect();
                    seal.spawn((
                        Text::new(spaced.trim_end().to_string()),
                        f.display(34.0, 900),
                        TextColor(GOLD_HI),
                    ));
                    let note = if copied {
                        "✓ Copié !"
                    } else {
                        "Cliquer pour copier"
                    };
                    seal.spawn((
                        Text::new(note),
                        f.text(11.0, 600),
                        TextColor(if copied { GREEN } else { MUTED }),
                    ));
                });
                head.spawn((
                    button(
                        ghost,
                        Node {
                            padding: UiRect::axes(px(18), px(10)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(8)),
                            ..default()
                        },
                    ),
                    BorderColor::all(hexa(0xd9b25f, 0.3)),
                    Act::Leave,
                ))
                .with_child((
                    Text::new("Quitter"),
                    f.text(15.0, 700),
                    TextColor(TEXT),
                ));
            });
            // ---------------------------------------------------------------- équipes
            root.spawn(Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(36),
                ..default()
            })
            .with_children(|arena| {
                banners(arena, f, Team::Blue, &lobby.players, me);
                arena.spawn((
                    Text::new("VS"),
                    f.display(56.0, 900),
                    TextColor(hexa(0xd9b25f, 0.6)),
                ));
                banners(arena, f, Team::Red, &lobby.players, me);
            });
            // ---------------------------------------------------------------- pied : rôle, prêt
            root.spawn(Node {
                align_items: AlignItems::End,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(32),
                ..default()
            })
            .with_children(|foot| {
                foot.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    ..default()
                })
                .with_children(|pick| {
                    pick.spawn((
                        Text::new("TON RÔLE PRÉFÉRÉ"),
                        f.text(12.0, 700),
                        TextColor(GOLD),
                    ));
                    pick.spawn(Node {
                        column_gap: px(10),
                        ..default()
                    })
                    .with_children(|tiles| {
                        for r in ROLES {
                            let on = self_player.and_then(|p| p.role) == Some(r);
                            tiles
                                .spawn((
                                    button(
                                        ButtonLook::new(
                                            if on { hexa(0xd9b25f, 0.22) } else { PANEL },
                                            hexa(0xd9b25f, 0.3),
                                        ),
                                        Node {
                                            width: px(150),
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::Center,
                                            row_gap: px(2),
                                            padding: UiRect::all(px(10)),
                                            border: UiRect::all(px(1)),
                                            border_radius: BorderRadius::all(px(8)),
                                            ..default()
                                        },
                                    ),
                                    BorderColor::all(if on {
                                        GOLD_HI
                                    } else {
                                        hexa(0xd9b25f, 0.25)
                                    }),
                                    Act::Role(r),
                                ))
                                .with_children(|t| {
                                    t.spawn((
                                        Text::new(role_label(r)),
                                        f.text(16.0, 800),
                                        TextColor(TEXT),
                                    ));
                                    t.spawn((
                                        Text::new(role_hint(r)),
                                        f.text(11.0, 500),
                                        TextColor(MUTED),
                                    ));
                                });
                        }
                    });
                });
                foot.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::End,
                    row_gap: px(10),
                    ..default()
                })
                .with_children(|zone| {
                    zone.spawn(Node {
                        column_gap: px(6),
                        ..default()
                    })
                    .with_children(|meter| {
                        for i in 0..(TEAM_SIZE * 2) as usize {
                            let c = match lobby.players.get(i) {
                                Some(p) if p.ready => GREEN,
                                Some(_) => GOLD,
                                None => hexa(0xffffff, 0.12),
                            };
                            meter.spawn((
                                Node {
                                    width: px(22),
                                    height: px(8),
                                    border_radius: BorderRadius::all(px(4)),
                                    ..default()
                                },
                                BackgroundColor(c),
                            ));
                        }
                    });
                    let color = if view.can_start { GREEN } else { TEXT };
                    zone.spawn((
                        Text::new(view.status.clone()),
                        f.text(15.0, 700),
                        TextColor(color),
                    ));
                    zone.spawn(Node {
                        column_gap: px(14),
                        ..default()
                    })
                    .with_children(|actions| {
                        let ready = self_player.is_some_and(|p| p.ready);
                        actions
                            .spawn((
                                button(
                                    ButtonLook::new(
                                        if ready { hexa(0x4fdc7c, 0.25) } else { PANEL },
                                        hexa(0x4fdc7c, 0.35),
                                    ),
                                    Node {
                                        padding: UiRect::axes(px(28), px(14)),
                                        border: UiRect::all(px(2)),
                                        border_radius: BorderRadius::all(px(10)),
                                        ..default()
                                    },
                                ),
                                BorderColor::all(if ready { GREEN } else { hexa(0xd9b25f, 0.4) }),
                                Act::Ready(!ready),
                            ))
                            .with_child((
                                Text::new(if ready { "✓ Prêt !" } else { "Prêt ?" }),
                                f.display(22.0, 800),
                                TextColor(TEXT),
                            ));
                        if self_player.is_some_and(|p| p.is_host) {
                            let sub = if view.can_start {
                                "Direction la sélection des champions"
                            } else if lobby.dev_mode {
                                "Tous les joueurs doivent être prêts"
                            } else {
                                "8 joueurs prêts requis"
                            };
                            let mut launch = actions.spawn((
                                button(
                                    ButtonLook::new(GOLD, GOLD_HI),
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        padding: UiRect::axes(px(30), px(10)),
                                        border_radius: BorderRadius::all(px(10)),
                                        ..default()
                                    },
                                ),
                                Act::Start,
                            ));
                            launch.with_children(|b| {
                                b.spawn((
                                    Text::new("LANCER"),
                                    f.display(24.0, 900),
                                    TextColor(Color::srgb_u8(0x1a, 0x10, 0x02)),
                                ));
                                b.spawn((
                                    Text::new(sub),
                                    f.text(11.0, 700),
                                    TextColor(hexa(0x281802, 0.8)),
                                ));
                            });
                            if !view.can_start {
                                launch.insert(bevy::ui::InteractionDisabled);
                            }
                        }
                    });
                });
            });
            if let Some(err) = error {
                root.spawn((Text::new(err), f.text(14.0, 600), TextColor(DANGER)));
            }
        });
}

fn banners(
    parent: &mut ChildSpawnerCommands,
    f: &UiFonts,
    team: Team,
    players: &[LobbyPlayer],
    me: Option<&str>,
) {
    let tp: Vec<&LobbyPlayer> = players.iter().filter(|p| p.team == team).collect();
    let mine = tp.iter().any(|p| Some(p.id.as_str()) == me);
    let color = team_color(team);
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new(format!(
                    "{}   {}/{TEAM_SIZE}",
                    team_name(team).to_uppercase(),
                    tp.len()
                )),
                f.display(18.0, 800),
                TextColor(color),
            ));
            col.spawn(Node {
                column_gap: px(10),
                ..default()
            })
            .with_children(|row| {
                for i in 0..TEAM_SIZE as usize {
                    let cloth = |border: Color| {
                        (
                            Node {
                                width: px(132),
                                height: px(300),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: px(6),
                                padding: UiRect::new(px(8), px(8), px(22), px(8)),
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::bottom(px(14)),
                                ..default()
                            },
                            BorderColor::all(border),
                        )
                    };
                    match tp.get(i) {
                        Some(p) => {
                            let me_here = Some(p.id.as_str()) == me;
                            let border = if me_here {
                                GOLD_HI
                            } else if p.ready {
                                GREEN
                            } else {
                                color.with_alpha(0.5)
                            };
                            row.spawn((
                                cloth(border),
                                BackgroundColor(color.with_alpha(if p.connected {
                                    0.22
                                } else {
                                    0.08
                                })),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Node {
                                        width: px(74),
                                        height: px(74),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        margin: UiRect::bottom(px(8)),
                                        border: UiRect::all(px(3)),
                                        border_radius: BorderRadius::MAX,
                                        ..default()
                                    },
                                    BackgroundColor(color.with_alpha(0.6)),
                                    BorderColor::all(GOLD),
                                ))
                                .with_child((
                                    Text::new(
                                        p.name
                                            .chars()
                                            .next()
                                            .map_or(String::new(), |c| c.to_uppercase().collect()),
                                    ),
                                    f.display(34.0, 900),
                                    TextColor(Color::WHITE),
                                ));
                                let host = if p.is_host { "★ " } else { "" };
                                b.spawn((
                                    Text::new(format!("{host}{}", p.name)),
                                    f.text(15.0, 800),
                                    TextColor(TEXT),
                                ));
                                let role = p.role.map_or("Rôle libre", role_label);
                                b.spawn((Text::new(role), f.text(12.0, 500), TextColor(MUTED)));
                                let (st, c) = if !p.connected {
                                    ("Déconnecté", DANGER)
                                } else if p.ready {
                                    ("Prêt", GREEN)
                                } else {
                                    ("En attente", MUTED)
                                };
                                b.spawn((Text::new(st), f.text(12.0, 700), TextColor(c)));
                            });
                        }
                        None => {
                            let mut slot = row.spawn((
                                button(
                                    ButtonLook::new(hexa(0xffffff, 0.02), color.with_alpha(0.15)),
                                    cloth(hexa(0xffffff, 0.12)).0,
                                ),
                                cloth(hexa(0xffffff, 0.12)).1,
                                Act::Team(team),
                            ));
                            slot.with_children(|b| {
                                b.spawn((
                                    Text::new("+"),
                                    f.display(40.0, 400),
                                    TextColor(hexa(0xffffff, 0.3)),
                                ));
                                b.spawn((
                                    Text::new(if mine { "Place libre" } else { "Rejoindre" }),
                                    f.text(12.0, 600),
                                    TextColor(if mine { MUTED } else { TEAL }),
                                ));
                            });
                            if mine {
                                slot.insert(bevy::ui::InteractionDisabled);
                            }
                        }
                    }
                }
            });
        });
}

fn on_activate(
    ev: On<Activate>,
    acts: Query<&Act>,
    conn: Res<Connection>,
    mut state: ResMut<GameState>,
    mut clipboard: ResMut<Clipboard>,
    mut copied: ResMut<Copied>,
    time: Res<Time>,
) {
    let Ok(act) = acts.get(ev.entity) else { return };
    match act.clone() {
        Act::Copy => {
            if let Some(lobby) = &state.lobby
                && clipboard.set_text(lobby.code.clone()).is_ok()
            {
                copied.0 = time.elapsed_secs() + 1.6;
            }
        }
        Act::Leave => {
            conn.send(ClientMsg::LeaveRoom);
            state.lobby = None;
            state.draft = None;
        }
        Act::Team(team) => conn.send(ClientMsg::SetTeam { team }),
        Act::Role(role) => conn.send(ClientMsg::SetRole { role }),
        Act::Ready(ready) => conn.send(ClientMsg::SetReady { ready }),
        Act::Start => conn.send(ClientMsg::StartGame),
    }
}

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .init_resource::<Copied>()
            .add_systems(Update, rebuild.run_if(in_state(Screen::Lobby)))
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::protocol::RoomPhase;

    fn player(id: &str, team: Team, ready: bool, host: bool) -> LobbyPlayer {
        LobbyPlayer {
            id: id.into(),
            name: id.into(),
            team,
            ready,
            is_host: host,
            connected: true,
            role: None,
        }
    }

    fn lobby(players: Vec<LobbyPlayer>, dev: bool) -> LobbyState {
        LobbyState {
            code: "ABCDE".into(),
            phase: RoomPhase::Lobby,
            players,
            dev_mode: dev,
        }
    }

    #[test]
    fn il_faut_huit_joueurs_prets_hors_entrainement() {
        let l = lobby(vec![player("a", Team::Blue, true, true)], false);
        let v = lobby_view(&l, Some("a"));
        assert!(!v.can_start);
        assert_eq!(v.status, "En attente de joueurs… 1/8");
    }

    #[test]
    fn en_entrainement_l_hote_lance_quand_tous_sont_prets() {
        let mut l = lobby(
            vec![
                player("a", Team::Blue, true, true),
                player("b", Team::Red, false, false),
            ],
            true,
        );
        assert_eq!(lobby_view(&l, Some("a")).status, "1/2 joueurs prêts");
        l.players[1].ready = true;
        assert_eq!(
            lobby_view(&l, Some("a")),
            LobbyView {
                can_start: true,
                status: "Tout le monde est prêt !".into()
            }
        );
        // un invité ne lance pas
        assert_eq!(lobby_view(&l, Some("b")).status, "En attente de l’hôte");
        // un joueur déconnecté ne bloque pas
        l.players[1].ready = false;
        l.players[1].connected = false;
        assert!(lobby_view(&l, Some("a")).can_start);
    }
}
