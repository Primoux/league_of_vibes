// Contour des champions : coque inversée extrudée le long des normales lissées.
// Portage de `OUTLINE_VERT` / `OUTLINE_FRAG` (`packages/client/src/render/champions/builder.ts`).

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_normal_local_to_world};
import bevy_pbr::render::mesh_view_bindings::view;

struct Outline {
    // rgb : couleur (linéaire), a : épaisseur
    color: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Outline;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> @builtin(position) vec4<f32> {
    let world = get_world_from_local(v.instance_index) * vec4<f32>(v.position, 1.0);
    var mv = view.view_from_world * world;
    let nw = mesh_normal_local_to_world(v.normal, v.instance_index);
    let n = normalize((view.view_from_world * vec4<f32>(nw, 0.0)).xyz);
    // épaisseur ~ constante à l'écran : proportionnelle à la distance
    mv = vec4<f32>(mv.xyz + n * u.color.a * max(1.0, -mv.z) * 0.01, mv.w);
    return view.clip_from_view * mv;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(u.color.rgb, 1.0);
}
