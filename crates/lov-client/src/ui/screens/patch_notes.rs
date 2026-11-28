//! Notes de patch, sur l'accueil (#330) : la version du jeu en pied de page et un panneau qui liste ce
//! qui a changé à chaque patch, comme le client de LoL.
//!
//! Le texte vient de `docs/patch-notes/patchs.md`, embarqué à la compilation et assemblé par
//! `scripts/patch-notes.mjs` (voir `docs/patch-notes/README.md`) : `# Patch 0.N — date`, puis des
//! catégories `##`, des sujets `###` (un champion ou un objet affiche son portrait ou son icône) et des
//! lignes `- …` (vertes si elles commencent par `+`, rouges par `-` ou `−`).
//!
//! La dernière version vue est enregistrée dans le dossier de config (`patch_vu.txt`) : après une mise à
//! jour, le panneau s'ouvre tout seul ; tant qu'il n'a pas été ouvert, le bouton porte un badge.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use bevy::ui_widgets::{Activate, ScrollArea};
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::data::items::ITEM_LIST;

use crate::ui::icons::draw::IconCache;
use crate::ui::icons::{ChampionPortrait, PortraitKind};
use crate::ui::theme::{GOLD, GOLD_HI, GREEN, MUTED, PANEL, RED, TEXT, hexa};
use crate::ui::{ButtonLook, Screen, UiFonts, button};

/// Notes de tous les patchs, la plus récente en haut.
const NOTES: &str = include_str!("../../../../../docs/patch-notes/patchs.md");
/// Fichier de la dernière version vue, dans le dossier de config.
const SEEN_FILE: &str = "patch_vu.txt";

/// Version du jeu (`Cargo.toml` du workspace).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nom du patch affiché aux joueurs : « 0.3.0 » → « 0.3 ».
pub fn patch_name(version: &str) -> &str {
    version.strip_suffix(".0").unwrap_or(version)
}

#[derive(Debug, PartialEq)]
pub struct Patch {
    pub name: String,
    pub date: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, PartialEq)]
pub struct Section {
    pub category: String,
    pub groups: Vec<Group>,
}

/// Lignes d'un sujet (`###`), ou de la catégorie elle-même si `subject` est vide.
#[derive(Debug, PartialEq)]
pub struct Group {
    pub subject: String,
    pub lines: Vec<String>,
}

/// Lit `patchs.md` : commentaires `<!-- -->` ignorés, `# Patch nom — date`, `##`, `###`, `- ligne`.
pub fn parse(text: &str) -> Vec<Patch> {
    let mut patches: Vec<Patch> = Vec::new();
    let mut in_comment = false;
    for raw in text.lines() {
        let line = raw.trim();
        if in_comment || line.starts_with("<!--") {
            in_comment = !line.contains("-->");
            continue;
        }
        if let Some(rest) = line.strip_prefix("# Patch ") {
            let (name, date) = rest.split_once(" — ").unwrap_or((rest, ""));
            patches.push(Patch {
                name: name.trim().into(),
                date: date.trim().into(),
                sections: Vec::new(),
            });
        } else if let Some(cat) = line.strip_prefix("## ")
            && let Some(p) = patches.last_mut()
        {
            p.sections.push(Section {
                category: cat.trim().into(),
                groups: Vec::new(),
            });
        } else if let Some(subject) = line.strip_prefix("### ")
            && let Some(s) = patches.last_mut().and_then(|p| p.sections.last_mut())
        {
            s.groups.push(Group {
                subject: subject.trim().into(),
                lines: Vec::new(),
            });
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* "))
            && let Some(s) = patches.last_mut().and_then(|p| p.sections.last_mut())
        {
            if s.groups.is_empty() {
                s.groups.push(Group {
                    subject: String::new(),
                    lines: Vec::new(),
                });
            }
            if let Some(g) = s.groups.last_mut() {
                g.lines.push(item.trim().into());
            }
        }
    }
    patches
}

/// Couleur d'une ligne : amélioration (`+`), affaiblissement (`-`, `−`), ou neutre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Buff,
    Nerf,
    Neutral,
}

pub fn tone(line: &str) -> Tone {
    match line.chars().next() {
        Some('+') => Tone::Buff,
        Some('-' | '−') => Tone::Nerf,
        _ => Tone::Neutral,
    }
}

