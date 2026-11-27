//! Instruments de la bande-son, 100 % synthétisés (aucun échantillon, voir `ASSETS.md`).
//!
//! Portage de `packages/client/src/audio/music/instruments.ts`, mêmes constantes. Chaque fonction ajoute
//! à une [`Score`] les notes qui commencent à l'instant `t` du bus (`bus.current_time()` remplace
//! `ctx.currentTime`). Les paramètres par défaut du web sont rappelés dans la documentation ; Rust n'en
//! ayant pas, l'appelant les passe tous.

use super::super::synth::{FilterKind, Param, Wave};
use super::note::{Layer, Note, Score};

/// Fréquence d'une note MIDI (La4 = 69 = 440 Hz).
pub fn midi(n: i32) -> f64 {
    440.0 * 2f64.powf(f64::from(n - 69) / 12.0)
}

/// Enveloppe attaque / maintien / relâchement sur un gain ; renvoie l'instant d'arrêt des sources.
fn env(g: &mut Param, t: f64, peak: f64, attack: f64, hold: f64, release: f64) -> f64 {
    g.set_value_at_time(0.0001, t)
        .linear_ramp_to_value_at_time(peak, t + attack)
        .set_value_at_time(peak, t + attack + hold)
        .exponential_ramp_to_value_at_time(0.0001, t + attack + hold + release);
    t + attack + hold + release + 0.05
}

/// Fréquence automatisée d'un filtre : départ, puis rampes `(valeur, instant, exponentielle ?)`.
fn sweep(from: f64, t: f64, ramps: &[(f64, f64, bool)]) -> Param {
    let mut p = Param::new(350.0);
    p.set_value_at_time(from, t);
    for &(v, at, exp) in ramps {
        if exp {
            p.exponential_ramp_to_value_at_time(v, at);
        } else {
            p.linear_ramp_to_value_at_time(v, at);
        }
    }
    p
}

/// Nappe chaude : 3 dents de scie désaccordées par note, filtre qui respire (web : `vol` 0,05, `bright` 1).
pub fn pad(s: &mut Score, notes: &[i32], t: f64, dur: f64, vol: f64, bright: f64) {
    let mut out = Note::voice(0.7, 0.05);
    let freq = sweep(
        500.0 * bright,
        t,
        &[
            (1500.0 * bright, t + dur * 0.5, false),
            (700.0 * bright, t + dur, false),
        ],
    );
    let mut lp = Layer::filtered(FilterKind::Lowpass, 0.7, freq);
    let end = env(
        &mut out.gain,
        t,
        vol,
        (dur * 0.35).min(1.4),
        (dur - 1.6).max(0.0),
        1.6,
    );
    for &n in notes {
        for det in [-9.0, 0.0, 8.0] {
            lp.osc(Wave::Sawtooth, midi(n), t, end, det);
        }
    }
    out.add(lp);
    s.push(out);
}

/// Chœur « aah » : dents de scie à travers deux formants (web : `vol` 0,04).
pub fn choir(s: &mut Score, notes: &[i32], t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.9, 0.1);
    let end = env(&mut out.gain, t, vol, 0.9, (dur - 1.6).max(0.0), 1.4);
    for (f, q, g) in [(730.0, 6.0, 1.0), (1150.0, 8.0, 0.6)] {
        let mut bp = Layer::filtered(FilterKind::Bandpass, q, Param::new(f));
        bp.gain = Param::new(g);
        for &n in notes {
            for det in [-6.0, 6.0] {
                bp.osc(Wave::Sawtooth, midi(n), t, end, det)
                    .vibrato(4.8, Param::new(4.0));
            }
        }
        out.add(bp);
    }
    s.push(out);
}

/// Cordes (ostinato) : dent de scie + vibrato, attaque nette (web : `vol` 0,035).
///
/// Le web n'y branche en fait aucun vibrato : seulement deux dents de scie désaccordées.
pub fn strings(s: &mut Score, note: i32, t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.45, 0.0);
    let mut lp = Layer::filtered(FilterKind::Lowpass, 1.0, Param::new(2400.0));
    let end = env(&mut out.gain, t, vol, 0.02, (dur - 0.08).max(0.0), 0.12);
    for det in [-5.0, 5.0] {
        lp.osc(Wave::Sawtooth, midi(note), t, end, det);
    }
    out.add(lp);
    s.push(out);
}

