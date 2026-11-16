// Particules GPU : portage du vertex/fragment shader de `packages/client/src/fx/particles/ParticleSystem.ts`.
//
// Chaque particule occupe 4 sommets d'un maillage fixe (coin du quad en xy, numéro de particule en z). Ses
// attributs, écrits une fois à l'émission, sont lus dans un tampon de stockage ; la trajectoire est calculée
// analytiquement depuis l'âge (p0 + v·k(t) − ½g·t²), couleur/alpha/taille sont lues dans la texture de courbes.

import bevy_pbr::render::mesh_view_bindings::view;

struct Params {
    // x : horloge des particules (s), y : 1 = mélange additif
    time: vec4<f32>,
}

struct Particle {
    p0_birth: vec4<f32>,   // position initiale, naissance
    vel_life: vec4<f32>,   // vitesse, durée de vie
    misc: vec4<f32>,       // taille, vitesse de rotation, gravité, traînée
    style: vec4<f32>,      // style + 256 × texture, étirement, rotation initiale, —
    tint: vec4<f32>,       // rgb
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: Params;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> particles: array<Particle>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var curves: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var atlas_sampler: sampler;

const CURVE_LAST: f32 = 63.0;

struct Vertex {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

fn curve_at(row: i32, t: f32) -> vec4<f32> {
    let x = clamp(t, 0.0, 1.0) * CURVE_LAST;
    let i = i32(floor(x));
    let j = min(i + 1, i32(CURVE_LAST));
    return mix(textureLoad(curves, vec2<i32>(i, row), 0), textureLoad(curves, vec2<i32>(j, row), 0), fract(x));
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let p = particles[u32(v.position.z)];
    let age = params.time.x - p.p0_birth.w;
    let t = age / p.vel_life.w;
    if t < 0.0 || t > 1.0 {
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }

    let gravity = p.misc.z;
    let drag = p.misc.w;
    var k = age;
    if drag > 0.0 {
        k = (1.0 - exp(-drag * age)) / drag;
    }
    let pos = p.p0_birth.xyz + p.vel_life.xyz * k - vec3<f32>(0.0, 0.5 * gravity * age * age, 0.0);

    let style = p.style.x % 256.0;
    let tex = floor(p.style.x / 256.0);
    let row = i32(style) * 2;
    let col = curve_at(row, t);
    let size = curve_at(row + 1, t).r * p.misc.x;

    var mv = view.view_from_world * vec4<f32>(pos, 1.0);
    let corner = v.position.xy;
    let stretch = p.style.y;
    if stretch > 0.0 {
        var damp = 1.0;
        if drag > 0.0 {
            damp = exp(-drag * age);
        }
        let vel = p.vel_life.xyz * damp - vec3<f32>(0.0, gravity * age, 0.0);
        let vv = (view.view_from_world * vec4<f32>(vel, 0.0)).xy;
        let sp = length(vv);
        var d = vec2<f32>(1.0, 0.0);
        if sp > 1e-4 {
            d = vv / sp;
        }
        let n = vec2<f32>(-d.y, d.x);
        let len = size + sp * stretch;
        mv = vec4<f32>(mv.xy + d * corner.x * len + n * corner.y * size, mv.zw);
    } else {
        let a = p.style.z + p.misc.y * age;
        let c = cos(a);
        let s = sin(a);
        mv = vec4<f32>(mv.xy + mat2x2<f32>(vec2<f32>(c, s), vec2<f32>(-s, c)) * corner * size, mv.zw);
    }
    out.clip = view.clip_from_view * mv;
    // Cellule de l'atlas (ligne 0 en haut) ; UV rentrés de ~2 px : le filtrage ne lit jamais la voisine.
    let cell = vec2<f32>(tex % 4.0, floor(tex / 4.0));
    let local = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    out.uv = (local * 0.97 + 0.015 + cell) * 0.25;
    out.color = vec4<f32>(col.rgb * p.tint.rgb, col.a);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tx = textureSample(atlas, atlas_sampler, in.uv);
    let a = tx.a * in.color.a;
    if a < 0.004 {
        discard;
    }
    let rgb = in.color.rgb * tx.rgb;
    if params.time.y > 0.5 {
        // Pipeline « prémultiplié » de Bevy (src + dst·(1 − αsrc)) : alpha nul = mélange additif.
        return vec4<f32>(rgb * a, 0.0);
    }
    return vec4<f32>(rgb, a);
}