/// `a` est une version plus récente que `b` (comparaison numérique, « 0.10 » > « 0.9 »).
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u32> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parts(a) > parts(b)
}

fn seen_path() -> Option<PathBuf> {
    crate::audio::settings::config_dir().map(|d| d.join(SEEN_FILE))
}

/// État du panneau.
#[derive(Resource)]
pub struct PatchNotesUi {
    pub open: bool,
    /// Patch affiché (index dans `patches`, 0 = le plus récent).
    pub selected: usize,
    /// La version actuelle n'a pas encore été lue : badge sur le bouton.
    pub unseen: bool,
    patches: Vec<Patch>,
}

impl Default for PatchNotesUi {
    fn default() -> Self {
        let seen = seen_path().and_then(|p| std::fs::read_to_string(p).ok());
        let seen = seen.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let patches = parse(NOTES);
        let has_notes = !patches.is_empty();
        Self {
            // ouvert tout seul après une mise à jour (pas au tout premier lancement)
            open: has_notes && seen.is_some_and(|s| newer(VERSION, s)),
            selected: 0,
            unseen: has_notes && seen.is_none_or(|s| newer(VERSION, s)),
            patches,
        }
    }
}

impl PatchNotesUi {
    /// Ouvre le panneau sur le patch le plus récent.
    pub fn show(&mut self) {
        self.open = true;
        self.selected = 0;
    }
}

/// Badge « nouveau » à poser sur le bouton qui ouvre les notes : visible tant qu'elles ne sont pas lues.
#[derive(Component)]
pub struct NewBadge;

#[derive(Component)]
struct NotesRoot;
#[derive(Component)]
struct NotesBody;
#[derive(Component)]
struct NotesList;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum NotesAct {
    Close,
    Select(usize),
}

/// Le panneau (caché), au-dessus de l'accueil.
fn spawn_panel(mut commands: Commands, fonts: Res<UiFonts>, mut ui: ResMut<PatchNotesUi>) {
    ui.set_changed();
    let f = &*fonts;
    commands
        .spawn((
            NotesRoot,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(hexa(0x000000, 0.6)),
            GlobalZIndex(20),
            DespawnOnExit(Screen::Home),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(70),
                    max_width: px(1100),
                    height: percent(80),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(hexa(0xd9b25f, 0.35)),
            ))
            .with_children(|panel| {
                // en-tête
                panel
                    .spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(24), px(16)),
                            border: UiRect::bottom(px(1)),
                            ..default()
                        },
                        BorderColor::all(hexa(0xffffff, 0.06)),
                    ))
                    .with_children(|head| {
                        head.spawn((
                            Text::new("NOTES DE PATCH"),
                            f.display(28.0, 900),
                            TextColor(GOLD_HI),
                        ));
                        head.spawn((
                            button(
                                ButtonLook::new(hexa(0xffffff, 0.04), hexa(0xffffff, 0.12)),
                                Node {
                                    padding: UiRect::axes(px(14), px(6)),
                                    border_radius: BorderRadius::all(px(8)),
                                    ..default()
                                },
                            ),
                            NotesAct::Close,
                        ))
                        .with_child((
                            Text::new("Fermer  (Échap)"),
                            f.text(14.0, 600),
                            TextColor(TEXT),
                        ));
                    });
                panel
                    .spawn(Node {
                        flex_grow: 1.0,
                        min_height: px(0),
                        flex_direction: FlexDirection::Row,
                        ..default()
                    })
                    .with_children(|body| {
                        body.spawn((
                            NotesList,
                            Node {
                                width: px(190),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(6),
                                padding: UiRect::all(px(14)),
                                border: UiRect::right(px(1)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            BorderColor::all(hexa(0xffffff, 0.06)),
                            ScrollArea,
                            ScrollPosition::default(),
                        ));
                        body.spawn((
                            NotesBody,
                            Node {
                                flex_grow: 1.0,
                                min_height: px(0),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(10),
                                padding: UiRect::new(px(28), px(28), px(20), px(28)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollArea,
                            ScrollPosition::default(),
                        ));
                    });
            });
        });
}

