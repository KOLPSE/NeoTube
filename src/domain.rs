//! Modelos de dominio de NeoTube 2.0.
//!
//! Regla de arquitectura (PLAN.md §5): estos tipos **no** son los formatos de
//! respuesta de YouTube. Los parsers de `innertube::*` los construyen y la
//! interfaz solo ve esto.

use serde::{Deserialize, Serialize};

/// Una miniatura con su ancho real, para poder pedir la que de verdad se pinta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// Lista de miniaturas ordenable por ancho.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbs(pub Vec<Thumbnail>);

impl Thumbs {
    pub fn new(list: impl IntoIterator<Item = Thumbnail>) -> Self {
        let mut v: Vec<Thumbnail> = list.into_iter().collect();
        v.sort_by_key(|t| t.width);
        Self(v)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn first(&self) -> Option<&Thumbnail> {
        self.0.first()
    }

    /// La miniatura más pequeña que **sigue siendo suficiente** para `want_px`.
    /// Pedir una 1280x720 para una tarjeta de 280 px era la mitad del consumo
    /// de datos de la versión Flutter (PLAN.md §8).
    pub fn best_for(&self, want_px: u32) -> Option<&Thumbnail> {
        if self.0.is_empty() {
            return None;
        }
        // Se acepta hasta 1.3x de sobremedida antes de preferir la siguiente:
        // evita elegir 480 para una tarjeta de 400 (se ve blanda) y también
        // elegir 1280 cuando hay 640.
        let limit = (want_px as f32 * 1.3).ceil() as u32;
        self.0
            .iter()
            .find(|t| t.width >= want_px && t.width <= limit)
            .or_else(|| self.0.iter().find(|t| t.width >= want_px))
            .or_else(|| self.0.last())
    }
}

/// Qué es un elemento de YouTube. Mismo id puede ser vídeo, Short o directo:
/// el tipo se guarda explícitamente (PLAN.md §5).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Video,
    Short,
    Live,
    Song,
    Album,
    Artist,
    Playlist,
    Channel,
    /// Mezcla / radio: no es una lista de ids, hay que resolverla.
    Mix,
}

impl MediaKind {
    pub fn is_audio_first(self) -> bool {
        matches!(self, MediaKind::Song | MediaKind::Album | MediaKind::Artist)
    }

    pub fn label(self) -> &'static str {
        match self {
            MediaKind::Video => "Vídeo",
            MediaKind::Short => "Short",
            MediaKind::Live => "Directo",
            MediaKind::Song => "Canción",
            MediaKind::Album => "Álbum",
            MediaKind::Artist => "Artista",
            MediaKind::Playlist => "Lista",
            MediaKind::Channel => "Canal",
            MediaKind::Mix => "Mezcla",
        }
    }
}

/// Tarjeta genérica: es lo que pinta la rejilla, el estante, la búsqueda y la
/// barra lateral de relacionados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Card {
    /// `videoId` o `browseId` según `kind`.
    pub id: String,
    pub kind: MediaKind,
    pub title: String,
    /// Línea secundaria: canal/artista.
    pub author: String,
    /// Metadatos ya compuestos: "1,2 M de visualizaciones · hace 3 días".
    pub meta: String,
    pub duration: Option<String>,
    pub thumbs: Thumbs,
    /// "EN VIVO", "NUEVO", "SHORTS"…
    pub badge: Option<String>,
    /// Id del canal/artista cuando se conoce, para poder navegar.
    pub author_id: Option<String>,
    /// Id de playlist si la tarjeta es una lista.
    pub playlist_id: Option<String>,
}

impl Card {
    pub fn new(id: impl Into<String>, kind: MediaKind, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            author: String::new(),
            meta: String::new(),
            duration: None,
            thumbs: Thumbs::default(),
            badge: None,
            author_id: None,
            playlist_id: None,
        }
    }
}

/// Grupo de tarjetas con título (estantes de Inicio y de Música).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Shelf {
    pub title: Option<String>,
    pub items: Vec<Card>,
    /// Token para pedir más de este estante (carruseles de Música).
    pub more: Option<String>,
    /// Estilo de presentación que insinúa el renderer original.
    pub layout: ShelfLayout,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShelfLayout {
    /// Estante horizontal (carrusel).
    #[default]
    Row,
    /// Lista vertical compacta (resultados de búsqueda, playlists).
    List,
    /// Rejilla de tarjetas grandes (Inicio).
    Grid,
}

/// Contenido paginado de una pantalla.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Feed {
    pub shelves: Vec<Shelf>,
    pub continuation: Option<String>,
    /// Texto del chip seleccionado en los filtros de búsqueda, si hay.
    pub chip: Option<String>,
    /// Chips disponibles en esta respuesta.
    pub chips: Vec<Card>,
}

/// Estado de "me gusta" tal y como lo devuelve YouTube para el usuario actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LikeState {
    #[default]
    None,
    Like,
    Dislike,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub handle: Option<String>,
    pub subscribers: Option<String>,
    pub avatar: Thumbs,
    pub banner: Thumbs,
}

/// Página de reproducción de un vídeo, Short o directo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchInfo {
    pub video_id: String,
    pub title: String,
    pub channel: ChannelInfo,
    pub view_count: Option<String>,
    pub published: Option<String>,
    pub description: String,
    pub duration_seconds: Option<f64>,
    pub like_state: LikeState,
    pub likes_text: Option<String>,
    pub subscribed: bool,
    pub is_live: bool,
    pub related: Vec<Card>,
    pub continuation: Option<String>,
    /// Capítulos detectados en la descripción o en el `playerOverlays`.
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub start_seconds: f64,
}

