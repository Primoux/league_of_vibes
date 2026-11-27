//! Bande-son générée : séquenceur des thèmes, intensité de combat, enchaînements et jingles de fin.
//!
//! Portage de `packages/client/src/audio/music.ts`.
//! - Menu / salon : « La Faille des Vibes » (héroïque). Draft : « Le Choix » (tendu).
//! - Partie : « Sous la canopée », ambiance douce et générative (motifs différents à chaque boucle) ; les
//!   combats ajoutent de légères couches puis tout redescend au calme.
//! - Fin : fanfare de victoire ou complainte de défaite.
//!
//! Le cœur ([`Music`]) est pur : il reçoit l'heure du bus (`ctx.currentTime`, pour dater les notes) et
//! l'heure réelle (`performance.now()`, pour les minuteries et la décroissance de l'intensité), et rend ses
//! commandes par une fonction. Le système Bevy [`advance_music`] le branche sur [`GameState`] (le `store`)
//! et sur les [`ServerEvent`] (le `fxBus`).

use bevy::prelude::*;
use lov_shared::protocol::RoomPhase;
use lov_shared::types::{EntityId, EntityKind, GameEvent, MonsterType};

use super::super::AudioManager;
use super::super::mixer::Channel;
use super::themes::{ThemeId, defeat, theme, victory};
use super::{BusCommand, MusicBusHandle, Score};
use crate::net::{GameState, ServerEvent};

/// Secondes programmées à l'avance.
pub const LOOKAHEAD: f64 = 0.18;
/// Durée de référence des fondus (secondes).
pub const FADE: f64 = 1.6;

/// Ce que l'écran demande : un thème, ou la fin de partie (`ThemeId | 'end'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Theme(ThemeId),
    End,
}

/// Thème voulu selon l'état du client (`themeFor`, pur, testé) : résultat, partie, draft hors salon, menu.
pub fn theme_for(game: bool, result: bool, draft: bool, lobby_phase: Option<RoomPhase>) -> Want {
    if result {
        Want::End
    } else if game {
        Want::Theme(ThemeId::Game)
    } else if draft && lobby_phase != Some(RoomPhase::Lobby) {
        Want::Theme(ThemeId::Draft)
    } else {
        Want::Theme(ThemeId::Menu)
    }
}

/// [`theme_for`] lu sur l'état du client.
pub fn want_of(s: &GameState) -> Want {
    theme_for(
        s.game.is_some(),
        s.result.is_some(),
        s.draft.is_some(),
        s.lobby.as_ref().map(|l| l.phase),
    )
}

/// Intensité de combat : montée par événements, décroissance continue (pur, testé).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Intensity {
    pub value: f64,
    pub floor: f64,
}

impl Intensity {
    pub fn bump(&mut self, amount: f64) {
        self.value = (self.value + amount).min(1.0);
    }

    pub fn decay(&mut self, dt: f64) {
        self.value = self.floor.max(self.value - dt * 0.06);
    }
}

/// Le séquenceur (`class Music`). Aucune allocation propre : la phrase en cours est un [`Score`] vide
/// tant qu'aucun pas n'est dû, et elle part en un seul message par image.
#[derive(Debug, Default)]
pub struct Music {
    theme: Option<ThemeId>,
    wanted: Option<Want>,
    step: u32,
    loop_: u32,
    next_time: f64,
    /// Le minuteur de la boucle tourne (`timer`).
    running: bool,
    /// Enchaînement en cours : heure réelle de fin du fondu, et thème demandé à son début.
    switch: Option<(f64, ThemeId)>,
    ended: bool,
    /// Jingle de fin programmé : heure réelle de départ, victoire ou défaite.
    stinger: Option<(f64, bool)>,
    pub intensity: Intensity,
    pending: Score,
}

impl Music {
    pub fn theme(&self) -> Option<ThemeId> {
        self.theme
    }

