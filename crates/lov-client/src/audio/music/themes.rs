//! Thèmes musicaux originaux de League of Vibes (compositions écrites pour le jeu, en Ré mineur).
//!
//! Portage de `packages/client/src/audio/music/themes.ts`. Un thème = tempo + boucle de N mesures de
//! 16 doubles-croches + fonction qui joue un pas. `intensity` (0..1) ajoute des couches (percussions,
//! cuivres, cordes) pendant les combats. Les notes s'ajoutent à une [`Score`], datées en temps du bus.
//!
//! Rien n'est alloué ici en dehors des notes elles-mêmes : les tables sont statiques et le plan d'une
//! mesure du thème de partie ([`AmbientBar`]) tient dans un tableau fixe.

use super::Score;
use super::instruments::*;

/// Identifiant d'un thème (`ThemeId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeId {
    Menu,
    Draft,
    Game,
}

/// Joue le pas `step` (0..bars*16-1) de la boucle numéro `loop_` à l'instant `t` (temps du bus).
pub type PlayFn = fn(s: &mut Score, step: u32, loop_: u32, t: f64, intensity: f64, step_dur: f64);

/// Un thème : tempo, nombre de mesures de la boucle, et la fonction qui joue un pas.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bpm: f64,
    pub bars: u32,
    pub play: PlayFn,
}

impl Theme {
    /// Durée d'une double-croche (secondes).
    pub fn step_dur(&self) -> f64 {
        60.0 / self.bpm / 4.0
    }
}

// notes MIDI utiles (Ré mineur)
const D2: i32 = 38;
const F2: i32 = 41;
const G2: i32 = 43;
const A2: i32 = 45;
const BB2: i32 = 46;
const C3: i32 = 48;

/// Accords (voicings serrés, registre médium).
#[derive(Debug, Clone, Copy)]
enum Chord {
    Dm,
    Bb,
    F,
    C,
    Gm,
    A,
}

impl Chord {
    const fn notes(self) -> [i32; 3] {
        match self {
            Chord::Dm => [62, 65, 69],
            Chord::Bb => [58, 62, 65],
            Chord::F => [60, 65, 69],
            Chord::C => [60, 64, 67],
            Chord::Gm => [58, 62, 67],
            Chord::A => [61, 64, 69],
        }
    }

    /// Fondamentale grave (`ROOT`).
    const fn root(self) -> i32 {
        match self {
            Chord::Dm => D2,
            Chord::Bb => BB2,
            Chord::F => F2,
            Chord::C => C3 - 12,
            Chord::Gm => G2,
            Chord::A => A2,
        }
    }
}

/// Mélodie : (pas dans la boucle, note MIDI, durée en pas).
type Phrase = [(u32, i32, u32)];

// ------------------------------------------------------------------ MENU : « La Faille des Vibes » (héroïque, ample)
const MENU_PROG: [Chord; 8] = {
    use Chord::*;
    [Dm, Bb, F, C, Gm, Bb, A, A]
};
const MENU_THEME: &Phrase = &[
    (0, 69, 4),
    (4, 74, 4),
    (8, 77, 6),
    (14, 76, 2),
    (16, 74, 8),
    (24, 77, 4),
    (28, 82, 4),
    (32, 81, 6),
    (38, 79, 2),
    (40, 77, 4),
    (44, 72, 4),
    (48, 76, 12),
    (60, 79, 4),
    (64, 82, 4),
    (68, 81, 4),
    (72, 79, 4),
    (76, 74, 4),
    (80, 77, 8),
    (88, 79, 4),
    (92, 81, 4),
    (96, 85, 6),
    (102, 81, 2),
    (104, 76, 8),
    (112, 81, 16),
];
/// Indices dans l'accord, en croches.
const MENU_ARP: [usize; 8] = [0, 1, 2, 1, 2, 0, 1, 2];

