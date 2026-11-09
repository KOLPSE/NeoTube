//! Interfaz de NeoTube 2.0.
//!
//! Composición, de fuera adentro:
//!
//! | Módulo | Contenido |
//! |---|---|
//! | [`app`] | Estado de la aplicación, navegación, tareas y bucle de fotograma |
//! | [`screens`] | Una pantalla por destino (Inicio, Búsqueda, Vídeo, Shorts, Música…) |
//! | [`widgets`] | Piezas reutilizables (tarjeta, fila, chip, icono, esqueleto) |
//! | [`thumbs`] | Caché de miniaturas acotada y asíncrona |
//! | [`theme`] | Paleta y medidas tomadas de youtube.com |
//!
//! Regla: aquí no entra JSON de YouTube ni nombres de renderers. Todo llega ya
//! convertido a los modelos de [`crate::domain`].

pub mod actions;
pub mod app;
pub mod icons;
pub mod login;
pub mod screens;
pub mod shell;
pub mod tasks;
pub mod theme;
pub mod thumbs;
pub mod widgets;

/// Traza de diagnóstico mientras se pule la interfaz contra las capturas.
/// Escribe a stderr para que se vea en `journalctl`/consola sin configurar
/// `env_logger`; se retira cuando la verificación visual termine.
pub fn dbg_log(message: String) {
    eprintln!("[neotube] {message}");
}
