// Givre cristallin (facettes) : portage de `createIceMaterial` (`packages/client/src/fx/shaders/materials.ts`).
// Normale de facette via dérivées écran : look low-poly sans dupliquer les sommets. Lumières en espace vue,
// comme le client web.

import bevy_pbr::render::mesh_functions::get_world_from_local;
import bevy_pbr::render::mesh_view_bindings::view;

struct Ice {
    color: vec3<f32>,
    opacity: f32,
    deep: vec3<f32>,
    _pad: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Ice;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) view_pos: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let mv = view.view_from_world * (get_world_from_local(v.instance_index) * vec4<f32>(v.position, 1.0));
    out.view_pos = mv.xyz;
    out.clip = view.clip_from_view * mv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let cr = cross(dpdx(in.view_pos), dpdy(in.view_pos));
    // facette dégénérée (éclat qui naît)
    var n = vec3<f32>(0.0, 0.0, 1.0);
    if dot(cr, cr) > 1e-20 {
        n = normalize(cr);
    }
    let v = normalize(-in.view_pos);
    let rim = pow(max(0.0, 1.0 - abs(dot(n, v))), 2.0);
    let light = 0.5 + 0.5 * dot(n, normalize(vec3<f32>(0.3, 0.8, 0.5)));
    let glint = pow(max(0.0, dot(reflect(-v, n), normalize(vec3<f32>(0.2, 0.9, 0.4)))), 24.0);
    let col = (mix(u.deep, u.color, light) * 0.85 + rim * u.color * 0.7) + glint * 1.2;
    return vec4<f32>(col, u.opacity * (0.75 + 0.25 * rim));
}