fn menu_play(b: &mut Score, step: u32, loop_: u32, t: f64, _i: f64, sd: f64) {
    let bar = step / 16;
    let s = step % 16;
    let name = MENU_PROG[bar as usize];
    let chord = name.notes();
    if s == 0 {
        pad(b, &chord, t, sd * 16.0, 0.035, 0.9);
        bass(b, name.root(), t, sd * 14.0, 0.1);
        if bar >= 4 && loop_ % 2 == 1 {
            choir(b, &[chord[0] + 12, chord[2] + 12], t, sd * 16.0, 0.022);
        }
    }
    // arpège en croches, plus présent à partir de la 2e boucle
    if s.is_multiple_of(2) && loop_ > 0 {
        let n = chord[MENU_ARP[(s / 2) as usize]] + 12;
        pluck(b, n, t, 0.022);
    }
    // percussions lentes et solennelles
    if s == 0 {
        taiko(b, t, if bar == 0 { 0.3 } else { 0.18 }, 0.9);
    }
    if s == 10 && bar % 2 == 1 {
        taiko(b, t, 0.12, 1.2);
    }
    if step == 7 * 16 {
        swell(b, t, sd * 16.0, 0.05);
    }
    if step == 0 && loop_ > 0 {
        crash(b, t, 0.045);
    }
    // le thème à la cloche, puis à la flûte une boucle sur deux
    for &(at, note, len) in MENU_THEME {
        if at == step {
            if loop_.is_multiple_of(2) {
                bell(b, note, t, 0.045, 2.2);
            } else {
                flute(b, note, t, f64::from(len) * sd, 0.045);
            }
        }
    }
}

// ------------------------------------------------------------------ DRAFT : « Le Choix » (tendu, ostinato)
const DRAFT_PROG: [Chord; 8] = {
    use Chord::*;
    [Dm, Dm, Bb, C, Dm, Dm, Gm, A]
};
/// Demi-tons au-dessus de la fondamentale, en doubles-croches (x2).
const DRAFT_OSTINATO: [i32; 8] = [0, 0, 7, 0, 12, 0, 7, 10];
const DRAFT_COUNTER: &Phrase = &[
    (64, 74, 4),
    (68, 76, 4),
    (72, 77, 8),
    (80, 79, 4),
    (84, 77, 4),
    (88, 76, 8),
    (96, 77, 4),
    (100, 79, 4),
    (104, 81, 12),
    (112, 85, 8),
    (120, 81, 8),
];

fn draft_play(b: &mut Score, step: u32, loop_: u32, t: f64, _i: f64, sd: f64) {
    let bar = step / 16;
    let s = step % 16;
    let name = DRAFT_PROG[bar as usize];
    let root = name.root() + 24;
    // ostinato de cordes en doubles-croches
    if s.is_multiple_of(2) {
        strings(
            b,
            root + DRAFT_OSTINATO[((s / 2) % 8) as usize],
            t,
            sd * 1.8,
            0.028,
        );
    }
    if s == 0 {
        pad(b, &name.notes(), t, sd * 16.0, 0.025, 0.7);
        bass(b, name.root(), t, sd * 7.0, 0.11);
    }
    if s == 8 {
        bass(b, name.root(), t, sd * 7.0, 0.09);
    }
    // tambours qui montent
    if s == 0 || s == 6 || s == 10 {
        let (vol, pitch) = if s == 0 { (0.28, 1.0) } else { (0.17, 1.25) };
        taiko(b, t, vol, pitch);
    }
    if bar >= 4 && (s == 4 || s == 12) {
        snare(b, t, 0.07);
    }
    if bar >= 6 && s % 2 == 1 {
        snare(b, t, 0.035);
    }
    // cuivres sur les temps forts de la 2e moitié
    if bar >= 4 && s == 0 {
        brass(b, &name.notes(), t, sd * 3.0, 0.03);
    }
    if step == 7 * 16 {
        swell(b, t, sd * 16.0, 0.06);
    }
    if step == 0 && loop_ > 0 {
        crash(b, t, 0.05);
    }
    for &(at, note, len) in DRAFT_COUNTER {
        if at == step && loop_ % 2 == 1 {
            brass(b, &[note], t, f64::from(len) * sd, 0.026);
        }
    }
}

