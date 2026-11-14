//! Tests de la grille de navigation : port de `packages/server/test/nav.test.ts` (bloc `NavGrid`),
//! plus comparaison avec les chemins produits par l'implémentation TypeScript.
//!
//! Le bloc `movement` du test TS dépend du monde, des commandes et des dashes (pas encore portés) :
//! il arrivera avec le portage de `systems/movement`.

use lov_shared::map::{Polygon, WALLS};
use lov_shared::math::dist;
use lov_shared::types::Vec2;
use lov_sim::nav::{NavGrid, nav_grid};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Polygon {
    vec![v(x, y), v(x + w, y), v(x + w, y + h), v(x, y + h)]
}

fn grid6000() -> NavGrid {
    NavGrid::new(&[rect(2000.0, 1000.0, 400.0, 3000.0)], 6000.0, 50.0, 35.0)
}

#[test]
fn ligne_droite_quand_rien_ne_bloque() {
    let grid = grid6000();
    let mut s = grid.new_search();
    assert_eq!(
        grid.find_path(&mut s, v(500.0, 500.0), v(1500.0, 1500.0)),
        vec![v(1500.0, 1500.0)]
    );
}

#[test]
fn contourne_un_mur_et_le_chemin_lisse_reste_accessible() {
    let grid = grid6000();
    let mut s = grid.new_search();
    let from = v(1500.0, 2500.0);
    let to = v(3000.0, 2500.0);
    let path = grid.find_path(&mut s, from, to);
    assert!(path.len() > 1);
    assert!(path.len() < 6);
    assert_eq!(path[path.len() - 1], to);
    let mut prev = from;
    let mut len = 0.0;
    for p in &path {
        assert!(grid.line_walkable(prev, *p));
        len += dist(prev, *p);
        prev = *p;
    }
    // Contourne par le haut (y=4000) ou le bas (y=1000) : ~ 2 × 1500 + 1500
    assert!(len < 5200.0);
}

#[test]
fn clic_dans_un_mur_point_accessible_le_plus_proche() {
    let grid = grid6000();
    let mut s = grid.new_search();
    let p = grid.nearest_walkable(v(2050.0, 2500.0));
    assert!(grid.walkable(p.x, p.y));
    assert!(p.x < 2000.0);
    let path = grid.find_path(&mut s, v(1500.0, 2500.0), v(2050.0, 2500.0));
    assert!(path[path.len() - 1].x < 2000.0);
}

#[test]
fn point_totalement_enferme_chemin_vide() {
    let boxed = NavGrid::new(
        &[
            rect(900.0, 900.0, 200.0, 1200.0),
            rect(2900.0, 900.0, 200.0, 1200.0),
            rect(900.0, 900.0, 2200.0, 200.0),
            rect(900.0, 1900.0, 2200.0, 200.0),
        ],
        4000.0,
        50.0,
        35.0,
    );
    let mut s = boxed.new_search();
    assert_eq!(
        boxed.find_path(&mut s, v(2000.0, 1500.0), v(3500.0, 3500.0)),
        Vec::<Vec2>::new()
    );
}

#[test]
fn grille_reelle_astar_rapide_sur_toute_la_carte() {
    let g = nav_grid();
    for w in WALLS.iter() {
        // centroïde (les murs sont convexes) : indépendant de la géométrie exacte de map.rs
        let n = w.len() as f64;
        let cx = w.iter().map(|p| p.x).sum::<f64>() / n;
        let cy = w.iter().map(|p| p.y).sum::<f64>() / n;
        assert!(!g.walkable(cx, cy));
    }
    let mut s = g.new_search();
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    for _ in 0..50 {
        g.find_path_into(&mut s, v(600.0, 600.0), v(11400.0, 11400.0), &mut out);
    }
    let per = t0.elapsed().as_secs_f64() * 1000.0 / 50.0;
    assert!(per < 15.0, "{per} ms par chemin");
}

