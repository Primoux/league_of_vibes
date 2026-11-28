//! Entrées du joueur : port de `packages/client/src/input/controls.ts`.
//!
//! - Clic droit : sur un ennemi = attaque, sur le sol = déplacement ; maintenu = suit le curseur.
//! - A puis clic gauche : attaque-déplacement (quick-cast : immédiat).
//! - Q W E R : cast normal (touche maintenue = indicateur, relâcher la touche ou clic gauche pour
//!   lancer, clic droit ou Échap pour annuler) ou quick-cast selon le réglage ; Maj inverse le mode.
//!   Ctrl/Alt + QWER : monter le sort.
//! - ² / ` (maintenu) : ne cibler que les champions. Option : clic gauche au sol = attaque-déplacement.
//! - D / F : sorts d'invocateur. 1 2 3 5 6 7 : objets actifs. 4 : balise. B : rappel. S : stop.
//! - P : boutique. Tab (maintenu) : scores. C : stats. Échap : annuler / fermer / réglages.
//! - Alt + clic gauche : ping.
//!
//! Et ce que le navigateur ne permettait pas : curseur confiné à la fenêtre en plein écran (F11), vsync
//! au choix (V). En fenêtré, hors partie, réglages ouverts (Échap) ou fenêtre inactive, la souris est libre. Ctrl+Q monte le
//! sort Q : on quitte par la fermeture de la fenêtre (Alt+F4).
//!
//! La caméra (Espace, Y, flèches, molette) reste dans `render::camera`. Les champs que l'interface et
//! les FX lisent (ciblage en cours, entité survolée, panneaux ouverts) sont dans [`PlayerInput`].

pub mod markers;
pub mod picking;
pub mod rules;

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::{
    CursorGrabMode, CursorIcon, CursorOptions, MonitorSelection, PresentMode, SystemCursorIcon,
    WindowMode,
};
use lov_shared::data::champions::get_champion;
use lov_shared::data::items::item;
use lov_shared::data::summoners::summoner;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::Targeting as SpellTargeting;
use lov_shared::types::{AbilitySlot, Cooldowns, EntityId, EntityKind, SummonerSlot, Vec2 as Pos};

use crate::net::{Connection, GameState};
use crate::render::camera::MainCamera;
use crate::render::map::MAP_SIZE;
use crate::render::{ground_height, to_game, to_ground, to_shared};
use picking::{ray_cylinder, world_hitbox};
use rules::{DigitAction, digit_action, is_attackable, key_letter, needs_target};

/// Clic droit maintenu : un ordre au plus toutes les 120 ms, et seulement si le curseur a bougé de 40 u.
const FOLLOW_EVERY: f32 = 0.12;
const FOLLOW_MIN_MOVE: f64 = 40.0;

/// Ciblage en cours (indicateur affiché, clic gauche pour confirmer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Targeting {
    Ability(AbilitySlot),
    Summoner(SummonerSlot),
    AttackMove,
    Ward,
}

/// Réglages de jeu qui changent les entrées (`Settings` du client web).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InputSettings {
    /// Lance le sort dès l'appui de la touche, vers le curseur ou l'entité survolée.
    pub quick_cast: bool,
    /// Clic gauche au sol (sans sort en cours) = attaque-déplacement.
    pub left_click_attack_move: bool,
}

/// État des entrées, lu par l'interface et les FX.
#[derive(Resource, Debug, Default)]
pub struct PlayerInput {
    pub settings: InputSettings,
    pub targeting: Option<Targeting>,
    /// Entité sous le curseur.
    pub hover: Option<EntityId>,
    /// Position de jeu du curseur au sol, `None` hors de la carte.
    pub cursor: Option<Pos>,
    pub shop_open: bool,
    pub scoreboard_open: bool,
    pub stats_open: bool,
    pub settings_open: bool,
    /// ² maintenu : seuls les champions sont ciblables.
    champions_only: bool,
    /// Touche physique qui a ouvert l'indicateur en cours : la relâcher lance le sort.
    cast_held: Option<KeyCode>,
    right_held: bool,
    since_follow: f32,
    last_follow: Option<Pos>,
    /// Ordres de l'image à marquer au sol ([`markers`]).
    clicks: Vec<markers::ClickMark>,
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerInput>()
            .init_resource::<markers::MarkerCursor>()
            .add_systems(Startup, markers::spawn_markers)
            .add_systems(
                Update,
                (
                    update_pointer,
                    mouse_orders,
                    key_orders,
                    update_cursor_icon,
                    markers::update_markers,
                )
                    .chain()
                    .after(crate::render::camera::drive_camera),
            )
            .add_systems(Update, (window_shortcuts, grab_cursor));
    }
}

