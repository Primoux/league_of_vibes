//! Serveur de League of Vibes.
//!
//! Portage de `packages/server/src/net`, `room.ts` et `index.ts`. Il parlera le protocole JSON v2
//! à l'identique du serveur Node, pour que le client web et `scripts/bots.ts` s'y connectent tels quels.
//!
//! Squelette : le serveur de référence reste `npm run dev:server` jusqu'à la bascule.

fn main() {
    eprintln!("lov-server : pas encore porté. Lance le serveur Node : npm run dev:server");
    std::process::exit(1);
}
