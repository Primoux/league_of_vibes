//! Jingles d'élimination (originaux, synthétisés) qui remplacent les voix : plus la série est longue,
//! plus le jingle est riche (plus aigu, plus de couches : cloches → cuivres → tambours → chœur → cymbales).
//! Allié = majeur et lumineux ; ennemi = mineur et sombre ; ton propre kill = plus fort et scintillant.
//!
//! Portage de `packages/client/src/audio/music/stings.ts`.

use super::instruments::{bass, bell, brass, choir, crash, pad, pluck, swell, taiko};
use super::note::Score;

/// Auteur du kill vu du joueur (`StingSide` : `'self' | 'ally' | 'enemy'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StingSide {
    /// `'self'` : le joueur lui-même.
    Own,
    Ally,
    Enemy,
}

/// Arpège montant (demi-tons depuis la tonique) : majeur pour nous, mineur pour l'ennemi.
const MAJOR: [i32; 7] = [0, 4, 7, 12, 16, 19, 24];
const MINOR: [i32; 7] = [0, 3, 7, 12, 15, 19, 24];

/// `level` 0..9 (voir `sting_level`) : nombre de notes, couches et hauteur montent avec le niveau.
/// Tonique : La4 (69) pour un kill simple, +1 demi-ton par niveau (jusqu'à Fa#5).
pub fn kill_sting(s: &mut Score, t: f64, level: u32, side: StingSide) {
    let enemy = side == StingSide::Enemy;
    let scale = if enemy { MINOR } else { MAJOR };
    let lvl = level.min(9) as i32;
    let root = if enemy { 57 } else { 69 } + lvl;
    let vol = match side {
        StingSide::Own => 1.25,
        StingSide::Enemy => 0.75,
        StingSide::Ally => 1.0,
    };
    // Au-delà de 9, seuls la tonique et le nombre de notes plafonnent, comme dans le web.
    let level = f64::from(level);
    let step = (0.1 - level * 0.005).max(0.055); // l'arpège s'accélère avec la série
    let notes = scale.len().min(2 + (level / 2.0).ceil() as usize);

    // 1. arpège de cloches (toujours) — signature du jingle
    for (i, &deg) in scale.iter().take(notes).enumerate() {
        let at = t + i as f64 * step;
        bell(s, root + deg, at, 0.045 * vol, 1.6 + level * 0.15);
    }
    let top = t + (notes - 1) as f64 * step;

    // 2. coup sourd : présent dès le kill simple, plus lourd ensuite
    taiko(
        s,
        t,
        (0.16 + level * 0.025) * vol,
        if enemy { 0.8 } else { 1.0 },
    );

    // 3. cuivres sur la note d'arrivée (dès le double)
    if level >= 2.0 {
        let chord = [root, root + scale[2], root + 12];
        brass(
            s,
            &chord,
            top,
            0.5 + level * 0.06,
            (0.026 + level * 0.003) * vol,
        );
    }

    // 4. basse + nappe (triple et plus)
    if level >= 4.0 {
        bass(s, root - 24, top, 1.2, 0.1 * vol);
        pad(
            s,
            &[root, root + scale[1], root + scale[2]],
            top,
            1.8,
            0.03 * vol,
            1.3,
        );
        taiko(s, t + step, 0.14 * vol, 1.25);
    }

    // 5. chœur + roulement (quadra et plus)
    if level >= 6.0 {
        choir(s, &[root + 12, root + 12 + scale[2]], top, 2.2, 0.03 * vol);
        for i in 0..4 {
            let i = f64::from(i);
            taiko(s, t + i * step * 0.5, (0.1 + i * 0.03) * vol, 1.15);
        }
    }

    // 6. apothéose : cymbales, second accord de cuivres, cascade de cloches (penta / série légendaire)
    if level >= 8.0 {
        crash(s, top, 0.06 * vol);
        brass(
            s,
            &[root + 5, root + 9, root + 12 + 5],
            top + 0.45,
            0.9,
            0.034 * vol,
        );
        brass(
            s,
            &[root + 7, root + 11, root + 12 + 7],
            top + 0.8,
            1.4,
            0.036 * vol,
        );
        for i in 0..5 {
            let at = top + 0.5 + i as f64 * 0.07;
            bell(s, root + 24 + scale[i % scale.len()], at, 0.03 * vol, 2.5);
        }
    }

    // scintillement de ton propre kill
    if side == StingSide::Own {
        for i in 0..3 {
            let at = top + 0.06 + i as f64 * 0.05;
            pluck(s, root + 24 + scale[(i * 2) % scale.len()], at, 0.02);
        }
    }
}

/// Premier sang : quinte ascendante solennelle, tambour grave, souffle de cymbale.
pub fn first_blood_sting(s: &mut Score, t: f64, side: StingSide) {
    let enemy = side == StingSide::Enemy;
    let r = if enemy { 50 } else { 62 };
    swell(s, t, 0.5, 0.05);
    taiko(s, t + 0.5, 0.36, 0.75);
    brass(s, &[r, r + 7], t + 0.5, 0.7, 0.04);
    brass(s, &[r + 5, r + 12], t + 0.85, 0.5, 0.036);
    let third = if enemy { 3 } else { 4 };
    brass(s, &[r + third, r + 7, r + 12], t + 1.15, 1.6, 0.045);
    bass(s, r - 24, t + 1.15, 1.6, 0.11);
    bell(s, r + 24, t + 1.15, 0.05, 2.8);
    crash(s, t + 1.15, 0.05);
}