/// Confinement de la souris : seulement en partie, en plein écran, fenêtre active et réglages fermés.
/// En fenêtré, la souris reste libre (on peut cliquer sur une autre fenêtre) ; un Alt+Tab la libère aussi.
fn cursor_grab(
    in_game: bool,
    focused: bool,
    settings_open: bool,
    mode: WindowMode,
) -> CursorGrabMode {
    if in_game && focused && !settings_open && mode != WindowMode::Windowed {
        CursorGrabMode::Confined
    } else {
        CursorGrabMode::None
    }
}

fn grab_cursor(
    state: Res<GameState>,
    input: Res<PlayerInput>,
    window: Single<(&Window, &mut CursorOptions)>,
) {
    let (window, mut cursor) = window.into_inner();
    let want = cursor_grab(
        in_game(&state),
        window.focused,
        input.settings_open,
        window.mode,
    );
    if cursor.grab_mode != want {
        cursor.grab_mode = want;
    }
}

/// En partie (jusqu'à l'écran de fin).
fn in_game(state: &GameState) -> bool {
    state.game.is_some() && state.result.is_none()
}

/// Entité survolée, si on peut l'attaquer.
fn enemy_hover(state: &GameState, input: &PlayerInput) -> Option<EntityId> {
    let team = state.game.as_ref()?.team;
    let id = input.hover?;
    state
        .entities()
        .get(&id)
        .filter(|e| is_attackable(e, team))
        .map(|e| e.id)
}

/// Rayon caméra → curseur.
fn cursor_ray(window: &Window, camera: &Camera, transform: &GlobalTransform) -> Option<Ray3d> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world(transform, cursor).ok()
}

/// Sol et entité sous le curseur, chaque image (la caméra bouge).
fn update_pointer(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    state: Res<GameState>,
    keys: Res<ButtonInput<KeyCode>>,
    over_ui: Option<Res<crate::ui::PointerOverUi>>,
    mut input: ResMut<PlayerInput>,
) {
    input.champions_only = keys.pressed(KeyCode::Backquote);
    let (camera, transform) = *camera;
    // souris sur un panneau du HUD : on garde le sol et l'unité de l'image précédente (objets actifs,
    // balise et sorts au clavier visent la dernière position au sol, comme `cursorGame` du web) ; les
    // clics, eux, sont ignorés par `mouse_orders`
    if over_ui.is_some_and(|o| o.0) {
        return;
    }
    let Some(ray) = cursor_ray(&window, camera, transform).filter(|_| in_game(&state)) else {
        input.cursor = None;
        input.hover = None;
        return;
    };
    input.cursor = pick_ground(ray)
        .filter(|p| (0.0..=MAP_SIZE).contains(&p.x) && (0.0..=MAP_SIZE).contains(&p.y))
        .map(to_shared);
    let (origin, dir) = (ray.origin, *ray.direction);
    let mut best: Option<(f32, EntityId)> = None;
    for (&id, e) in state.entities() {
        if e.dead == Some(true) || (input.champions_only && e.kind != EntityKind::Champion) {
            continue;
        }
        let (Some((r, h)), Some(pose)) = (world_hitbox(e), state.pose(id)) else {
            continue;
        };
        // cylindre posé sur le relief : on ramène le rayon à un sol en y = 0
        let at = to_ground(Vec2::new(pose.x as f32, pose.y as f32), 0.0);
        if let Some(t) = ray_cylinder(origin - Vec3::Y * at.y, dir, at.x, at.z, r, h)
            && best.is_none_or(|(b, _)| t < b)
        {
            best = Some((t, id));
        }
    }
    input.hover = best.map(|(_, id)| id);
}

