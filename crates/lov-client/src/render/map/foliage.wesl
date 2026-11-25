// Végétation : portage de `addWind` (`packages/client/src/render/map/vegetation.ts`).
//
// - vent : balancement proportionnel au carré de la hauteur locale, phase selon la position de l'instance ;
// - teinte d'instance (`instanceColor` du client web) : couleur RGB8 rangée dans le `MeshTag` de l'entité,
//   multipliée aux couleurs de sommets ;
// - liseré lumineux de feuillage (`rim` > 0) : les bords tournés à l'opposé de la caméra s'éclaircissent,
//   effet de translucidité bon marché.
// La passe d'ombre garde la géométrie au repos (comme le client web, dont l'ombre ignore le vent).

import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput, FragmentOutput, decompress_vertex},
    mesh_functions,
    mesh_view_bindings::globals,
    view_transformations::position_world_to_clip,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

// x : force du vent, y : liseré
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> foliage: vec4<f32>;

fn tint(tag: u32) -> vec3<f32> {
    return vec3(f32(tag & 255u), f32((tag >> 8u) & 255u), f32((tag >> 16u) & 255u)) / 255.0;
}

@vertex
fn vertex(vin: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let v = decompress_vertex(vin, vin.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vin.instance_index);
    let ip = world_from_local[3].xyz;
    let t = globals.time;
    let sway = max(v.position.y, 0.0);
    var pos = v.position;
    pos.x += sin(t * 1.7 + ip.x * 1.3 + ip.z * 0.7) * foliage.x * sway * sway;
    pos.z += cos(t * 1.3 + ip.z * 1.1) * foliage.x * 0.6 * sway * sway;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, vin.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(pos, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
@if(VERTEX_COLORS)
    out.color = vec4(v.color.rgb * tint(mesh_functions::get_tag(vin.instance_index)), v.color.a);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vin.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vin.instance_index, world_from_local[3]);
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    if foliage.y > 0.0 {
        let f = 1.0 - clamp(dot(pbr_input.N, pbr_input.V), 0.0, 1.0);
        let glow = pbr_input.material.base_color.rgb * f * f * f * foliage.y;
        pbr_input.material.emissive = vec4(pbr_input.material.emissive.rgb + glow, pbr_input.material.emissive.a);
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
