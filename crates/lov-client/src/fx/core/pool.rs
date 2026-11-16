//! Pool d'objets générique : évite de recréer meshes/matériaux/effets à chaque sort.
//!
//! Portage de `packages/client/src/fx/core/pool.ts`. L'objet sort du pool par valeur (`acquire`) et y
//! revient par `release`.

type Create<T> = Box<dyn Fn() -> T + Send + Sync>;
type Reset<T> = Box<dyn Fn(&mut T) + Send + Sync>;

pub struct Pool<T> {
    free: Vec<T>,
    created: usize,
    create: Create<T>,
    reset: Option<Reset<T>>,
    max: usize,
}

impl<T> Pool<T> {
    /// Pool sans limite de taille.
    pub fn new(create: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            free: Vec::new(),
            created: 0,
            create: Box::new(create),
            reset: None,
            max: usize::MAX,
        }
    }

    /// Fonction appliquée à chaque objet rendu au pool.
    pub fn with_reset(mut self, reset: impl Fn(&mut T) + Send + Sync + 'static) -> Self {
        self.reset = Some(Box::new(reset));
        self
    }

    /// Nombre maximal d'objets créés.
    pub fn with_max(mut self, max: usize) -> Self {
        self.max = max;
        self
    }

    /// Objet libre, ou un nouveau ; `None` si la limite `max` est atteinte.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.free.pop() {
            return Some(item);
        }
        if self.created >= self.max {
            return None;
        }
        self.created += 1;
        Some((self.create)())
    }

    pub fn release(&mut self, mut item: T) {
        if let Some(reset) = &self.reset {
            reset(&mut item);
        }
        self.free.push(item);
    }

    /// Nombre total d'objets créés (libres + utilisés).
    pub fn size(&self) -> usize {
        self.created
    }

    pub fn available(&self) -> usize {
        self.free.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Obj {
        id: usize,
        used: bool,
    }

    #[test]
    fn reutilise_les_objets_liberes_et_applique_reset() {
        let n = AtomicUsize::new(0);
        let mut pool = Pool::new(move || Obj {
            id: n.fetch_add(1, Ordering::Relaxed),
            used: false,
        })
        .with_reset(|o| o.used = false);
        let mut a = pool.acquire().expect("pool sans limite");
        let id = a.id;
        a.used = true;
        pool.release(a);
        let b = pool.acquire().expect("objet libre");
        assert_eq!(b.id, id);
        assert!(!b.used);
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn respecte_la_limite_max() {
        let mut pool = Pool::new(|| ()).with_max(2);
        assert!(pool.acquire().is_some());
        assert!(pool.acquire().is_some());
        assert!(pool.acquire().is_none());
    }
}
