//! Navigation : grille dérivée des murs de la carte et recherche de chemin (port de `server/src/nav`).

pub mod grid;

pub use grid::{NAV_CELL, NAV_MARGIN, NavGrid, NavSearch, nav_grid};