    /// Suit l'écran (`follow`) : `win` dit si la partie terminée est gagnée, `game_time` est le temps de
    /// partie (secondes), `real` l'heure réelle, `now` l'heure du bus.
    pub fn follow(
        &mut self,
        want: Want,
        win: bool,
        game_time: f64,
        real: f64,
        now: f64,
        out: &mut impl FnMut(BusCommand),
    ) {
        if want == Want::End {
            if !self.ended {
                self.ended = true;
                self.running = false;
                // Écart : le web laisse un enchaînement en cours relancer l'ancien thème sous le jingle.
                self.switch = None;
                out(BusCommand::Fade {
                    to: 0.0,
                    seconds: 0.4,
                });
                self.stinger = Some((real + 0.45, win));
                self.theme = None;
            }
            self.wanted = Some(Want::End);
            return;
        }
        self.ended = false;
        self.wanted = Some(want);
        // fond de jeu : la tension de base monte avec le temps de partie (fin de partie plus épique)
        self.intensity.floor = if game_time > 25.0 * 60.0 * 0.75 {
            0.3
        } else if game_time > 15.0 * 60.0 * 0.75 {
            0.15
        } else {
            0.0
        };
        self.ensure(real, now, out);
    }

    /// Programme le jingle de fin `win` dans 450 ms (le `setTimeout` de `follow`).
    pub fn schedule_stinger(&mut self, win: bool, real: f64) {
        self.stinger = Some((real + 0.45, win));
    }

    fn ensure(&mut self, real: f64, now: f64, out: &mut impl FnMut(BusCommand)) {
        let Some(Want::Theme(want)) = self.wanted else {
            return;
        };
        if Some(want) == self.theme || self.switch.is_some() {
            return;
        }
        if self.theme.is_none() {
            self.begin(want, now, out);
            return;
        }
        // fondu enchaîné : on baisse, on change de thème, on remonte
        self.switch = Some((real + FADE * 0.6, want));
        out(BusCommand::Fade {
            to: 0.0,
            seconds: FADE * 0.6,
        });
    }

    fn begin(&mut self, id: ThemeId, now: f64, out: &mut impl FnMut(BusCommand)) {
        let th = theme(id);
        self.theme = Some(id);
        self.step = 0;
        self.loop_ = 0;
        self.next_time = now + 0.12;
        out(BusCommand::SetDelay(60.0 / th.bpm * 0.75));
        let game = id == ThemeId::Game;
        out(BusCommand::Fade {
            to: if game { 0.55 } else { 0.95 },
            seconds: FADE * if game { 3.0 } else { 1.0 },
        });
        self.running = true;
    }

    /// Avance d'une image : minuteries échues, décroissance de l'intensité (`dt` réel), puis tous les pas
    /// dus avant `now + LOOKAHEAD`, envoyés en une seule phrase.
    pub fn tick(&mut self, real: f64, dt: f64, now: f64, out: &mut impl FnMut(BusCommand)) {
        if let Some((at, want)) = self.switch
            && real >= at
        {
            self.switch = None;
            let id = match self.wanted {
                Some(Want::Theme(w)) => w,
                _ => want,
            };
            self.begin(id, now, out);
        }
        if let Some((at, win)) = self.stinger
            && real >= at
        {
            self.stinger = None;
            out(BusCommand::Fade {
                to: 0.9,
                seconds: 0.3,
            });
            let mut s = Score::new();
            if win {
                victory(&mut s, now + 0.1);
            } else {
                defeat(&mut s, now + 0.1);
            }
            out(BusCommand::Play(s));
        }
        if !self.running || self.theme.is_none() {
            return;
        }
        self.intensity.decay(dt);
        let mut pending = std::mem::take(&mut self.pending);
        self.schedule(now, |th, step, loop_, t, intensity, sd| {
            (th.play)(&mut pending, step, loop_, t, intensity, sd)
        });
        if pending.is_empty() {
            self.pending = pending;
        } else {
            out(BusCommand::Play(pending));
        }
    }

    /// Le cœur de `tick` : appelle `play` pour chaque pas dû, en temps du bus.
    fn schedule(
        &mut self,
        now: f64,
        mut play: impl FnMut(&super::themes::Theme, u32, u32, f64, f64, f64),
    ) {
        let Some(id) = self.theme else {
            return;
        };
        let th = theme(id);
        let sd = th.step_dur();
        let total = th.bars * 16;
        // fenêtre en arrière-plan ou image très longue : on ne rattrape pas des secondes de retard
        if self.next_time < now - 0.5 {
            self.next_time = now + 0.05;
        }
        while self.next_time < now + LOOKAHEAD {
            play(
                th,
                self.step,
                self.loop_,
                self.next_time,
                self.intensity.value,
                sd,
            );
            self.next_time += sd;
            self.step += 1;
            if self.step >= total {
                self.step = 0;
                self.loop_ += 1;
            }
        }
    }

