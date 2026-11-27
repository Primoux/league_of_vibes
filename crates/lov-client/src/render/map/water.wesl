// Rivière animée : portage du shader de `packages/client/src/render/map/water.ts`.
//
// Profondeur simulée par la distance aux berges (bords transparents : on voit le lit sableux et ses
// caustiques, rendus par le shader du sol), pseudo-normales issues de deux couches de bruit qui défilent,
// reflet du ciel (Fresnel), reflets du soleil en HDR (captés par le bloom), écume des berges, filets de
// courant. Tout le bruit vient de la texture partagée : 5 lectures de texture, aucun bruit calculé.

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_position_local_to_world};
import bevy_pbr::render::mesh_view_bindings::{globals, view};
import bevy_pbr::render::view_transformations::position_world_to_clip;

struct River {
    // longueur, largeur (monde)
    size: vec2<f32>,
    _pad: vec2<f32>,
    deep: vec4<f32>,
    shallow: vec4<f32>,
    foam: vec4<f32>,
    sky: vec4<f32>,
    sun: vec4<f32>,
    sun_dir: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: River;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var noise: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var noise_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.uv = v.uv;
    let w = mesh_position_local_to_world(get_world_from_local(v.instance_index), vec4<f32>(v.position, 1.0));
    out.world = w.xyz;
    out.clip = position_world_to_clip(w.xyz);
    return out;
}

fn n(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(noise, noise_sampler, uv);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * u.size;
    let t = globals.time;
    let across = abs(in.uv.y - 0.5) * 2.0; // 0 au centre, 1 aux berges
    let ends = min(in.uv.x, 1.0 - in.uv.x) * u.size.x;
    let depth = 1.0 - across * across;

    // deux couches de vagues qui défilent avec le courant (+x local)
    let a = n(p * vec2(0.07, 0.11) + vec2(-t * 0.05, t * 0.012));
    let b = n(p * vec2(0.16, 0.21) + vec2(-t * 0.11, -t * 0.02));
    let nrm = normalize(vec3((a.g - 0.5) * 0.4 + (b.b - 0.5) * 0.25, 1.0, (a.b - 0.5) * 0.4 + (b.g - 0.5) * 0.25));
    let v = normalize(view.world_position - in.world);

    var col = mix(u.shallow.rgb, u.deep.rgb, smoothstep(0.05, 0.85, depth + (a.r - 0.5) * 0.15));
    col *= 0.94 + 0.12 * b.g;
    // filets de courant étirés dans le sens de l'écoulement
    let streak = smoothstep(0.72, 0.9, n(vec2(p.x * 0.035 - t * 0.09, p.y * 0.45)).g);
    col += u.foam.rgb * streak * 0.045 * depth;
    // reflet du ciel selon l'angle, reflets du soleil (HDR : ils passent le seuil du bloom)
    let fres = pow(1.0 - clamp(dot(nrm, v), 0.0, 1.0), 4.0);
    col = mix(col, u.sky.rgb, clamp(fres * 1.6, 0.0, 0.55));
    let h = normalize(u.sun_dir.xyz + v);
    let spec = pow(clamp(dot(nrm, h), 0.0, 1.0), 220.0);
    col += u.sun.rgb * spec * 1.4;
    let glint = smoothstep(0.93, 0.99, b.a) * smoothstep(0.62, 0.9, a.g) * depth;
    col += u.sun.rgb * glint * 0.6;
    // écume des berges, animée
    let foam_band = smoothstep(0.74, 0.95, across + (a.g - 0.5) * 0.18);
    let foam_tex = smoothstep(0.35, 0.65, n(p * 0.45 + vec2(-t * 0.16, t * 0.05)).a);
    let foam = foam_band * mix(0.45, 1.0, foam_tex);
    col = mix(col, u.foam.rgb, foam * 0.75);
    var alpha = mix(0.42, 0.86, smoothstep(0.0, 0.65, depth));
    alpha = max(alpha, foam * 0.85);
    alpha *= (1.0 - smoothstep(0.97, 1.0, across)) * smoothstep(0.0, 4.0, ends);
    return vec4(col, alpha);
}
