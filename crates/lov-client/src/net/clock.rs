//! Synchronisation d'horloge client/serveur : port de `packages/client/src/net/clock.ts`.
//!
//! Estime `offset` tel que `serveur ≈ local + offset`, d'après les `serverTime` des snapshots. Chaque
//! échantillon vaut `serverTime − réception = offset − latence aller` : le MAX sur une fenêtre glissante
//! est l'échantillon le moins retardé. Le résultat est lissé pour éviter les sauts d'interpolation.
//!
//! Le TS gardait aussi la latence (`setRtt`) sans jamais s'en servir : la latence est compensée par la
//! prédiction, pas par l'horloge. Ce champ mort n'est pas porté.

#[derive(Debug, Clone)]
pub struct ServerClock {
    samples: Vec<f64>,
    window: usize,
    head: usize,
    target: f64,
    smoothed: f64,
    initialized: bool,
}

impl Default for ServerClock {
    fn default() -> Self {
        Self::new(40)
    }
}

impl ServerClock {
    pub fn new(window: usize) -> Self {
        Self {
            samples: Vec::with_capacity(window),
            window: window.max(1),
            head: 0,
            target: 0.0,
            smoothed: 0.0,
            initialized: false,
        }
    }

    /// À appeler à chaque snapshot reçu (`local_recv` : horloge locale, ms).
    pub fn sample(&mut self, server_time: f64, local_recv: f64) {
        let s = server_time - local_recv;
        if self.samples.len() < self.window {
            self.samples.push(s);
        } else {
            self.samples[self.head] = s;
            self.head = (self.head + 1) % self.window;
        }
        // Base = temps des données REÇUES : l'interpolation garde `INTERP_DELAY_MS` de marge sur le
        // dernier snapshot arrivé ; la latence est compensée par la prédiction.
        self.target = self
            .samples
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if !self.initialized {
            self.smoothed = self.target;
            self.initialized = true;
        }
    }

    /// Lissage exponentiel vers la cible ; gros écart (> 250 ms) : recalage immédiat.
    pub fn update(&mut self, dt_ms: f64) {
        let err = self.target - self.smoothed;
        if err.abs() > 250.0 {
            self.smoothed = self.target;
        } else {
            self.smoothed += err * (dt_ms / 1000.0).min(1.0);
        }
    }

    pub fn server_now(&self, local_now: f64) -> f64 {
        local_now + self.smoothed
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.head = 0;
        self.initialized = false;
    }
}
