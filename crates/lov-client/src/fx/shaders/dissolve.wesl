// Dissolution (mort) : extension du matériau standard, portage de `makeDissolvable` /
// `makeDissolveDepthMaterial` (`packages/client/src/fx/shaders/materials.ts`).
//
// Le même bruit sert au rendu et à la passe d'ombre (`PREPASS_PIPELINE`) : sinon l'ombre ne disparaît pas
// en même temps que le modèle. La position monde vient après le skinning : la découpe suit les membres.

@if(PREPASS_PIPELINE)
import bevy_pbr::render::pbr_prepass_functions;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};

struct Dissolve {
    edge_color: vec3<f32>,
    // 0 = intact, 1 = entièrement dissous
    amount: f32,
    noise_scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> dissolve: Dissolve;

fn d_hash(q: vec3<f32>) -> f32 {
    var p = fract(q * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

fn d_noise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(d_hash(i), d_hash(i + vec3<f32>(1.0, 0.0, 0.0)), f.x),
            mix(d_hash(i + vec3<f32>(0.0, 1.0, 0.0)), d_hash(i + vec3<f32>(1.0, 1.0, 0.0)), f.x),
            f.y,
        ),
        mix(
            mix(d_hash(i + vec3<f32>(0.0, 0.0, 1.0)), d_hash(i + vec3<f32>(1.0, 0.0, 1.0)), f.x),
            mix(d_hash(i + vec3<f32>(0.0, 1.0, 1.0)), d_hash(i + vec3<f32>(1.0, 1.0, 1.0)), f.x),
            f.y,
        ),
        f.z,
    );
}

// bruit + biais vertical : le haut du modèle disparaît en premier
fn dissolve_value(pos: vec3<f32>) -> f32 {
    return d_noise(pos * dissolve.noise_scale) * 0.8 + (1.0 - clamp(pos.y / 2.5, 0.0, 1.0)) * 0.2;
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    if dissolve.amount > 0.0 && dissolve_value(in.world_position.xyz) < dissolve.amount {
        discard;
    }
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
    var out: FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
@if(NORMAL_PREPASS)
    out.normal = vec4<f32>(normalize(in.world_normal) * 0.5 + vec3<f32>(0.5), 1.0);
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
    return out;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOutput) {
    if dissolve.amount > 0.0 && dissolve_value(in.world_position.xyz) < dissolve.amount {
        discard;
    }
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var dn = 1.0;
    if dissolve.amount > 0.0 {
        dn = dissolve_value(in.world_position.xyz);
        if dn < dissolve.amount {
            discard;
        }
    }
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    if dissolve.amount > 0.0 {
        let edge = 1.0 - smoothstep(0.0, 0.07, dn - dissolve.amount);
        out.color = vec4<f32>(mix(out.color.rgb, dissolve.edge_color, edge), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