/// Ouvre ou ferme le panneau, remplit la liste et le patch choisi quand l'état change.
#[allow(clippy::too_many_arguments)] // un système Bevy lit autant de requêtes que le panneau a de zones
fn refresh(
    mut commands: Commands,
    mut ui: ResMut<PatchNotesUi>,
    fonts: Res<UiFonts>,
    mut icons: ResMut<IconCache>,
    mut images: ResMut<Assets<Image>>,
    mut roots: Query<&mut Node, With<NotesRoot>>,
    mut badges: Query<&mut Visibility, With<NewBadge>>,
    list: Query<Entity, With<NotesList>>,
    body: Query<Entity, With<NotesBody>>,
) {
    if !ui.is_changed() {
        return;
    }
    if ui.open && ui.unseen {
        ui.unseen = false;
        if let Some(p) = seen_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(&p, VERSION) {
                warn!("notes de patch : version vue non enregistrée ({e})");
            }
        }
    }
    for mut badge in &mut badges {
        badge.set_if_neq(if ui.unseen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for mut node in &mut roots {
        let display = if ui.open {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    if !ui.open {
        return;
    }
    let f = &*fonts;
    let (Ok(list), Ok(body)) = (list.single(), body.single()) else {
        return;
    };
    commands.entity(list).despawn_related::<Children>();
    commands.entity(body).despawn_related::<Children>();
    let selected = ui.selected.min(ui.patches.len().saturating_sub(1));
    commands.entity(list).with_children(|l| {
        for (i, p) in ui.patches.iter().enumerate() {
            let look = if i == selected {
                ButtonLook::new(hexa(0xd9b25f, 0.22), hexa(0xd9b25f, 0.3))
            } else {
                ButtonLook::new(hexa(0xffffff, 0.03), hexa(0xffffff, 0.1))
            };
            l.spawn((
                button(
                    look,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(px(12), px(8)),
                        border_radius: BorderRadius::all(px(8)),
                        ..default()
                    },
                ),
                NotesAct::Select(i),
            ))
            .with_children(|b| {
                let color = if i == selected { GOLD_HI } else { TEXT };
                b.spawn((
                    Text::new(format!("Patch {}", p.name)),
                    f.display(18.0, 800),
                    TextColor(color),
                ));
                b.spawn((
                    Text::new(p.date.clone()),
                    f.text(12.0, 500),
                    TextColor(MUTED),
                ));
            });
        }
    });
    commands.entity(body).with_children(|b| {
        let Some(p) = ui.patches.get(selected) else {
            b.spawn((
                Text::new("Pas encore de notes de patch."),
                f.text(16.0, 500),
                TextColor(MUTED),
            ));
            return;
        };
        b.spawn((
            Text::new(format!("Patch {}", p.name)),
            f.display(40.0, 900),
            TextColor(GOLD_HI),
        ));
        if !p.date.is_empty() {
            b.spawn((
                Text::new(p.date.clone()),
                f.text(14.0, 500),
                TextColor(MUTED),
            ));
        }
        for s in &p.sections {
            b.spawn((
                Node {
                    margin: UiRect::top(px(14)),
                    padding: UiRect::bottom(px(4)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(hexa(0xd9b25f, 0.3)),
            ))
            .with_child((
                Text::new(s.category.to_uppercase()),
                f.display(20.0, 800),
                TextColor(GOLD),
            ));
            for g in &s.groups {
                b.spawn(Node {
                    column_gap: px(12),
                    align_items: AlignItems::FlexStart,
                    ..default()
                })
                .with_children(|row| {
                    if !g.subject.is_empty() {
                        subject_icon(row, &mut icons, &mut images, &g.subject);
                    }
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 1.0,
                        row_gap: px(3),
                        ..default()
                    })
                    .with_children(|col| {
                        if !g.subject.is_empty() {
                            col.spawn((
                                Text::new(g.subject.clone()),
                                f.text(16.0, 700),
                                TextColor(TEXT),
                            ));
                        }
                        for line in &g.lines {
                            let color = match tone(line) {
                                Tone::Buff => GREEN,
                                Tone::Nerf => RED,
                                Tone::Neutral => hexa(0xece7da, 0.85),
                            };
                            col.spawn((
                                Text::new(format!("•  {line}")),
                                f.text(14.0, 500),
                                TextColor(color),
                            ));
                        }
                    });
                });
            }
        }
    });
}

/// Portrait du champion ou icône de l'objet nommé par le sujet, s'il y en a un.
fn subject_icon(
    row: &mut ChildSpawnerCommands,
    icons: &mut IconCache,
    images: &mut Assets<Image>,
    subject: &str,
) {
    if let Some(c) = CHAMPIONS.iter().find(|c| c.name == subject) {
        row.spawn(ChampionPortrait::new(&c.id, PortraitKind::Round).sized(40.0));
    } else if let Some(it) = ITEM_LIST.iter().find(|i| i.name == subject) {
        row.spawn((
            ImageNode::new(icons.item(images, it)),
            Node {
                width: px(40),
                height: px(40),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
        ));
    }
}

fn on_activate(ev: On<Activate>, acts: Query<&NotesAct>, mut ui: ResMut<PatchNotesUi>) {
    match acts.get(ev.entity) {
        Ok(NotesAct::Close) => ui.open = false,
        Ok(NotesAct::Select(i)) => ui.selected = *i,
        Err(_) => {}
    }
}

/// Échap ferme le panneau.
fn close_on_escape(keys: Res<ButtonInput<KeyCode>>, mut ui: ResMut<PatchNotesUi>) {
    if ui.open && keys.just_pressed(KeyCode::Escape) {
        ui.open = false;
    }
}

pub struct PatchNotesPlugin;

impl Plugin for PatchNotesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PatchNotesUi>()
            .add_systems(OnEnter(Screen::Home), spawn_panel)
            .add_systems(
                Update,
                (close_on_escape, refresh)
                    .chain()
                    .run_if(in_state(Screen::Home)),
            )
            .add_observer(on_activate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<!-- en-tête\n     sur deux lignes -->\n\n# Patch 0.2 — 2026-12-05\n\n## Champions\n### Corvin\n- + Q : 60 ⇒ 70\n- − W : portée 650 ⇒ 600\n## Système\n- Rappel interrompu par les sorts\n### Tours\n- Ciblage\n\n# Patch 0.1 — 2026-11-28\n\n## Correctifs\n- Une tour tire sur le visible\n";

    #[test]
    fn lit_les_patchs_categories_sujets_et_lignes() {
        let p = parse(SAMPLE);
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0].name.as_str(), p[0].date.as_str()),
            ("0.2", "2026-12-05")
        );
        assert_eq!(p[0].sections.len(), 2);
        let champs = &p[0].sections[0];
        assert_eq!(champs.category, "Champions");
        assert_eq!(champs.groups[0].subject, "Corvin");
        assert_eq!(
            champs.groups[0].lines,
            ["+ Q : 60 ⇒ 70", "− W : portée 650 ⇒ 600"]
        );
        let sys = &p[0].sections[1];
        assert_eq!(sys.groups[0].subject, "", "lignes sans sujet");
        assert_eq!(sys.groups[0].lines, ["Rappel interrompu par les sorts"]);
        assert_eq!(sys.groups[1].subject, "Tours");
        assert_eq!(
            p[1].sections[0].groups[0].lines,
            ["Une tour tire sur le visible"]
        );
    }

    #[test]
    fn le_fichier_embarque_se_lit() {
        // aucun patch publié tant que la première release n'est pas faite : on vérifie juste que ça passe
        for p in parse(NOTES) {
            assert!(!p.name.is_empty());
        }
    }

    #[test]
    fn couleur_des_lignes() {
        assert_eq!(tone("+ Q : 60 ⇒ 70"), Tone::Buff);
        assert_eq!(tone("- W : 650 ⇒ 600"), Tone::Nerf);
        assert_eq!(tone("− R : 1 s ⇒ 1,2 s"), Tone::Nerf);
        assert_eq!(tone("Correctif"), Tone::Neutral);
    }

    #[test]
    fn comparaison_des_versions_et_nom_du_patch() {
        assert!(newer("0.10.0", "0.9.0"));
        assert!(newer("0.2.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.2.0"));
        assert_eq!(patch_name("0.3.0"), "0.3");
        assert_eq!(patch_name("1.2.4"), "1.2.4");
    }
}
