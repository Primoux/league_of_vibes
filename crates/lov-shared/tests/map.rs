//! Géométrie de la carte : port de `packages/shared/test/map.test.ts` et `shared.test.ts` (partie
//! carte), plus comparaison exacte avec les tables calculées par le TypeScript
//! (`fixtures/ts_tables.json`, généré en important `map.ts` / `constants.ts` avec tsx).

mod common;

use common::{fixture, json_diff};
use lov_shared::constants::{MAP_SIZE, TIMERS, scaled};
use lov_shared::map::*;
use lov_shared::types::{Lane, Team, Vec2};
use serde_json::Value;

fn is_convex_ccw(p: &[Vec2]) -> bool {
    for i in 0..p.len() {
        let a = p[i];
        let b = p[(i + 1) % p.len()];
        let c = p[(i + 2) % p.len()];
        if (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) < 0.0 {
            return false;
        }
    }
    polygon_area(p) > 0.0
}

/// Distance d'un point à un segment.
fn dist_seg(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

fn j<T: serde::Serialize + ?Sized>(v: &T) -> Value {
    serde_json::to_value(v).expect("sérialisation")
}

fn mirror_poly(p: &[Vec2]) -> Vec<Vec2> {
    p.iter().map(|&q| mirror(q)).collect()
}

#[test]
fn murs_convexes_anti_horaires() {
    for w in WALLS.iter() {
        assert!(is_convex_ccw(w));
    }
}

#[test]
fn murs_et_brousses_symetriques_par_rotation_180() {
    assert_eq!(WALLS.len(), 28);
    assert_eq!(BRUSHES.len(), 32);
    let half = WALLS.len() / 2;
    for i in 0..half {
        assert_eq!(WALLS[i + half], mirror_poly(&WALLS[i]));
    }
    let bh = BRUSHES.len() / 2;
    for i in 0..bh {
        assert_eq!(BRUSHES[i + bh], mirror_poly(&BRUSHES[i]));
    }
}

#[test]
fn camps_symetriques_et_ids_uniques() {
    assert_eq!(CAMPS.len(), 14);
    let mut ids: Vec<&str> = CAMPS.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), CAMPS.len());
    for c in CAMPS.iter().filter(|c| c.side == CampSide::Blue) {
        let red = CAMPS
            .iter()
            .find(|r| r.id == c.id.replacen("blue_", "red_", 1))
            .expect("camp rouge");
        assert_eq!(red.pos, mirror(c.pos));
        assert_eq!(
            red.smalls.as_ref().map(Vec::len),
            c.smalls.as_ref().map(Vec::len)
        );
    }
    assert!(
        CAMPS
            .iter()
            .any(|c| c.id == "blue_raptors" && c.smalls.as_ref().is_some_and(|s| s.len() == 5))
    );
}

#[test]
fn tours_et_inhibiteurs_symetriques() {
    assert_eq!(TOWERS.len(), 22);
    assert_eq!(INHIBITORS.len(), 6);
    let blue: Vec<_> = TOWERS.iter().filter(|t| t.team == Team::Blue).collect();
    let red: Vec<_> = TOWERS.iter().filter(|t| t.team == Team::Red).collect();
    assert_eq!(blue.len(), 11);
    for (b, r) in blue.iter().zip(&red) {
        assert_eq!(r.pos, mirror(b.pos));
        assert_eq!(r.tier, b.tier);
    }
    assert_eq!(NEXUS.red, mirror(NEXUS.blue));
    assert_eq!(FOUNTAINS.red, mirror(FOUNTAINS.blue));
}

#[test]
fn structures_hors_des_murs_et_dans_la_carte() {
    let pts = TOWERS
        .iter()
        .map(|t| t.pos)
        .chain(INHIBITORS.iter().map(|i| i.pos))
        .chain(CAMPS.iter().map(|c| c.pos));
    for p in pts {
        assert!(!is_in_wall(p), "{p:?} dans un mur");
        assert!(p.x > 0.0 && p.x < MAP_SIZE);
    }
}

