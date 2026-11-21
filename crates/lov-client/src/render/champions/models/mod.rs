//! Modèles dédiés des champions : squelette, clips, port d'arme et extras propres à chacun. Un fichier
//! par champion (tranche verticale, area `champion`) ; chaque tranche ajoute ici sa ligne `pub mod` et
//! son entrée dans `MODELS`.

use super::model::ChampionModelDef;

pub mod corvin;
pub mod grondak;
pub mod ilyra;
pub mod kaelen;
pub mod master67;
pub mod nyx;
pub mod sable;
pub mod solenne;
pub mod tharox;
pub mod touilleur;
pub mod vrenna;
pub mod zephyr;

/// Modèles dédiés, par identifiant de champion (`MODELS` de `index.ts`).
pub const MODELS: &[(&str, ChampionModelDef)] = &[
    ("corvin", corvin::MODEL),
    ("grondak", grondak::MODEL),
    ("ilyra", ilyra::MODEL),
    ("kaelen", kaelen::MODEL),
    ("master67", master67::MODEL),
    ("nyx", nyx::MODEL),
    ("sable", sable::MODEL),
    ("solenne", solenne::MODEL),
    ("tharox", tharox::MODEL),
    ("vrenna", vrenna::MODEL),
    ("zephyr", zephyr::MODEL),
    ("touilleur", touilleur::MODEL),
];

#[cfg(test)]
pub(crate) mod testing {
    use crate::render::champions::anim::pose::{ClipCtx, POSE_SIZE, Pose};
    use crate::render::champions::model::ChampionModelDef;

    /// Chaque clip propre au modèle écrit une pose finie, qui varie dans le temps (`master67-model.test`).
    pub fn clips_finis_et_animes(def: &ChampionModelDef) {
        let ctx = ClipCtx {
            hand: def.hand.unwrap_or(1.0),
            energy: def.energy.unwrap_or(1.0),
        };
        for (name, clip) in def.clips {
            let (mut a, mut b): (Pose, Pose) = ([0.0; POSE_SIZE], [0.0; POSE_SIZE]);
            (clip.sample)(0.1, &mut a, &ctx);
            (clip.sample)(0.4, &mut b, &ctx);
            assert!(a.iter().all(|v| v.is_finite()), "{name} : pose non finie");
            assert!(a != b, "{name} : pose figée entre 0,1 s et 0,4 s");
        }
    }
}