// ------------------------------------------------------------------ PARTIE : « Sous la canopée » (ambiance douce, générative)
// Ré dorien, tempo lent, harmonie qui change toutes les 2 mesures. Chaque boucle tire une progression
// et des motifs différents (aléatoire déterministe par boucle/mesure) : rien ne se répète à l'identique.
// Le combat ajoute seulement de légères couches (pulsation sourde, arpège feutré) : jamais de cuivres ni de
// caisse claire.

/// Accords ouverts et doux (nappes) + fondamentale grave (`AMB`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amb {
    Dm9,
    Fmaj9,
    G69,
    Am7,
    Cadd9,
    Bbmaj7,
}

impl Amb {
    pub const fn notes(self) -> [i32; 4] {
        match self {
            Amb::Dm9 => [57, 60, 64, 65],
            Amb::Fmaj9 => [57, 60, 64, 67],
            Amb::G69 => [57, 59, 62, 64],
            Amb::Am7 => [55, 60, 64, 69],
            Amb::Cadd9 => [55, 62, 64, 67],
            Amb::Bbmaj7 => [57, 62, 65, 69],
        }
    }

    pub const fn root(self) -> i32 {
        match self {
            Amb::Dm9 => 38,
            Amb::Fmaj9 => 41,
            Amb::G69 => 43,
            Amb::Am7 => 45,
            Amb::Cadd9 => 48,
            Amb::Bbmaj7 => 46,
        }
    }
}

const AMB_PROGS: [[Amb; 4]; 5] = {
    use Amb::*;
    [
        [Dm9, Fmaj9, Cadd9, G69],
        [Dm9, Bbmaj7, Fmaj9, Am7],
        [Bbmaj7, Cadd9, Dm9, Dm9],
        [Dm9, G69, Am7, Fmaj9],
        [Fmaj9, Am7, Bbmaj7, Cadd9],
    ]
};
/// Pentatonique de Ré mineur (consonante sur tous les accords ci-dessus).
const PENTA: [i32; 10] = [62, 65, 67, 69, 72, 74, 77, 79, 81, 84];
const MOTIF_STEPS: [u32; 7] = [0, 2, 4, 6, 8, 10, 12];

/// Aléatoire déterministe dans `[0, 1)` (pur, testable). Même arithmétique entière 32 bits que le web :
/// somme exacte (`| 0` tronque à 32 bits), `Math.imul`, décalages logiques.
pub fn hash3(a: u32, b: u32, c: u32) -> f64 {
    let sum =
        i64::from(a) * 374_761_393 + i64::from(b) * 668_265_263 + i64::from(c) * 2_147_483_647;
    let mut h = sum as u32; // `| 0` : troncature modulo 2^32
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    f64::from(h) / 4_294_967_296.0
}

/// Leitmotiv rare (une boucle sur quatre, au calme).
const GAME_MELODY: &Phrase = &[
    (0, 74, 6),
    (8, 72, 4),
    (12, 69, 8),
    (32, 67, 6),
    (40, 69, 10),
    (64, 74, 4),
    (70, 77, 4),
    (76, 74, 8),
    (96, 72, 6),
    (104, 69, 16),
];

/// Notes au plus dans le motif d'une mesure (2 à 4).
pub const MAX_MOTIF: usize = 4;

/// Plan d'une mesure du thème de partie : accord + motif généré. Taille fixe, sans allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmbientBar {
    pub chord: Amb,
    notes: [(u32, i32, u32); MAX_MOTIF],
    len: usize,
}

