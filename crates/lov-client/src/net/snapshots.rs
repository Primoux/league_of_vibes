//! Tampon de snapshots et interpolation des entités : port de `packages/client/src/net/snapshots.ts`.
//!
//! - Fusionne les snapshots (complets ou delta) en une vue complète `entities`. Avec `removed`, le
//!   snapshot est un DELTA (les entités absentes sont conservées, `patches` porte les mises à jour
//!   partielles) ; sans, il est COMPLET (toute entité absente disparaît).
//! - Garde pour chaque entité un court historique de positions horodatées (temps serveur, ms) et
//!   échantillonne la position et l'orientation à un instant donné (interpolation, extrapolation bornée).
//! - Les sauts (Saut éclair, réapparition, téléportation) ne sont pas interpolés : l'historique repart.

use std::collections::HashMap;

use lov_shared::protocol::{Snapshot, apply_entity_patch};
use lov_shared::types::{EntityId, EntitySnapshot};

const HISTORY: usize = 12;
/// Au-delà de cette distance entre deux échantillons consécutifs : saut, pas d'interpolation.
pub const TELEPORT_DIST: f64 = 350.0;
/// Extrapolation maximale au-delà du dernier échantillon (ms), pour un paquet en retard.
pub const MAX_EXTRAPOLATE_MS: f64 = 120.0;

/// Une position horodatée (temps serveur, ms).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub facing: f64,
}

/// Position et orientation interpolées.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub facing: f64,
}

/// Historique circulaire d'une entité, du plus ancien au plus récent.
#[derive(Debug, Clone, Default)]
struct Track {
    samples: [Sample; HISTORY],
    count: usize,
    /// Index du prochain écrit.
    head: usize,
}

impl Track {
    fn push(&mut self, s: Sample) {
        self.samples[self.head] = s;
        self.head = (self.head + 1) % HISTORY;
        self.count = (self.count + 1).min(HISTORY);
    }

    /// `i = 0` : plus ancien ; `count - 1` : plus récent.
    fn at(&self, i: usize) -> Sample {
        self.samples[(self.head + HISTORY * 2 - self.count + i) % HISTORY]
    }

    fn latest(&self) -> Option<Sample> {
        (self.count > 0).then(|| self.at(self.count - 1))
    }

    fn clear(&mut self) {
        self.count = 0;
    }
}

/// Interpolation d'angle par le plus court chemin.
pub fn lerp_angle(a: f64, b: f64, t: f64) -> f64 {
    use std::f64::consts::PI;
    let mut d = (b - a) % (PI * 2.0);
    if d > PI {
        d -= PI * 2.0;
    } else if d < -PI {
        d += PI * 2.0;
    }
    a + d * t
}

#[derive(Debug, Default)]
pub struct SnapshotBuffer {
    /// Vue complète fusionnée : l'état le plus récent de chaque entité.
    pub entities: HashMap<EntityId, EntitySnapshot>,
    tracks: HashMap<EntityId, Track>,
    pub last_tick: Option<u64>,
    pub latest_server_time: f64,
    /// Entités retirées par le dernier `push` (réutilisé d'un snapshot à l'autre).
    pub removed: Vec<EntityId>,
}

impl SnapshotBuffer {
    /// Intègre un snapshot. Faux s'il est obsolète (tick déjà vu) et a été ignoré. Les entités
    /// retirées sont listées dans `removed`.
    pub fn push(&mut self, snap: &Snapshot) -> bool {
        self.removed.clear();
        if self.last_tick.is_some_and(|t| snap.tick <= t) {
            return false;
        }
        self.last_tick = Some(snap.tick);
        self.latest_server_time = snap.server_time;
        let t = snap.server_time;
        if let Some(removed) = &snap.removed {
            for &id in removed {
                self.remove(id);
            }
            for e in &snap.entities {
                self.upsert(e.clone(), t);
            }
            for p in snap.patches.iter().flatten() {
                if let Some(prev) = self.entities.get(&p.id) {
                    let next = apply_entity_patch(prev, p);
                    self.upsert(next, t);
                }
            }
        } else {
            for e in &snap.entities {
                self.upsert(e.clone(), t);
            }
            let gone: Vec<EntityId> = self
                .entities
                .keys()
                .copied()
                .filter(|id| !snap.entities.iter().any(|e| e.id == *id))
                .collect();
            for id in gone {
                self.remove(id);
            }
        }
        true
    }

    fn upsert(&mut self, e: EntitySnapshot, t: f64) {
        let was_dead = self
            .entities
            .get(&e.id)
            .is_some_and(|p| p.dead == Some(true));
        let track = self.tracks.entry(e.id).or_default();
        if let Some(last) = track.latest() {
            let jump = (e.x - last.x).hypot(e.y - last.y) > TELEPORT_DIST;
            let revived = was_dead && e.dead != Some(true);
            if jump || revived {
                track.clear();
            }
        }
        track.push(Sample {
            t,
            x: e.x,
            y: e.y,
            facing: e.facing,
        });
        self.entities.insert(e.id, e);
    }

    fn remove(&mut self, id: EntityId) {
        if self.entities.remove(&id).is_some() {
            self.tracks.remove(&id);
            self.removed.push(id);
        }
    }

    /// Position et orientation de `id` au temps serveur `render_time` (ms) ; `None` si inconnue.
    pub fn sample(&self, id: EntityId, render_time: f64) -> Option<Pose> {
        let track = self.tracks.get(&id).filter(|t| t.count > 0)?;
        let n = track.count;
        let newest = track.at(n - 1);
        let pose = |s: Sample| Pose {
            x: s.x,
            y: s.y,
            facing: s.facing,
        };
        if n == 1 || render_time >= newest.t {
            if n >= 2 && render_time > newest.t {
                // Extrapolation bornée à partir des deux derniers échantillons.
                let prev = track.at(n - 2);
                let span = newest.t - prev.t;
                if span > 0.0 {
                    let k = (render_time - newest.t).min(MAX_EXTRAPOLATE_MS) / span;
                    return Some(Pose {
                        x: newest.x + (newest.x - prev.x) * k,
                        y: newest.y + (newest.y - prev.y) * k,
                        facing: newest.facing,
                    });
                }
            }
            return Some(pose(newest));
        }
        let oldest = track.at(0);
        if render_time <= oldest.t {
            return Some(pose(oldest));
        }
        // Segment [a, b] qui encadre `render_time`, du plus récent au plus ancien.
        for i in (1..n).rev() {
            let a = track.at(i - 1);
            if a.t <= render_time {
                let b = track.at(i);
                let k = if b.t == a.t {
                    1.0
                } else {
                    (render_time - a.t) / (b.t - a.t)
                };
                return Some(Pose {
                    x: a.x + (b.x - a.x) * k,
                    y: a.y + (b.y - a.y) * k,
                    facing: lerp_angle(a.facing, b.facing, k),
                });
            }
        }
        Some(pose(newest))
    }

    /// Vitesse serveur récente (unités de jeu / s) d'après les deux derniers échantillons.
    pub fn velocity(&self, id: EntityId) -> Option<(f64, f64)> {
        let track = self.tracks.get(&id).filter(|t| t.count >= 2)?;
        let b = track.at(track.count - 1);
        let a = track.at(track.count - 2);
        let dt = (b.t - a.t) / 1000.0;
        (dt > 0.0).then(|| ((b.x - a.x) / dt, (b.y - a.y) / dt))
    }

    pub fn latest_sample(&self, id: EntityId) -> Option<Sample> {
        self.tracks.get(&id)?.latest()
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.tracks.clear();
        self.removed.clear();
        self.last_tick = None;
    }
}