/// Pista reproducible ya resuelta: la URL caduca, así que se guarda con su hora.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamTrack {
    pub url: String,
    pub mime: String,
    pub itag: u32,
    pub bitrate: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub audio_channels: Option<u32>,
    pub approx_duration_ms: Option<u64>,
}

impl StreamTrack {
    pub fn is_audio(&self) -> bool {
        self.mime.starts_with("audio/")
    }

    /// "1080p60", "720p", "Opus 160 kbps"…
    pub fn quality_label(&self) -> String {
        match (self.height, self.width) {
            (Some(h), _) if h > 0 => match self.fps {
                Some(f) if f >= 50 => format!("{h}p{f}"),
                _ => format!("{h}p"),
            },
            _ => format!("{} kbps", self.bitrate / 1000),
        }
    }
}

/// Resultado de resolver un vídeo. Puede venir muxado (una sola URL) o en DASH
/// (dos URLs), y es el reproductor quien decide cómo montarlo.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedStream {
    pub video_id: String,
    /// Pista única con audio y vídeo, si YouTube la ofrece.
    pub muxed: Option<StreamTrack>,
    pub video: Option<StreamTrack>,
    pub audio: Option<StreamTrack>,
    /// Cabeceras con las que **hay que abrir** la URL, no solo pedirla
    /// (PORTING: ANDROID_VR → UA + Referer + Origin).
    pub headers: Vec<(String, String)>,
    /// Milisegundos de validez aproximada (de `streamingData.expiresInSeconds`).
    pub expires_in_seconds: Option<u64>,
    pub resolved_at: std::time::Instant,
    pub duration_seconds: Option<f64>,
}

impl ResolvedStream {
    /// ¿Sigue sirviendo? Se renueva con un margen para no caducar en mitad de
    /// una canción.
    pub fn is_fresh(&self) -> bool {
        let ttl = self.expires_in_seconds.unwrap_or(3600).saturating_sub(120);
        self.resolved_at.elapsed().as_secs() < ttl
    }

    pub fn audio_url(&self) -> Option<&str> {
        self.muxed
            .as_ref()
            .or(self.audio.as_ref())
            .map(|t| t.url.as_str())
    }

    pub fn best_quality_label(&self) -> String {
        if let Some(v) = &self.video {
            v.quality_label()
        } else if let Some(m) = &self.muxed {
            m.quality_label()
        } else {
            "Audio".to_string()
        }
    }
}

/// Identidad de la sesión.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub handle: Option<String>,
    pub avatar: Thumbs,
    /// Canal activo (YouTube permite varios por cuenta).
    pub channel_id: Option<String>,
}

impl Account {
    pub fn is_logged_in(&self) -> bool {
        !self.name.is_empty()
    }
}

/// Qué sabe hacer la app en este momento, para no pintar botones que mienten
/// (PLAN.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Ready,
    NeedsLogin,
    /// Requiere componente web / apertura externa.
    NeedsWeb,
    Unavailable,
}

/// A dónde navega la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Route {
    Home,
    Search { query: String, chip: Option<String> },
    Watch { video_id: String },
    Shorts { start_id: Option<String> },
    MusicHome,
    MusicSearch { query: String },
    Album { browse_id: String },
    Artist { browse_id: String },
    Playlist { browse_id: String },
    Channel { browse_id: String },
    Library,
    History,
    #[allow(dead_code)]
    Settings,
}

impl Route {
    pub fn is_music(&self) -> bool {
        matches!(
            self,
            Route::MusicHome | Route::MusicSearch { .. } | Route::Album { .. } | Route::Artist { .. }
        )
    }

    pub fn title(&self) -> String {
        match self {
            Route::Home => "Inicio".into(),
            Route::Search { query, .. } => {
                if query.is_empty() {
                    "Buscar".into()
                } else {
                    format!("Buscar: {query}")
                }
            }
            Route::Watch { .. } => "Reproduciendo".into(),
            Route::Shorts { .. } => "Shorts".into(),
            Route::MusicHome => "Música".into(),
            Route::MusicSearch { query } => format!("Música: {query}"),
            Route::Album { .. } => "Álbum".into(),
            Route::Artist { .. } => "Artista".into(),
            Route::Playlist { .. } => "Lista".into(),
            Route::Channel { .. } => "Canal".into(),
            Route::Library => "Biblioteca".into(),
            Route::History => "Historial".into(),
            Route::Settings => "Ajustes".into(),
        }
    }
}

/// Una entrada de la cola de reproducción (local, distinta de las playlists de
/// la cuenta — PLAN.md §5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueueItem {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub duration: Option<String>,
    pub thumbs: Thumbs,
    pub kind: MediaKind,
}

impl QueueItem {
    pub fn from_card(card: &Card) -> Option<Self> {
        if card.kind != MediaKind::Video && card.kind != MediaKind::Short && card.kind != MediaKind::Live && card.kind != MediaKind::Song {
            return None;
        }
        Some(Self {
            video_id: card.id.clone(),
            title: card.title.clone(),
            author: card.author.clone(),
            duration: card.duration.clone(),
            thumbs: card.thumbs.clone(),
            kind: card.kind,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Off
    }
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RepeatMode::Off => "Repetir: no",
            RepeatMode::All => "Repetir: cola",
            RepeatMode::One => "Repetir: una",
        }
    }
}

/// Opción de calidad que el usuario puede elegir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityOption {
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub label: String,
}
