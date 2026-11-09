//! Despachador de tareas: convierte una intención de la interfaz en una
//! petición asíncrona y devuelve el resultado como mensaje.
//!
//! Aquí está **toda** la política de caché, para que las pantallas solo pidan
//! «dame el inicio» y no sepan si viene de SQLite o de la red.

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::domain::{Account, Card, Feed};
use crate::store::Store;
use crate::ui::actions::{Payload, TaskDone, TaskKind};
use crate::yt::client::{ApiError, Innertube};

/// Tiempos de vida de la caché, en el orden en que cambian las cosas:
/// la portada cambia mucho, la biblioteca poco, los álbumes casi nada.
pub mod ttl {
    use super::Duration;
    pub const HOME: Duration = Duration::from_secs(10 * 60);
    pub const SEARCH: Duration = Duration::from_secs(15 * 60);
    pub const MUSIC_HOME: Duration = Duration::from_secs(30 * 60);
    pub const WATCH: Duration = Duration::from_secs(5 * 60);
    pub const BROWSE: Duration = Duration::from_secs(60 * 60);
    pub const LIBRARY: Duration = Duration::from_secs(10 * 60);
}

/// Lanza una tarea en el runtime de Tokio. El resultado se entrega por
/// `tx` como [`TaskDone`].
pub fn spawn(
    runtime: &tokio::runtime::Handle,
    api: Innertube,
    store: Arc<Store>,
    tx: Sender<TaskDone>,
    id: u64,
    kind: TaskKind,
) {
    runtime.spawn(async move {
        let kind_for_log = kind.clone();
        let result = run(api, store, &kind).await.map_err(|e| e.user_message());
        if let Err(e) = &result {
            log::warn!("tarea {kind_for_log:?} falló: {e}");
        }
        let _ = tx.send(TaskDone { id, kind, result });
    });
}

async fn run(
    api: Innertube,
    store: Arc<Store>,
    kind: &TaskKind,
) -> Result<Payload, ApiError> {
    match kind {
        TaskKind::Home => {
            let key = "home";
            if let Some(feed) = store.cached_feed(key, ttl::HOME) {
                return Ok(Payload::Feed {
                    key: key.into(),
                    feed,
                    append: false,
                    music: false,
                });
            }
            let feed = api.home().await?;
            if !feed.shelves.is_empty() {
                store.cache_feed(key, &feed, ttl::HOME);
            }
            Ok(Payload::Feed {
                key: key.into(),
                feed,
                append: false,
                music: false,
            })
        }
        TaskKind::Search {
            query,
            music,
            params,
        } => {
            let key = format!(
                "search:{}:{}:{}",
                if *music { "music" } else { "yt" },
                params.as_deref().unwrap_or(""),
                query.to_lowercase()
            );
            if let Some(feed) = store.cached_feed(&key, ttl::SEARCH) {
                return Ok(Payload::Feed {
                    key,
                    feed,
                    append: false,
                    music: *music,
                });
            }
            let feed = if *music {
                api.music_search(query, params.as_deref()).await?
            } else {
                api.search(query, params.as_deref()).await?
            };
            if !feed.shelves.is_empty() {
                store.cache_feed(&key, &feed, ttl::SEARCH);
            }
            Ok(Payload::Feed {
                key,
                feed,
                append: false,
                music: *music,
            })
        }
        TaskKind::Watch { video_id } => {
            let key = format!("watch:{video_id}");
            // La página de vídeo se cachea poco: los "me gusta" y los
            // contadores cambian, y el usuario espera ver lo que acaba de hacer.
            if let Some(info) = store.cache_get::<crate::domain::WatchInfo>(&key, ttl::WATCH) {
                return Ok(Payload::Watch(Box::new(info)));
            }
            let info = api.watch(video_id).await?;
            store.cache_put(&key, &info, ttl::WATCH);
            Ok(Payload::Watch(Box::new(info)))
        }
        TaskKind::Resolve {
            video_id,
            max_height,
        } => {
            let stream = api.resolve_stream(video_id, *max_height).await?;
            Ok(Payload::Stream(Box::new(stream)))
        }
        TaskKind::Shorts { seed } => {
            let cards = api.shorts_feed(seed).await?;
            Ok(Payload::Cards(cards))
        }
        TaskKind::MusicHome => {
            let key = "music:home";
            if let Some(feed) = store.cached_feed(key, ttl::MUSIC_HOME) {
                return Ok(Payload::Feed {
                    key: key.into(),
                    feed,
                    append: false,
                    music: true,
                });
            }
            let feed = api.music_home().await?;
            if !feed.shelves.is_empty() {
                store.cache_feed(key, &feed, ttl::MUSIC_HOME);
            }
            Ok(Payload::Feed {
                key: key.into(),
                feed,
                append: false,
                music: true,
            })
        }
        TaskKind::Browse { browse_id, music } => {
            let key = format!("browse:{browse_id}");
            if let Some(feed) = store.cached_feed(&key, ttl::BROWSE) {
                return Ok(Payload::Feed {
                    key,
                    feed,
                    append: false,
                    music: *music,
                });
            }
            // Los canales y artistas responden por el host de música; si esa
            // respuesta viniera vacía, se reintenta por el host web. Antes se
            // llamaba dos veces al mismo endpoint, así que el reintento no
            // reintentaba nada.
            let feed = if *music {
                api.music_browse(browse_id).await?
            } else {
                let feed = api.music_browse(browse_id).await?;
                if feed.shelves.is_empty() {
                    api.browse_web(browse_id).await?
                } else {
                    feed
                }
            };
            if !feed.shelves.is_empty() {
                store.cache_feed(&key, &feed, ttl::BROWSE);
            }
            Ok(Payload::Feed {
                key,
                feed,
                append: false,
                music: *music,
            })
        }
        TaskKind::Library => {
            if !api.is_logged_in() {
                return Err(ApiError::NeedsLogin);
            }
            let key = "library";
            if let Some(feed) = store.cached_feed(key, ttl::LIBRARY) {
                return Ok(Payload::Feed {
                    key: key.into(),
                    feed,
                    append: false,
                    music: true,
                });
            }
            let feed = api.library().await?;
            store.cache_feed(key, &feed, ttl::LIBRARY);
            Ok(Payload::Feed {
                key: key.into(),
                feed,
                append: false,
                music: true,
            })
        }
        TaskKind::History => {
            if !api.is_logged_in() {
                return Err(ApiError::NeedsLogin);
            }
            let feed = api.music_history().await?;
            Ok(Payload::Feed {
                key: "history".into(),
                feed,
                append: false,
                music: true,
            })
        }
        TaskKind::More {
            token,
            music,
            key,
        } => {
            let feed = api.continue_feed(token, *music).await?;
            Ok(Payload::Feed {
                key: key.clone(),
                feed,
                append: true,
                music: *music,
            })
        }
        TaskKind::Account => {
            if !api.is_logged_in() {
                return Ok(Payload::Account(None));
            }
            let account = fetch_account(&api).await;
            Ok(Payload::Account(account))
        }
        TaskKind::Like { video_id, on } => {
            api.set_like(video_id, *on).await?;
            // Un "me gusta" cambia la biblioteca y la portada de música: se
            // invalida lo que dependa de ello en vez de arriesgar datos viejos.
            store.cache_clear_prefix("library");
            store.cache_clear_prefix("browse:FEmusic_liked");
            store.cache_clear_prefix("browse:VLLM");
            store.cache_clear_prefix("music:home");
            Ok(Payload::Done {
                what: "like",
                ok: true,
            })
        }
        TaskKind::Subscribe { channel_id, on } => {
            api.set_subscribed(channel_id, *on).await?;
            store.cache_clear_prefix("home");
            Ok(Payload::Done {
                what: "subscribe",
                ok: true,
            })
        }
        TaskKind::Login => Ok(Payload::Account(api.session().account())),
    }
}

