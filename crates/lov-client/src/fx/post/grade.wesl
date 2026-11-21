// Étalonnage de sortie : portage du `GradedOutputPass` de `packages/client/src/fx/post/PostFx.ts`.
//
// Après le tone mapping, en linéaire (la conversion sRGB vient ensuite) : saturation, teinte des ombres,
// gain des hautes lumières, contraste autour du gris moyen, vignette. Filet anti-NaN/Inf : un pixel invalide
// sort noir (le filet principal, avant le bloom, est `sanitize.wesl`).

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct Grade {
    lift: vec3<f32>,
    saturation: f32,
    gain: vec3<f32>,
    contrast: f32,
    vignette: f32,
}

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> g: Grade;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSample(screen, screen_sampler, in.uv);
    // NaN/Inf testés sur les bits (exposant à 0xff), avant tout `max` : `c != c` peut être replié à faux.
    let e = bitcast<vec3<u32>>(src.rgb) & vec3<u32>(0x7f800000u);
    var c = select(max(src.rgb, vec3<f32>(0.0)), vec3<f32>(0.0), any(e == vec3<u32>(0x7f800000u)));
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(l), c, g.saturation);
    c += g.lift * (1.0 - smoothstep(0.0, 0.3, l));
    c *= mix(vec3<f32>(1.0), g.gain, smoothstep(0.25, 0.9, l));
    c = 0.18 * pow(max(c, vec3<f32>(0.0)) / 0.18 + 1e-6, vec3<f32>(g.contrast));
    let dv = (in.uv - 0.5) * vec2<f32>(1.0, 0.8);
    c *= 1.0 - g.vignette * smoothstep(0.3, 0.75, length(dv));
    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), src.a);
}
