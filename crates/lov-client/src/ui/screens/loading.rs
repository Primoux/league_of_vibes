//! Écran de chargement : portage de `packages/client/src/ui/screens/Loading.tsx`.
//!
//! Cartes des joueurs des deux équipes (illustration, champion, titre, classe, sorts d'invocateur), barre de
//! progression jusqu'au début de la partie, astuce qui change toutes les 4 s.

use std::f32::consts::PI;

use bevy::prelude::*;
use lov_shared::constants::LOADING_SECONDS;
use lov_shared::data::champions::get_champion;
use lov_shared::data::summoners::summoner;
use lov_shared::types::Team;

use super::rift::RiftBackground;
use super::{screen_root, team_color};
use crate::net::GameState;
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::logic::format::class_label;
use crate::ui::theme::{GOLD, GOLD_HI, MUTED, TEXT, css, hex, hexa};
use crate::ui::{Screen, UiFonts, remaining};

pub const TIPS: [&str; 8] = [
    "Le Léviathan apparaît à 15:00 : son buff renforce le rappel et les sbires proches.",
    "Les tours externes sont fortifiées pendant les 3 premières minutes.",
    "Une balise révèle une zone de 900 unités pendant 90 secondes. Pose-les aux entrées de la rivière !",
    "Le 3e drake détermine le type de la Faille. À 4 drakes, ton équipe obtient l’âme.",
    "Châtiment est indispensable en jungle : sans lui, les camps rapportent moins.",
    "Ctrl + Q/W/E/R monte un sort. Le bouton + apparaît quand un point est disponible.",
    "Tab affiche le tableau des scores, P la boutique, C tes statistiques détaillées.",
    "Trois larves tuées : des larvelettes t’aident à démolir les tours.",
];