    /// Réagit à un événement de jeu (`onEvent`) : seul le thème de partie s'intensifie. `me` est l'entité
    /// du joueur, `kind` donne la nature d'une entité.
    pub fn on_event(
        &mut self,
        e: &GameEvent,
        me: Option<EntityId>,
        kind: impl Fn(EntityId) -> Option<EntityKind>,
    ) {
        if self.theme != Some(ThemeId::Game) {
            return;
        }
        let is_me = |id: EntityId| me == Some(id);
        match e {
            GameEvent::Damage { source, target, .. } => {
                if is_me(*target) || is_me(*source) {
                    self.intensity
                        .bump(if is_me(*target) { 0.07 } else { 0.05 });
                }
            }
            GameEvent::AbilityCast { source, .. } => {
                if is_me(*source) {
                    self.intensity.bump(0.03);
                }
            }
            GameEvent::Death { target, killer, .. } => {
                if kind(*target) == Some(EntityKind::Champion) {
                    let mine = is_me(*target) || killer.is_some_and(is_me);
                    self.intensity.bump(if mine { 0.45 } else { 0.25 });
                }
            }
            GameEvent::StructureDestroyed { .. } => self.intensity.bump(0.3),
            GameEvent::ObjectiveTaken { monster, .. } => {
                if matches!(
                    monster,
                    MonsterType::Dragon
                        | MonsterType::Leviathan
                        | MonsterType::ElderDragon
                        | MonsterType::Herald
                ) {
                    self.intensity.bump(0.5);
                }
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------------------------------- Bevy

/// Musique du client : le séquenceur et son bus (canal Musique), créé au premier thème.
#[derive(Resource, Default)]
pub struct MusicPlayer {
    pub music: Music,
    bus: Option<MusicBusHandle>,
}

/// Branche la musique sur l'état du client (`music.install()`). Inactif sous `LOV_AUDIOLAB`, dont la
/// galerie joue ses propres sons.
pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        if !game_mode() {
            return;
        }
        app.add_message::<ServerEvent>()
            .init_resource::<MusicPlayer>()
            .add_systems(Update, advance_music);
    }
}

/// Le web n'installe l'audio que dans le jeu, pas dans ses bancs d'essai : on reprend ici les modes
/// hors ligne de `main.rs` (charge, vue, interface, FX, audio), où la musique ne suit pas l'écran.
fn game_mode() -> bool {
    crate::render::stress::units_from_env().is_none()
        && !crate::render::viewlab::enabled()
        && crate::ui::lab::requested().is_none()
        && !crate::fx::demo::enabled()
        && !super::super::demo::enabled()
}

/// Chaque image : suit l'écran, lit les événements de jeu, puis avance le séquenceur. Rien n'est alloué
/// sur une image sans pas dû.
pub fn advance_music(
    time: Res<Time<Real>>,
    state: Res<GameState>,
    mut events: MessageReader<ServerEvent>,
    mut audio: ResMut<AudioManager>,
    mut player: ResMut<MusicPlayer>,
) {
    let MusicPlayer { music, bus } = &mut *player;
    let want = want_of(&state);
    // le web ne crée le bus qu'en suivant un thème : une fin de partie vue en premier reste muette
    if bus.is_none() && want == Want::End {
        events.clear();
        return;
    }
    let bus = bus.get_or_insert_with(|| audio.music_bus(Channel::Music));
    let real = time.elapsed_secs_f64();
    let now = bus.current_time();
    let mut out = |cmd| audio.bus(bus, cmd);
    let win = match (&state.result, &state.game) {
        (Some(r), Some(g)) => r.winner == g.team,
        _ => false,
    };
    music.follow(want, win, state.time, real, now, &mut out);
    // `snapshot.self.entityId ?? game.entityId`
    let me = state
        .self_state
        .as_ref()
        .map(|x| x.entity_id)
        .or(state.game.as_ref().map(|g| g.entity_id));
    let entities = state.entities();
    for ServerEvent(e) in events.read() {
        music.on_event(e, me, |id| entities.get(&id).map(|x| x.kind));
    }
    music.tick(real, time.delta_secs_f64(), now, &mut out);
}

#[cfg(test)]
mod tests {
    use super::super::super::Command;
    use super::super::super::synth::SAMPLE_RATE;
    use super::*;

    const SR: f64 = SAMPLE_RATE as f64;

    /// Heure du bus telle que le jeu la lit : publiée tous les blocs de 256 échantillons.
    fn bus_clock(t: f64) -> f64 {
        ((t * SR) as u64 / 256 * 256) as f64 / SR
    }

    fn collect(v: &mut Vec<BusCommand>) -> impl FnMut(BusCommand) + '_ {
        |c| v.push(c)
    }

    // Port de `music.test.ts` : « choisit le thème selon l'écran ».
    #[test]
    fn choisit_le_theme_selon_l_ecran() {
        assert_eq!(
            theme_for(false, false, false, None),
            Want::Theme(ThemeId::Menu)
        );
        assert_eq!(
            theme_for(false, false, true, Some(RoomPhase::Draft)),
            Want::Theme(ThemeId::Draft)
        );
        assert_eq!(
            theme_for(true, false, false, None),
            Want::Theme(ThemeId::Game)
        );
        assert_eq!(theme_for(true, true, false, None), Want::End);
        // le draft affiché depuis le salon reste sur le menu
        assert_eq!(
            theme_for(false, false, true, Some(RoomPhase::Lobby)),
            Want::Theme(ThemeId::Menu)
        );
        assert_eq!(want_of(&GameState::default()), Want::Theme(ThemeId::Menu));
    }

    // Port de `music.test.ts` : « fait monter puis redescendre l'intensité de combat ».
    #[test]
    fn fait_monter_puis_redescendre_l_intensite_de_combat() {
        let mut i = Intensity::default();
        i.bump(0.5);
        i.bump(0.8);
        assert_eq!(i.value, 1.0);
        i.decay(10.0);
        assert!((i.value - 0.4).abs() < 1e-9);
        i.floor = 0.3;
        i.decay(100.0);
        assert_eq!(i.value, 0.3);
    }

    /// Le séquenceur ne programme jamais un pas dans le passé de l'horloge, et la grille ne dérive pas
    /// sur dix minutes simulées, avec des images irrégulières (5 à 60 ms) et une horloge publiée par blocs.
    #[test]
    fn ne_programme_jamais_en_retard_et_ne_derive_pas() {
        for id in [ThemeId::Menu, ThemeId::Draft, ThemeId::Game] {
            let mut m = Music::default();
            let mut cmds = Vec::new();
            m.follow(
                Want::Theme(id),
                false,
                0.0,
                0.0,
                0.0,
                &mut collect(&mut cmds),
            );
            let sd = theme(id).step_dur();
            let t0 = 0.12;
            let (mut real, mut seed, mut k) = (0.0f64, 7u32, 0u64);
            while real < 600.0 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                real += 0.005 + f64::from(seed >> 8) / f64::from(1u32 << 24) * 0.055;
                let now = bus_clock(real);
                m.schedule(now, |_, _, _, t, _, _| {
                    assert!(t >= now, "{id:?} : pas à {t} programmé à {now}");
                    let ideal = t0 + k as f64 * sd;
                    assert!((t - ideal).abs() < 1e-6, "{id:?} : dérive de {}", t - ideal);
                    k += 1;
                });
                // toujours au moins `LOOKAHEAD` d'avance, jamais beaucoup plus
                assert!(
                    m.next_time >= now + LOOKAHEAD && m.next_time < now + LOOKAHEAD + sd + 1e-9
                );
            }
            let expected = (bus_clock(real) + LOOKAHEAD - t0) / sd;
            assert!((k as f64 - expected).abs() <= 1.0, "{id:?} : {k} pas");
        }
    }

