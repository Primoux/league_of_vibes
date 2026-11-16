// Ruban de traînée : portage du shader de `packages/client/src/fx/shaders/ribbon.ts`.
//
// UV : x = alpha du point (vie × effilement), y = côté du ruban (0 ou 1).

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_position_local_to_clip};

struct Params {
    color: vec3<f32>,
    opacity: f32,
    core: vec3<f32>,
    _pad: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Params;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) alpha: f32,
    @location(1) side: f32,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.alpha = v.uv.x;
    out.side = v.uv.y * 2.0 - 1.0;
    out.clip = mesh_position_local_to_clip(get_world_from_local(v.instance_index), vec4<f32>(v.position, 1.0));
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // max() : pow(x < 0) est indéfini → NaN, que le bloom étale en grande tache
    let across = max(0.0, 1.0 - in.side * in.side);
    let core = pow(across, 6.0);
    let col = mix(u.color, u.core, core * in.alpha);
    let a = across * in.alpha * u.opacity;
    // mélange additif via le pipeline « prémultiplié » de Bevy (alpha nul)
    return vec4<f32>(col * a, 0.0);
}