#[test]
fn aucun_mur_sur_les_voies() {
    for path in [&LANE_PATHS.top, &LANE_PATHS.mid, &LANE_PATHS.bot] {
        for seg in path.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            for w in WALLS.iter() {
                for &p in w {
                    assert!(dist_seg(p, a, b) > LANE_HALF_WIDTH);
                }
            }
            for k in 0..=40 {
                let t = f64::from(k) / 40.0;
                let p = Vec2 {
                    x: a.x + (b.x - a.x) * t,
                    y: a.y + (b.y - a.y) * t,
                };
                assert!(!is_in_wall(p));
            }
        }
    }
}

#[test]
fn camps_fosses_et_structures_degages_rayon_150() {
    let mut pts: Vec<Vec2> = CAMPS.iter().map(|c| c.pos).collect();
    pts.extend(TOWERS.iter().map(|t| t.pos));
    pts.extend(INHIBITORS.iter().map(|i| i.pos));
    pts.extend([NEXUS.blue, NEXUS.red, PITS.dragon.pos, PITS.leviathan.pos]);
    for p in pts {
        for a in 0..8 {
            let a = f64::from(a);
            assert!(!is_in_wall(Vec2 {
                x: p.x + a.cos() * 150.0,
                y: p.y + a.sin() * 150.0
            }));
        }
    }
}

#[test]
fn tout_est_accessible_depuis_la_fontaine_bleue() {
    // flood fill, grille 50 u, unité de rayon 40
    const CELL: f64 = 50.0;
    let n = (MAP_SIZE / CELL) as usize;
    let mut free = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let c = Vec2 {
                x: i as f64 * CELL + 25.0,
                y: j as f64 * CELL + 25.0,
            };
            let mut ok = !is_in_wall(c);
            let mut a = 0;
            while ok && a < 6 {
                let af = f64::from(a);
                ok = !is_in_wall(Vec2 {
                    x: c.x + af.cos() * 40.0,
                    y: c.y + af.sin() * 40.0,
                });
                a += 1;
            }
            free[j * n + i] = ok;
        }
    }
    let idx = |p: Vec2| (p.y / CELL).floor() as usize * n + (p.x / CELL).floor() as usize;
    let mut seen = vec![false; n * n];
    let mut stack = vec![idx(FOUNTAINS.blue)];
    seen[stack[0]] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = ((k % n) as i64, (k / n) as i64);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (ni, nj) = (i + di, j + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let nk = nj as usize * n + ni as usize;
            if !seen[nk] && free[nk] {
                seen[nk] = true;
                stack.push(nk);
            }
        }
    }
    for c in CAMPS.iter() {
        assert!(seen[idx(c.pos)], "{}", c.id);
    }
    assert!(seen[idx(FOUNTAINS.red)]);
    assert!(seen[idx(PITS.dragon.pos)]);
    let unreachable = (0..n * n).filter(|&k| free[k] && !seen[k]).count();
    assert_eq!(unreachable, 0);
}

#[test]
fn point_in_polygon_et_voies() {
    let sq = vec![
        Vec2 { x: 0.0, y: 0.0 },
        Vec2 { x: 10.0, y: 0.0 },
        Vec2 { x: 10.0, y: 10.0 },
        Vec2 { x: 0.0, y: 10.0 },
    ];
    assert!(point_in_polygon(Vec2 { x: 5.0, y: 5.0 }, &sq));
    assert!(!point_in_polygon(Vec2 { x: 15.0, y: 5.0 }, &sq));
    assert!(!point_in_polygon(Vec2 { x: 5.0, y: 5.0 }, &[]));
    assert!(is_in_wall(Vec2 {
        x: 1800.0,
        y: 4000.0
    }));
    assert!(is_in_brush(Vec2 {
        x: 500.0,
        y: 9000.0
    }));
    assert_eq!(lane_path_for(Lane::Mid, Team::Blue), LANE_PATHS.mid);
    assert_eq!(
        lane_path_for(Lane::Mid, Team::Red),
        vec![
            Vec2 {
                x: 10000.0,
                y: 10000.0
            },
            Vec2 {
                x: 2000.0,
                y: 2000.0
            }
        ]
    );
    assert_eq!(BASE_RADIUS, 2600.0);
    assert_eq!(RIVER_HALF_WIDTH, 520.0);
}

