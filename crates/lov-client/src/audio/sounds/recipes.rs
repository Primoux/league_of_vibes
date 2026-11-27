//! Recettes des sorts : une recette procédurale par sort de champion (dérivée du champion et du slot),
//! des recettes dédiées pour les ults, et la table des sorts d'invocateur.
//!
//! Portage de la première moitié de `packages/client/src/audio/sounds.ts`. Les constantes sont
//! recopiées telles quelles, y compris les `slide` en Hz (voir [`ToneOpts::slide`] : c'est un
//! multiplicateur, `slide: 220` sur 110 Hz vise 24 kHz). Ce n'est pas corrigé ici.

use lov_shared::data::champions::CHAMPIONS;
use lov_shared::types::{SummonerId, Vec2 as GameVec2};

use super::super::synth::{FilterKind, NoiseOpts, ToneOpts, Wave};
use super::super::{AudioManager, SfxOpts};

// ------------------------------------------------------------------------------- écriture compacte

/// `tone(ctx, d, { type, freq, dur, vol })` ; les autres champs par [`ToneExt`].
pub(super) fn tone(wave: Wave, freq: f64, dur: f64, vol: f64) -> ToneOpts {
    ToneOpts {
        wave,
        freq,
        dur,
        vol,
        ..ToneOpts::default()
    }
}

/// `noise(ctx, d, { dur, vol })` ; les autres champs par [`NoiseExt`].
pub(super) fn noise(dur: f64, vol: f64) -> NoiseOpts {
    NoiseOpts {
        dur,
        vol,
        ..NoiseOpts::default()
    }
}

/// Champs optionnels d'un `tone`, dans l'ordre d'écriture du web.
pub(super) trait ToneExt {
    fn slide(self, slide: f64) -> Self;
    fn delay(self, delay: f64) -> Self;
    fn filter(self, hz: f64) -> Self;
}

impl ToneExt for ToneOpts {
    fn slide(mut self, slide: f64) -> Self {
        self.slide = slide;
        self
    }
    fn delay(mut self, delay: f64) -> Self {
        self.delay = delay;
        self
    }
    fn filter(mut self, hz: f64) -> Self {
        self.filter = Some(hz);
        self
    }
}

/// Champs optionnels d'un `noise`.
pub(super) trait NoiseExt {
    fn filter(self, hz: f64) -> Self;
    fn slide_filter(self, hz: f64) -> Self;
    fn delay(self, delay: f64) -> Self;
    fn kind(self, kind: FilterKind) -> Self;
}

impl NoiseExt for NoiseOpts {
    fn filter(mut self, hz: f64) -> Self {
        self.filter = Some(hz);
        self
    }
    fn slide_filter(mut self, hz: f64) -> Self {
        self.slide_filter = Some(hz);
        self
    }
    fn delay(mut self, delay: f64) -> Self {
        self.delay = delay;
        self
    }
    fn kind(mut self, kind: FilterKind) -> Self {
        self.kind = kind;
        self
    }
}

use FilterKind::{Bandpass, Highpass};
use Wave::{Sawtooth, Sine, Square, Triangle};

/// Effet positionné sur le canal des effets (`{ pos }`).
pub(super) fn at(pos: Option<GameVec2>) -> SfxOpts {
    SfxOpts {
        pos,
        ..SfxOpts::default()
    }
}

// --------------------------------------------------------------------------------- procédurales

/// `note(base, semis)`.
fn note(base: f64, semis: f64) -> f64 {
    base * 2f64.powf(semis / 12.0)
}

/// Demi-tons, forme d'onde et durée de chaque slot, dans l'ordre Q, W, E, R.
const SLOTS: [(f64, Wave, f64); 4] = [
    (0.0, Square, 0.18),
    (4.0, Sawtooth, 0.22),
    (7.0, Triangle, 0.25),
    (12.0, Sawtooth, 0.5),
];

/// Recette dédiée : `(am, pos, vol = 1) => …`.
pub type CustomFn = fn(&mut AudioManager, Option<GameVec2>, f64);

