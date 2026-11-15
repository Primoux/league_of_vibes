//! Serveur de League of Vibes : portage de `packages/server/src/{index.ts,net/server.ts,net/validate.ts}`.
//!
//! Les salles (`room.ts`) ne sont pas portées : voir [`room`].

pub mod room;
pub mod server;
pub mod validate;