    /// Une fenêtre gelée plus de 0,5 s : on reprend juste après l'horloge au lieu de tout rattraper.
    #[test]
    fn ne_rattrape_pas_une_longue_absence() {
        let mut m = Music::default();
        let mut cmds = Vec::new();
        m.follow(
            Want::Theme(ThemeId::Menu),
            false,
            0.0,
            0.0,
            0.0,
            &mut collect(&mut cmds),
        );
        m.schedule(0.0, |_, _, _, _, _, _| {});
        let mut first = None;
        m.schedule(3.0, |_, _, _, t, _, _| {
            first.get_or_insert(t);
        });
        assert_eq!(first, Some(3.05));
    }

    #[test]
    fn le_changement_de_theme_fait_un_fondu() {
        let mut m = Music::default();
        let mut cmds = Vec::new();
        m.follow(
            Want::Theme(ThemeId::Menu),
            false,
            0.0,
            0.0,
            0.0,
            &mut collect(&mut cmds),
        );
        assert!(
            matches!(cmds[..], [BusCommand::SetDelay(_), BusCommand::Fade { to, seconds }] if to == 0.95 && seconds == FADE)
        );
        m.tick(0.0, 0.0, 0.0, &mut collect(&mut cmds));
        assert!(matches!(cmds.last(), Some(BusCommand::Play(_))));
        cmds.clear();

        // le draft arrive : on baisse le menu, qui continue de jouer pendant le fondu
        m.follow(
            Want::Theme(ThemeId::Draft),
            false,
            0.0,
            10.0,
            10.0,
            &mut collect(&mut cmds),
        );
        assert!(
            matches!(cmds[..], [BusCommand::Fade { to, seconds }] if to == 0.0 && (seconds - 0.96).abs() < 1e-9)
        );
        cmds.clear();
        for i in 1..=18 {
            let t = 10.0 + f64::from(i) * 0.05;
            m.tick(t, 0.05, t, &mut collect(&mut cmds));
        }
        assert_eq!(m.theme(), Some(ThemeId::Menu));
        assert!(!cmds.is_empty());
        assert!(cmds.iter().all(|c| matches!(c, BusCommand::Play(_))));
        cmds.clear();
        // rien ne se relance tant que le fondu n'est pas fini
        m.follow(
            Want::Theme(ThemeId::Draft),
            false,
            0.0,
            10.9,
            10.9,
            &mut collect(&mut cmds),
        );
        assert!(cmds.is_empty());

        // fin du fondu : le draft démarre, délai calé sur son tempo, et remonte
        m.tick(10.97, 0.47, 10.97, &mut collect(&mut cmds));
        assert_eq!(m.theme(), Some(ThemeId::Draft));
        assert!(
            matches!(cmds[0], BusCommand::SetDelay(d) if (d - 60.0 / 112.0 * 0.75).abs() < 1e-9)
        );
        assert!(
            matches!(cmds[1], BusCommand::Fade { to, seconds } if to == 0.95 && seconds == FADE)
        );
        cmds.clear();

        // la partie : fondu plus long et plus bas
        m.follow(
            Want::Theme(ThemeId::Game),
            false,
            0.0,
            20.0,
            20.0,
            &mut collect(&mut cmds),
        );
        m.tick(21.0, 1.0, 21.0, &mut collect(&mut cmds));
        assert_eq!(m.theme(), Some(ThemeId::Game));
        assert!(cmds.iter().any(|c| matches!(c, BusCommand::Fade { to, seconds } if *to == 0.55 && (*seconds - 4.8).abs() < 1e-9)));
    }

