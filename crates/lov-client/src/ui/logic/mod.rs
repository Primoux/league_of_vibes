//! Logique pure de l'interface (testée sans rendu) : portage de `packages/client/src/ui/logic/`.

// Le fil des éliminations, les règles du HUD et la boutique servent au HUD (#113), branché pièce par
// pièce dans les PR suivantes : ce qui n'est pas encore affiché n'est lu que par les tests.
#[allow(dead_code)]
pub mod feed;
#[allow(dead_code)]
pub mod format;
#[allow(dead_code)]
pub mod game;
#[allow(dead_code)]
pub mod shop;