/// Les tables Rust sont ÉGALES à celles calculées par le TypeScript (sérialisées en JSON).
#[test]
fn tables_identiques_au_typescript() {
    let ts: Value = serde_json::from_str(&fixture("ts_tables.json")).expect("json");
    let check = |name: &str, rust: Value| {
        if let Some(d) = json_diff(&ts[name], &rust, name) {
            panic!("{d}");
        }
    };
    check("FOUNTAINS", j(&FOUNTAINS));
    check("NEXUS", j(&NEXUS));
    check("TOWERS", j(&*TOWERS));
    check("INHIBITORS", j(&*INHIBITORS));
    check("LANE_PATHS", j(&*LANE_PATHS));
    check("CAMPS", j(&*CAMPS));
    check("PITS", j(&PITS));
    check("WALLS", j(&*WALLS));
    check("BRUSHES", j(&*BRUSHES));
    check("lanePathRedMid", j(&lane_path_for(Lane::Mid, Team::Red)));
    check("lanePathRedTop", j(&lane_path_for(Lane::Top, Team::Red)));
    check("RIVER_HALF_WIDTH", j(&RIVER_HALF_WIDTH));
    check("LANE_HALF_WIDTH", j(&LANE_HALF_WIDTH));
    check("BASE_RADIUS", j(&BASE_RADIUS));
    check("TEAMS", j(&lov_shared::types::TEAMS));
    // scaled()
    for pair in ts["scaledSamples"].as_array().expect("tableau") {
        let (s, want) = (
            pair[0].as_f64().expect("f64"),
            pair[1].as_f64().expect("f64"),
        );
        assert_eq!(scaled(s), want);
    }
    // TIMERS : mêmes valeurs, clés camelCase du TS <-> snake_case Rust (conversion via serde_json)
    let t = &ts["TIMERS"];
    let get = |k: &str| t[k].as_f64().unwrap_or_else(|| panic!("TIMERS.{k}"));
    assert_eq!(TIMERS.minion_first_wave, get("minionFirstWave"));
    assert_eq!(TIMERS.minion_wave_interval, get("minionWaveInterval"));
    assert_eq!(
        f64::from(TIMERS.cannon_every_n_waves),
        get("cannonEveryNWaves")
    );
    assert_eq!(
        f64::from(TIMERS.cannon_every_n_waves_late),
        get("cannonEveryNWavesLate")
    );
    assert_eq!(TIMERS.cannon_late_at, get("cannonLateAt"));
    assert_eq!(TIMERS.tower_plates_until, get("towerPlatesUntil"));
    assert_eq!(TIMERS.inhibitor_respawn, get("inhibitorRespawn"));
    assert_eq!(TIMERS.jungle_first_spawn, get("jungleFirstSpawn"));
    assert_eq!(TIMERS.jungle_buff_respawn, get("jungleBuffRespawn"));
    assert_eq!(TIMERS.jungle_small_respawn, get("jungleSmallRespawn"));
    assert_eq!(TIMERS.scuttle_first_spawn, get("scuttleFirstSpawn"));
    assert_eq!(TIMERS.scuttle_respawn, get("scuttleRespawn"));
    assert_eq!(TIMERS.dragon_first_spawn, get("dragonFirstSpawn"));
    assert_eq!(TIMERS.dragon_respawn, get("dragonRespawn"));
    assert_eq!(TIMERS.elder_respawn, get("elderRespawn"));
    assert_eq!(TIMERS.grubs_spawn, get("grubsSpawn"));
    assert_eq!(TIMERS.herald_spawn, get("heraldSpawn"));
    assert_eq!(TIMERS.herald_despawn, get("heraldDespawn"));
    assert_eq!(TIMERS.leviathan_spawn, get("leviathanSpawn"));
    assert_eq!(TIMERS.leviathan_respawn, get("leviathanRespawn"));
    assert_eq!(TIMERS.leviathan_buff_duration, get("leviathanBuffDuration"));
    assert_eq!(TIMERS.elder_buff_duration, get("elderBuffDuration"));
    assert_eq!(
        t.as_object().map(|o| o.len()),
        Some(22),
        "nouveau timer côté TS non porté ?"
    );
}