/// Point du sol sous le rayon (coordonnées de jeu) : intersection avec le plan y = 0, puis deux
/// raffinements sur la hauteur réelle du terrain (rivière creusée), comme `pickGround` du web.
fn pick_ground(ray: Ray3d) -> Option<Vec2> {
    let mut height = 0.0;
    let mut hit = None;
    for _ in 0..3 {
        let p = ray.plane_intersection_point(Vec3::Y * height, InfinitePlane3d::new(Vec3::Y))?;
        let g = to_game(p);
        hit = Some(g);
        height = ground_height(f64::from(g.x), f64::from(g.y));
    }
    hit
}

fn ability_targeting(state: &GameState, slot: AbilitySlot) -> Option<SpellTargeting> {
    let me = state.entities().get(&state.my_entity?)?;
    let def = get_champion(me.subtype.as_deref()?)?;
    Some(def.abilities.get(slot).targeting)
}

fn summoner_targeting(state: &GameState, slot: SummonerSlot) -> Option<SpellTargeting> {
    let s = state.self_state.as_ref()?;
    Some(summoner(*s.summoners.get(slot)).targeting)
}

fn cooldown(cd: &Cooldowns, t: Targeting) -> f64 {
    match t {
        Targeting::Ability(AbilitySlot::Q) => cd.q,
        Targeting::Ability(AbilitySlot::W) => cd.w,
        Targeting::Ability(AbilitySlot::E) => cd.e,
        Targeting::Ability(AbilitySlot::R) => cd.r,
        Targeting::Summoner(SummonerSlot::D) => cd.d,
        Targeting::Summoner(SummonerSlot::F) => cd.f,
        Targeting::AttackMove | Targeting::Ward => 0.0,
    }
}

/// Confirme un ciblage (clic gauche, relâchement de la touche, quick-cast).
fn confirm(t: Targeting, state: &GameState, input: &mut PlayerInput, conn: &Connection) {
    let pos = input.cursor;
    let hover = input.hover;
    match t {
        Targeting::AttackMove => match enemy_hover(state, input) {
            Some(target_id) => conn.send(ClientMsg::Attack { target_id }),
            None => {
                if let Some(pos) = pos {
                    conn.send(ClientMsg::AttackMove { pos });
                    input.clicks.push(markers::ClickMark { pos, attack: true });
                }
            }
        },
        Targeting::Ward => {
            if let Some(pos) = pos {
                conn.send(ClientMsg::PlaceWard { pos });
            }
        }
        Targeting::Ability(slot) => {
            if ability_targeting(state, slot).is_some_and(needs_target) && hover.is_none() {
                return; // attendre un clic sur une unité
            }
            conn.send(ClientMsg::Cast {
                slot,
                target_pos: pos,
                target_id: hover,
            });
        }
        Targeting::Summoner(slot) => {
            if summoner_targeting(state, slot).is_some_and(needs_target) && hover.is_none() {
                return;
            }
            conn.send(ClientMsg::Summoner {
                slot,
                target_pos: pos,
                target_id: hover,
            });
        }
    }
    set_targeting(input, None);
}

/// Touche 4 : la Sentinelle de vision passe en visée de balise ; un trinket actif (Lanterne
/// révélatrice…) est utilisé aussitôt, comme au clic sur son emplacement du HUD (#336).
fn trinket_key(state: &GameState, input: &mut PlayerInput, conn: &Connection) {
    let trinket = state
        .my_entity
        .and_then(|id| state.entities().get(&id))
        .and_then(|e| e.champ.as_ref())
        .and_then(|c| c.items.get(6).cloned().flatten());
    match trinket.as_deref().and_then(item) {
        Some(d) if d.active.is_some() && d.id != "stealth_ward" => conn.send(ClientMsg::UseItem {
            slot: -1,
            target_pos: input.cursor,
        }),
        _ => set_targeting(input, Some(Targeting::Ward)),
    }
}

fn set_targeting(input: &mut PlayerInput, t: Option<Targeting>) {
    if t.is_none() {
        input.cast_held = None;
    }
    input.targeting = t;
}

/// Q W E R / D F : cast normal ou rapide.
fn cast_key(
    t: Targeting,
    invert_quick: bool,
    code: KeyCode,
    state: &GameState,
    input: &mut PlayerInput,
    conn: &Connection,
) {
    let targeting = match t {
        Targeting::Ability(slot) => ability_targeting(state, slot),
        Targeting::Summoner(slot) => summoner_targeting(state, slot),
        _ => None,
    };
    if let Some(s) = &state.self_state {
        if let Targeting::Ability(slot) = t
            && *s.ability_levels.get(slot) == 0
        {
            return;
        }
        if cooldown(&s.cooldowns, t) > 0.0 {
            return;
        }
    }
    let quick = input.settings.quick_cast != invert_quick;
    if targeting == Some(SpellTargeting::SelfCast) || quick {
        confirm(t, state, input, conn);
        return;
    }
    set_targeting(input, Some(t));
    input.cast_held = Some(code);
}