#[test]
fn last_walkable_on_segment_s_arrete_au_mur() {
    let grid = grid6000();
    let end = grid.last_walkable_on_segment(v(1500.0, 2500.0), v(3000.0, 2500.0));
    assert!(end.x < 2000.0 && grid.walkable(end.x, end.y));
    let free = grid.last_walkable_on_segment(v(500.0, 500.0), v(1500.0, 1500.0));
    assert_eq!(free, v(1500.0, 1500.0));
}

// --- Comparaison avec le TypeScript -------------------------------------------------------------

/// Mini lecteur JSON (la crate n'a pas de dépendance serde) : nombres, tableaux, objets.
enum J {
    Num(f64),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(kv) => &kv.iter().find(|(k, _)| k == key).expect("clé").1,
            _ => panic!("pas un objet"),
        }
    }
    fn num(&self) -> f64 {
        match self {
            J::Num(n) => *n,
            _ => panic!("pas un nombre"),
        }
    }
    fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => panic!("pas un tableau"),
        }
    }
    fn vec2(&self) -> Vec2 {
        v(self.get("x").num(), self.get("y").num())
    }
}

fn parse(s: &[u8], i: &mut usize) -> J {
    let skip = |i: &mut usize| {
        while s[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    skip(i);
    match s[*i] {
        b'[' => {
            *i += 1;
            let mut items = Vec::new();
            loop {
                skip(i);
                if s[*i] == b']' {
                    *i += 1;
                    return J::Arr(items);
                }
                if s[*i] == b',' {
                    *i += 1;
                    continue;
                }
                items.push(parse(s, i));
            }
        }
        b'{' => {
            *i += 1;
            let mut kv = Vec::new();
            loop {
                skip(i);
                match s[*i] {
                    b'}' => {
                        *i += 1;
                        return J::Obj(kv);
                    }
                    b',' => *i += 1,
                    _ => {
                        assert_eq!(s[*i], b'"');
                        let start = *i + 1;
                        *i = start;
                        while s[*i] != b'"' {
                            *i += 1;
                        }
                        let key = String::from_utf8_lossy(&s[start..*i]).into_owned();
                        *i += 1;
                        skip(i);
                        assert_eq!(s[*i], b':');
                        *i += 1;
                        kv.push((key, parse(s, i)));
                    }
                }
            }
        }
        _ => {
            let start = *i;
            while *i < s.len() && !b",]} \n\r\t".contains(&s[*i]) {
                *i += 1;
            }
            let txt = String::from_utf8_lossy(&s[start..*i]);
            J::Num(txt.parse().expect("nombre"))
        }
    }
}

#[test]
fn memes_chemins_que_l_implementation_typescript() {
    // Généré avec la vraie grille TS (`navGrid().findPath`), voir tests/fixtures/nav_paths.json.
    let raw = include_str!("fixtures/nav_paths.json");
    let mut pos = 0;
    let doc = parse(raw.as_bytes(), &mut pos);
    let cases = doc.arr();
    assert!(cases.len() >= 40);

    let g = nav_grid();
    // Même nombre de cellules bloquées que la grille TS.
    assert_eq!(
        g.blocked.iter().map(|&b| usize::from(b)).sum::<usize>(),
        7668
    );

    let mut s = g.new_search();
    for (n, case) in cases.iter().enumerate() {
        let from = case.get("from").vec2();
        let to = case.get("to").vec2();
        let want: Vec<Vec2> = case.get("path").arr().iter().map(J::vec2).collect();
        let got = g.find_path(&mut s, from, to);
        assert_eq!(got.len(), want.len(), "cas {n} : {from:?} -> {to:?}");
        for (k, (a, b)) in got.iter().zip(&want).enumerate() {
            assert!(
                (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6,
                "cas {n}, point {k} : {a:?} != {b:?}"
            );
        }
    }
}
