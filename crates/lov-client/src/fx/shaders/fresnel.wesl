// Fresnel / halo de bord (boucliers, dômes, auras) : portage de `createFresnelMaterial`
// (`packages/client/src/fx/shaders/materials.ts`). Calcul en espace vue, comme le client web.

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_normal_local_to_world};
import bevy_pbr::render::mesh_view_bindings::{globals, view};

struct Fresnel {
    color: vec3<f32>,
    power: f32,
    base: f32,
    bands: f32,
    opacity: f32,
    _pad: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Fresnel;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) n: vec3<f32>,
    @location(1) v: vec3<f32>,
    @location(2) local: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world = get_world_from_local(v.instance_index) * vec4<f32>(v.position, 1.0);
    let mv = view.view_from_world * world;
    let n = mesh_normal_local_to_world(v.normal, v.instance_index);
    out.n = normalize((view.view_from_world * vec4<f32>(n, 0.0)).xyz);
    out.v = normalize(-mv.xyz);
    out.local = v.position;
    out.clip = view.clip_from_view * mv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let f = pow(max(0.0, 1.0 - abs(dot(normalize(in.n), normalize(in.v)))), u.power);
    let bands = u.bands * (0.5 + 0.5 * sin(in.local.y * 18.0 - globals.time * 3.0));
    let a = clamp((u.base + f + bands * f) * u.opacity, 0.0, 1.0);
    // mélange additif via le pipeline « prémultiplié » de Bevy (alpha nul)
    return vec4<f32>(u.color * a, 0.0);
}