    #[test]
    fn la_fin_de_partie_coupe_la_boucle_puis_joue_le_jingle() {
        for win in [true, false] {
            let mut m = Music::default();
            let mut cmds = Vec::new();
            m.follow(
                Want::Theme(ThemeId::Game),
                false,
                0.0,
                0.0,
                0.0,
                &mut collect(&mut cmds),
            );
            // un enchaînement en cours ne relance pas l'ancien thème sous le jingle
            m.follow(
                Want::Theme(ThemeId::Menu),
                false,
                0.0,
                1.0,
                1.0,
                &mut collect(&mut cmds),
            );
            cmds.clear();
            m.follow(Want::End, win, 0.0, 1.2, 1.2, &mut collect(&mut cmds));
            assert!(
                matches!(cmds[..], [BusCommand::Fade { to, seconds }] if to == 0.0 && seconds == 0.4)
            );
            assert_eq!(m.theme(), None);
            cmds.clear();
            m.tick(1.4, 0.2, 1.4, &mut collect(&mut cmds));
            assert!(cmds.is_empty(), "boucle arrêtée, jingle pas encore dû");
            m.follow(Want::End, win, 0.0, 1.5, 1.5, &mut collect(&mut cmds));
            m.tick(1.7, 0.3, 1.7, &mut collect(&mut cmds));
            match &cmds[..] {
                [BusCommand::Fade { to, seconds }, BusCommand::Play(s)] => {
                    assert!(*to == 0.9 && *seconds == 0.3);
                    let mut expected = Score::new();
                    if win {
                        victory(&mut expected, 1.8);
                    } else {
                        defeat(&mut expected, 1.8);
                    }
                    assert_eq!(s.len(), expected.len());
                    assert!((s.end() - expected.end()).abs() < 1e-9);
                }
                other => panic!("fondu puis jingle attendus : {other:?}"),
            }
            cmds.clear();
            for i in 0..100 {
                let t = 2.0 + f64::from(i) * 0.1;
                m.tick(t, 0.1, t, &mut collect(&mut cmds));
            }
            assert!(cmds.is_empty(), "plus rien après le jingle");
            assert_eq!(m.theme(), None);
            // retour au menu : le thème repart
            m.follow(
                Want::Theme(ThemeId::Menu),
                false,
                0.0,
                20.0,
                20.0,
                &mut collect(&mut cmds),
            );
            assert_eq!(m.theme(), Some(ThemeId::Menu));
        }
    }

