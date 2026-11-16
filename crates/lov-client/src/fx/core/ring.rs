//! Allocateur circulaire d'emplacements de particules + suivi des plages modifiées
//! (pour n'envoyer au GPU que ce qui a été écrit depuis la dernière frame).
//!
//! Portage de `packages/client/src/fx/core/ring.ts`.

pub struct RingAllocator {
    capacity: usize,
    head: usize,
    dirty_start: usize,
    dirty_count: usize,
    /// Plages modifiées `[début, longueur]`, valides après `flush()` pour `i <` la valeur renvoyée.
    pub range_start: [usize; 2],
    pub range_count: [usize; 2],
}

impl RingAllocator {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacité nulle");
        Self {
            capacity,
            head: 0,
            dirty_start: 0,
            dirty_count: 0,
            range_start: [0; 2],
            range_count: [0; 2],
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Réserve un emplacement (écrase le plus ancien quand le tampon est plein).
    pub fn next(&mut self) -> usize {
        let i = self.head;
        self.head = (self.head + 1) % self.capacity;
        if self.dirty_count == 0 {
            self.dirty_start = i;
        }
        if self.dirty_count < self.capacity {
            self.dirty_count += 1;
        }
        i
    }

    /// Calcule les plages modifiées depuis le dernier appel (0, 1 ou 2 si le tampon a bouclé) et remet le
    /// suivi à zéro.
    pub fn flush(&mut self) -> usize {
        let n = self.dirty_count;
        self.dirty_count = 0;
        if n == 0 {
            return 0;
        }
        if n >= self.capacity {
            self.set(0, 0, self.capacity);
            return 1;
        }
        let end = self.dirty_start + n;
        if end <= self.capacity {
            self.set(0, self.dirty_start, n);
            return 1;
        }
        self.set(0, self.dirty_start, self.capacity - self.dirty_start);
        self.set(1, 0, end - self.capacity);
        2
    }

    /// Repart de zéro (tous les emplacements sont libres : rien de vivant).
    pub fn reset(&mut self) {
        self.head = 0;
        self.dirty_count = 0;
    }

    fn set(&mut self, i: usize, start: usize, count: usize) {
        self.range_start[i] = start;
        self.range_count[i] = count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suit_une_plage_contigue() {
        let mut r = RingAllocator::new(8);
        r.next();
        r.next();
        r.next();
        assert_eq!(r.flush(), 1);
        assert_eq!([r.range_start[0], r.range_count[0]], [0, 3]);
        assert_eq!(r.flush(), 0);
    }

    #[test]
    fn decoupe_en_2_plages_quand_le_tampon_boucle() {
        let mut r = RingAllocator::new(8);
        for _ in 0..6 {
            r.next();
        }
        r.flush();
        for _ in 0..4 {
            r.next(); // 6, 7, 0, 1
        }
        assert_eq!(r.flush(), 2);
        assert_eq!(
            [
                r.range_start[0],
                r.range_count[0],
                r.range_start[1],
                r.range_count[1]
            ],
            [6, 2, 0, 2]
        );
    }

    #[test]
    fn reset_repart_de_l_emplacement_0_sans_plage_en_attente() {
        let mut r = RingAllocator::new(8);
        for _ in 0..5 {
            r.next();
        }
        r.reset();
        assert_eq!(r.flush(), 0);
        assert_eq!(r.next(), 0);
    }

    #[test]
    fn plage_complete_si_plus_de_capacity_ecritures() {
        let mut r = RingAllocator::new(4);
        for _ in 0..10 {
            r.next();
        }
        assert_eq!(r.flush(), 1);
        assert_eq!([r.range_start[0], r.range_count[0]], [0, 4]);
    }
}
