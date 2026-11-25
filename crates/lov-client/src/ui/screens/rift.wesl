// Fond animé des menus : portage du canvas 2D de `packages/client/src/ui/stage/RiftBackground.tsx`.
//
// Faille lumineuse en diagonale qui ondule (trois traits additifs), étoiles scintillantes, braises qui
// montent. Tout est fonction du temps : l'écran peut être reconstruit sans que le fond saute. Les braises
// sont déterministes (graine par emplacement et par cycle) au lieu du `Math.random` du web.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Rift {
    // teinte (linéaire)
    tint: vec4<f32>,
    // intensité, nombre de braises
    params: vec4<f32>,
}

@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> u: Rift;

const PI: f32 = 3.14159265;
const VIOLET: vec3<f32> = vec3<f32>(0.434, 0.168, 1.0);
const GOLD: vec3<f32> = vec3<f32>(0.922, 0.644, 0.188);
const STAR: vec3<f32> = vec3<f32>(0.807, 0.776, 1.0);

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Ordonnée de la faille à l'abscisse `x` (trait `k`).
fn rift_y(x: f32, size: vec2<f32>, t: f32, k: f32) -> f32 {
    let f = x / size.x;
    return size.y - f * size.y + sin(f * 7.0 + t * 0.6 + k) * 18.0 + sin(f * 2.3 - t * 0.3) * 30.0;
}

// Dégradé de la faille, du bas-gauche au haut-droite : couleur prémultipliée.
fn rift_gradient(g: f32, intensity: f32) -> vec4<f32> {
    let tint = u.tint.rgb;
    var c = vec4<f32>(0.0);
    if g < 0.35 {
        c = mix(vec4<f32>(tint, 0.0), vec4<f32>(tint, 0.07 * intensity), g / 0.35);
    } else if g < 0.5 {
        c = mix(vec4<f32>(tint, 0.07 * intensity), vec4<f32>(tint, 0.16 * intensity), (g - 0.35) / 0.15);
    } else if g < 0.65 {
        c = mix(vec4<f32>(tint, 0.16 * intensity), vec4<f32>(VIOLET, 0.08), (g - 0.5) / 0.15);
    } else {
        c = mix(vec4<f32>(VIOLET, 0.08), vec4<f32>(VIOLET, 0.0), (g - 0.65) / 0.35);
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = in.size;
    let p = in.uv * size;
    let t = globals.time;
    let intensity = u.params.x;

    // faille lumineuse (composition additive)
    var glow = vec3<f32>(0.0);
    let dir = vec2<f32>(size.x, -size.y);
    let g = clamp(dot(p - vec2<f32>(0.0, size.y), dir) / dot(dir, dir), 0.0, 1.0);
    let grad = rift_gradient(g, intensity);
    for (var k = 0; k < 3; k++) {
        let kf = f32(k);
        let base = select(select(6.0, 40.0, k == 1), 120.0, k == 0);
        let width = base * (1.0 + sin(t * 0.7 + kf) * 0.08);
        let alpha = select(0.6, 0.9, k == 2);
        let y = rift_y(p.x, size, t, kf);
        let slope = (rift_y(p.x + 1.0, size, t, kf) - y);
        let d = abs(p.y - y) / sqrt(1.0 + slope * slope);
        let cover = clamp(width * 0.5 - d + 0.5, 0.0, 1.0);
        glow += grad.rgb * alpha * cover;
    }

    // braises : montent du bas, ondulent, s'allument puis s'éteignent (additives)
    let n = i32(u.params.y);
    for (var i = 0; i < n; i++) {
        let fi = f32(i);
        let life0 = hash(vec2<f32>(fi, 0.5));
        let period = (1.6 - life0) / 0.12;
        let s = t + hash(vec2<f32>(fi, 1.5)) * period;
        let cycle = floor(s / period);
        let age = s - cycle * period;
        let seed = vec2<f32>(fi, cycle);
        let vx = (hash(seed + 0.11) - 0.5) * 12.0;
        let vy = -(14.0 + hash(seed + 0.23) * 38.0);
        let r = 0.6 + hash(seed + 0.37) * 2.2;
        let hue = hash(seed + 0.41);
        let half = r * 3.5;
        let y = size.y + 10.0 + vy * age;
        if abs(p.y - y) > half {
            continue;
        }
        let wob = 6.0 * (cos(t - age + hue * 10.0) - cos(t + hue * 10.0));
        let x = hash(seed + 0.53) * size.x + vx * age + wob;
        let dist = length(p - vec2<f32>(x, y)) / half;
        if dist >= 1.0 {
            continue;
        }
        let life = life0 + age * 0.12;
        let a = max(0.0, sin(min(1.0, life) * PI)) * 0.9;
        var col = GOLD;
        if hue >= 0.8 {
            col = VIOLET;
        } else if hue >= 0.55 {
            col = u.tint.rgb;
        }
        // sprite : blanc au centre, couleur à 18 %, transparent au bord
        var sprite = vec4<f32>(0.0);
        if dist < 0.18 {
            sprite = mix(vec4<f32>(1.0), vec4<f32>(col, 0.7), dist / 0.18);
        } else {
            sprite = vec4<f32>(col, 0.7 * (1.0 - (dist - 0.18) / 0.82));
        }
        glow += sprite.rgb * sprite.a * a;
    }

    // l'addition sur un fond sombre devient un mélange : alpha = luminosité ajoutée
    var alpha = clamp(max(glow.r, max(glow.g, glow.b)), 0.0, 1.0);
    var color = glow;

    // étoiles scintillantes (une par case de 95 px, comme w × h / 9000)
    let cell = floor(p / 95.0);
    let sp = (cell + vec2<f32>(hash(cell), hash(cell + 7.3))) * 95.0;
    let sr = (hash(cell + 3.1) * 1.3 + 0.2) * 0.8;
    let sd = max(abs(p.x - sp.x), abs(p.y - sp.y));
    if sd < max(sr, 1.0) * 0.5 {
        let sa = (0.15 + 0.35 * (0.5 + 0.5 * sin(t * 1.3 + hash(cell + 5.9) * 2.0 * PI))) * min(sr, 1.0);
        color = STAR * sa + color * (1.0 - sa);
        alpha = sa + alpha * (1.0 - sa);
    }
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(color / alpha, alpha);
}
