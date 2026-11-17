// Filet anti-NaN avant le bloom : portage du filtre de luminance de `packages/client/src/fx/post/PostFx.ts`.
//
// Un seul pixel NaN/Inf (shader d'un autre objet, GPU qui diverge de la norme…) serait étalé par le flou du
// bloom en gros blocs noirs : on le remplace par du noir, et on plafonne le HDR pour éviter les halos géants.
// Le test se fait sur les bits (exposant à 0xff = NaN ou Inf) : `x != x` peut être replié à faux par le
// compilateur, WGSL l'autorisant à supposer qu'aucun NaN n'existe.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Sanitize {
    max_hdr: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> s: Sanitize;

fn non_fini(v: vec4<f32>) -> bool {
    let e = bitcast<vec4<u32>>(v) & vec4<u32>(0x7f800000u);
    return any(e == vec4<u32>(0x7f800000u));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSample(screen, screen_sampler, in.uv);
    if non_fini(src) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(clamp(src.rgb, vec3<f32>(0.0), vec3<f32>(s.max_hdr)), src.a);
}