/// Avancement 0..1 de la barre : 0,5 tant que la fin du chargement n'est pas connue.
pub fn progress(left: f64, known: bool) -> f64 {
    if known {
        (1.0 - left / LOADING_SECONDS).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Tip;

/// Astuce affichée et instant du dernier changement.
#[derive(Resource, Default)]
struct TipState(usize, f32);

fn on_enter(
    mut commands: Commands,
    state: Res<GameState>,
    fonts: Res<UiFonts>,
    time: Res<Time>,
    mut tip: ResMut<TipState>,
) {
    let f = &*fonts;
    tip.0 = (time.elapsed_secs_f64() * 1000.0) as usize % TIPS.len();
    tip.1 = time.elapsed_secs();
    let me = state.player_id.as_deref();
    let name = |id: &str| {
        state
            .lobby
            .iter()
            .flat_map(|l| &l.players)
            .find(|p| p.id == id)
            .map_or("Invocateur".to_string(), |p| p.name.clone())
    };
    let picks = state
        .draft
        .as_ref()
        .map(|d| d.picks.clone())
        .unwrap_or_default();
    commands
        .spawn(screen_root(
            Screen::Loading,
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(24)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(RiftBackground::new(hex(0xb072ff), 0.8).bundle());
            for team in [Team::Blue, Team::Red] {
                root.spawn(Node {
                    column_gap: px(16),
                    ..default()
                })
                .with_children(|row| {
                    for p in picks.iter().filter(|p| p.team == team) {
                        let c = p.champion_id.as_deref().and_then(get_champion);
                        let color = c.map_or(team_color(team), |c| css(&c.palette.primary));
                        let border = if Some(p.player_id.as_str()) == me {
                            GOLD_HI
                        } else {
                            team_color(team).with_alpha(0.6)
                        };
                        row.spawn((
                            Node {
                                width: px(200),
                                height: px(300),
                                flex_direction: FlexDirection::Column,
                                border: UiRect::all(px(2)),
                                border_radius: BorderRadius::all(px(10)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundGradient::from(LinearGradient {
                                angle: 0.0,
                                stops: vec![
                                    ColorStop::new(hexa(0x08091a, 0.97), percent(0)),
                                    ColorStop::new(hexa(0x08091a, 0.85), percent(35)),
                                    ColorStop::new(color.with_alpha(0.6), percent(100)),
                                ],
                                ..default()
                            }),
                            BorderColor::all(border),
                        ))
                        .with_children(|card| {
                            // illustration (la zone est presque carrée : recadrage carré)
                            let mut art = card.spawn(Node {
                                flex_grow: 1.0,
                                min_height: px(0),
                                ..default()
                            });
                            if let Some(c) = c {
                                art.with_child(
                                    ChampionPortrait::new(&c.id, PortraitKind::Square).with(Node {
                                        width: percent(100),
                                        height: percent(100),
                                        ..default()
                                    }),
                                );
                            }
                            card.spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(3),
                                    padding: UiRect::new(px(12), px(12), px(10), px(12)),
                                    ..default()
                                },
                                BackgroundGradient::from(LinearGradient {
                                    angle: PI,
                                    stops: vec![
                                        ColorStop::new(hexa(0x0a0c18, 0.9), percent(0)),
                                        ColorStop::new(hex(0x05060c), percent(100)),
                                    ],
                                    ..default()
                                }),
                            ))
                            .with_children(|meta| {
                                meta.spawn((
                                    Text::new(c.map_or("???".into(), |c| c.name.clone())),
                                    f.display(20.0, 900),
                                    TextColor(GOLD_HI),
                                ));
                                meta.spawn((
                                    Text::new(c.map_or(String::new(), |c| c.title.clone())),
                                    f.text(11.0, 500),
                                    TextColor(MUTED),
                                ));
                                meta.spawn((
                                    Text::new(name(&p.player_id)),
                                    f.text(13.0, 700),
                                    TextColor(TEXT),
                                ));
                                let class = c.map_or("", |c| class_label(c.class));
                                let sums = format!(
                                    "{class}   ·   {} / {}",
                                    summoner(p.summoners[0]).name,
                                    summoner(p.summoners[1]).name
                                );
                                meta.spawn((Text::new(sums), f.text(10.0, 600), TextColor(MUTED)));
                            });
                        });
                    }
                });
            }
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(12),
                ..default()
            })
            .with_children(|foot| {
                foot.spawn((
                    Text::new("FAILLE DES VIBES"),
                    f.display(30.0, 900),
                    TextColor(GOLD_HI),
                ));
                foot.spawn((
                    Node {
                        width: px(520),
                        height: px(6),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(hexa(0xffffff, 0.08)),
                ))
                .with_child((
                    Bar,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(GOLD),
                ));
                foot.spawn((Tip, Text::new(""), f.text(14.0, 500), TextColor(TEXT)));
            });
        });
}

fn tick(
    state: Res<GameState>,
    time: Res<Time>,
    mut tip: ResMut<TipState>,
    mut bars: Query<&mut Node, With<Bar>>,
    mut tips: Query<&mut Text, With<Tip>>,
) {
    let left = remaining(&state, state.loading_ends_at);
    for mut node in &mut bars {
        node.width = percent(progress(left, state.loading_ends_at.is_some()) * 100.0);
    }
    if time.elapsed_secs() - tip.1 >= 4.0 {
        tip.0 = (tip.0 + 1) % TIPS.len();
        tip.1 = time.elapsed_secs();
    }
    for mut text in &mut tips {
        let v = format!("Astuce : {}", TIPS[tip.0]);
        if text.0 != v {
            text.0 = v;
        }
    }
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TipState>()
            .add_systems(OnEnter(Screen::Loading), on_enter)
            .add_systems(Update, tick.run_if(in_state(Screen::Loading)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progression_du_chargement() {
        assert_eq!(progress(0.0, false), 0.5);
        assert_eq!(progress(LOADING_SECONDS, true), 0.0);
        assert_eq!(progress(0.0, true), 1.0);
        assert_eq!(progress(LOADING_SECONDS * 2.0, true), 0.0);
    }
}
