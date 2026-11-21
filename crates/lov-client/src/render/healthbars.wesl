// Barre de vie en monde : portage du shader de `packages/client/src/render/HealthBars.ts`.
//
// Taille fixe en pixels : l'ancre 3D (origine de l'entité de la barre) est projetée, puis le coin du quad est
// décalé à l'écran. PV, bouclier, mana (champions), graduations tous les 100 PV (épaisses tous les 1000).

import bevy_pbr::render::mesh_functions::get_world_from_local;
import bevy_pbr::render::mesh_view_bindings::view;

struct Bar {
    // couleur des PV (linéaire)
    color: vec4<f32>,
    // PV, bouclier, mana (-1 = pas de mana), PV max (0 = pas de graduations)
    bars: vec4<f32>,
    // largeur, hauteur (px)
    size: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> u: Bar;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let anchor = get_world_from_local(v.instance_index) * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    var clip = view.clip_from_world * anchor;
    clip = vec4<f32>(clip.xy + v.position.xy * u.size.xy * 2.0 / view.viewport.zw * clip.w, clip.zw);
    out.clip = clip;
    out.uv = v.position.xy + 0.5;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = u.size.xy;
    let px = in.uv * size;
    let border = 1.0;
    let has_mana = u.bars.z >= 0.0;
    // cadre : le client web le renvoie sans conversion d'espace couleur, donc (0.04, 0.04, 0.06) en sRGB ;
    // ici la sortie est linéaire, d'où les valeurs converties
    if px.x < border || px.y < border || px.x > size.x - border || px.y > size.y - border {
        return vec4<f32>(0.0031, 0.0031, 0.0048, 0.95);
    }
    var mana_h = 0.0;
    if has_mana {
        mana_h = max(3.0, size.y * 0.28);
    }
    let inner = vec2<f32>((px.x - border) / (size.x - 2.0 * border), px.y);
    var col = vec3<f32>(0.07, 0.07, 0.09);
    if has_mana && px.y < border + mana_h {
        // barre de mana (en bas)
        if inner.x < u.bars.z {
            col = mix(vec3<f32>(0.20, 0.45, 0.95), vec3<f32>(0.35, 0.65, 1.0), (px.y - border) / mana_h);
        }
        if px.y > border + mana_h - 1.0 {
            col = vec3<f32>(0.04);
        }
    } else {
        let hp = u.bars.x;
        let sh = u.bars.y;
        let y_n = (px.y - border - mana_h) / (size.y - 2.0 * border - mana_h);
        if inner.x < hp {
            col = u.color.rgb * (0.82 + 0.3 * y_n);
        } else if inner.x < hp + sh {
            col = vec3<f32>(0.92, 0.92, 0.95);
        }
        // graduations tous les 100 PV (épaisses tous les 1000)
        let max_hp = u.bars.w;
        if max_hp > 0.0 && inner.x < hp + sh {
            let w = size.x - 2.0 * border;
            let step = w * 100.0 / max_hp;
            if step > 3.0 {
                // `mod` de GLSL (le reste de WGSL garde le signe, ici tout est positif)
                let m = (inner.x * w) % step;
                let big = (inner.x * w) % (step * 10.0);
                if m < 1.0 && y_n > 0.45 {
                    col *= 0.35;
                }
                if big < 1.5 {
                    col *= 0.35;
                }
            }
        }
    }
    return vec4<f32>(col, 1.0);
}
