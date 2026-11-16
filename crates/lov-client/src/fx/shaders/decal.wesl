// Décal au sol : portage du shader de `packages/client/src/fx/shaders/decal.ts`.
//
// Un seul shader paramétré par forme (cercle, anneau d'onde de choc, rectangle, cône). Les distances (bord,
// motif) sont en unités monde, quelle que soit l'échelle du plan.

import bevy_pbr::render::mesh_functions::{get_world_from_local, mesh_position_local_to_clip};
import bevy_pbr::render::mesh_view_bindings::globals;

struct Decal {
    color: vec3<f32>,
    shape: u32,
    color2: vec3<f32>,
    // 1 = mélange additif
    additive: f32,
    // demi-dimensions monde (x = rayon ou demi-longueur)
    size: vec2<f32>,
    opacity: f32,
    // opacité du remplissage de fond
    fill: f32,
    // épaisseur du bord (monde)
    edge: f32,
    // remplissage temporel 0..1 (télégraphie), < 0 = désactivé
    progress: f32,
    // demi-angle du cône (rad)
    angle: f32,
    // intensité du motif animé
    pattern: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Decal;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) p: vec2<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.p = vec2<f32>(v.position.x, -v.position.z);
    out.clip = mesh_position_local_to_clip(get_world_from_local(v.instance_index), vec4<f32>(v.position, 1.0));
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.p;
    let r = length(p);
    var d: f32;      // distance au bord (monde), > 0 à l'intérieur
    var along: f32;  // 0..1 pour la progression
    if u.shape == 0u || u.shape == 1u {
        d = (1.0 - r) * u.size.x;
        along = r;
    } else if u.shape == 2u {
        d = min((1.0 - abs(p.x)) * u.size.x, (1.0 - abs(p.y)) * u.size.y);
        along = p.x * 0.5 + 0.5;
    } else {
        var a = 0.0; // atan2(0, 0) est indéfini sur certains GPU
        if r > 1e-5 {
            a = abs(atan2(p.y, p.x));
        }
        d = min((1.0 - r) * u.size.x, (u.angle - a) * r * u.size.x);
        along = r;
    }
    if d < 0.0 {
        discard;
    }

    var alpha: f32;
    var col = u.color;
    if u.shape == 1u {
        // onde de choc : anneau fin + halo intérieur
        let band = 1.0 - smoothstep(0.0, u.edge, d);
        let inner = (1.0 - smoothstep(0.0, u.edge * 4.0, d)) * 0.35;
        alpha = band + inner;
    } else {
        let edge = 1.0 - smoothstep(0.0, u.edge, d);
        let pattern = u.pattern * (0.5 + 0.5 * sin(along * u.size.x * 6.0 - globals.time * 4.0));
        alpha = edge + u.fill * (0.7 + 0.3 * pattern);
        if u.progress >= 0.0 {
            let inside = step(along, u.progress);
            let front = 1.0 - smoothstep(0.0, 0.06, abs(along - u.progress));
            alpha += inside * 0.25 + front * 0.6 * step(0.001, u.progress);
            col = mix(u.color, u.color2, inside * 0.5);
        }
    }
    let a = clamp(alpha, 0.0, 1.0) * u.opacity;
    if u.additive > 0.5 {
        // Pipeline « prémultiplié » de Bevy (src + dst·(1 − αsrc)) : alpha nul = mélange additif.
        return vec4<f32>(col * a, 0.0);
    }
    return vec4<f32>(col, a);
}