/// Une entrée de `fxRecipes` ou de `customRecipes`.
#[derive(Clone, Copy)]
pub enum FxRecipe {
    /// Sort Q/W/E/R du champion d'index `champion`, slot d'index `slot` dans [`SLOTS`].
    Ability {
        champion: usize,
        slot: usize,
    },
    /// Passif du champion d'index `champion`.
    Passive {
        champion: usize,
    },
    Custom(CustomFn),
}

impl std::fmt::Debug for FxRecipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ability { champion, slot } => write!(f, "Ability({champion}, {slot})"),
            Self::Passive { champion } => write!(f, "Passive({champion})"),
            Self::Custom(_) => write!(f, "Custom"),
        }
    }
}

impl FxRecipe {
    /// Joue la recette ; `vol` vaut `undefined` côté web quand il manque (défauts 0,25 et 0,2).
    pub fn play(self, am: &mut AudioManager, pos: Option<GameVec2>, vol: Option<f64>) {
        match self {
            Self::Ability { champion, slot } => {
                // chaque champion a sa couleur fréquentielle
                let base = note(196.0, ((champion * 3) % 12) as f64);
                let (semis, wave, dur) = SLOTS[slot];
                let vol = vol.unwrap_or(0.25);
                am.sfx(at(pos), |r| {
                    r.tone(
                        tone(wave, base * 2f64.powf(semis / 12.0), dur, vol * 0.7)
                            .slide(1.5)
                            .filter(2400.0),
                    )
                    .noise(
                        noise(dur * 0.6, vol * 0.25)
                            .filter(3000.0)
                            .slide_filter(600.0),
                    );
                });
            }
            Self::Passive { champion } => {
                let base = note(196.0, ((champion * 3) % 12) as f64);
                let vol = vol.unwrap_or(0.2);
                am.sfx(at(pos), |r| {
                    r.tone(tone(Sine, base, 0.4, vol * 0.5).slide(0.5));
                });
            }
            Self::Custom(f) => f(am, pos, vol.unwrap_or(1.0)),
        }
    }
}

/// `fxRecipes.get(key)` : la recette procédurale dont le `fxKey` vérifie `is_key`. Comme le `Map.set`
/// du web, une clé enregistrée deux fois garde la dernière.
pub fn procedural(is_key: impl Fn(&str) -> bool) -> Option<FxRecipe> {
    let mut found = None;
    for (champion, c) in CHAMPIONS.iter().enumerate() {
        let a = &c.abilities;
        for (slot, def) in [&a.q, &a.w, &a.e, &a.r].into_iter().enumerate() {
            if is_key(&def.fx_key) {
                found = Some(FxRecipe::Ability { champion, slot });
            }
        }
        if is_key(&c.passive.fx_key) {
            found = Some(FxRecipe::Passive { champion });
        }
    }
    found
}

/// `customRecipes.get(key)`.
pub fn custom(is_key: impl Fn(&str) -> bool) -> Option<FxRecipe> {
    CUSTOM
        .iter()
        .find(|(k, _)| is_key(k))
        .map(|&(_, f)| FxRecipe::Custom(f))
}

/// `customFxKeys.has(key)` : ces sons gèrent eux-mêmes leur impact (l'`areaEffect` générique se tait).
pub fn is_custom_fx_key(key: &str) -> bool {
    CUSTOM.iter().any(|(k, _)| *k == key)
}

// ------------------------------------------------------------------------------------ dédiées

/// Recettes dédiées des ults, par `fxKey`, dans l'ordre du web.
pub const CUSTOM: &[(&str, CustomFn)] = &[
    ("melenchon_r", melenchon_r),
    ("grondak_r", grondak_r),
    ("tharox_r", tharox_r),
    ("kaelen_r", kaelen_r),
    ("vrenna_r", vrenna_r),
    ("ilyra_r", ilyra_r),
    ("zephyr_r", zephyr_r),
    ("sable_r", sable_r),
    ("corvin_r", corvin_r),
    ("nyx_r", nyx_r),
    ("solenne_r", solenne_r),
];