fn mouse_orders(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<GameState>,
    conn: Res<Connection>,
    time: Res<Time>,
    over_ui: Option<Res<crate::ui::PointerOverUi>>,
    mut input: ResMut<PlayerInput>,
) {
    if buttons.just_released(MouseButton::Right) {
        input.right_held = false;
    }
    if !in_game(&state) {
        input.right_held = false;
        return;
    }
    // un clic sur le HUD lui appartient : ni ordre, ni annulation du ciblage en cours
    let over_ui = over_ui.is_some_and(|o| o.0);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if buttons.just_pressed(MouseButton::Left) && !over_ui {
        if alt {
            if let Some(pos) = input.cursor {
                let kind = if enemy_hover(&state, &input).is_some() {
                    "danger"
                } else {
                    "generic"
                };
                conn.send(ClientMsg::Ping {
                    pos,
                    kind: kind.into(),
                });
            }
        } else if let Some(t) = input.targeting {
            confirm(t, &state, &mut input, &conn);
        } else if input.settings.left_click_attack_move {
            confirm(Targeting::AttackMove, &state, &mut input, &conn);
        }
    }
    if buttons.just_pressed(MouseButton::Right) && !over_ui {
        set_targeting(&mut input, None);
        if let Some(target_id) = enemy_hover(&state, &input) {
            conn.send(ClientMsg::Attack { target_id });
            // marqueur rouge sous la cible, comme `markers.spawn(hoverEntity())` du web
            if let Some(p) = state.pose(target_id) {
                let pos = Pos { x: p.x, y: p.y };
                input.clicks.push(markers::ClickMark { pos, attack: true });
            }
        } else if let Some(pos) = input.cursor {
            conn.send(ClientMsg::Move { pos });
            input.clicks.push(markers::ClickMark { pos, attack: false });
            input.right_held = true;
            input.since_follow = 0.0;
            input.last_follow = Some(pos);
        }
        return;
    }
    // Clic droit maintenu : le champion suit le curseur.
    input.since_follow += time.delta_secs();
    if input.right_held
        && let Some(pos) = input.cursor
        && input.since_follow >= FOLLOW_EVERY
        && input
            .last_follow
            .is_none_or(|l| (pos.x - l.x).hypot(pos.y - l.y) > FOLLOW_MIN_MOVE)
    {
        input.since_follow = 0.0;
        input.last_follow = Some(pos);
        conn.send(ClientMsg::Move { pos });
    }
}

