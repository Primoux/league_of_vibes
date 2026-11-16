// Eau stylisée (zones, souffle océan) : portage de `createWaterMaterial`
// (`packages/client/src/fx/shaders/materials.ts`). Disque inscrit dans les UV du maillage.

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_position_local_to_clip};
import bevy_pbr::render::mesh_view_bindings::globals;

struct Water {
    color: vec3<f32>,
    opacity: f32,
    foam: vec3<f32>,
    _pad: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Water;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.uv = v.uv;
    out.clip = mesh_position_local_to_clip(get_world_from_local(v.instance_index), vec4<f32>(v.position, 1.0));
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * 2.0 - 1.0;
    let r = length(p);
    if r > 1.0 {
        discard;
    }
    let t = globals.time;
    let w = sin(p.x * 9.0 + t * 1.7) * sin(p.y * 7.0 - t * 1.3);
    let caustic = smoothstep(0.55, 0.95, 0.5 + 0.5 * w);
    let foam = smoothstep(0.82, 0.97, r) * (1.0 - smoothstep(0.97, 1.0, r));
    let col = u.color * (0.8 + 0.2 * w) + u.foam * (caustic * 0.12 + foam * 0.8);
    return vec4<f32>(col, u.opacity * (0.55 + 0.45 * foam) * (1.0 - smoothstep(0.98, 1.0, r)));
}
