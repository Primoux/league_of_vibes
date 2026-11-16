//! Minuteur d'effets : `after(0.6, f)` pour enchaîner anticipation → impact → persistance.
//! Temps en secondes, avancé par `update(dt)` (donc figé si le jeu est en pause, comme le rendu).
//!
//! Portage de `packages/client/src/fx/core/scheduler.ts`. Les tâches reçoivent le minuteur (pour en
//! planifier d'autres) et un contexte `C` (le runtime FX), puisqu'une fermeture Rust ne peut pas garder une
//! référence mutable vers son propriétaire comme le fait le client web.

type TaskFn<C> = Box<dyn FnOnce(&mut Scheduler<C>, &mut C) + Send + Sync>;

struct Task<C> {
    at: f32,
    f: TaskFn<C>,
}

pub struct Scheduler<C> {
    tasks: Vec<Task<C>>,
    now: f32,
}

impl<C> Default for Scheduler<C> {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            now: 0.0,
        }
    }
}

impl<C> Scheduler<C> {
    pub fn after(
        &mut self,
        delay: f32,
        f: impl FnOnce(&mut Scheduler<C>, &mut C) + Send + Sync + 'static,
    ) {
        self.tasks.push(Task {
            at: self.now + delay.max(0.0),
            f: Box::new(f),
        });
    }

    pub fn update(&mut self, dt: f32, ctx: &mut C) {
        self.now += dt;
        // Parcours à l'envers avec retrait par échange : O(1) par retrait, l'ordre d'exécution n'importe pas
        // à l'intérieur d'une frame. Une tâche planifiée pendant le parcours part en fin de liste : elle
        // n'est pas visitée avant la frame suivante.
        let mut i = self.tasks.len();
        while i > 0 {
            i -= 1;
            if self.tasks[i].at > self.now {
                continue;
            }
            let task = self.tasks.swap_remove(i);
            (task.f)(self, ctx);
        }
    }

    pub fn pending(&self) -> usize {
        self.tasks.len()
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_les_taches_a_echeance_y_compris_celles_planifiees_pendant_update() {
        let mut s: Scheduler<Vec<&'static str>> = Scheduler::default();
        let mut log = Vec::new();
        s.after(0.5, |s, log| {
            log.push("a");
            s.after(0.2, |_, log| log.push("b"));
        });
        s.update(0.3, &mut log);
        assert!(log.is_empty());
        s.update(0.3, &mut log);
        assert_eq!(log, ["a"]);
        s.update(0.1, &mut log);
        assert_eq!(log, ["a"]);
        s.update(0.15, &mut log);
        assert_eq!(log, ["a", "b"]);
        assert_eq!(s.pending(), 0);
    }
}
