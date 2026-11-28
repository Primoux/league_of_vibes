// Sol de la carte : portage de `TERRAIN_DECL` / `TERRAIN_MAP` (`packages/client/src/render/map/terrain.ts`).
//
// Extension du matériau standard : la couleur de base est composée ici (herbe, terre, cailloux, sable humide,
// dallage des bases, caustiques) à partir des masques (r voie, g sable, b dallage, a occlusion) et de la
// texture de bruit échantillonnée en coordonnées monde. Unités monde : 1 = 100 unités de jeu.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
};
import super::fog::{fog_visibility, apply_fog};

struct Terrain {
    fount_b: vec4<f32>,
    fount_r: vec4<f32>,
    grass: vec4<f32>,
    grass_dark: vec4<f32>,
    grass_dry: vec4<f32>,
    dirt: vec4<f32>,
    dirt_dark: vec4<f32>,
    pebble: vec4<f32>,
    sand: vec4<f32>,
    sand_wet: vec4<f32>,
    stone_a: vec4<f32>,
    stone_b: vec4<f32>,
    mortar: vec4<f32>,
    caustic: vec4<f32>,
    team_b: vec4<f32>,
    team_r: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> u: Terrain;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var masks: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var masks_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var noise: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var noise_sampler: sampler;

// Brouillard de guerre (`fog.wesl`).
@group(constants::MATERIAL_BIND_GROUP) @binding(110) var fog_vis: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111) var fog_sampler: sampler;

fn n(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(noise, noise_sampler, uv);
}

fn terr_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let sp = textureSample(masks, masks_sampler, in.uv);
    let wp = in.world_position.xz;
    let nL = n(wp * 0.011).r;
    let nL2 = n(wp * 0.0041 + 0.37).r;
    let nM = n(wp * 0.045).g;
    let nH = n(wp * 0.31).b;
    let nC = n(wp * 0.23).a;
    var glow = vec3(0.0);

    // herbe : deux tons + zones plus sèches, grain fin
    let tone = nL * 0.6 + nL2 * 0.4;
    var col = mix(u.grass_dark.rgb, u.grass.rgb, smoothstep(0.28, 0.72, tone));
    col = mix(col, u.grass_dry.rgb, smoothstep(0.55, 0.85, nM) * smoothstep(0.45, 0.8, nL2) * 0.5);
    col *= 0.84 + 0.3 * nH;

    // terre des voies : bords bruités, ornières sombres, cailloux
    let laneV = sp.r + (nM - 0.5) * 0.34 + (nH - 0.5) * 0.12;
    let laneM = smoothstep(0.43, 0.55, laneV);
    var dirt = mix(u.dirt_dark.rgb, u.dirt.rgb, smoothstep(0.2, 0.85, nL + (nH - 0.5) * 0.4));
    dirt *= 1.0 - (1.0 - smoothstep(0.5, 0.72, laneV)) * 0.22;
    let pebble = (1.0 - smoothstep(0.07, 0.15, nC)) * step(0.52, n(wp * 0.06 + 0.5).b);
    dirt = mix(dirt, u.pebble.rgb * (0.75 + 0.4 * nH), pebble * 0.85);
    col = mix(col, dirt, laneM);
    // liseré d'herbe foncée au bord du chemin (herbe piétinée)
    col *= 1.0 - (smoothstep(0.3, 0.43, laneV) - laneM) * 0.18;

    // sable des berges et lit de la rivière (humide sous l'eau), plus de sable dans les pentes
    let slope = 1.0 - clamp(normalize(in.world_normal).y, 0.0, 1.0);
    let sandM = smoothstep(0.38, 0.6, sp.g + (nM - 0.5) * 0.3 + slope * 1.5);
    let wet = 1.0 - smoothstep(-0.2, -0.06, in.world_position.y);
    let sand = mix(u.sand.rgb, u.sand_wet.rgb, wet) * (0.86 + 0.26 * nH);
    col = mix(col, sand, sandM);

    // dallage circulaire des bases (calculé : joints nets à tout zoom)
    if sp.b > 0.02 {
        let fb = wp - u.fount_b.xy;
        let fr = wp - u.fount_r.xy;
        let blue = dot(fb, fb) < dot(fr, fr);
        let d = select(fr, fb, blue);
        let r = length(d);
        let ringW = 1.9;
        let ri = floor(r / ringW);
        let fri = fract(r / ringW);
        let circ = 6.2831853 * max(ri + 0.5, 0.5) * ringW;
        let nst = max(5.0, floor(circ / 2.3));
        let seg = (atan2(d.y, d.x) / 6.2831853 + 0.5) * nst + ri * 0.5;
        let fs = fract(seg);
        let cell = terr_hash(vec2(ri, floor(seg)) + select(17.0, 0.0, blue));
        let edge = min(min(fri, 1.0 - fri) * ringW, min(fs, 1.0 - fs) * circ / nst);
        let joint = 1.0 - smoothstep(0.022, 0.055, edge + (nH - 0.5) * 0.03);
        var stone = mix(u.stone_a.rgb, u.stone_b.rgb, cell) * (0.8 + 0.32 * nH)
            * (0.92 + 0.16 * terr_hash(vec2(floor(seg), ri) + 3.1));
        // pierres légèrement bombées : bords plus sombres, mousse dans certains joints
        stone *= 0.8 + 0.2 * smoothstep(0.03, 0.35, edge);
        stone = mix(stone, mix(u.mortar.rgb, u.grass_dark.rgb * 0.8, step(0.6, nM) * 0.7), joint);
        // incrustations lumineuses aux couleurs de l'équipe
        let team = select(u.team_r.rgb, u.team_b.rgb, blue);
        let inlay = (1.0 - smoothstep(0.05, 0.13, abs(r - 6.5))) + (1.0 - smoothstep(0.06, 0.16, abs(r - 22.6)));
        stone = mix(stone, team * 0.6, inlay * 0.8);
        let pm = smoothstep(0.45, 0.6, sp.b + (nH - 0.5) * 0.1);
        glow = team * inlay * pm * 0.55;
        col = mix(col, stone, pm);
    }

    // occlusion : pied des murs, sol des brousses
    col *= 1.0 - sp.a * 0.6;

    // caustiques animées sur le lit de la rivière (échantillonnées hors du `if` du client web : WGSL
    // interdit les lectures de texture filtrées dans un flot de contrôle non uniforme ; `wet` vaut 0 hors
    // de l'eau, le résultat est le même)
    let t = globals.time;
    let c1 = n(wp * 0.33 + vec2(t * 0.045, t * 0.03)).a;
    let c2 = n(wp * 0.27 - vec2(t * 0.035, -t * 0.025)).a;
    let ca = smoothstep(0.42, 0.8, c1) * smoothstep(0.42, 0.8, c2);
    col += u.caustic.rgb * ca * wet * 0.45;

    pbr_input.material.base_color = vec4(pbr_input.material.base_color.rgb * col, pbr_input.material.base_color.a);
    pbr_input.material.emissive = vec4(pbr_input.material.emissive.rgb + glow, pbr_input.material.emissive.a);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    let fog = fog_visibility(fog_vis, fog_sampler, in.world_position.xyz);
    out.color = vec4(apply_fog(out.color.rgb, fog), out.color.a);
    return out;
}
