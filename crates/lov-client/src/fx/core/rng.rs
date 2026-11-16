//! Générateur pseudo-aléatoire rapide (mulberry32), déterministe si on fixe la graine (tests, démo).
//!
//! Portage de `packages/client/src/fx/core/rng.ts` : même suite de nombres que le client web pour une même
//! graine (`next`). Le client web partage un `rng` global ; ici chaque propriétaire (système de particules…)
//! garde le sien.

use std::time::{SystemTime, UNIX_EPOCH};

/// Valeur fixe ou plage `[min, max]` tirée au hasard (`number | [number, number]` du client web).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Range {
    Fixed(f32),
    Between(f32, f32),
}

impl From<f32> for Range {
    fn from(v: f32) -> Self {
        Range::Fixed(v)
    }
}

impl From<[f32; 2]> for Range {
    fn from([min, max]: [f32; 2]) -> Self {
        Range::Between(min, max)
    }
}

#[derive(Clone, Debug)]
pub struct Rng {
    s: u32,
}

impl Default for Rng {
    /// Graine tirée de l'horloge (le client web prend `Math.random()`).
    fn default() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() ^ d.as_secs() as u32);
        Self::new(nanos)
    }
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { s: seed }
    }

    /// Nombre dans `[0, 1)`, identique au client web pour une même graine.
    pub fn next(&mut self) -> f64 {
        self.s = self.s.wrapping_add(0x6d2b_79f5);
        let mut t = self.s;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    /// `next()` en `f32`, garanti `< 1` (l'arrondi direct en `f32` pourrait donner exactement 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next() as f32).min(1.0 - f32::EPSILON / 2.0)
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// Valeur dans une plage `[min, max]` (ou valeur fixe).
    pub fn pick(&mut self, r: Range) -> f32 {
        match r {
            Range::Fixed(v) => v,
            Range::Between(min, max) => self.range(min, max),
        }
    }

    pub fn sign(&mut self) -> f32 {
        if self.next() < 0.5 { -1.0 } else { 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn est_deterministe_et_borne() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            let x = a.range(2.0, 5.0);
            assert_eq!(x, b.range(2.0, 5.0));
            assert!((2.0..5.0).contains(&x));
        }
        assert_eq!(a.pick(7.0.into()), 7.0);
    }

    #[test]
    fn meme_suite_que_le_client_web() {
        // `new Rng(42)` du client web : 3 premiers `next()`.
        let mut r = Rng::new(42);
        let got: Vec<f64> = (0..3).map(|_| r.next()).collect();
        assert_eq!(
            got,
            [0.6011037519201636, 0.44829055899754167, 0.8524657934904099]
        );
    }
}