    #[test]
    fn les_evenements_font_monter_l_intensite_en_partie_seulement() {
        let mut m = Music::default();
        let kind = |id: EntityId| {
            Some(if id < 10 {
                EntityKind::Champion
            } else {
                EntityKind::Minion
            })
        };
        let death = |target, killer| GameEvent::Death {
            target,
            killer,
            assists: None,
            bounty: None,
        };
        m.on_event(&death(1, Some(2)), Some(1), kind);
        assert_eq!(m.intensity.value, 0.0, "hors partie : rien");
        let mut cmds = Vec::new();
        m.follow(
            Want::Theme(ThemeId::Game),
            false,
            0.0,
            0.0,
            0.0,
            &mut collect(&mut cmds),
        );
        m.on_event(&death(1, Some(2)), Some(1), kind);
        assert!((m.intensity.value - 0.45).abs() < 1e-9, "ma mort");
        m.intensity.value = 0.0;
        m.on_event(&death(3, Some(4)), Some(1), kind);
        assert!((m.intensity.value - 0.25).abs() < 1e-9, "un autre champion");
        m.intensity.value = 0.0;
        m.on_event(&death(30, Some(1)), Some(1), kind);
        assert_eq!(m.intensity.value, 0.0, "un sbire");
        m.on_event(
            &GameEvent::ObjectiveTaken {
                monster: MonsterType::Leviathan,
                team: lov_shared::types::Team::Blue,
                killer: None,
                element: None,
            },
            Some(1),
            kind,
        );
        assert!((m.intensity.value - 0.5).abs() < 1e-9);
        // fin de partie : la tension de base suit le temps de jeu
        m.follow(
            Want::Theme(ThemeId::Game),
            false,
            20.0 * 60.0,
            0.0,
            0.0,
            &mut collect(&mut cmds),
        );
        assert_eq!(m.intensity.floor, 0.3);
        m.intensity.decay(1000.0);
        assert_eq!(m.intensity.value, 0.3);
    }

    /// Sur les images sans pas dû, le séquenceur n'envoie rien et n'alloue rien : la phrase en attente
    /// reste un `Vec` vide de capacité nulle, réutilisé d'une image à l'autre.
    #[test]
    fn aucune_allocation_sur_une_image_sans_pas_du() {
        let mut m = Music::default();
        let mut cmds = Vec::new();
        m.follow(
            Want::Theme(ThemeId::Game),
            false,
            0.0,
            0.0,
            0.0,
            &mut collect(&mut cmds),
        );
        m.tick(0.0, 0.0, 0.0, &mut collect(&mut cmds));
        cmds.clear();
        // l'horloge du bus est figée (pas de carte son) : plus aucun pas n'est dû
        let mut idle = 0;
        for i in 0..1000 {
            let real = f64::from(i) / 60.0;
            m.follow(
                Want::Theme(ThemeId::Game),
                false,
                0.0,
                real,
                0.0,
                &mut collect(&mut cmds),
            );
            m.tick(real, 1.0 / 60.0, 0.0, &mut collect(&mut cmds));
            assert!(cmds.is_empty());
            assert_eq!(m.pending.notes.capacity(), 0);
            idle += 1;
        }
        assert_eq!(idle, 1000);
        // une image avec des pas dus envoie une seule phrase, et la suivante repart d'un `Vec` vide
        let mut played = 0;
        for i in 0..120 {
            let t = 20.0 + f64::from(i) / 60.0;
            m.tick(t, 1.0 / 60.0, t - 19.0, &mut collect(&mut cmds));
            assert!(cmds.len() <= 1);
            played += cmds.len();
            cmds.clear();
            assert_eq!(m.pending.notes.capacity(), 0);
        }
        assert!(played > 0);
    }