#[allow(clippy::too_many_arguments)] // un système Bevy : ses paramètres sont ses accès au monde
pub(crate) fn key_orders(
    mut events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<GameState>,
    conn: Res<Connection>,
    mut input: ResMut<PlayerInput>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    fields: Query<(), With<bevy::text::EditableText>>,
) {
    // saisie en cours (recherche de la boutique, chat) : les lettres vont au champ, pas aux raccourcis
    let typing = focus.is_some_and(|f| f.get().is_some_and(|e| fields.contains(e)));
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for ev in events.read() {
        if !in_game(&state) {
            continue;
        }
        if ev.state == ButtonState::Released {
            key_released(ev, &state, &mut input, &conn);
            continue;
        }
        if ev.key_code == KeyCode::Tab {
            input.scoreboard_open = true;
            continue;
        }
        if ev.repeat {
            continue;
        }
        if ev.key_code == KeyCode::Escape {
            if input.targeting.is_some() {
                set_targeting(&mut input, None);
            } else if input.shop_open {
                input.shop_open = false;
            } else if typing {
                // saisie du chat : le champ se ferme lui-même
            } else {
                // Panneau de réglages : la souris est rendue au système tant qu'il est ouvert.
                input.settings_open = !input.settings_open;
            }
            continue;
        }
        if typing {
            continue;
        }
        if let Some(action) = digit_action(ev.key_code) {
            match action {
                DigitAction::Ward => trinket_key(&state, &mut input, &conn),
                DigitAction::Item(slot) => conn.send(ClientMsg::UseItem {
                    slot,
                    target_pos: input.cursor,
                }),
            }
            continue;
        }
        let Some(letter) = key_letter(&ev.logical_key, ev.key_code) else {
            continue;
        };
        let ability = match letter {
            'Q' => Some(AbilitySlot::Q),
            'W' => Some(AbilitySlot::W),
            'E' => Some(AbilitySlot::E),
            'R' => Some(AbilitySlot::R),
            _ => None,
        };
        if let Some(slot) = ability {
            if ctrl || alt {
                conn.send(ClientMsg::LevelAbility { slot });
            } else {
                cast_key(
                    Targeting::Ability(slot),
                    shift,
                    ev.key_code,
                    &state,
                    &mut input,
                    &conn,
                );
            }
            continue;
        }
        match letter {
            'D' | 'F' => {
                let slot = if letter == 'D' {
                    SummonerSlot::D
                } else {
                    SummonerSlot::F
                };
                cast_key(
                    Targeting::Summoner(slot),
                    shift,
                    ev.key_code,
                    &state,
                    &mut input,
                    &conn,
                );
            }
            'A' => {
                if input.settings.quick_cast {
                    confirm(Targeting::AttackMove, &state, &mut input, &conn);
                } else {
                    set_targeting(&mut input, Some(Targeting::AttackMove));
                }
            }
            'S' => {
                conn.send(ClientMsg::Stop);
                set_targeting(&mut input, None);
            }
            'B' => conn.send(ClientMsg::Recall),
            'P' => input.shop_open = !input.shop_open,
            'C' => input.stats_open = !input.stats_open,
            _ => {}
        }
    }
}

/// Relâchement : la touche qui a ouvert l'indicateur lance le sort ; Tab ferme les scores.
fn key_released(ev: &KeyboardInput, state: &GameState, input: &mut PlayerInput, conn: &Connection) {
    if input.cast_held == Some(ev.key_code) {
        input.cast_held = None;
        if let Some(t @ (Targeting::Ability(_) | Targeting::Summoner(_))) = input.targeting {
            confirm(t, state, input, conn);
        }
        return;
    }
    if ev.key_code == KeyCode::Tab {
        input.scoreboard_open = false;
    }
}

/// Viseur pendant un ciblage, main sur un ennemi survolé, flèche sinon.
fn update_cursor_icon(
    mut commands: Commands,
    window: Single<(Entity, Option<&CursorIcon>), With<Window>>,
    state: Res<GameState>,
    input: Res<PlayerInput>,
) {
    let icon: CursorIcon = if input.targeting.is_some() {
        SystemCursorIcon::Crosshair.into()
    } else if enemy_hover(&state, &input).is_some() {
        SystemCursorIcon::Pointer.into()
    } else {
        SystemCursorIcon::Default.into()
    };
    let (entity, current) = *window;
    if current != Some(&icon) {
        commands.entity(entity).insert(icon);
    }
}

