// Falaises : portage du `onBeforeCompile` de `buildWallBases` (`packages/client/src/render/map/vegetation.ts`).
//
// Couleurs de sommets (strates, pied assombri) ; le dessus (normale verticale) est recoloré par pixel avec
// la texture de bruit en coordonnées monde, comme le sol : pas d'aplat par triangle.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};
import super::fog::{fog_visibility, apply_fog};

struct Cliff {
    moss: vec4<f32>,
    moss_light: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> u: Cliff;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var noise: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var noise_sampler: sampler;
// Brouillard de guerre (`fog.wesl`) : les murs hors de vue sont assombris comme le sol.
@group(constants::MATERIAL_BIND_GROUP) @binding(110) var fog_vis: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111) var fog_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let wp = in.world_position.xz;
    // lectures hors du `if` (flot de contrôle uniforme exigé par WGSL)
    let nn = textureSample(noise, noise_sampler, wp * 0.011).r * 0.6 + textureSample(noise, noise_sampler, wp * 0.05).g * 0.4;
    let grain = textureSample(noise, noise_sampler, wp * 0.31).b;
    if in.world_normal.y > 0.9 {
        let top = mix(u.moss.rgb, u.moss_light.rgb, smoothstep(0.25, 0.75, nn)) * (0.84 + 0.3 * grain);
        pbr_input.material.base_color = vec4(top, pbr_input.material.base_color.a);
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    let fog = fog_visibility(fog_vis, fog_sampler, in.world_position.xyz);
    out.color = vec4(apply_fog(out.color.rgb, fog), out.color.a);
    return out;
}
