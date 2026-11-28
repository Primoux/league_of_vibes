// Brouillard de guerre (`applyFog` / `FOG_GLSL` du web, voir `fog.rs`) : code commun aux shaders du décor
// (`terrain.wesl`, `foliage.wesl`, `cliff.wesl`, `water.wesl`), qui l'importent par `super::fog`.
//
// Chaque shader déclare lui-même la texture de visibilité et son échantillonneur (ses numéros de liaison
// lui sont propres) et les passe à `fog_visibility`.

// Côté de la carte en unités monde (`MAP_SIZE × RENDER_SCALE`, vérifié par un test de `fog.rs`).
const MAP_WORLD: f32 = 120.0;
// Luminosité gardée dans une zone non visible.
const DARK: f32 = 0.42;

// Visibilité 0..1 (1 = visible) au point monde `world` : la texture couvre la carte, x vers +x, y vers −z.
fn fog_visibility(vis: texture_2d<f32>, smp: sampler, world: vec3<f32>) -> f32 {
    return textureSampleLevel(vis, smp, vec2(world.x, -world.z) / MAP_WORLD, 0.0).r;
}

// Couleur d'une surface éclairée : hors de vue, assombrie, désaturée et bleutée (`applyFog`).
fn apply_fog(c: vec3<f32>, v: f32) -> vec3<f32> {
    let grey = vec3(dot(c, vec3(0.299, 0.587, 0.114)));
    return mix(mix(grey, c, 0.45) * DARK * vec3(0.85, 0.9, 1.05), c, v);
}

// Variante de l'eau (`FOG_GLSL`) : seulement assombrie.
fn dim_fog(c: vec3<f32>, v: f32) -> vec3<f32> {
    return mix(c * DARK, c, v);
}