    /// Le système Bevy : bus créé sur le canal Musique, menu lancé, puis la partie sur événement d'état.
    #[test]
    fn le_systeme_suit_l_etat_du_client() {
        let (am, rx) = AudioManager::for_tests();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<ServerEvent>()
            .insert_resource(am)
            .init_resource::<GameState>()
            .init_resource::<MusicPlayer>()
            .add_systems(Update, advance_music);
        app.update();
        let cmds: Vec<Command> = rx.try_iter().collect();
        assert!(matches!(
            cmds.first(),
            Some(Command::AddBus(Channel::Music, _))
        ));
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Command::Bus(_, BusCommand::Play(_))))
        );
        assert_eq!(
            app.world().resource::<MusicPlayer>().music.theme(),
            Some(ThemeId::Menu)
        );
        // sans fil audio, l'horloge du bus reste à 0 : après la première anticipation, plus rien ne part
        for _ in 0..100 {
            app.update();
        }
        let again: Vec<Command> = rx.try_iter().collect();
        assert!(
            !again.iter().any(|c| matches!(c, Command::AddBus(..))),
            "un seul bus"
        );
        assert!(
            !again
                .iter()
                .any(|c| matches!(c, Command::Bus(_, BusCommand::Play(_)))),
            "aucune phrase sur les images sans pas dû"
        );
    }

    /// Rend `seconds` de musique hors ligne : le séquenceur lit l'horloge publiée par un vrai bus, rendu
    /// par blocs de 256 trames. Renvoie la crête, la moyenne quadratique et le maximum de notes actives.
    fn render(
        mut setup: impl FnMut(&mut Music, f64, &mut dyn FnMut(BusCommand)),
        seconds: f64,
    ) -> (f32, f32, usize) {
        let (mut bus, handle) = super::super::MusicBus::new(1);
        let mut m = Music::default();
        let mut buf = [0.0f32; 512];
        let (mut peak, mut sum, mut n, mut most) = (0.0f32, 0.0f64, 0usize, 0usize);
        let mut first = true;
        while handle.current_time() < seconds {
            let now = handle.current_time();
            {
                let mut out = |c| bus.command(c);
                if first {
                    setup(&mut m, now, &mut out);
                    first = false;
                }
                m.tick(now, 256.0 / SR, now, &mut out);
            }
            buf.fill(0.0);
            bus.render_add(&mut buf, 1.0);
            most = most.max(bus.active());
            for x in buf {
                assert!(x.is_finite());
                peak = peak.max(x.abs());
                sum += f64::from(x * x);
                n += 1;
            }
        }
        (peak, (sum / n as f64).sqrt() as f32, most)
    }

    /// Chaque thème et chaque jingle, rendus par le vrai bus : audibles, bornés, et loin du plafond de notes
    /// du bus (au-delà, des notes seraient ignorées sans bruit).
    #[test]
    fn chaque_theme_et_chaque_jingle_se_rendent_par_le_bus() {
        use super::super::bus::MAX_NOTES;
        for (id, intensity) in [
            (ThemeId::Menu, 0.0),
            (ThemeId::Draft, 0.0),
            (ThemeId::Game, 0.0),
            (ThemeId::Game, 1.0),
        ] {
            let (peak, rms, most) = render(
                |m, now, out| {
                    m.follow(Want::Theme(id), false, 0.0, now, now, &mut |c| out(c));
                    m.intensity.value = intensity;
                    m.intensity.floor = intensity;
                },
                8.0,
            );
            assert!(
                rms > 1e-3 && peak < 1.5,
                "{id:?} à {intensity} : crête {peak}, rms {rms}"
            );
            assert!(most < MAX_NOTES / 4, "{id:?} à {intensity} : {most} notes");
        }
        for win in [true, false] {
            let (peak, rms, most) = render(
                |m, now, out| {
                    m.follow(Want::Theme(ThemeId::Menu), false, 0.0, now, now, &mut |c| {
                        out(c)
                    });
                    m.follow(Want::End, win, 0.0, now, now, &mut |c| out(c));
                },
                7.0,
            );
            assert!(
                rms > 1e-3 && peak < 1.5,
                "fin {win} : crête {peak}, rms {rms}"
            );
            assert!(most < MAX_NOTES / 4, "fin {win} : {most} notes");
        }
    }
}