impl AmbientBar {
    /// Notes du motif : (pas dans la mesure, note MIDI, durée en pas).
    pub fn motif(&self) -> &[(u32, i32, u32)] {
        &self.notes[..self.len]
    }
}

/// Plan d'une mesure du thème de partie : accord + motif généré (pur, testé).
pub fn game_bar(loop_: u32, bar: u32) -> AmbientBar {
    let prog = &AMB_PROGS[(hash3(loop_, 0, 7) * AMB_PROGS.len() as f64) as usize];
    let chord = prog[(bar / 2) as usize];
    let mut out = AmbientBar {
        chord,
        notes: [(0, 0, 0); MAX_MOTIF],
        len: 0,
    };
    // une boucle sur quatre (à partir de la 3e) : respiration, nappe seule
    let breath = loop_ % 4 == 2;
    // le motif apparaît dans ~40 % des mesures, jamais deux mesures de suite
    let prev_played = bar > 0 && hash3(loop_, bar - 1, 1) < 0.4;
    if breath || prev_played || hash3(loop_, bar, 1) >= 0.4 {
        return out;
    }
    let count = 2 + (hash3(loop_, bar, 2) * 3.0) as u32; // 2 à 4 notes
    let mut idx = 2 + (hash3(loop_, bar, 3) * 5.0) as i32;
    let mut pos = (hash3(loop_, bar, 4) * 3.0) as u32 * 2;
    let mut i = 0;
    while i < count && pos < 16 {
        let gap = pos + 1 + (hash3(loop_, bar, 10 + i) * 4.0) as u32;
        let next = MOTIF_STEPS.iter().copied().find(|&p| p > gap).unwrap_or(16);
        out.notes[out.len] = (pos, PENTA[idx as usize], (next - pos).max(2));
        out.len += 1;
        // petit pas mélodique (-2..+2), tendance descendante pour rester apaisé
        let delta = (hash3(loop_, bar, 20 + i) * 5.0) as i32 - 2 - (i % 2) as i32;
        idx = (idx + delta).clamp(0, PENTA.len() as i32 - 1);
        pos = next;
        i += 1;
    }
    out
}

fn game_play(b: &mut Score, step: u32, loop_: u32, t: f64, intensity: f64, sd: f64) {
    let bar = step / 16;
    let s = step % 16;
    let plan = game_bar(loop_, bar);
    let notes = plan.chord.notes();
    let root = plan.chord.root();
    // nappe et basse : une par accord (toutes les 2 mesures), qui se chevauchent en fondu
    if s == 0 && bar.is_multiple_of(2) {
        soft_pad(
            b,
            &notes,
            t,
            sd * 34.0,
            0.022 + 0.008 * intensity,
            0.8 + intensity * 0.5,
        );
        bass(b, root, t, sd * 30.0, 0.05 + 0.02 * intensity);
    }
    // de temps en temps, une quinte grave au milieu de l'accord
    if s == 0 && bar % 2 == 1 && hash3(loop_, bar, 30) < 0.35 {
        bass(b, root + 7, t, sd * 12.0, 0.035);
    }
    // motif généré (kalimba ou flûte selon la boucle), atténué en combat
    if intensity < 0.7 {
        let use_flute = hash3(loop_, 0, 40) < 0.35;
        for &(at, note, len) in plan.motif() {
            if at == s {
                if use_flute {
                    flute(b, note, t, f64::from(len) * sd, 0.022 * (1.0 - intensity));
                } else {
                    kalimba(b, note, t, 0.03 * (1.0 - intensity * 0.8));
                }
            }
        }
    }
    // leitmotiv, rarement et seulement au calme
    if loop_ % 4 == 3 && intensity < 0.3 {
        for &(at, note, len) in GAME_MELODY {
            if at == step {
                flute(b, note, t, f64::from(len) * sd, 0.02);
            }
        }
    }
    // étincelles aiguës très rares
    if s == 6 && hash3(loop_, bar, 50) < 0.12 {
        let n = PENTA[6 + (hash3(loop_, bar, 51) * 4.0) as usize] + 12;
        bell(b, n, t, 0.012, 2.5);
    }
    // combat 1 : pulsation sourde et lointaine
    if intensity > 0.3 && (s == 0 || s == 8) {
        taiko(b, t, 0.04 + 0.07 * intensity, 0.75);
    }
    // combat 2 : arpège feutré en croches sur l'accord
    if intensity > 0.55 && s.is_multiple_of(2) {
        let n = notes[(s / 2) as usize % notes.len()] + 12;
        pluck(b, n, t, 0.008 + 0.01 * intensity);
    }
    // combat 3 : souffle qui monte avant chaque nouvelle boucle
    if intensity > 0.8 && step == 7 * 16 {
        swell(b, t, sd * 16.0, 0.025);
    }
}