fn window_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut Window>,
    focus: Option<Res<bevy::input_focus::InputFocus>>,
    fields: Query<(), With<bevy::text::EditableText>>,
) {
    if keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
    // un « v » tapé dans un champ (nom, chat, recherche) ne bascule pas la synchro verticale
    let typing = focus.is_some_and(|f| f.get().is_some_and(|e| fields.contains(e)));
    if !typing && keys.just_pressed(KeyCode::KeyV) {
        window.present_mode = match window.present_mode {
            PresentMode::AutoVsync => PresentMode::AutoNoVsync,
            _ => PresentMode::AutoVsync,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lov_shared::protocol::{GameStart, GameStartPlayer};
    use lov_shared::types::{EntitySnapshot, SummonerId, Team, TeamOrNeutral};
    use std::sync::mpsc::Receiver;

    /// La souris n'est confinée qu'en partie, en plein écran, fenêtre active et réglages fermés.
    #[test]
    fn souris_confinee_seulement_en_plein_ecran_en_partie() {
        let full = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
        assert_eq!(
            cursor_grab(true, true, false, full),
            CursorGrabMode::Confined
        );
        assert_eq!(
            cursor_grab(true, true, false, WindowMode::Windowed),
            CursorGrabMode::None,
            "fenêtré"
        );
        assert_eq!(
            cursor_grab(true, false, false, full),
            CursorGrabMode::None,
            "Alt+Tab"
        );
        assert_eq!(
            cursor_grab(true, true, true, full),
            CursorGrabMode::None,
            "réglages ouverts"
        );
        assert_eq!(
            cursor_grab(false, true, false, full),
            CursorGrabMode::None,
            "menus"
        );
    }

    /// #274 : un clic au milieu de la rivière (sol creusé) vise le point du lit sous le curseur, pas
    /// celui du plan y = 0, décalé vers la caméra.
    #[test]
    fn le_clic_vise_le_lit_de_la_riviere() {
        let target = Vec2::new(6000.0, 6000.0);
        let ground = to_ground(target, 0.0);
        assert!(ground.y < -0.2);
        let origin = ground + Vec3::new(0.0, 20.0, 12.0);
        let ray = Ray3d::new(origin, Dir3::new(ground - origin).expect("direction"));
        let flat = to_game(
            ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
                .expect("plan"),
        );
        assert!(flat.distance(target) > 10.0, "{flat}");
        let picked = pick_ground(ray).expect("sol");
        assert!(picked.distance(target) < 1.0, "{picked}");
        // sur une voie plate, rien ne change
        let lane = to_ground(Vec2::new(1100.0, 6000.0), 0.0);
        let o = lane + Vec3::new(0.0, 20.0, 12.0);
        let ray = Ray3d::new(o, Dir3::new(lane - o).expect("direction"));
        assert!(
            pick_ground(ray)
                .expect("sol")
                .distance(Vec2::new(1100.0, 6000.0))
                < 0.5
        );
    }

    fn champion(id: EntityId, team: TeamOrNeutral, subtype: &str) -> EntitySnapshot {
        EntitySnapshot {
            id,
            kind: EntityKind::Champion,
            team,
            x: 1000.0,
            y: 1000.0,
            facing: 0.0,
            hp: 100.0,
            max_hp: 100.0,
            mana: None,
            max_mana: None,
            shield: None,
            dead: None,
            subtype: Some(subtype.into()),
            owner_id: None,
            level: None,
            anim: None,
            buffs: None,
            champ: None,
        }
    }

    /// Partie avec Grondak (joueur local, id 1) et un ennemi (id 2).
    fn setup() -> (GameState, PlayerInput, Connection, Receiver<ClientMsg>) {
        let mut state = GameState::default();
        state.game = Some(GameStart {
            you: "p".into(),
            team: Team::Blue,
            entity_id: 1,
            players: vec![GameStartPlayer {
                id: "p".into(),
                name: "P".into(),
                team: Team::Blue,
                champion_id: "grondak".into(),
                summoners: [SummonerId::Flash, SummonerId::Ignite],
            }],
            server_time: 0.0,
        });
        state.my_entity = Some(1);
        state
            .buffer
            .entities
            .insert(1, champion(1, TeamOrNeutral::Blue, "grondak"));
        state
            .buffer
            .entities
            .insert(2, champion(2, TeamOrNeutral::Red, "tharox"));
        let input = PlayerInput {
            cursor: Some(Pos {
                x: 1200.0,
                y: 1000.0,
            }),
            ..PlayerInput::default()
        };
        let (conn, sent) = Connection::for_tests();
        (state, input, conn, sent)
    }

    /// Un clic (gauche ou droit) sur un panneau du HUD pendant un ciblage n'envoie rien et garde le
    /// ciblage ; hors du HUD, le même clic lance le sort.
    #[test]
    fn un_clic_sur_l_interface_n_envoie_rien_et_garde_le_ciblage() {
        use bevy::ecs::system::RunSystemOnce;
        let (state, mut input, conn, sent) = setup();
        // W de Grondak : sans cible, lancé au clic
        set_targeting(&mut input, Some(Targeting::Ability(AbilitySlot::W)));
        let mut world = World::new();
        let mut buttons = ButtonInput::<MouseButton>::default();
        buttons.press(MouseButton::Left);
        buttons.press(MouseButton::Right);
        world.insert_resource(buttons);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(state);
        world.insert_resource(conn);
        world.insert_resource(Time::<()>::default());
        world.insert_resource(input);
        world.insert_resource(crate::ui::PointerOverUi(true));
        assert!(world.run_system_once(mouse_orders).is_ok());
        assert!(sent.try_recv().is_err(), "aucun ordre parti");
        assert_eq!(
            world.resource::<PlayerInput>().targeting,
            Some(Targeting::Ability(AbilitySlot::W))
        );
        // hors du HUD, le clic gauche lance le sort
        world.insert_resource(crate::ui::PointerOverUi(false));
        let mut buttons = ButtonInput::<MouseButton>::default();
        buttons.press(MouseButton::Left);
        world.insert_resource(buttons);
        assert!(world.run_system_once(mouse_orders).is_ok());
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::Cast {
                slot: AbilitySlot::W,
                ..
            })
        ));
    }

    #[test]
    fn un_sort_cible_attend_un_clic_sur_une_unite() {
        let (state, mut input, conn, sent) = setup();
        // E de Grondak : charge ciblée.
        set_targeting(&mut input, Some(Targeting::Ability(AbilitySlot::E)));
        confirm(
            Targeting::Ability(AbilitySlot::E),
            &state,
            &mut input,
            &conn,
        );
        assert!(
            sent.try_recv().is_err(),
            "pas d'unité survolée : rien n'est envoyé"
        );
        assert_eq!(input.targeting, Some(Targeting::Ability(AbilitySlot::E)));
        input.hover = Some(2);
        confirm(
            Targeting::Ability(AbilitySlot::E),
            &state,
            &mut input,
            &conn,
        );
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::Cast {
                slot: AbilitySlot::E,
                target_id: Some(2),
                ..
            })
        ));
        assert_eq!(input.targeting, None);
    }

    #[test]
    fn cast_normal_ouvre_l_indicateur_et_le_quick_cast_lance_aussitot() {
        let (state, mut input, conn, sent) = setup();
        let q = Targeting::Ability(AbilitySlot::Q);
        cast_key(q, false, KeyCode::KeyQ, &state, &mut input, &conn);
        assert_eq!(input.targeting, Some(q));
        assert!(sent.try_recv().is_err());
        // Maj inverse le mode : lancer immédiat vers le curseur.
        set_targeting(&mut input, None);
        cast_key(q, true, KeyCode::KeyQ, &state, &mut input, &conn);
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::Cast {
                slot: AbilitySlot::Q,
                target_pos: Some(_),
                ..
            })
        ));
    }

    #[test]
    fn attaque_deplacement_attaque_l_ennemi_survole_sinon_va_au_sol() {
        let (state, mut input, conn, sent) = setup();
        confirm(Targeting::AttackMove, &state, &mut input, &conn);
        assert!(matches!(sent.try_recv(), Ok(ClientMsg::AttackMove { .. })));
        input.hover = Some(2);
        confirm(Targeting::AttackMove, &state, &mut input, &conn);
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::Attack { target_id: 2 })
        ));
        // Un allié survolé ne se fait pas attaquer.
        input.hover = Some(1);
        confirm(Targeting::AttackMove, &state, &mut input, &conn);
        assert!(matches!(sent.try_recv(), Ok(ClientMsg::AttackMove { .. })));
    }

    /// #273 : clic droit au sol → marqueur vert ; attaque-déplacement au sol → rouge ; le marqueur
    /// se pose, puis s'éteint au bout de 0,45 s.
    #[test]
    fn les_ordres_au_sol_posent_un_marqueur() {
        use bevy::ecs::system::RunSystemOnce;
        let (state, mut input, conn, _sent) = setup();
        let ground = input.cursor.expect("curseur au sol");
        confirm(Targeting::AttackMove, &state, &mut input, &conn);
        assert_eq!(
            input.clicks,
            [markers::ClickMark {
                pos: ground,
                attack: true
            }]
        );
        // l'ennemi survolé est attaqué : pas de marqueur au sol
        input.clicks.clear();
        input.hover = Some(2);
        confirm(Targeting::AttackMove, &state, &mut input, &conn);
        assert!(input.clicks.is_empty());
        input.hover = None;

        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<StandardMaterial>>();
        world.init_resource::<markers::MarkerCursor>();
        let mut buttons = ButtonInput::<MouseButton>::default();
        buttons.press(MouseButton::Right);
        world.insert_resource(buttons);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(state);
        world.insert_resource(conn);
        world.insert_resource(Time::<()>::default());
        world.insert_resource(input);
        assert!(world.run_system_once(markers::spawn_markers).is_ok());
        assert!(world.run_system_once(mouse_orders).is_ok());
        assert_eq!(
            world.resource::<PlayerInput>().clicks,
            [markers::ClickMark {
                pos: ground,
                attack: false
            }]
        );
        assert!(world.run_system_once(markers::update_markers).is_ok());
        assert!(world.resource::<PlayerInput>().clicks.is_empty());
        let shown = |world: &mut World| {
            world
                .query::<(&Visibility, &Transform)>()
                .iter(world)
                .filter(|(v, _)| **v != Visibility::Hidden)
                .map(|(_, t)| t.translation)
                .collect::<Vec<_>>()
        };
        let on = shown(&mut world);
        assert_eq!(on.len(), 1);
        assert_eq!(on[0], markers::marker_pos(ground));
        // 0,45 s plus tard, il est éteint
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(460));
        assert!(world.run_system_once(markers::update_markers).is_ok());
        assert!(shown(&mut world).is_empty());
    }

    /// #336 : touche 4 = visée de balise avec la Sentinelle, utilisation immédiate d'un trinket actif.
    #[test]
    fn la_touche_4_utilise_le_trinket_equipe() {
        let (mut state, mut input, conn, sent) = setup();
        let equip = |state: &mut GameState, trinket: &str| {
            let me = state.buffer.entities.get_mut(&1).unwrap();
            let mut items = vec![None; 6];
            items.push(Some(trinket.to_owned()));
            me.champ = Some(lov_shared::types::ChampSnapshot {
                player_id: "p".into(),
                name: "P".into(),
                kills: 0,
                deaths: 0,
                assists: 0,
                cs: 0,
                items,
                respawn_in: None,
            });
        };
        equip(&mut state, "stealth_ward");
        trinket_key(&state, &mut input, &conn);
        assert_eq!(input.targeting, Some(Targeting::Ward));
        assert!(sent.try_recv().is_err());
        set_targeting(&mut input, None);
        // Trinket à actif (même chemin que la Lanterne révélatrice) : utilisé aussitôt.
        equip(&mut state, "colossus_eye");
        trinket_key(&state, &mut input, &conn);
        assert_eq!(input.targeting, None);
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::UseItem { slot: -1, .. })
        ));
    }

    #[test]
    fn un_sort_non_appris_ou_en_recharge_ne_part_pas() {
        let (mut state, mut input, conn, sent) = setup();
        let mut self_state = lov_shared::types::SelfState {
            entity_id: 1,
            gold: 0.0,
            xp: 0.0,
            xp_to_next: 0.0,
            level: 1,
            skill_points: 0,
            ability_levels: lov_shared::types::AbilityMap {
                q: 0,
                w: 1,
                e: 1,
                r: 0,
            },
            cooldowns: Cooldowns {
                q: 0.0,
                w: 3.0,
                e: 0.0,
                r: 0.0,
                d: 0.0,
                f: 0.0,
            },
            summoners: lov_shared::types::SummonerMap {
                d: SummonerId::Flash,
                f: SummonerId::Ignite,
            },
            stats: get_champion("grondak").unwrap().base_stats.clone(),
            recalling: None,
            can_shop: false,
            ward_charges: None,
            item_charges: None,
            item_cooldowns: None,
        };
        input.settings.quick_cast = true;
        state.self_state = Some(self_state.clone());
        cast_key(
            Targeting::Ability(AbilitySlot::Q),
            false,
            KeyCode::KeyQ,
            &state,
            &mut input,
            &conn,
        );
        cast_key(
            Targeting::Ability(AbilitySlot::W),
            false,
            KeyCode::KeyW,
            &state,
            &mut input,
            &conn,
        );
        assert!(sent.try_recv().is_err(), "Q non appris, W en recharge");
        self_state.cooldowns.w = 0.0;
        state.self_state = Some(self_state);
        cast_key(
            Targeting::Ability(AbilitySlot::W),
            false,
            KeyCode::KeyW,
            &state,
            &mut input,
            &conn,
        );
        assert!(matches!(
            sent.try_recv(),
            Ok(ClientMsg::Cast {
                slot: AbilitySlot::W,
                ..
            })
        ));
    }
}