/// Mélenchon — R « La République, c'est moi ! » : déclaration solennelle (0,6 s) puis onde tricolore.
fn melenchon_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        // 0 -> 0,6 s : montée grave de la déclaration
        r.tone(
            tone(Sawtooth, 110.0, 0.6, 0.22 * vol)
                .slide(220.0)
                .filter(900.0),
        )
        .tone(
            tone(Square, 55.0, 0.6, 0.18 * vol)
                .slide(110.0)
                .filter(500.0),
        )
        // 0,6 s : impact de l'onde
        .noise(
            noise(0.9, 0.45 * vol)
                .filter(500.0)
                .slide_filter(60.0)
                .delay(0.6),
        )
        .tone(tone(Sine, 55.0, 1.0, 0.5 * vol).delay(0.6))
        // motif tricolore : bleu, rouge, cyan
        .tone(
            tone(Square, 220.0, 0.18, 0.2 * vol)
                .delay(0.6)
                .filter(2400.0),
        )
        .tone(
            tone(Square, 277.0, 0.18, 0.2 * vol)
                .delay(0.72)
                .filter(2400.0),
        )
        .tone(
            tone(Square, 554.0, 0.32, 0.22 * vol)
                .delay(0.84)
                .filter(3200.0),
        )
        // envolée aiguë
        .tone(
            tone(Sine, 1108.0, 0.35, 0.12 * vol)
                .slide(1660.0)
                .delay(0.6),
        );
    });
}

/// Grondak — R « Effondrement » : grondement de terre (0,75 s) puis choc sismique et débris.
fn grondak_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.noise(noise(0.75, 0.2 * vol).filter(220.0).slide_filter(90.0)) // grondement montant
            .noise(
                noise(1.1, 0.5 * vol)
                    .filter(700.0)
                    .slide_filter(70.0)
                    .delay(0.75),
            ) // choc
            .tone(tone(Sine, 48.0, 1.2, 0.5 * vol).slide(30.0).delay(0.75)); // sub sismique
        for i in 0..5 {
            let i = f64::from(i);
            // débris
            r.noise(
                noise(0.08, 0.15 * vol)
                    .filter(2000.0 + i * 500.0)
                    .delay(0.75 + i * 0.09),
            );
        }
    });
}

/// Tharox — R « Ruée sauvage » : galop qui s'accélère (1 s) puis barrissement de charge.
fn tharox_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        for i in 0..6 {
            let i = f64::from(i);
            let t = i * (0.16 - i * 0.018); // tempo accéléré
            r.tone(
                tone(Square, 90.0, 0.07, 0.25 * vol)
                    .slide(60.0)
                    .delay(t)
                    .filter(400.0),
            );
        }
        r.noise(
            noise(0.7, 0.4 * vol)
                .filter(1200.0)
                .slide_filter(300.0)
                .kind(Bandpass)
                .delay(1.0),
        ) // barrissement
        .tone(
            tone(Sawtooth, 180.0, 0.5, 0.25 * vol)
                .slide(90.0)
                .delay(1.0)
                .filter(800.0),
        );
    });
}

/// Kaelen — R « Jugement solaire » : accord céleste majeur, flash blanc, frappe métallique lourde.
fn kaelen_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        // do majeur
        for (i, f) in [523.0, 659.0, 784.0, 1047.0].into_iter().enumerate() {
            r.tone(
                tone(Triangle, f, 0.5, 0.16 * vol)
                    .delay(i as f64 * 0.07)
                    .filter(4000.0),
            );
        }
        r.tone(
            tone(Sine, 2093.0, 0.25, 0.12 * vol)
                .slide(3136.0)
                .delay(0.4),
        ) // flash blanc
        .noise(
            noise(0.3, 0.4 * vol)
                .filter(3000.0)
                .slide_filter(400.0)
                .delay(0.4),
        ) // lame
        .tone(
            tone(Square, 130.0, 0.5, 0.3 * vol)
                .slide(65.0)
                .delay(0.4)
                .filter(600.0),
        ); // impact
    });
}