/// Cuivres : accord avec ouverture de filtre (« stab » héroïque ; web : `vol` 0,045).
pub fn brass(s: &mut Score, notes: &[i32], t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.5, 0.08);
    let freq = sweep(
        350.0,
        t,
        &[(2800.0, t + 0.08, true), (1100.0, t + dur.min(0.6), true)],
    );
    let mut lp = Layer::filtered(FilterKind::Lowpass, 2.0, freq);
    let end = env(&mut out.gain, t, vol, 0.035, (dur - 0.2).max(0.0), 0.35);
    for &n in notes {
        for det in [-7.0, 7.0] {
            lp.osc(Wave::Sawtooth, midi(n), t, end, det);
        }
    }
    out.add(lp);
    s.push(out);
}

/// Basse : sinus + octave triangle, filtrée (web : `vol` 0,12).
pub fn bass(s: &mut Score, note: i32, t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.08, 0.0);
    let mut lp = Layer::filtered(FilterKind::Lowpass, 1.0, Param::new(420.0));
    let end = env(&mut out.gain, t, vol, 0.03, (dur - 0.25).max(0.0), 0.25);
    lp.osc(Wave::Sine, midi(note), t, end, 0.0);
    lp.osc(Wave::Triangle, midi(note + 12), t, end, 0.0)
        .level(0.35);
    out.add(lp);
    s.push(out);
}

/// Partiels sinus `(rapport, gain, décroissance)`, chacun avec sa propre enveloppe.
fn partials(s: &mut Score, mut out: Note, f: f64, t: f64, vol: f64, list: &[(f64, f64, f64)]) {
    for &(ratio, g, d) in list {
        let mut gn = Layer::plain();
        let end = env(&mut gn.gain, t, vol * g, 0.004, 0.0, d);
        gn.osc(Wave::Sine, f * ratio, t, end, 0.0);
        out.add(gn);
    }
    s.push(out);
}

/// Cloche / célesta : partiels inharmoniques à décroissance longue (web : `vol` 0,05, `decay` 2,4).
pub fn bell(s: &mut Score, note: i32, t: f64, vol: f64, decay: f64) {
    let list = [
        (1.0, 1.0, decay),
        (2.76, 0.35, decay * 0.5),
        (5.4, 0.15, decay * 0.25),
        (2.0, 0.25, decay * 0.7),
    ];
    partials(s, Note::voice(0.6, 0.35), midi(note), t, vol, &list);
}

/// Flûte : sinus + souffle, vibrato qui s'installe (web : `vol` 0,05).
pub fn flute(s: &mut Score, note: i32, t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.6, 0.25);
    let end = env(&mut out.gain, t, vol, 0.08, (dur - 0.2).max(0.0), 0.3);
    let mut depth = Param::new(0.0);
    depth
        .set_value_at_time(0.0, t)
        .linear_ramp_to_value_at_time(12.0, t + dur.min(0.6));
    let mut tones = Layer::plain();
    tones
        .osc(Wave::Sine, midi(note), t, end, 0.0)
        .vibrato(5.2, depth.clone());
    tones
        .osc(Wave::Triangle, midi(note), t, end, 0.0)
        .vibrato(5.2, depth)
        .level(0.4);
    out.add(tones);
    let mut breath = Layer::filtered(FilterKind::Bandpass, 3.0, Param::new(midi(note) * 2.0));
    breath.gain = Param::new(0.08);
    breath.noise(t, end);
    out.add(breath);
    s.push(out);
}

/// Pincé (arpèges) : triangle à décroissance rapide, filtre qui se referme (web : `vol` 0,04).
pub fn pluck(s: &mut Score, note: i32, t: f64, vol: f64) {
    let mut out = Note::voice(0.35, 0.3);
    let freq = sweep(4200.0, t, &[(600.0, t + 0.35, true)]);
    let mut lp = Layer::filtered(FilterKind::Lowpass, 1.0, freq);
    let end = env(&mut out.gain, t, vol, 0.004, 0.0, 0.55);
    lp.osc(Wave::Triangle, midi(note), t, end, 0.0);
    lp.osc(Wave::Square, midi(note), t, end, 3.0);
    out.add(lp);
    s.push(out);
}