/// Les trois thèmes (`THEMES`), dans l'ordre de [`ThemeId`].
pub const THEMES: [Theme; 3] = [
    Theme {
        bpm: 76.0,
        bars: 8,
        play: menu_play,
    },
    Theme {
        bpm: 112.0,
        bars: 8,
        play: draft_play,
    },
    Theme {
        bpm: 66.0,
        bars: 8,
        play: game_play,
    },
];

/// Le thème `id` (`THEMES[id]`).
pub fn theme(id: ThemeId) -> &'static Theme {
    match id {
        ThemeId::Menu => &THEMES[0],
        ThemeId::Draft => &THEMES[1],
        ThemeId::Game => &THEMES[2],
    }
}

// ------------------------------------------------------------------ jingles de fin
/// Fanfare de victoire (≈ 6 s) : montée de cuivres, timbales, accord majeur final.
pub fn victory(s: &mut Score, t: f64) {
    let b = 60.0 / 96.0 / 4.0;
    let seq: [(u32, [i32; 3]); 4] = [
        (0, [62, 66, 69]),  // Ré majeur
        (6, [62, 67, 71]),  // Sol
        (10, [64, 69, 73]), // La
        (16, [66, 69, 74]), // Ré majeur, aigu
    ];
    for i in 0..12 {
        let i = f64::from(i);
        taiko(s, t + i * b * 1.2, 0.08 + i * 0.012, 1.1);
    }
    for (at, ch) in seq {
        let start = t + f64::from(at) * b * 2.0;
        let dur = if at == 16 { 3.5 } else { b * 8.0 };
        brass(s, &ch, start, dur, 0.05);
        bass(s, ch[0] - 24, start, dur, 0.12);
    }
    pad(s, &[62, 66, 69, 74], t + 32.0 * b, 4.0, 0.04, 1.2);
    choir(s, &[74, 78, 81], t + 32.0 * b, 4.0, 0.03);
    bell(s, 86, t + 32.0 * b, 0.05, 3.0);
    bell(s, 90, t + 34.0 * b, 0.04, 3.0);
    bell(s, 93, t + 36.0 * b, 0.04, 3.5);
    crash(s, t + 32.0 * b, 0.07);
    taiko(s, t + 32.0 * b, 0.4, 0.8);
}