/// Vrenna — R « Lance céleste » : envol (souffle montant) puis impact doré.
fn vrenna_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.noise(
            noise(0.45, 0.3 * vol)
                .filter(600.0)
                .slide_filter(4000.0)
                .kind(Bandpass),
        ) // envol
        .tone(
            tone(Sawtooth, 300.0, 0.4, 0.2 * vol)
                .slide(900.0)
                .filter(1500.0),
        ) // montée
        .noise(
            noise(0.5, 0.45 * vol)
                .filter(2500.0)
                .slide_filter(200.0)
                .delay(0.45),
        ) // impact
        .tone(
            tone(Square, 784.0, 0.4, 0.22 * vol)
                .slide(392.0)
                .delay(0.45)
                .filter(3000.0),
        );
    });
}

/// Ilyra — R « Blizzard » : vent glacé 4 s, cristaux de glace qui tintent, ronronnement de givre.
fn ilyra_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.noise(noise(4.0, 0.22 * vol).filter(3500.0).kind(Highpass)) // vent
            .tone(tone(Sine, 110.0, 4.0, 0.12 * vol).filter(300.0)); // hum de givre
        for i in 0..8 {
            // cristaux : hauteur tirée au hasard (générateur semé ici, `Math.random()` sur le web)
            let f = 1800.0 + r.random() * 1800.0;
            r.tone(tone(Sine, f, 0.12, 0.07 * vol).delay(0.2 + f64::from(i) * 0.48));
        }
    });
}

/// Zephyr — R « Barrage arcanique » : charge arcanique montante puis trois décharges de rayon.
fn zephyr_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.tone(
            tone(Sawtooth, 220.0, 0.5, 0.2 * vol)
                .slide(880.0)
                .filter(1800.0),
        ); // charge
        for i in 0..3 {
            let t = 0.5 + f64::from(i) * 0.35;
            r.tone(
                tone(Square, 1200.0, 0.25, 0.25 * vol)
                    .slide(300.0)
                    .delay(t)
                    .filter(5000.0),
            ) // rayon
            .noise(
                noise(0.2, 0.2 * vol)
                    .filter(6000.0)
                    .slide_filter(1000.0)
                    .delay(t),
            );
        }
    });
}

/// Sable — R « Balle du jugement » : visée (1 s, sifflement montant) puis détonation globale.
fn sable_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.tone(tone(Sine, 600.0, 1.0, 0.15 * vol).slide(1800.0)) // visée
            .noise(
                noise(0.15, 0.5 * vol)
                    .filter(4000.0)
                    .slide_filter(500.0)
                    .delay(1.0),
            ) // détonation
            .tone(tone(Sine, 1200.0, 0.8, 0.3 * vol).slide(200.0).delay(1.0)) // onde de choc
            .tone(
                tone(Square, 90.0, 0.6, 0.3 * vol)
                    .slide(45.0)
                    .delay(1.0)
                    .filter(500.0),
            );
    });
}

/// Corvin — R « Nuée » : nuée de corbeaux 3 s (battements d'ailes, croassements) sur fond sinistre.
fn corvin_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        r.tone(
            tone(Sawtooth, 140.0, 3.0, 0.1 * vol)
                .slide(90.0)
                .filter(500.0),
        ); // fond sinistre
        for i in 0..10 {
            // battements
            r.noise(
                noise(0.05, 0.12 * vol)
                    .filter(1800.0)
                    .kind(Bandpass)
                    .delay(f64::from(i) * 0.3),
            );
        }
        for i in 0..4 {
            // croassement
            let t = 0.3 + f64::from(i) * 0.7;
            r.tone(
                tone(Sawtooth, 700.0, 0.18, 0.15 * vol)
                    .slide(350.0)
                    .delay(t)
                    .filter(1200.0),
            );
        }
    });
}

