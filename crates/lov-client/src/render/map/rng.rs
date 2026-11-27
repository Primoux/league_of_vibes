//! PRNG déterministe (mulberry32) : le décor est identique pour tous les joueurs. Portage de
//! `packages/client/src/render/map/rng.ts`, bit pour bit.

pub struct Mulberry32(u32);

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    /// Nombre dans [0, 1[.
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valeurs de `rng(4242)` et `rng(99)` du client web (Node 24).
    #[test]
    fn memes_tirages_que_le_typescript() {
        let mut r = Mulberry32::new(4242);
        let got: Vec<f64> = (0..3).map(|_| r.next()).collect();
        assert_eq!(got, RNG_4242);
        let mut r = Mulberry32::new(99);
        assert_eq!(r.next(), RNG_99);
    }

    const RNG_4242: [f64; 3] = [0.5467061335220933, 0.27860878920182586, 0.9312369171530008];
    const RNG_99: f64 = 0.2604658124037087;
}