/// Complainte de défaite (≈ 6 s) : cordes graves descendantes, chœur sombre.
pub fn defeat(s: &mut Score, t: f64) {
    let b = 0.55;
    let seq: [(u32, [i32; 3]); 4] = [
        (0, [62, 65, 69]), // Ré m
        (2, [58, 62, 65]), // Si♭
        (4, [55, 58, 62]), // Sol m
        (6, [57, 61, 64]), // La
    ];
    for (at, ch) in seq {
        let start = t + f64::from(at) * b;
        pad(s, &ch, start, b * 2.6, 0.035, 0.6);
        bass(s, ch[0] - 24, start, b * 2.0, 0.1);
    }
    choir(s, &[57, 62], t + 6.0 * b, 3.5, 0.03);
    taiko(s, t, 0.3, 0.7);
    taiko(s, t + 6.0 * b, 0.25, 0.6);
    flute(s, 74, t + b, b * 1.5, 0.035);
    flute(s, 72, t + 2.5 * b, b * 1.5, 0.035);
    flute(s, 70, t + 4.0 * b, b * 1.5, 0.035);
    flute(s, 69, t + 5.5 * b, b * 3.0, 0.035);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie la cohérence d'une phrase (`phraseInBounds`).
    fn phrase_in_bounds(phrase: &Phrase, bars: u32) -> bool {
        phrase
            .iter()
            .all(|&(at, note, len)| at < bars * 16 && len > 0 && (36..=100).contains(&note))
    }

    // Port de `music.test.ts` : « écrit des phrases dans les bornes de leur boucle ».
    #[test]
    fn ecrit_des_phrases_dans_les_bornes_de_leur_boucle() {
        assert!(phrase_in_bounds(MENU_THEME, theme(ThemeId::Menu).bars));
        assert!(phrase_in_bounds(DRAFT_COUNTER, theme(ThemeId::Draft).bars));
        assert!(phrase_in_bounds(GAME_MELODY, theme(ThemeId::Game).bars));
    }

    // Port de `music.test.ts` : « joue chaque thème sur deux boucles, à toutes les intensités, sans
    // erreur ». Le web compte plus de 1 000 nœuds WebAudio créés (5 à 10 par note) ; ici on compte les
    // notes, et chaque thème, chaque intensité et chaque jingle doivent en produire.
    #[test]
    fn joue_chaque_theme_sur_deux_boucles_a_toutes_les_intensites_sans_erreur() {
        let mut total = 0;
        for (id, th) in [ThemeId::Menu, ThemeId::Draft, ThemeId::Game]
            .iter()
            .zip(&THEMES)
        {
            for intensity in [0.0, 0.3, 0.6, 1.0] {
                let mut s = Score::new();
                for loop_ in 0..2 {
                    for step in 0..th.bars * 16 {
                        (th.play)(&mut s, step, loop_, f64::from(step) * 0.1, intensity, 0.1);
                    }
                }
                assert!(s.len() > 20, "{id:?} à {intensity} : {} notes", s.len());
                total += s.len();
            }
        }
        let mut v = Score::new();
        victory(&mut v, 0.0);
        let mut d = Score::new();
        defeat(&mut d, 0.0);
        assert!(v.len() > 20 && d.len() > 10);
        assert!(v.end() > 5.0 && d.end() > 5.0, "jingles d'environ 6 s");
        total += v.len() + d.len();
        assert!(total > 1000, "{total} notes");
    }

    #[test]
    fn le_combat_ajoute_des_couches_au_theme_de_partie() {
        let count = |intensity| {
            let mut s = Score::new();
            for step in 0..128 {
                game_play(&mut s, step, 0, f64::from(step) * 0.1, intensity, 0.1);
            }
            s.len()
        };
        assert!(count(1.0) > count(0.0) + 40);
    }

    // Port de `music.test.ts` : « varie le thème de partie d'une boucle à l'autre, en restant clairsemé ».
    #[test]
    fn varie_le_theme_de_partie_d_une_boucle_a_l_autre_en_restant_clairseme() {
        let mut loops: Vec<[AmbientBar; 8]> = Vec::new();
        let mut bars = 0;
        let mut with_motif = 0;
        for loop_ in 0..12 {
            let plan: [AmbientBar; 8] = std::array::from_fn(|bar| game_bar(loop_, bar as u32));
            for b in &plan {
                bars += 1;
                if !b.motif().is_empty() {
                    with_motif += 1;
                }
                for &(at, _, len) in b.motif() {
                    assert!(at < 16 && len > 0);
                }
            }
            assert_eq!(plan, std::array::from_fn(|bar| game_bar(loop_, bar as u32)));
            if !loops.contains(&plan) {
                loops.push(plan);
            }
        }
        assert_eq!(loops.len(), 12);
        assert!((with_motif as f64 / bars as f64) < 0.45);
        assert!(with_motif > 0);
    }

    /// Valeurs relevées sur le client web (`hash3` et `gameBar` de `themes.ts`, exécutés sous Node) :
    /// le portage tire exactement les mêmes progressions et les mêmes motifs.
    #[test]
    fn tire_les_memes_valeurs_que_le_client_web() {
        let golden = [
            ((0, 0, 7), 0.789634131594),
            ((3, 5, 1), 0.975162530085),
            ((11, 7, 20), 0.486232060473),
            ((123_456, 789, 2147), 0.300784734776),
        ];
        for ((a, b, c), v) in golden {
            assert!((hash3(a, b, c) - v).abs() < 1e-11, "hash3({a}, {b}, {c})");
        }
        use Amb::*;
        type Golden = ((u32, u32), Amb, &'static [(u32, i32, u32)]);
        let bars: [Golden; 6] = [
            ((0, 0), Dm9, &[(4, 74, 4), (8, 77, 8)]),
            ((0, 1), Dm9, &[]),
            ((4, 6), G69, &[(0, 74, 6), (6, 77, 2), (8, 79, 4)]),
            ((9, 1), Fmaj9, &[(2, 77, 2), (4, 74, 6)]),
            ((11, 7), Dm9, &[(0, 77, 4), (4, 77, 4), (8, 69, 8)]),
            ((3, 2), Cadd9, &[(2, 74, 4), (6, 77, 2)]),
        ];
        for ((l, b), chord, motif) in bars {
            let plan = game_bar(l, b);
            assert_eq!(plan.chord, chord, "gameBar({l}, {b})");
            assert_eq!(plan.motif(), motif, "gameBar({l}, {b})");
        }
        // mesures avec motif sur 12 boucles, et leur nombre total de notes
        let played: Vec<(u32, u32)> = (0..12)
            .flat_map(|l| (0..8).map(move |b| (l, b)))
            .filter(|&(l, b)| !game_bar(l, b).motif().is_empty())
            .collect();
        let expected = [
            (0, 0),
            (0, 3),
            (1, 2),
            (1, 5),
            (3, 2),
            (3, 6),
            (4, 0),
            (4, 4),
            (4, 6),
            (5, 5),
            (5, 7),
            (7, 0),
            (7, 5),
            (8, 0),
            (8, 4),
            (9, 1),
            (11, 1),
            (11, 7),
        ];
        assert_eq!(played, expected);
        let notes: usize = played
            .iter()
            .map(|&(l, b)| game_bar(l, b).motif().len())
            .sum();
        assert_eq!(notes, 52);
        // progressions tirées pour les 12 premières boucles
        let progs: [[Amb; 4]; 12] = [
            [Dm9, G69, Am7, Fmaj9],
            [Dm9, Bbmaj7, Fmaj9, Am7],
            [Fmaj9, Am7, Bbmaj7, Cadd9],
            [Bbmaj7, Cadd9, Dm9, Dm9],
            [Dm9, Fmaj9, Cadd9, G69],
            [Dm9, Fmaj9, Cadd9, G69],
            [Dm9, G69, Am7, Fmaj9],
            [Fmaj9, Am7, Bbmaj7, Cadd9],
            [Bbmaj7, Cadd9, Dm9, Dm9],
            [Fmaj9, Am7, Bbmaj7, Cadd9],
            [Bbmaj7, Cadd9, Dm9, Dm9],
            [Bbmaj7, Cadd9, Dm9, Dm9],
        ];
        for (l, prog) in progs.iter().enumerate() {
            let got: [Amb; 4] = std::array::from_fn(|i| game_bar(l as u32, i as u32 * 2).chord);
            assert_eq!(&got, prog, "boucle {l}");
        }
    }
}