/// Nyx — R « Mort certaine » : marque obsessionnelle pulsée (3 s) puis détonation sombre différée.
fn nyx_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        for i in 0..6 {
            // pulsation de la marque
            r.tone(tone(Sine, 220.0, 0.25, 0.18 * vol).delay(f64::from(i) * 0.5));
        }
        r.tone(
            tone(Sawtooth, 880.0, 0.4, 0.25 * vol)
                .slide(220.0)
                .delay(3.0)
                .filter(2000.0),
        ) // cri
        .noise(
            noise(0.8, 0.5 * vol)
                .filter(900.0)
                .slide_filter(80.0)
                .delay(3.0),
        ) // explosion
        .tone(tone(Sine, 65.0, 1.0, 0.45 * vol).slide(40.0).delay(3.0));
    });
}

/// Solenne — R « Aube » : arpège chaud montant (0,5 s) puis colonne de lumière douce.
fn solenne_r(am: &mut AudioManager, pos: Option<GameVec2>, vol: f64) {
    am.sfx(at(pos), |r| {
        // sol majeur céleste
        for (i, f) in [392.0, 523.0, 659.0, 784.0, 1047.0].into_iter().enumerate() {
            r.tone(
                tone(Triangle, f, 0.6, 0.15 * vol)
                    .delay(i as f64 * 0.1)
                    .filter(3500.0),
            );
        }
        r.noise(
            noise(1.2, 0.18 * vol)
                .filter(5000.0)
                .kind(Highpass)
                .delay(0.5),
        ) // colonne
        .tone(tone(Sine, 1568.0, 0.8, 0.12 * vol).slide(2093.0).delay(0.5));
    });
}

// ----------------------------------------------------------------------------------- invocateur

/// `SUMMONER_RECIPES[spell](am, pos)`.
pub fn summoner(am: &mut AudioManager, spell: SummonerId, pos: Option<GameVec2>) {
    am.sfx(at(pos), |r| match spell {
        SummonerId::Flash => {
            r.tone(tone(Square, 1200.0, 0.12, 0.3).slide(300.0))
                .noise(noise(0.15, 0.2).filter(5000.0).slide_filter(800.0));
        }
        SummonerId::Ignite => {
            r.noise(noise(0.5, 0.3).filter(1800.0).slide_filter(300.0))
                .tone(tone(Sawtooth, 180.0, 0.5, 0.25).slide(90.0).filter(900.0));
        }
        SummonerId::Heal => {
            r.tone(tone(Sine, 520.0, 0.4, 0.25).slide(1.5))
                .tone(tone(Sine, 780.0, 0.4, 0.2).slide(1.4).delay(0.08));
        }
        SummonerId::Barrier => {
            r.tone(tone(Triangle, 300.0, 0.5, 0.3).slide(600.0));
        }
        SummonerId::Exhaust => {
            r.tone(tone(Sawtooth, 220.0, 0.4, 0.25).slide(0.6).filter(700.0));
        }
        SummonerId::Teleport => {
            r.tone(tone(Sine, 400.0, 0.8, 0.25).slide(2.0))
                .tone(tone(Sine, 800.0, 0.8, 0.15).slide(2.0).delay(0.1));
        }
        SummonerId::Smite => {
            r.tone(tone(Square, 90.0, 0.3, 0.35).slide(40.0).filter(500.0))
                .noise(noise(0.25, 0.3).filter(1200.0));
        }
        SummonerId::Ghost => {
            r.noise(noise(0.6, 0.2).filter(2500.0).kind(Highpass))
                .tone(tone(Sine, 600.0, 0.6, 0.15).slide(900.0));
        }
        SummonerId::Cleanse => {
            r.tone(tone(Triangle, 700.0, 0.3, 0.28).slide(1200.0))
                .tone(tone(Sine, 1050.0, 0.25, 0.2).delay(0.1));
        }
    });
}
