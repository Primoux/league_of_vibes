//! Prédiction locale du déplacement du joueur : port de `packages/client/src/net/prediction.ts`.
//!
//! Dès un ordre de déplacement, le champion local avance en ligne droite vers le point à sa vitesse,
//! sans attendre le serveur. À chaque image, la position prédite est rappelée en douceur vers
//! l'estimation de la position serveur actuelle (dernière position reçue extrapolée de sa vitesse) ;
//! gros écart = recalage immédiat.
//!
//! Pas de prédiction quand la ligne droite traverse un mur (le serveur contourne, on le suit), pendant
//! un dash, mort, ou si le serveur reste immobile alors qu'on prédit un mouvement (contrôle, attaque…).

use lov_shared::map::is_in_wall;
use lov_shared::types::Vec2;

/// La ligne droite `a → b` traverse-t-elle un mur ? (échantillonnage tous les 50 u)
pub fn segment_blocked(a: Vec2, b: Vec2) -> bool {
    let d = (b.x - a.x).hypot(b.y - a.y);
    let n = (d / 50.0).ceil() as u32;
    (1..=n).any(|i| {
        let k = f64::from(i) / f64::from(n);
        is_in_wall(Vec2 {
            x: a.x + (b.x - a.x) * k,
            y: a.y + (b.y - a.y) * k,
        })
    })
}

pub const SNAP_DIST: f64 = 250.0;
/// Taux de correction (1/s) pendant un déplacement prédit et à l'arrêt.
const CORRECT_MOVING: f64 = 2.5;
const CORRECT_IDLE: f64 = 8.0;
/// Vitesse serveur sous laquelle l'entité est considérée immobile (u/s).
const STILL_SPEED: f64 = 20.0;

/// Ce que le serveur dit de l'entité.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ServerView {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// Avance à donner à la position serveur pour estimer sa position actuelle (s) : âge + latence/2.
    pub lead: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct MovePredictor {
    pub pos: Vec2,
    initialized: bool,
    target: Option<Vec2>,
    suspended_for: f64,
    server_still_for: f64,
    /// Délai de grâce après un ordre avant de se fier à l'immobilité serveur (s).
    grace: f64,
    blocked: fn(Vec2, Vec2) -> bool,
}

impl Default for MovePredictor {
    fn default() -> Self {
        Self::new(segment_blocked)
    }
}

impl MovePredictor {
    pub fn new(blocked: fn(Vec2, Vec2) -> bool) -> Self {
        Self {
            pos: Vec2 { x: 0.0, y: 0.0 },
            initialized: false,
            target: None,
            suspended_for: 0.0,
            server_still_for: 0.0,
            grace: 0.0,
            blocked,
        }
    }

    pub fn moving(&self) -> bool {
        self.target.is_some()
    }

    /// Ordre de déplacement (`move` / `attackMove` au sol). `latency` = RTT estimé (s).
    pub fn command(&mut self, target: Vec2, latency: f64) {
        if self.initialized && (self.blocked)(self.pos, target) {
            self.target = None;
            return;
        }
        self.target = Some(target);
        self.grace = latency + 0.2;
        self.server_still_for = 0.0;
    }

    /// Ordre non prédit (attaque, arrêt, sort) : on suit le serveur.
    pub fn cancel(&mut self) {
        self.target = None;
    }

    /// Suspend la prédiction (dash, incantation…) pendant `seconds`.
    pub fn suspend(&mut self, seconds: f64) {
        self.suspended_for = self.suspended_for.max(seconds);
        self.target = None;
    }

    pub fn reset(&mut self, x: f64, y: f64) {
        self.pos = Vec2 { x, y };
        self.target = None;
        self.initialized = true;
    }

    pub fn update(&mut self, dt: f64, server: ServerView, speed: f64, can_move: bool) -> Vec2 {
        if !self.initialized {
            self.reset(server.x, server.y);
        }
        let lead = server.lead.min(0.25);
        let ex = server.x + server.vx * lead;
        let ey = server.y + server.vy * lead;

        if self.suspended_for > 0.0 {
            self.suspended_for -= dt;
        }
        if !can_move {
            self.target = None;
        }

        let server_speed = server.vx.hypot(server.vy);
        if self.grace > 0.0 {
            self.grace -= dt;
        } else if server_speed < STILL_SPEED {
            self.server_still_for += dt;
        } else {
            self.server_still_for = 0.0;
        }
        if self.target.is_some() && self.grace <= 0.0 && self.server_still_for > 0.15 {
            self.target = None;
        }

        if let Some(target) = self.target
            && self.suspended_for <= 0.0
        {
            let (dx, dy) = (target.x - self.pos.x, target.y - self.pos.y);
            let d = dx.hypot(dy);
            let step = speed * dt;
            if d <= step {
                self.pos = target;
                self.target = None;
            } else {
                self.pos.x += dx / d * step;
                self.pos.y += dy / d * step;
            }
        }

        let (err_x, err_y) = (ex - self.pos.x, ey - self.pos.y);
        if err_x.hypot(err_y) > SNAP_DIST || self.suspended_for > 0.0 {
            self.pos = Vec2 { x: ex, y: ey };
        } else {
            let rate = if self.target.is_some() {
                CORRECT_MOVING
            } else {
                CORRECT_IDLE
            };
            let k = (dt * rate).min(1.0);
            self.pos.x += err_x * k;
            self.pos.y += err_y * k;
        }
        self.pos
    }
}
