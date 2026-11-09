//! NeoTube 2.0 — cliente nativo de escritorio de YouTube y YouTube Music.
//!
//! Capas, de abajo arriba:
//!
//! | Módulo | Responsabilidad |
//! |---|---|
//! | [`domain`] | Modelos de la aplicación; no sabe nada del JSON de YouTube |
//! | [`session`] | Cookies, firma `SAPISIDHASH` y sesión persistida |
//! | [`store`] | SQLite: preferencias, historial y caché de respuestas |
//! | [`yt`] | Transporte Innertube, parseo, operaciones y resolución de streams |
//! | [`playback`] | Reproductor (VLC) y cola |
//! | [`ui`] | Interfaz, navegación y estados |
//!
//! Regla que se respeta en todo el árbol: la interfaz **no** ve JSON de
//! YouTube ni conoce los nombres de sus renderers.

pub mod domain;
pub mod playback;
pub mod session;
pub mod store;
pub mod ui;
pub mod yt;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;

/// Rutas de datos del usuario (`%APPDATA%\NeoTube2` en Windows).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let root = directories::ProjectDirs::from("xyz", "NeoTube", "NeoTube2")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            ;
        Self { root }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }

    pub fn db(&self) -> PathBuf {
        self.root.join("neotube.db")
    }

    pub fn thumb_cache(&self) -> PathBuf {
        self.root.join("thumbs")
    }
}

/// Todo lo que compone la aplicación, para no pasarlo suelto por parámetros.
pub struct AppContext {
    pub paths: Paths,
    pub session: Arc<session::SessionStore>,
    pub store: Arc<store::Store>,
    pub api: yt::client::Innertube,
}

impl AppContext {
    pub fn bootstrap() -> anyhow::Result<Self> {
        let paths = Paths::resolve();
        paths.ensure().context("crear el directorio de datos")?;
        let session = session::SessionStore::new();
        session.load(&paths.root);
        let store = Arc::new(store::Store::open(&paths.db()).context("abrir la base de datos")?);
        let api = yt::client::Innertube::new(session.clone()).context("crear el cliente HTTP")?;
        std::fs::create_dir_all(paths.thumb_cache()).ok();
        Ok(Self {
            paths,
            session,
            store,
            api,
        })
    }
}
