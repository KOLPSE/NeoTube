//! Integración con YouTube: transporte (`client`), parseo (`parse`),
//! operaciones por pantalla (`api`) y resolución de streams (`stream`).

pub mod api;
pub mod client;
pub mod parse;
pub mod stream;

/// El `User-Agent` de navegador que se usa para todo lo que no sea `ANDROID_VR`.
/// Fijar uno real importa: con el `UA` por defecto de reqwest YouTube sirve
/// variantes antiguas de la interfaz y algunas respuestas llegan vacías.
pub const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