/// Taiko : coup grave à chute de hauteur + attaque de bruit (web : `vol` 0,32, `pitch` 1).
pub fn taiko(s: &mut Score, t: f64, vol: f64, pitch: f64) {
    let mut out = Note::voice(0.25, 0.0);
    let end = env(&mut out.gain, t, vol, 0.003, 0.0, 0.55);
    let mut body = Layer::plain();
    if let Some(f) = body.osc(Wave::Sine, 110.0 * pitch, t, end, 0.0).freq() {
        f.exponential_ramp_to_value_at_time(42.0 * pitch, t + 0.35);
    }
    out.add(body);
    let mut hit = Layer::filtered(FilterKind::Lowpass, 1.0, Param::new(900.0));
    env(&mut hit.gain, t, vol * 0.5, 0.002, 0.0, 0.09);
    hit.noise(t, t + 0.2);
    out.add(hit);
    s.push(out);
}

/// Caisse claire militaire : bruit filtré + corps (web : `vol` 0,12).
pub fn snare(s: &mut Score, t: f64, vol: f64) {
    let mut out = Note::voice(0.3, 0.0);
    let mut bp = Layer::filtered(FilterKind::Bandpass, 0.8, Param::new(2200.0));
    let end = env(&mut out.gain, t, vol, 0.002, 0.0, 0.16);
    bp.noise(t, end);
    out.add(bp);
    let mut body = Layer::plain();
    env(&mut body.gain, t, vol * 0.6, 0.002, 0.0, 0.08);
    body.osc(Wave::Triangle, 190.0, t, t + 0.12, 0.0);
    out.add(body);
    s.push(out);
}

/// Montée de cymbale (souffle aigu crescendo ; web : `vol` 0,06).
pub fn swell(s: &mut Score, t: f64, dur: f64, vol: f64) {
    let mut out = Note::voice(0.6, 0.0);
    let freq = sweep(3000.0, t, &[(8000.0, t + dur, false)]);
    let mut hp = Layer::filtered(FilterKind::Highpass, 1.0, freq);
    hp.noise(t, t + dur + 0.7);
    out.add(hp);
    out.gain
        .set_value_at_time(0.0001, t)
        .exponential_ramp_to_value_at_time(vol, t + dur)
        .exponential_ramp_to_value_at_time(0.0001, t + dur + 0.6);
    s.push(out);
}

/// Crash : impact de cymbale (web : `vol` 0,06).
pub fn crash(s: &mut Score, t: f64, vol: f64) {
    let mut out = Note::voice(0.5, 0.0);
    let mut hp = Layer::filtered(FilterKind::Highpass, 1.0, Param::new(5000.0));
    let end = env(&mut out.gain, t, vol, 0.003, 0.0, 1.6);
    hp.noise(t, end);
    out.add(hp);
    s.push(out);
}

/// Nappe douce (thème de partie) : sinus + triangle légèrement désaccordés, filtre bas, attaque lente
/// (web : `vol` 0,03, `bright` 1).
pub fn soft_pad(s: &mut Score, notes: &[i32], t: f64, dur: f64, vol: f64, bright: f64) {
    let mut out = Note::voice(0.8, 0.0);
    let freq = sweep(
        600.0 * bright,
        t,
        &[
            (1100.0 * bright, t + dur * 0.5, false),
            (650.0 * bright, t + dur, false),
        ],
    );
    let mut lp = Layer::filtered(FilterKind::Lowpass, 0.5, freq);
    let end = env(
        &mut out.gain,
        t,
        vol,
        (dur * 0.4).min(2.6),
        (dur - 2.6).max(0.0),
        3.0,
    );
    for &n in notes {
        lp.osc(Wave::Triangle, midi(n), t, end, -5.0);
        lp.osc(Wave::Sine, midi(n), t, end, 6.0);
    }
    out.add(lp);
    s.push(out);
}

/// Kalimba / boîte à musique : sinus et partiel doux, décroissance moyenne, écho léger (web : `vol` 0,03).
pub fn kalimba(s: &mut Score, note: i32, t: f64, vol: f64) {
    let list = [(1.0, 1.0, 1.4), (3.0, 0.12, 0.35)];
    partials(s, Note::voice(0.55, 0.22), midi(note), t, vol, &list);
}
