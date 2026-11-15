//! Mesures de performance de la boucle d'une salle (`LOV_PERF=1`) : port de
//! `packages/server/src/net/perf.ts`.
//!
//! Journal toutes les 10 s : durée des ticks, coût des snapshots, régularité réelle des envois, débit
//! par client. Le TS mesurait aussi le retard de la boucle d'événements de Node ; avec tokio, ce retard
//! se lit dans l'intervalle réel entre deux envois.

use std::time::{Duration, Instant};

const FLUSH_EVERY: Duration = Duration::from_secs(10);

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Percentile `p` (0 à 100) d'une série, 0 si elle est vide.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut s = values.to_vec();
    s.sort_by(f64::total_cmp);
    let i = ((p / 100.0) * s.len() as f64).floor() as usize;
    s[i.min(s.len() - 1)]
}

pub struct LoopPerf {
    label: String,
    log: Box<dyn Fn(&str) + Send>,
    ticks: Vec<f64>,
    snaps: Vec<f64>,
    gaps: Vec<f64>,
    bytes: usize,
    clients: usize,
    last_snap_at: Option<Instant>,
    since: Instant,
}

impl LoopPerf {
    pub fn new(label: &str) -> Self {
        Self::with_log(label, Box::new(|m| println!("{m}")))
    }

    pub fn with_log(label: &str, log: Box<dyn Fn(&str) + Send>) -> Self {
        Self {
            label: label.to_owned(),
            log,
            ticks: Vec::new(),
            snaps: Vec::new(),
            gaps: Vec::new(),
            bytes: 0,
            clients: 0,
            last_snap_at: None,
            since: Instant::now(),
        }
    }

    pub fn tick(&mut self, d: Duration) {
        self.ticks.push(ms(d));
    }

    pub fn snapshot(&mut self, d: Duration, bytes: usize, clients: usize) {
        let now = Instant::now();
        if let Some(last) = self.last_snap_at {
            self.gaps.push(ms(now - last));
        }
        self.last_snap_at = Some(now);
        self.snaps.push(ms(d));
        self.bytes += bytes;
        self.clients = clients;
        if now - self.since >= FLUSH_EVERY {
            self.flush(now);
        }
    }

    fn flush(&mut self, now: Instant) {
        let secs = (now - self.since).as_secs_f64();
        let avg = self.ticks.iter().sum::<f64>() / self.ticks.len().max(1) as f64;
        (self.log)(&format!(
            "[perf {}] tick moy {avg:.2} ms p99 {:.2} max {:.1} | snapshot p99 {:.2} ms | intervalle envois p50 {:.0} p95 {:.0} max {:.0} ms | {:.1} ticks/s | {:.1} Ko/s/client (brut)",
            self.label,
            percentile(&self.ticks, 99.0),
            percentile(&self.ticks, 100.0),
            percentile(&self.snaps, 99.0),
            percentile(&self.gaps, 50.0),
            percentile(&self.gaps, 95.0),
            percentile(&self.gaps, 100.0),
            self.ticks.len() as f64 / secs,
            self.bytes as f64 / 1024.0 / secs / self.clients.max(1) as f64,
        ));
        self.ticks.clear();
        self.snaps.clear();
        self.gaps.clear();
        self.bytes = 0;
        self.since = now;
    }
}
