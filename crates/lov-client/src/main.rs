//! Client natif de League of Vibes.
//!
//! Portage de `packages/client/src` (rendu, FX, interface, audio, entrées, réseau). Il se connecte
//! au même protocole JSON v2 que le client web, donc au serveur Node tant que `lov-server` n'est pas porté.
//!
//! Squelette : le client de référence reste `npm run dev:client` jusqu'à la bascule.

fn main() {
    eprintln!("lov-client : pas encore porté. Lance le client web : npm run dev:client");
    std::process::exit(1);
}
