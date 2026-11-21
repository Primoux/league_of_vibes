// Matériau des modèles low-poly : portage de `addRim` (`packages/client/src/render/models/rig.ts`).
//
// - occlusion de contact : le bas des modèles s'assombrit près du sol (hauteur monde), ce qui les « pose » ;
// - liseré (rim) : les faces tournées à l'opposé de la caméra s'éclaircissent, ce qui détache les silhouettes
//   du sol en vue plongeante.
// Les matériaux lumineux (`glow`) n'ont ni l'un ni l'autre : `rim.x` vaut alors 0.

import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing};
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};

// x : 1 = liseré et occlusion actifs
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> rim: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    if rim.x > 0.5 {
        let ao = mix(0.5, 1.0, smoothstep(-0.05, 0.55, in.world_position.y));
        let base = pbr_input.material.base_color.rgb * ao;
        pbr_input.material.base_color = vec4(base, pbr_input.material.base_color.a);
        let f = 1.0 - clamp(dot(pbr_input.N, pbr_input.V), 0.0, 1.0);
        let glow = base * vec3(0.95, 0.97, 1.1) * f * f * f * 0.6;
        pbr_input.material.emissive = vec4(pbr_input.material.emissive.rgb + glow, pbr_input.material.emissive.a);
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
