//! Styles de particules standard, partagés par tous les FX.
//! La plupart sont blancs : la couleur vient de la teinte passée à `emit` (palette du champion).
//!
//! Portage de `packages/client/src/fx/particles/styles.ts`.

use bevy::prelude::Resource;

use super::atlas::Tex;
use super::system::{Blend, ParticleStyle, ParticleStyleDef, ParticleSystem};
use crate::fx::core::curves::presets::*;

#[derive(Resource, Clone, Copy, Debug)]
pub struct StdStyles {
    pub glow: ParticleStyle,
    pub glow_pop: ParticleStyle,
    pub spark: ParticleStyle,
    pub ember: ParticleStyle,
    pub smoke: ParticleStyle,
    pub dust: ParticleStyle,
    pub ring: ParticleStyle,
    pub star: ParticleStyle,
    pub snow: ParticleStyle,
    pub feather: ParticleStyle,
    pub rune: ParticleStyle,
    pub shard: ParticleStyle,
    pub debris: ParticleStyle,
    pub cross: ParticleStyle,
    pub coin: ParticleStyle,
    pub flame: ParticleStyle,
    pub swirl: ParticleStyle,
    pub sparkles: ParticleStyle,
    pub dark_wisp: ParticleStyle,
    pub blood: ParticleStyle,
}

const fn def(texture: Tex) -> ParticleStyleDef {
    ParticleStyleDef::new(texture)
}

impl StdStyles {
    pub fn create(ps: &mut ParticleSystem) -> Self {
        Self {
            glow: ps.define_style(ParticleStyleDef {
                alpha: FADE_OUT,
                ..def(Tex::Glow)
            }),
            glow_pop: ps.define_style(ParticleStyleDef {
                alpha: QUICK_IN_SLOW_OUT,
                size: POP_SHRINK,
                ..def(Tex::Glow)
            }),
            spark: ps.define_style(ParticleStyleDef {
                alpha: &[[0.0, 1.0], [0.7, 0.8], [1.0, 0.0]],
                size: SHRINK,
                ..def(Tex::Streak)
            }),
            ember: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: SHRINK,
                ..def(Tex::Dot)
            }),
            smoke: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                color: &[[0.0, 0.5, 0.5, 0.52], [1.0, 0.25, 0.25, 0.27]],
                alpha: &[[0.0, 0.0], [0.15, 0.55], [1.0, 0.0]],
                size: SMOKE_GROW,
                ..def(Tex::Smoke)
            }),
            dust: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                color: &[[0.0, 0.45, 0.36, 0.25], [1.0, 0.3, 0.25, 0.18]],
                alpha: &[[0.0, 0.0], [0.1, 0.6], [1.0, 0.0]],
                size: SMOKE_GROW,
                ..def(Tex::Smoke)
            }),
            ring: ps.define_style(ParticleStyleDef {
                alpha: FADE_OUT,
                size: &[[0.0, 0.2], [1.0, 1.0]],
                ..def(Tex::Ring)
            }),
            star: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: ONE,
                ..def(Tex::Star)
            }),
            snow: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: ONE,
                ..def(Tex::Snowflake)
            }),
            feather: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                alpha: FADE_IN_OUT,
                size: ONE,
                ..def(Tex::Feather)
            }),
            rune: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: GROW,
                ..def(Tex::Rune)
            }),
            shard: ps.define_style(ParticleStyleDef {
                alpha: &[[0.0, 1.0], [0.8, 1.0], [1.0, 0.0]],
                size: ONE,
                ..def(Tex::Shard)
            }),
            debris: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                alpha: &[[0.0, 1.0], [0.8, 1.0], [1.0, 0.0]],
                size: ONE,
                ..def(Tex::Shard)
            }),
            cross: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: POP_SHRINK,
                ..def(Tex::Cross)
            }),
            coin: ps.define_style(ParticleStyleDef {
                alpha: &[[0.0, 1.0], [0.75, 1.0], [1.0, 0.0]],
                size: ONE,
                ..def(Tex::Coin)
            }),
            flame: ps.define_style(ParticleStyleDef {
                color: &[
                    [0.0, 3.0, 2.6, 1.6],
                    [0.3, 2.4, 1.1, 0.25],
                    [0.7, 1.0, 0.25, 0.05],
                    [1.0, 0.2, 0.05, 0.02],
                ],
                alpha: FADE_IN_OUT,
                size: &[[0.0, 0.6], [0.3, 1.0], [1.0, 0.3]],
                ..def(Tex::Flame)
            }),
            swirl: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: GROW,
                ..def(Tex::Swirl)
            }),
            sparkles: ps.define_style(ParticleStyleDef {
                alpha: FADE_IN_OUT,
                size: ONE,
                ..def(Tex::Sparkles)
            }),
            dark_wisp: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                color: &[[0.0, 0.08, 0.04, 0.12], [1.0, 0.02, 0.01, 0.04]],
                alpha: &[[0.0, 0.0], [0.2, 0.75], [1.0, 0.0]],
                size: SMOKE_GROW,
                ..def(Tex::Smoke)
            }),
            // gouttes opaques (mélange normal) : un rouge sombre additif serait invisible sur l'herbe
            blood: ps.define_style(ParticleStyleDef {
                blend: Blend::Normal,
                color: &[[0.0, 0.55, 0.02, 0.04], [1.0, 0.3, 0.01, 0.02]],
                alpha: &[[0.0, 1.0], [0.7, 1.0], [1.0, 0.0]],
                size: SHRINK,
                ..def(Tex::Dot)
            }),
        }
    }
}