/// Saca el perfil de la cuenta del propio YouTube (nombre, avatar, canal).
///
/// Se hace con la portada autenticada: el bloque de la cuenta viene en la
/// respuesta de `browse`. Si no aparece, se devuelve `None` y la interfaz
/// muestra «Sesión iniciada» sin nombre, en vez de inventarlo.
async fn fetch_account(api: &Innertube) -> Option<Account> {
    let json = api
        .post_web("browse", serde_json::json!({ "browseId": "FEwhat_to_watch" }))
        .await
        .ok()?;
    // Nombre y avatar aparecen en el botón de cuenta / identidad activa.
    let active = crate::yt::parse::find_key(&json, "activeAccountHeaderRenderer")
        .or_else(|| crate::yt::parse::find_key(&json, "accountItem"));
    let mut account = Account::default();
    if let Some(node) = active {
        account.name = crate::yt::parse::text_at(node, &["accountName"]);
        if account.name.is_empty() {
            account.name = crate::yt::parse::text_at(node, &["title"]);
        }
        account.avatar = crate::yt::parse::thumbs_of(node);
    }
    if account.name.is_empty() {
        if let Some(node) = crate::yt::parse::find_key(&json, "topbarMenuButtonRenderer") {
            account.name = crate::yt::parse::text_at(node, &["accessibility", "accessibilityData", "label"]);
            if account.avatar.is_empty() {
                account.avatar = crate::yt::parse::thumbs_of(node);
            }
        }
    }
    if account.name.is_empty() {
        return None;
    }
    api.session().set_account(account.clone());
    Some(account)
}

/// Cuenta cuántos elementos reproducibles hay en un feed (para la cola).
pub fn playable(feed: &Feed) -> Vec<Card> {
    feed.shelves
        .iter()
        .flat_map(|s| s.items.iter().cloned())
        .filter(|c| {
            matches!(
                c.kind,
                crate::domain::MediaKind::Video
                    | crate::domain::MediaKind::Short
                    | crate::domain::MediaKind::Live
                    | crate::domain::MediaKind::Song
            )
        })
        .collect()
}
