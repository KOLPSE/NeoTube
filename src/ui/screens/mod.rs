//! Pantallas, una por destino de navegación.
//!
//! Todas siguen el mismo contrato: reciben el estado que necesitan, pintan, y
//! devuelven una [`crate::ui::app::Intent`] con lo que el usuario ha pedido.
//! No hacen peticiones de red ni tocan el reproductor: eso es de [`crate::ui::app`].

pub mod channel;
pub mod home;
pub mod library;
pub mod music;
pub mod search;
pub mod settings;
pub mod shorts;
pub mod subscriptions;
pub mod watch;

pub use channel::channel;
pub use home::home;
pub use library::{history, library};
pub use music::{album, artist, music_home, playlist};
pub use search::search;
pub use settings::settings;
pub use shorts::shorts;
pub use subscriptions::subscriptions;
pub use watch::watch;
