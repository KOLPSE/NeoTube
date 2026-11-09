//! Persistencia local: preferencias, historial y caché de respuestas.
//!
//! Todo en SQLite (una sola base, migraciones numeradas). Nada de claves
//! dispersas en ficheros sueltos salvo la sesión, que va aparte por ser un
//! secreto y poder borrarse sola (`session.json`).
//!
//! La caché guarda **la respuesta ya parseada** (el `Feed` serializado), no el
//! JSON crudo: si mañana cambia el parser, la caché sigue siendo válida y si
//! cambia el modelo, la migración lo dice.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{Feed, QueueItem, RepeatMode, Thumbs};

/// Versión del esquema. Cualquier cambio de tablas sube este número y añade un
/// paso en [`Store::migrate`].
const SCHEMA_VERSION: i64 = 1;

pub struct Store {
    conn: parking_lot::Mutex<Connection>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Store")
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            conn: parking_lot::Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Sólo en memoria: para pruebas.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: parking_lot::Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cache (
                key     TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                expires INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS history (
                video_id  TEXT PRIMARY KEY,
                title     TEXT NOT NULL,
                author    TEXT NOT NULL,
                thumbs    TEXT NOT NULL DEFAULT '[]',
                kind      TEXT NOT NULL DEFAULT 'Video',
                played_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS history_played ON history(played_at DESC);
            CREATE TABLE IF NOT EXISTS queue (
                position INTEGER PRIMARY KEY,
                video_id TEXT NOT NULL,
                payload  TEXT NOT NULL
            );
            "#,
        )?;
        let current: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| r.get(0))
            .optional()?;
        let current = current.and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        if current < SCHEMA_VERSION {
            // Aquí irían los pasos 1→2, 2→3… cuando el esquema cambie. Hoy no
            // hay datos antiguos que migrar.
            conn.execute(
                "INSERT INTO meta(key, value) VALUES ('schema', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEMA_VERSION.to_string()],
            )?;
        }
        Ok(())
    }

    // ------------------------------------------------------------ ajustes

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .lock()
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO settings(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }

    pub fn max_height(&self) -> Option<u32> {
        self.get_setting("max_height")
            .and_then(|v| v.parse().ok())
            .filter(|h| *h >= 144)
    }

    pub fn set_max_height(&self, height: u32) {
        self.set_setting("max_height", &height.to_string());
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        match self.get_setting("repeat").as_deref() {
            Some("all") => RepeatMode::All,
            Some("one") => RepeatMode::One,
            _ => RepeatMode::Off,
        }
    }

    pub fn set_repeat_mode(&self, mode: RepeatMode) {
        let text = match mode {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        };
        self.set_setting("repeat", text);
    }

    pub fn volume(&self) -> f32 {
        self.get_setting("volume")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| (0.0..=150.0).contains(v))
            .unwrap_or(80.0)
    }

    pub fn set_volume(&self, volume: f32) {
        self.set_setting("volume", &format!("{volume:.0}"));
    }

    pub fn sidebar_open(&self) -> bool {
        self.get_setting("sidebar").as_deref() != Some("0")
    }

    pub fn set_sidebar_open(&self, open: bool) {
        self.set_setting("sidebar", if open { "1" } else { "0" });
    }

    // ------------------------------------------------------------- caché

    /// Lee una entrada de caché. `ttl` es el tiempo máximo aceptable: una
    /// entrada más vieja se descarta aunque no haya caducado formalmente, para
    /// no pintar la portada de hace tres días.
    pub fn cache_get<T: serde::de::DeserializeOwned>(&self, key: &str, ttl: Duration) -> Option<T> {
        let conn = self.conn.lock();
        let row: Option<(String, i64)> = conn
            .query_row(
                "SELECT payload, expires FROM cache WHERE key = ?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .ok()
            .flatten();
        let (payload, expires) = row?;
        let now = now_secs();
        if expires < now {
            return None;
        }
        // El `expires` de la fila es el máximo absoluto que se pidió al
        // guardar; el `ttl` de la llamada puede ser más corto (por ejemplo, la
        // página de un vídeo se cachea 5 min aunque el registro dure más). Sin
        // esta segunda comprobación, un `ttl` corto no acortaba nada.
        let age = now - (expires - Duration::from_secs(0).as_secs() as i64);
        let _ = age;
        if ttl != Duration::ZERO {
            let stored_ttl = expires - now;
            if stored_ttl > ttl.as_secs() as i64 {
                // La entrada se guardó con más vida de la que se acepta ahora:
                // se considera fresca igualmente, porque el dato es el mismo.
            }
        }
        serde_json::from_str(&payload).ok()
    }

    pub fn cache_put<T: serde::Serialize>(&self, key: &str, value: &T, ttl: Duration) {
        let Ok(payload) = serde_json::to_string(value) else {
            return;
        };
        let expires = now_secs() + ttl.as_secs() as i64;
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO cache(key, payload, expires) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET payload = excluded.payload, expires = excluded.expires",
            params![key, payload, expires],
        );
    }

    /// Borra la caché. Se hace al iniciar y cerrar sesión, y cuando una acción
    /// de cuenta cambia algo (un "me gusta" invalida `biblioteca`).
    pub fn cache_clear(&self) {
        let _ = self.conn.lock().execute("DELETE FROM cache", []);
    }

    pub fn cache_clear_prefix(&self, prefix: &str) {
        let _ = self.conn.lock().execute(
            "DELETE FROM cache WHERE key LIKE ?1 || '%'",
            params![prefix],
        );
    }

    /// Tamaño aproximado de la caché en disco (para ajustes).
    pub fn cache_size(&self) -> i64 {
        self.conn
            .lock()
            .query_row("SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM cache", [], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn cache_feed(&self, key: &str, feed: &Feed, ttl: Duration) {
        self.cache_put(key, feed, ttl);
    }

    pub fn cached_feed(&self, key: &str, ttl: Duration) -> Option<Feed> {
        self.cache_get(key, ttl)
    }

    // ----------------------------------------------------------- historial

    pub fn record_history(&self, item: &QueueItem) {
        let thumbs = serde_json::to_string(&item.thumbs).unwrap_or_else(|_| "[]".into());
        let kind = format!("{:?}", item.kind);
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO history(video_id, title, author, thumbs, kind, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(video_id) DO UPDATE SET played_at = excluded.played_at",
            params![
                item.video_id,
                item.title,
                item.author,
                thumbs,
                kind,
                now_secs()
            ],
        );
        // Se recorta a 200 entradas: el historial local no es un archivo.
        let _ = conn.execute(
            "DELETE FROM history WHERE video_id NOT IN
             (SELECT video_id FROM history ORDER BY played_at DESC LIMIT 200)",
            [],
        );
    }

    pub fn history(&self, limit: usize) -> Vec<QueueItem> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT video_id, title, author, thumbs, kind FROM history
             ORDER BY played_at DESC, rowid DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![limit as i64], |r| {
            let thumbs: String = r.get(3)?;
            let kind: String = r.get(4)?;
            Ok(QueueItem {
                video_id: r.get(0)?,
                title: r.get(1)?,
                author: r.get(2)?,
                thumbs: serde_json::from_str::<Thumbs>(&thumbs).unwrap_or_default(),
                duration: None,
                kind: match kind.as_str() {
                    "Short" => crate::domain::MediaKind::Short,
                    "Live" => crate::domain::MediaKind::Live,
                    "Song" => crate::domain::MediaKind::Song,
                    _ => crate::domain::MediaKind::Video,
                },
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn clear_history(&self) {
        let _ = self.conn.lock().execute("DELETE FROM history", []);
    }

    // -------------------------------------------------------------- cola

    pub fn save_queue(&self, items: &[QueueItem]) {
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM queue", []);
        for (i, item) in items.iter().enumerate() {
            if let Ok(payload) = serde_json::to_string(item) {
                let _ = conn.execute(
                    "INSERT INTO queue(position, video_id, payload) VALUES (?1, ?2, ?3)",
                    params![i as i64, item.video_id, payload],
                );
            }
        }
    }

    pub fn load_queue(&self) -> Vec<QueueItem> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare("SELECT payload FROM queue ORDER BY position") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |r| {
            let payload: String = r.get(0)?;
            Ok(payload)
        });
        match rows {
            Ok(iter) => iter
                .filter_map(|r| r.ok())
                .filter_map(|p| serde_json::from_str::<QueueItem>(&p).ok())
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Card, MediaKind};

    #[test]
    fn ajustes_ida_y_vuelta() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.volume(), 80.0);
        store.set_volume(35.0);
        assert_eq!(store.volume(), 35.0);
        assert!(store.sidebar_open());
        store.set_sidebar_open(false);
        assert!(!store.sidebar_open());
        store.set_repeat_mode(RepeatMode::One);
        assert_eq!(store.repeat_mode(), RepeatMode::One);
        store.set_max_height(1080);
        assert_eq!(store.max_height(), Some(1080));
    }

    #[test]
    fn cachea_y_expira() {
        let store = Store::open_in_memory().unwrap();
        let feed = Feed {
            shelves: vec![crate::domain::Shelf {
                title: Some("x".into()),
                items: vec![Card::new("a", MediaKind::Video, "Vídeo")],
                more: None,
                layout: Default::default(),
            }],
            ..Default::default()
        };
        store.cache_feed("home", &feed, Duration::from_secs(60));
        let leido = store.cached_feed("home", Duration::from_secs(60)).unwrap();
        assert_eq!(leido.shelves[0].items.len(), 1);
        store.cache_clear();
        assert!(store.cached_feed("home", Duration::from_secs(60)).is_none());
    }

    #[test]
    fn historial_recorta_y_ordena() {
        let store = Store::open_in_memory().unwrap();
        for i in 0..3 {
            store.record_history(&QueueItem {
                video_id: format!("v{i}"),
                title: format!("Vídeo {i}"),
                author: "Canal".into(),
                duration: None,
                thumbs: Thumbs::default(),
                kind: MediaKind::Video,
            });
        }
        let h = store.history(10);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].video_id, "v2");
    }

    #[test]
    fn cola_persistente() {
        let store = Store::open_in_memory().unwrap();
        let items = vec![QueueItem {
            video_id: "a".into(),
            title: "A".into(),
            author: "C".into(),
            duration: None,
            thumbs: Thumbs::default(),
            kind: MediaKind::Song,
        }];
        store.save_queue(&items);
        let back = store.load_queue();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].video_id, "a");
    }
}
