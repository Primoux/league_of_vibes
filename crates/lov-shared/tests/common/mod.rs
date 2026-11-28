//! Aides communes aux tests d'intégration.

// Chaque fichier de test n'utilise qu'une partie de ces aides : sans cet `allow`, le compilateur
// signalerait du code mort dans chacun d'eux (limite connue des modules de tests partagés).
#![allow(dead_code)]

use serde_json::Value;

/// Compare deux JSON structurellement : ordre des clés ignoré, nombres comparés comme des f64
/// (`500` et `500.0` sont égaux). Renvoie le chemin de la première différence.
pub fn json_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64(), y.as_f64());
            (x != y).then(|| format!("{path}: {x:?} != {y:?}"))
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: longueur {} != {}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (p, q))| json_diff(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            if let Some(k) = x.keys().find(|k| !y.contains_key(*k)) {
                return Some(format!(
                    "{path}.{k}: présent à l'origine, absent après aller-retour"
                ));
            }
            if let Some(k) = y.keys().find(|k| !x.contains_key(*k)) {
                return Some(format!(
                    "{path}.{k}: absent à l'origine, présent après aller-retour"
                ));
            }
            x.iter()
                .find_map(|(k, p)| json_diff(p, &y[k], &format!("{path}.{k}")))
        }
        _ => (a != b).then(|| format!("{path}: {a} != {b}")),
    }
}

pub fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub fn lines(name: &str) -> Vec<String> {
    fixture(name)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// Décode, ré-encode et compare ; renvoie le message décodé. Panique avec le chemin du champ fautif.
pub fn round_trip<T>(line: &str) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let original: Value = serde_json::from_str(line).expect("fixture JSON valide");
    let typed: T =
        serde_json::from_str(line).unwrap_or_else(|e| panic!("décodage impossible: {e}\n{line}"));
    let again = serde_json::to_value(&typed).expect("encodage");
    if let Some(d) = json_diff(&original, &again, "$") {
        panic!("aller-retour différent en {d}\n{line}");
    }
    // et via le texte : le texte ré-encodé se redécode identique
    let text = serde_json::to_string(&typed).expect("encodage texte");
    let reparsed: Value = serde_json::from_str(&text).expect("texte valide");
    assert_eq!(json_diff(&original, &reparsed, "$"), None);
    typed
}

/// Mode régénération des instantanés (`LOV_BLESS=1`) : voir [`check_snapshot`].
pub fn bless() -> bool {
    std::env::var_os("LOV_BLESS").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Compare `actual` à l'instantané `fixtures/<name>`, ou le réécrit avec `text` en mode `LOV_BLESS=1`.
///
/// Les fixtures sont la référence des données depuis la bascule (#276) : une modification
/// **volontaire** de données se fait en régénérant le fichier (`scripts/bless-fixtures.sh`) et le diff
/// du fixture se relit dans la PR. Une modification involontaire fait échouer le test.
pub fn check_snapshot(name: &str, text: &str, actual: &Value) {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    if bless() {
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path}: {e}"));
        return;
    }
    let expected: Value = serde_json::from_str(&fixture(name)).expect("fixture JSON valide");
    if let Some(d) = json_diff(&expected, actual, name) {
        panic!(
            "{d}\n\nLa donnée ne correspond plus à l'instantané `tests/fixtures/{name}`.\n\
             Changement voulu ? Régénère avec `scripts/bless-fixtures.sh` et relis le diff du \
             fixture dans ta PR (AGENTS.md §5)."
        );
    }
}

/// JSON indenté, terminé par un saut de ligne : un diff de fixture se relit ligne à ligne.
pub fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("encodage");
    s.push('\n');
    s
}
