//! Estado de la aplicación y bucle de interfaz.
//!
//! Es el único sitio que conoce a la vez la red, el reproductor y las pantallas.
//! Las pantallas devuelven [`Intent`] y aquí se ejecutan; así ninguna pantalla
//! necesita saber si abrir un vídeo implica resolver un stream, crear la cola o
//! tocar la base de datos.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use egui::{Color32, RichText, Sense, Vec2};

use crate::domain::{
    Account, Card, Feed, MediaKind, QueueItem, RepeatMode, ResolvedStream, Route, WatchInfo,
};
use crate::playback::engine::{Engine, State as PlayerState};
use crate::playback::queue::Queue;
use crate::ui::actions::{Load, Payload, TaskDone, TaskIds, TaskKind};
use crate::ui::icons::{self as icons_mod, Icon};
use crate::ui::login::LoginOutcome;
use crate::ui::screens;
use crate::ui::shell::{self, ShellIntent};
use crate::ui::tasks;
use crate::ui::theme::{metrics, palette};
use crate::ui::thumbs::{LoadedThumb, ThumbCache};
use crate::ui::widgets;
use crate::AppContext;

/// Lo que el usuario ha pedido en una pantalla.
#[derive(Debug, Clone)]
pub enum Intent {
    /// Nada.
    None,
    /// Reintentar la carga de la pantalla actual.
    Reload,
    /// Abrir la página de un contenido.
    Open(Card),
    /// Abrir el menú contextual de una tarjeta.
    CardMenu(Card),
    /// Reproducir ya.
    PlayNow(Card),
    /// Añadir a la cola sin cambiar lo que suena.
    Enqueue(Card),
    /// Reproducir todos los elementos de la pantalla.
    PlayAll,
    /// Reproducir todos, en orden aleatorio.
    PlayAllShuffled,
    /// Añadir todo a la cola.
    EnqueueAll,
    TogglePlay,
    NextTrack,
    PreviousTrack,
    SeekFraction(f32),
    SeekSeconds(f64),
    ToggleLike {
        video_id: String,
        on: bool,
    },
    ToggleSubscribe {
        channel_id: String,
        on: bool,
    },
    ToggleQueue,
    HomeChip(String),
    SearchFilter {
        params: Option<String>,
    },
    ChannelTab(usize),
    ShortsNext,
    ShortsPrevious,
    Navigate(Route),
    Back,
    Forward,
    SubmitSearch(String),
    Login,
    LoginWithCookies(String),
    Logout,
    SetMaxHeight(Option<u32>),
    /// Cambiar entre tema claro y oscuro.
    SetTheme(bool),
    /// Mostrar u ocultar la barra lateral desplegada.
    ToggleSidebar,
    /// Abrir o cerrar el panel de notificaciones.
    ToggleNotifications,
    /// Abrir o cerrar el menú de perfil.
    ToggleProfileMenu,
    /// Abrir una URL en el navegador del sistema.
    OpenExternal(String),
    /// Cambiar la pestaña de un canal.
    SetChannelTab(usize),
    /// Cargar más resultados de la pantalla actual.
    LoadMore,
    /// Expandir o plegar la descripción del vídeo.
    ToggleDescription,
    /// Mostrar u ocultar el panel de comentarios (en Shorts cambia el reparto
    /// de espacio; en vídeo, cuántos se listan).
    ToggleComments,
    /// Publicar un comentario con ese texto.
    PostComment(String),
    /// Me gusta / no me gusta en un comentario.
    RateComment {
        comment_id: String,
        up: bool,
    },
    /// Mostrar u ocultar el panel de playlist de la página de vídeo.
    TogglePlaylistPanel,
    SetVolume(f32),
    ToggleShuffle,
    CycleRepeat,
    ClearHistory,
}

/// Estado visible del reproductor (lo que las pantallas necesitan).
#[derive(Default)]
pub struct PlayerView {
    pub position_ms: i64,
    pub duration_ms: i64,
    pub is_playing: bool,
    pub stream: Option<ResolvedStream>,
    pub error: Option<String>,
    pub texture: Option<egui::TextureHandle>,
}

/// Contenido cargado de una pantalla.
struct ScreenState {
    feed: Feed,
    load: Load,
    /// Clave de caché del feed, para la paginación.
    key: String,
    music: bool,
}

impl Default for ScreenState {
    fn default() -> Self {
        Self {
            feed: Feed::default(),
            load: Load::Idle,
            key: String::new(),
            music: false,
        }
    }
}

pub struct App {
    ctx: AppContext,
    runtime: tokio::runtime::Handle,
    /// Runtime propio mantenido vivo (el `Handle` solo presta).
    _runtime_guard: Arc<tokio::runtime::Runtime>,
    engine: Option<Arc<Engine>>,
    queue: Queue,
    player: PlayerView,
    player_state: PlayerState,

    route: Route,
    back_stack: Vec<Route>,
    forward_stack: Vec<Route>,

    home: ScreenState,
    search: ScreenState,
    music: ScreenState,
    browse: ScreenState,
    channel_state: ScreenState,
    library_state: ScreenState,

    watch_info: Option<WatchInfo>,
    watch_load: Load,
    /// Tarjetas de la sección Shorts del inicio (independientes del feed de
    /// Shorts a pantalla completa: aquí son una fila, no un reproductor).
    home_shorts: Vec<Card>,
    shorts_cards: Vec<Card>,
    shorts_index: usize,
    shorts_load: Load,

    account: Option<Account>,
    search_input: String,
    search_filter: usize,
    home_chip: String,
    channel_tab: usize,
    queue_open: bool,
    sidebar_open: bool,
    dark: bool,
    volume: f32,
    settings_max_height: Option<u32>,
    login_paste: String,
    login_error: Option<String>,
    show_login: bool,
    toast: Option<(String, std::time::Instant)>,

    // --- estructura global (cabecera, barra lateral, overlays)
    /// Estado de la barra lateral (Mostrar más / Mostrar menos).
    sidebar: crate::ui::shell::SidebarState,
    /// Panel de notificaciones abierto.
    show_notifications: bool,
    /// Menú de perfil abierto.
    show_profile: bool,
    /// Menú contextual de una tarjeta: (tarjeta, posición donde se abrió).
    card_menu: Option<(Card, egui::Pos2)>,
    /// Descripción del vídeo expandida.
    description_open: bool,
    /// Panel de comentarios abierto (afecta a vídeo y a Shorts).
    comments_open: bool,
    /// Texto que se está escribiendo como comentario.
    comment_input: String,
    /// Comentarios del vídeo actual.
    comments: Vec<crate::yt::comments::Comment>,
    comments_load: Load,
    /// Playlist lateral de la página de vídeo.
    playlist_panel: Vec<Card>,
    playlist_panel_title: Option<String>,
    playlist_open: bool,
    /// Notificaciones ya cargadas.
    notifications: Vec<crate::ui::shell::NotificationView>,
    notifications_load: Load,
    /// Suscripciones para la barra lateral: (nombre, avatar, en_directo).
    subscriptions: Vec<(String, crate::domain::Thumbs, bool)>,
    /// Resultado de la petición de notificaciones.
    /// Resultado de la carga de comentarios.
    comments_tx: Sender<Result<crate::yt::comments::CommentsPage, crate::yt::client::ApiError>>,
    comments_rx: Receiver<Result<crate::yt::comments::CommentsPage, crate::yt::client::ApiError>>,
    /// Resultado de publicar un comentario.
    comment_tx: Sender<Result<(), crate::yt::client::ApiError>>,
    comment_rx: Receiver<Result<(), crate::yt::client::ApiError>>,
    /// Shorts del inicio.
    shorts_tx: Sender<Vec<Card>>,
    shorts_rx: Receiver<Vec<Card>>,
    notify_tx: Sender<Result<Vec<crate::yt::comments::Notification>, crate::yt::client::ApiError>>,
    notify_rx:
        Receiver<Result<Vec<crate::yt::comments::Notification>, crate::yt::client::ApiError>>,

    tasks: TaskIds,
    tx: Sender<TaskDone>,
    rx: Receiver<TaskDone>,
    login_tx: Sender<LoginOutcome>,
    login_rx: Receiver<LoginOutcome>,
    thumb_tx: Sender<LoadedThumb>,
    thumb_rx: Receiver<LoadedThumb>,
    thumbs: Arc<parking_lot::Mutex<ThumbCache>>,
    http: reqwest::Client,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let ctx = AppContext::bootstrap()?;
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let (login_tx, login_rx) = std::sync::mpsc::channel();
        let (thumb_tx, thumb_rx) = std::sync::mpsc::channel();
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let (shorts_tx, shorts_rx) = std::sync::mpsc::channel();
        let (comment_tx, comment_rx) = std::sync::mpsc::channel();
        let (comments_tx, comments_rx) = std::sync::mpsc::channel();
        let http = reqwest::Client::new();

        let dark = ctx.store.get_setting("theme").as_deref() != Some("light");
        let volume = ctx.store.volume();
        let sidebar_open = ctx.store.sidebar_open();
        let max_height = ctx.store.max_height();
        let repeat = ctx.store.repeat_mode();
        let mut queue = Queue::new(repeat);
        queue.restore(&ctx.store);

        crate::ui::theme::apply(&cc.egui_ctx, dark);

        let mut app = Self {
            ctx,
            runtime: runtime.handle().clone(),
            _runtime_guard: runtime,
            engine: None,
            queue,
            player: PlayerView::default(),
            player_state: PlayerState::Idle,
            route: Route::Home,
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            home: ScreenState::default(),
            search: ScreenState::default(),
            music: ScreenState::default(),
            browse: ScreenState::default(),
            channel_state: ScreenState::default(),
            library_state: ScreenState::default(),
            watch_info: None,
            watch_load: Load::Idle,
            home_shorts: Vec::new(),
            shorts_cards: Vec::new(),
            shorts_index: 0,
            shorts_load: Load::Idle,
            account: ctx_account_placeholder(),
            search_input: String::new(),
            search_filter: 0,
            home_chip: "Todo".to_string(),
            channel_tab: 0,
            queue_open: false,
            sidebar_open,
            dark,
            volume,
            settings_max_height: max_height,
            login_paste: String::new(),
            login_error: None,
            show_login: false,
            toast: None,
            sidebar: crate::ui::shell::SidebarState {
                show_all_subscriptions: false,
                show_all_explore: false,
            },
            show_notifications: false,
            show_profile: false,
            card_menu: None,
            description_open: false,
            comments_open: false,
            comment_input: String::new(),
            comments: Vec::new(),
            comments_load: Load::Idle,
            playlist_panel: Vec::new(),
            playlist_panel_title: None,
            playlist_open: true,
            notifications: Vec::new(),
            notifications_load: Load::Idle,
            subscriptions: Vec::new(),
            comments_tx,
            comments_rx,
            comment_tx,
            comment_rx,
            shorts_tx,
            shorts_rx,
            notify_tx,
            notify_rx,
            tasks: TaskIds::default(),
            tx,
            rx,
            login_tx,
            login_rx,
            thumb_tx,
            thumb_rx,
            thumbs: Arc::new(parking_lot::Mutex::new(ThumbCache::default())),
            http,
        };
        app.account = app.ctx.session.account();
        // Pantalla inicial: por defecto, Inicio. `NEOTUBE_START` permite abrir
        // directamente en otra (útil para capturas de verificación y, más
        // adelante, para abrir enlaces `neotube://`).
        let start = std::env::var("NEOTUBE_START").unwrap_or_default();
        if let Some(video_id) = start.strip_prefix("watch:") {
            app.route = Route::Watch {
                video_id: video_id.to_string(),
            };
            app.on_route_loaded(&app.route.clone());
        } else {
            app.load_home();
        }
        app.refresh_account();
        Ok(app)
    }

    // ------------------------------------------------------------ tareas

    fn spawn(&mut self, kind: TaskKind) {
        let id = self.tasks.take();
        tasks::spawn(
            &self.runtime,
            self.ctx.api.clone(),
            self.ctx.store.clone(),
            self.tx.clone(),
            id,
            kind,
        );
    }

    /// Pide los comentarios del vídeo. Solo con sesión: sin ella YouTube los
    /// devuelve igual, pero el orden y el recuento cambian y no se puede
    /// escribir; se avisa en la propia sección.
    fn load_comments(&mut self, video_id: &str) {
        if !self.ctx.session.is_logged_in() {
            self.comments_load = Load::Ready;
            self.comments.clear();
            return;
        }
        self.comments_load = Load::Loading;
        let api = self.ctx.api.clone();
        let tx = self.comments_tx.clone();
        let video_id = video_id.to_string();
        self.runtime.spawn(async move {
            let result = api.comments(&video_id).await;
            let _ = tx.send(result);
        });
    }

    /// Pide las notificaciones de la cuenta. Si no hay sesión, el panel ya lo
    /// explica; no se llama a la red.
    fn load_notifications(&mut self) {
        if !self.ctx.session.is_logged_in() {
            self.notifications_load = Load::Ready;
            return;
        }
        self.notifications_load = Load::Loading;
        let api = self.ctx.api.clone();
        let tx = self.notify_tx.clone();
        self.runtime.spawn(async move {
            let result = api.notifications().await;
            let _ = tx.send(result);
        });
    }

    fn refresh_account(&mut self) {
        self.spawn(TaskKind::Account);
    }

    // ---------------------------------------------------------- cargas

    fn load_home(&mut self) {
        self.home.load = Load::Loading;
        self.spawn(TaskKind::Home);
        // La fila de Shorts del inicio es contenido distinto del feed: si
        // fallara (YouTube no siempre los devuelve), el inicio sigue lleno.
        if self.home_shorts.is_empty() {
            let api = self.ctx.api.clone();
            let tx = self.shorts_tx.clone();
            self.runtime.spawn(async move {
                match api.shorts_feed("tendencias").await {
                    Ok(cards) => {
                        crate::ui::dbg_log(format!("home_shorts: {} tarjetas", cards.len()));
                        let _ = tx.send(cards);
                    }
                    Err(e) => crate::ui::dbg_log(format!("home_shorts falló: {e}")),
                }
            });
        }
    }

    fn load_search(&mut self, query: String, params: Option<String>) {
        self.search_input = query.clone();
        self.search.load = Load::Loading;
        self.spawn(TaskKind::Search {
            query,
            music: false,
            params,
        });
    }

    fn load_music(&mut self) {
        self.music.load = Load::Loading;
        self.spawn(TaskKind::MusicHome);
    }

    fn load_browse(&mut self, browse_id: &str) {
        self.browse.load = Load::Loading;
        self.spawn(TaskKind::Browse {
            browse_id: browse_id.to_string(),
            music: true,
        });
    }

    fn load_library(&mut self) {
        self.library_state.load = Load::Loading;
        self.spawn(TaskKind::Library);
    }

    fn load_history(&mut self) {
        self.library_state.load = Load::Loading;
        self.spawn(TaskKind::History);
    }

    fn load_watch(&mut self, video_id: &str) {
        self.watch_load = Load::Loading;
        self.spawn(TaskKind::Watch {
            video_id: video_id.to_string(),
        });
        self.spawn(TaskKind::Resolve {
            video_id: video_id.to_string(),
            max_height: self.settings_max_height,
        });
    }

    fn load_shorts(&mut self, seed: String) {
        self.shorts_load = Load::Loading;
        self.spawn(TaskKind::Shorts { seed });
    }

    // ------------------------------------------------------- navegación

    fn navigate(&mut self, route: Route) {
        if route == self.route {
            return;
        }
        self.back_stack.push(self.route.clone());
        self.forward_stack.clear();
        self.route = route.clone();
        self.on_route_loaded(&route);
    }

    fn on_route_loaded(&mut self, route: &Route) {
        match route {
            Route::Home => {
                if self.home.load != Load::Ready || self.home.feed.shelves.is_empty() {
                    self.load_home();
                }
            }
            Route::Search { query, chip } => {
                self.search_input = query.clone();
                let params = chip.clone();
                self.load_search(query.clone(), params);
            }
            Route::Watch { video_id } => {
                self.watch_info = None;
                self.comments.clear();
                self.comments_load = Load::Idle;
                self.description_open = false;
                self.load_watch(video_id);
                self.load_comments(video_id);
            }
            Route::Shorts { .. } => {
                if self.shorts_cards.is_empty() {
                    self.load_shorts("tendencias".to_string());
                }
            }
            Route::MusicHome => {
                if self.music.load != Load::Ready || self.music.feed.shelves.is_empty() {
                    self.load_music();
                }
            }
            Route::MusicSearch { query } => {
                self.search.load = Load::Loading;
                self.spawn(TaskKind::Search {
                    query: query.clone(),
                    music: true,
                    params: None,
                });
            }
            Route::Album { browse_id }
            | Route::Artist { browse_id }
            | Route::Channel { browse_id } => {
                self.browse = ScreenState::default();
                self.load_browse(browse_id);
            }
            Route::Playlist { browse_id } => {
                self.browse = ScreenState::default();
                self.load_browse(browse_id);
            }
            Route::Library => self.load_library(),
            Route::Subscriptions => {
                // El feed de suscripciones es contenido de la cuenta: sin
                // sesión no hay nada que pedir y la pantalla lo explica.
                if self.ctx.session.is_logged_in() {
                    self.library_state.load = Load::Loading;
                    self.spawn(TaskKind::Browse {
                        browse_id: "FEsubscriptions".to_string(),
                        music: false,
                    });
                } else {
                    self.library_state.load = Load::Failed(
                        "Inicia sesión para ver las novedades de tus suscripciones.".into(),
                    );
                }
            }
            Route::History => self.load_history(),
            Route::Settings => {}
        }
    }

    fn back(&mut self) {
        if let Some(previous) = self.back_stack.pop() {
            self.forward_stack.push(self.route.clone());
            self.route = previous.clone();
            self.on_route_loaded(&previous);
        }
    }

    fn forward(&mut self) {
        if let Some(next) = self.forward_stack.pop() {
            self.back_stack.push(self.route.clone());
            self.route = next.clone();
            self.on_route_loaded(&next);
        }
    }

    // ------------------------------------------------------- reproducción

    fn ensure_engine(&mut self) -> Option<Arc<Engine>> {
        if let Some(engine) = &self.engine {
            return Some(engine.clone());
        }
        match Engine::new(false) {
            Ok(engine) => {
                engine.set_volume(self.volume as i32);
                self.engine = Some(engine.clone());
                Some(engine)
            }
            Err(e) => {
                self.player.error = Some(e.to_string());
                self.toast(format!("No se puede reproducir: {e}"));
                None
            }
        }
    }

    /// Abre un stream ya resuelto en el motor.
    ///
    /// `audio_only` es lo que separa el modo música del modo vídeo: con él se
    /// abre solo la pista de audio y no se decodifica ni se transfiere ningún
    /// fotograma. Es la diferencia entre ~150 MB y ~400 MB con un vídeo musical
    /// sonando de fondo.
    fn play_stream(&mut self, stream: ResolvedStream, title: String, audio_only: bool) {
        let Some(engine) = self.ensure_engine() else {
            return;
        };
        let (url, slave, has_video) = if audio_only {
            // Modo música: solo la pista de audio, sin esclavo y sin vídeo.
            match &stream.audio {
                Some(a) => (a.url.clone(), None, false),
                // Si no hay audio suelto, se acepta el muxado (mejor eso que
                // nada), pero se marca como vídeo para que el motor no lo
                // trate como audio puro.
                None => match &stream.muxed {
                    Some(m) => (m.url.clone(), None, true),
                    None => {
                        self.player.error =
                            Some("Esta canción no traía ninguna pista de audio.".into());
                        return;
                    }
                },
            }
        } else {
            match (&stream.video, &stream.muxed, &stream.audio) {
                (Some(v), _, Some(a)) => (v.url.clone(), Some(a.url.clone()), true),
                (Some(v), _, None) => (v.url.clone(), None, true),
                (None, Some(m), _) => (m.url.clone(), None, true),
                (None, None, Some(a)) => (a.url.clone(), None, false),
                _ => {
                    self.player.error = Some("La pista no traía ninguna URL.".into());
                    return;
                }
            }
        };
        if let Err(e) = engine.open(&url, slave.as_deref(), &stream.headers, has_video) {
            self.player.error = Some(e.to_string());
            return;
        }
        if let Err(e) = engine.play() {
            self.player.error = Some(e.to_string());
            return;
        }
        engine.set_volume(self.volume as i32);
        self.player.duration_ms = stream.duration_seconds.unwrap_or(0.0) as i64 * 1000;
        self.player.error = None;
        self.player.stream = Some(stream);
        self.player_state = PlayerState::Playing;
        let _ = title;
    }

    /// Reproduce un elemento de la cola, resolviéndolo si hace falta.
    fn play_current(&mut self) {
        let Some(item) = self.queue.current().cloned() else {
            return;
        };
        self.ctx.store.record_history(&item);
        self.spawn(TaskKind::Resolve {
            video_id: item.video_id.clone(),
            max_height: self.settings_max_height,
        });
    }

    fn toggle_play(&mut self) {
        let Some(engine) = &self.engine else {
            self.play_current();
            return;
        };
        match self.player_state {
            PlayerState::Playing => {
                engine.set_paused(true);
                self.player_state = PlayerState::Paused;
            }
            PlayerState::Paused => {
                engine.set_paused(false);
                self.player_state = PlayerState::Playing;
            }
            _ => self.play_current(),
        }
    }

    fn next_track(&mut self) {
        if let Some(item) = self.queue.advance() {
            self.queue.persist(&self.ctx.store);
            self.ctx.store.record_history(&item);
            self.spawn(TaskKind::Resolve {
                video_id: item.video_id,
                max_height: self.settings_max_height,
            });
        }
    }

    fn previous_track(&mut self) {
        if let Some(item) = self.queue.previous() {
            self.queue.persist(&self.ctx.store);
            self.spawn(TaskKind::Resolve {
                video_id: item.video_id,
                max_height: self.settings_max_height,
            });
        }
    }

    /// Empieza a reproducir una tarjeta: la pone al frente de la cola.
    fn play_card(&mut self, card: &Card) {
        if let Some(item) = QueueItem::from_card(card) {
            self.queue.replace(vec![item], 0);
            self.queue.persist(&self.ctx.store);
            self.ctx.store.record_history(self.queue.current().unwrap());
            self.spawn(TaskKind::Resolve {
                video_id: card.id.clone(),
                max_height: self.settings_max_height,
            });
        }
    }

    /// Reproduce todos los elementos reproducibles del feed actual.
    fn play_all(&mut self, shuffled: bool) {
        let cards = tasks::playable(self.current_feed());
        if cards.is_empty() {
            self.toast("No hay nada que reproducir aquí.".into());
            return;
        }
        let items: Vec<QueueItem> = cards.iter().filter_map(QueueItem::from_card).collect();
        if items.is_empty() {
            self.toast("Esta pantalla no tiene pistas reproducibles.".into());
            return;
        }
        self.queue.replace(items, 0);
        self.queue.set_shuffle(shuffled);
        self.queue.persist(&self.ctx.store);
        self.play_current();
    }

    fn enqueue_all(&mut self) {
        let cards = tasks::playable(self.current_feed());
        let added = self.queue.extend_cards(cards);
        self.queue.persist(&self.ctx.store);
        self.toast(format!("{added} elemento(s) añadidos a la cola"));
    }

    fn current_feed(&self) -> &Feed {
        match &self.route {
            Route::Home => &self.home.feed,
            Route::Search { .. } | Route::MusicSearch { .. } => &self.search.feed,
            Route::MusicHome => &self.music.feed,
            Route::Library | Route::History => &self.library_state.feed,
            _ => &self.browse.feed,
        }
    }

    fn current_feed_mut(&mut self) -> &mut ScreenState {
        match self.route {
            Route::Home => &mut self.home,
            Route::Search { .. } | Route::MusicSearch { .. } => &mut self.search,
            Route::MusicHome => &mut self.music,
            Route::Library | Route::History => &mut self.library_state,
            Route::Channel { .. } => &mut self.channel_state,
            _ => &mut self.browse,
        }
    }

    fn toast(&mut self, message: String) {
        self.toast = Some((message, std::time::Instant::now()));
    }

    // ----------------------------------------------------------- mensajes

    fn pump_tasks(&mut self, ctx: &egui::Context) {
        while let Ok(done) = self.rx.try_recv() {
            match done.result {
                Ok(payload) => self.apply_payload(done.kind, payload),
                Err(message) => self.apply_error(done.kind, message),
            }
            ctx.request_repaint();
        }
        // Resultados del login.
        while let Ok(outcome) = self.login_rx.try_recv() {
            match outcome {
                LoginOutcome::Cookies(raw) => {
                    let cookies = crate::session::Cookies::from_raw(raw);
                    if cookies.signing_value().is_some() {
                        self.ctx.session.set_cookies(cookies);
                        self.ctx.store.cache_clear();
                        self.show_login = false;
                        self.login_error = None;
                        self.toast("Sesión iniciada".into());
                        self.refresh_account();
                        let route = self.route.clone();
                        self.on_route_loaded(&route);
                    } else {
                        self.login_error =
                            Some("La ventana se cerró sin una sesión válida de YouTube.".into());
                    }
                }
                LoginOutcome::Cancelled => {
                    self.login_error = None;
                }
                LoginOutcome::Failed(error) => {
                    self.login_error = Some(error);
                }
            }
            ctx.request_repaint();
        }
        // Carga de comentarios.
        while let Ok(result) = self.comments_rx.try_recv() {
            match result {
                Ok(page) => {
                    self.comments = page.comments;
                    self.comments_load = Load::Ready;
                }
                Err(e) => self.comments_load = Load::Failed(e.user_message()),
            }
            ctx.request_repaint();
        }

        // Resultado de publicar un comentario.
        while let Ok(result) = self.comment_rx.try_recv() {
            match result {
                Ok(()) => {
                    self.toast("Comentario publicado".into());
                    // Se recargan para verlo en su sitio.
                    if let Route::Watch { video_id } = self.route.clone() {
                        self.load_comments(&video_id);
                    }
                }
                Err(e) => self.toast(e.user_message()),
            }
            ctx.request_repaint();
        }

        // Shorts del inicio.
        while let Ok(cards) = self.shorts_rx.try_recv() {
            // Se toman 8 como máximo: una fila, no un feed.
            self.home_shorts = cards.into_iter().take(8).collect();
            ctx.request_repaint();
        }

        // Notificaciones.
        while let Ok(result) = self.notify_rx.try_recv() {
            match result {
                Ok(items) => {
                    self.notifications = items
                        .into_iter()
                        .map(|n| crate::ui::shell::NotificationView {
                            title: n.title,
                            text: n.text,
                            thumbnail: n.thumbnail,
                            avatar: crate::domain::Thumbs::default(),
                            published: n.published,
                            unread: n.unread,
                        })
                        .collect();
                    self.notifications_load = Load::Ready;
                }
                Err(e) => self.notifications_load = Load::Failed(e.user_message()),
            }
            ctx.request_repaint();
        }

        // Miniaturas.
        let mut loaded = Vec::new();
        while let Ok(thumb) = self.thumb_rx.try_recv() {
            loaded.push(thumb);
        }
        if !loaded.is_empty() {
            for thumb in loaded {
                self.thumbs.lock().insert(ctx, thumb);
            }
            ctx.request_repaint();
        }
    }

    fn apply_payload(&mut self, kind: TaskKind, payload: Payload) {
        match (kind, payload) {
            (
                TaskKind::Home,
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.home.feed = feed;
                self.home.load = Load::Ready;
                self.home.key = key;
                self.home.music = music;
            }
            (
                TaskKind::Search { params, .. },
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.search.feed = feed;
                self.search.load = Load::Ready;
                self.search.key = key;
                self.search.music = music;
                self.search_filter = if params.is_some() { 1 } else { 0 };
            }
            (
                TaskKind::MusicHome,
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.music.feed = feed;
                self.music.load = Load::Ready;
                self.music.key = key;
                self.music.music = music;
            }
            (
                TaskKind::Browse { .. },
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.browse.feed = feed;
                self.browse.load = Load::Ready;
                self.browse.key = key;
                self.browse.music = music;
                if matches!(self.route, Route::Channel { .. }) {
                    self.channel_state = ScreenState {
                        feed: self.browse.feed.clone(),
                        load: Load::Ready,
                        key: self.browse.key.clone(),
                        music,
                    };
                }
            }
            (
                TaskKind::Library,
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.library_state.feed = feed;
                self.library_state.load = Load::Ready;
                self.library_state.key = key;
                self.library_state.music = music;
            }
            (
                TaskKind::History,
                Payload::Feed {
                    key, feed, music, ..
                },
            ) => {
                self.library_state.feed = feed;
                self.library_state.load = Load::Ready;
                self.library_state.key = key;
                self.library_state.music = music;
            }
            (
                TaskKind::More { .. },
                Payload::Feed {
                    key, feed, append, ..
                },
            ) => {
                if append {
                    let state = self.current_feed_mut();
                    for shelf in feed.shelves {
                        if let Some(existing) = state
                            .feed
                            .shelves
                            .iter_mut()
                            .find(|s| s.title == shelf.title)
                        {
                            existing.items.extend(shelf.items);
                        } else {
                            state.feed.shelves.push(shelf);
                        }
                    }
                    state.feed.continuation = feed.continuation;
                    state.load = Load::Ready;
                } else {
                    let _ = key;
                }
            }
            (TaskKind::Watch { .. }, Payload::Watch(info)) => {
                // Si el usuario ya se ha ido de la pantalla, se descarta.
                if let Route::Watch { video_id } = &self.route {
                    if *video_id == info.video_id {
                        if self.playlist_panel.is_empty() {
                            // La referencia enseña un mix: la lista lateral es
                            // el propio vídeo más los relacionados. Se marca
                            // como mix para no hacerlo pasar por una lista
                            // guardada del usuario.
                            self.playlist_panel = info.related.clone();
                            self.playlist_panel.insert(
                                0,
                                crate::domain::Card {
                                    id: info.video_id.clone(),
                                    kind: crate::domain::MediaKind::Video,
                                    title: info.title.clone(),
                                    author: info.channel.name.clone(),
                                    meta: String::new(),
                                    duration: info.duration_seconds.map(|d| {
                                        crate::ui::screens::watch::format_time((d * 1000.0) as i64)
                                    }),
                                    thumbs: info.channel.avatar.clone(),
                                    badge: None,
                                    author_id: Some(info.channel.id.clone()),
                                    playlist_id: None,
                                },
                            );
                            self.playlist_panel_title =
                                Some(format!("Mix: {} - {}", info.channel.name, info.title));
                        }
                        self.watch_info = Some(*info);
                        self.watch_load = Load::Ready;
                    }
                }
            }
            (TaskKind::Resolve { video_id, .. }, Payload::Stream(stream)) => {
                // Solo se abre si sigue siendo lo que debe sonar.
                let expected = self.queue.current().map(|c| c.video_id.clone());
                if expected.as_deref() == Some(video_id.as_str()) || expected.is_none() {
                    let current = self.queue.current().cloned();
                    let title = current
                        .as_ref()
                        .map(|c| c.title.clone())
                        .unwrap_or_default();
                    // Música (canción) se reproduce sin vídeo; un vídeo normal,
                    // con imagen. Es la decisión que enciende o apaga la
                    // decodificación de vídeo.
                    let audio_only = current
                        .as_ref()
                        .map(|c| c.kind.is_audio_first())
                        .unwrap_or(false);
                    self.play_stream(*stream, title, audio_only);
                }
            }
            (TaskKind::Shorts { .. }, Payload::Cards(cards)) => {
                self.shorts_cards = cards;
                self.shorts_load = Load::Ready;
                self.shorts_index = 0;
                if let Some(first) = self.shorts_cards.first().cloned() {
                    self.play_card(&first);
                }
            }
            (TaskKind::Account, Payload::Account(account)) => {
                self.account = account;
            }
            (TaskKind::Like { on, .. }, Payload::Done { .. }) => {
                if let Some(info) = self.watch_info.as_mut() {
                    info.like_state = if on {
                        crate::domain::LikeState::Like
                    } else {
                        crate::domain::LikeState::None
                    };
                }
                self.toast(
                    if on {
                        "Añadido a Me gusta"
                    } else {
                        "Quitado de Me gusta"
                    }
                    .into(),
                );
            }
            (TaskKind::Subscribe { on, .. }, Payload::Done { .. }) => {
                if let Some(info) = self.watch_info.as_mut() {
                    info.subscribed = on;
                }
                self.toast(
                    if on {
                        "Suscrito"
                    } else {
                        "Suscripción cancelada"
                    }
                    .into(),
                );
            }
            _ => {}
        }
    }

    fn apply_error(&mut self, kind: TaskKind, message: String) {
        match kind {
            TaskKind::Home => self.home.load = Load::Failed(message),
            TaskKind::Search { .. } => self.search.load = Load::Failed(message),
            TaskKind::MusicHome => self.music.load = Load::Failed(message),
            TaskKind::Browse { .. } => {
                self.browse.load = Load::Failed(message.clone());
                self.channel_state.load = Load::Failed(message);
            }
            TaskKind::Library | TaskKind::History => {
                self.library_state.load = Load::Failed(message)
            }
            TaskKind::Watch { .. } => self.watch_load = Load::Failed(message),
            TaskKind::Resolve { .. } => {
                self.player.error = Some(message);
                // Una URL que el motor rechaza sugiere identidad de visitante
                // gastada: se renueva para la siguiente.
                self.ctx.api.invalidate_visitor();
            }
            TaskKind::Shorts { .. } => self.shorts_load = Load::Failed(message),
            TaskKind::Like { .. } | TaskKind::Subscribe { .. } => self.toast(message),
            _ => {}
        }
    }

    // --------------------------------------------------------------- pintar

    fn handle(&mut self, ctx: &egui::Context, intent: Intent) {
        match intent {
            Intent::None => {}
            Intent::Reload => {
                let route = self.route.clone();
                self.on_route_loaded(&route);
            }
            Intent::Open(card) => {
                let route = match card.kind {
                    MediaKind::Video | MediaKind::Song | MediaKind::Live => Route::Watch {
                        video_id: card.id.clone(),
                    },
                    MediaKind::Short => Route::Shorts {
                        start_id: Some(card.id.clone()),
                    },
                    MediaKind::Album => Route::Album {
                        browse_id: card.id.clone(),
                    },
                    MediaKind::Artist => Route::Artist {
                        browse_id: card.id.clone(),
                    },
                    MediaKind::Playlist | MediaKind::Mix => Route::Playlist {
                        browse_id: card.playlist_id.clone().unwrap_or(card.id.clone()),
                    },
                    MediaKind::Channel => Route::Channel {
                        browse_id: card.id.clone(),
                    },
                };
                self.navigate(route);
            }
            Intent::PlayNow(card) => {
                self.play_card(&card);
                if matches!(
                    card.kind,
                    MediaKind::Video | MediaKind::Short | MediaKind::Live
                ) {
                    self.navigate(Route::Watch {
                        video_id: card.id.clone(),
                    });
                }
            }
            Intent::CardMenu(card) => {
                // La posición se toma del puntero: el menú se ancla donde el
                // usuario ha pulsado, como en la web.
                let pos = ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default();
                self.card_menu = Some((card, pos));
            }
            Intent::Enqueue(card) => {
                if self.queue.push_card(&card) {
                    self.queue.persist(&self.ctx.store);
                    self.toast(format!("«{}» añadido a la cola", card.title));
                } else {
                    self.toast("Eso no se puede reproducir directamente.".into());
                }
            }
            Intent::PlayAll => self.play_all(false),
            Intent::PlayAllShuffled => self.play_all(true),
            Intent::EnqueueAll => self.enqueue_all(),
            Intent::TogglePlay => self.toggle_play(),
            Intent::NextTrack => self.next_track(),
            Intent::PreviousTrack => self.previous_track(),
            Intent::SeekFraction(ratio) => {
                if let Some(engine) = &self.engine {
                    let duration = if self.player.duration_ms > 0 {
                        self.player.duration_ms
                    } else {
                        engine.duration_ms()
                    };
                    engine.seek_ms((duration as f32 * ratio) as i64);
                }
            }
            Intent::SeekSeconds(seconds) => {
                if let Some(engine) = &self.engine {
                    engine.seek_ms((seconds * 1000.0) as i64);
                }
            }
            Intent::ToggleLike { video_id, on } => {
                if !self.ctx.session.is_logged_in() {
                    self.show_login = true;
                    self.toast("Inicia sesión para dar me gusta.".into());
                } else {
                    self.spawn(TaskKind::Like { video_id, on });
                }
            }
            Intent::ToggleSubscribe { channel_id, on } => {
                if !self.ctx.session.is_logged_in() {
                    self.show_login = true;
                    self.toast("Inicia sesión para suscribirte.".into());
                } else {
                    self.spawn(TaskKind::Subscribe { channel_id, on });
                }
            }
            Intent::ToggleQueue => self.queue_open = !self.queue_open,
            Intent::HomeChip(chip) => {
                self.home_chip = chip.clone();
                if chip == "Todo" {
                    let route = self.route.clone();
                    self.on_route_loaded(&route);
                } else {
                    self.navigate(Route::Search {
                        query: chip,
                        chip: None,
                    });
                }
            }
            Intent::SearchFilter { params } => {
                self.search_filter = screens::search::FILTERS
                    .iter()
                    .position(|(_, p)| *p == params.as_deref())
                    .unwrap_or(0);
                self.load_search(self.search_input.clone(), params);
            }
            Intent::ChannelTab(index) => self.channel_tab = index,
            Intent::ShortsNext => {
                if self.shorts_index + 1 < self.shorts_cards.len() {
                    self.shorts_index += 1;
                    if let Some(card) = self.shorts_cards.get(self.shorts_index).cloned() {
                        self.play_card(&card);
                    }
                } else {
                    // Al llegar al final se piden más en lugar de parar.
                    let seed = format!("tendencias {}", self.shorts_cards.len());
                    self.load_shorts(seed);
                }
            }
            Intent::ShortsPrevious => {
                if self.shorts_index > 0 {
                    self.shorts_index -= 1;
                    if let Some(card) = self.shorts_cards.get(self.shorts_index).cloned() {
                        self.play_card(&card);
                    }
                }
            }
            Intent::Navigate(route) => self.navigate(route),
            Intent::Back => self.back(),
            Intent::Forward => self.forward(),
            Intent::SubmitSearch(query) => {
                if !query.trim().is_empty() {
                    self.navigate(Route::Search {
                        query: query.trim().to_string(),
                        chip: None,
                    });
                }
            }
            Intent::Login => {
                self.show_login = true;
                self.login_error = None;
                crate::ui::login::spawn_login(self.login_tx.clone());
            }
            Intent::LoginWithCookies(text) => {
                let cookies = crate::ui::login::cookies_from_paste(&text);
                if cookies.signing_value().is_some() {
                    self.ctx.session.set_cookies(cookies);
                    self.ctx.store.cache_clear();
                    self.show_login = false;
                    self.login_paste.clear();
                    self.login_error = None;
                    self.toast("Sesión iniciada".into());
                    self.refresh_account();
                    let route = self.route.clone();
                    self.on_route_loaded(&route);
                } else {
                    self.login_error =
                        Some("No se encontró la cookie SAPISID en lo que pegaste.".into());
                }
            }
            Intent::Logout => {
                self.ctx.session.clear();
                self.ctx.store.cache_clear();
                self.account = None;
                self.show_login = false;
                self.toast("Sesión cerrada".into());
            }
            Intent::ToggleSidebar => {
                self.sidebar_open = !self.sidebar_open;
                self.ctx.store.set_sidebar_open(self.sidebar_open);
                // Los otros dos overlays se cierran: en la web, abrir la guía
                // cierra los menús de la cabecera.
                self.show_notifications = false;
                self.show_profile = false;
            }
            Intent::ToggleNotifications => {
                self.show_notifications = !self.show_notifications;
                self.show_profile = false;
                if self.show_notifications && self.notifications.is_empty() {
                    self.load_notifications();
                }
            }
            Intent::ToggleProfileMenu => {
                self.show_profile = !self.show_profile;
                self.show_notifications = false;
            }
            Intent::OpenExternal(url) => {
                if url.is_empty() {
                    self.toast("Esa función no está integrada todavía.".into());
                } else if let Err(e) = open_in_browser(&url) {
                    self.toast(format!("No se pudo abrir el navegador: {e}"));
                }
            }
            Intent::SetChannelTab(index) => self.channel_tab = index,
            Intent::ToggleDescription => self.description_open = !self.description_open,
            Intent::ToggleComments => self.comments_open = !self.comments_open,
            Intent::TogglePlaylistPanel => self.playlist_open = !self.playlist_open,
            Intent::PostComment(text) => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return;
                }
                if !self.ctx.session.is_logged_in() {
                    self.show_login = true;
                    self.toast("Inicia sesión para comentar.".into());
                    return;
                }
                let api = self.ctx.api.clone();
                let video_id = match &self.route {
                    Route::Watch { video_id } => video_id.clone(),
                    _ => String::new(),
                };
                let tx = self.comment_tx.clone();
                self.runtime.spawn(async move {
                    let result = api.post_comment(&video_id, &text).await;
                    let _ = tx.send(result);
                });
                self.comment_input.clear();
            }
            Intent::RateComment { comment_id, up } => {
                if !self.ctx.session.is_logged_in() {
                    self.show_login = true;
                    return;
                }
                let api = self.ctx.api.clone();
                self.runtime.spawn(async move {
                    if let Err(e) = api.rate_comment(&comment_id, up).await {
                        crate::ui::dbg_log(format!("valorar comentario: {e}"));
                    }
                });
            }
            Intent::LoadMore => {
                let token = {
                    let state = self.current_feed_mut();
                    state.feed.continuation.clone()
                };
                if let Some(token) = token {
                    let music = self.current_feed_mut().music;
                    let key = self.current_feed_mut().key.clone();
                    self.spawn(TaskKind::More { token, music, key });
                }
            }
            Intent::SetTheme(dark) => {
                self.dark = dark;
                self.ctx
                    .store
                    .set_setting("theme", if dark { "dark" } else { "light" });
                // Se aplica en el acto: el tema no espera a reabrir la ventana.
                crate::ui::theme::apply(ctx, dark);
            }
            Intent::SetMaxHeight(height) => {
                self.settings_max_height = height;
                match height {
                    Some(h) => self.ctx.store.set_max_height(h),
                    None => self.ctx.store.set_setting("max_height", "0"),
                }
            }
            Intent::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 150.0);
                self.ctx.store.set_volume(self.volume);
                if let Some(engine) = &self.engine {
                    engine.set_volume(self.volume as i32);
                }
            }
            Intent::ToggleShuffle => {
                let on = !self.queue.shuffle();
                self.queue.set_shuffle(on);
                self.toast(
                    if on {
                        "Aleatorio activado"
                    } else {
                        "Aleatorio desactivado"
                    }
                    .into(),
                );
            }
            Intent::CycleRepeat => {
                let mode = self.queue.repeat().cycle();
                self.queue.set_repeat(mode);
                self.ctx.store.set_repeat_mode(mode);
                self.toast(mode.label().to_string());
            }
            Intent::ClearHistory => {
                self.ctx.store.clear_history();
                self.toast("Historial local borrado".into());
            }
        }
    }

    /// Sube el último fotograma del vídeo a una textura.
    fn update_frame(&mut self, ctx: &egui::Context) {
        let Some(engine) = &self.engine else {
            return;
        };
        if let Some(frame) = engine.buffer().next_frame() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width, frame.height],
                &frame.pixels,
            );
            match &mut self.player.texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.player.texture =
                        Some(ctx.load_texture("video", image, egui::TextureOptions::LINEAR))
                }
            }
            ctx.request_repaint();
        }
        // Estado y posición: se consultan a VLC, no se suponen.
        self.player_state = engine.refresh_state();
        self.player.position_ms = engine.position_ms();
        let reported = engine.duration_ms();
        if reported > 0 {
            self.player.duration_ms = reported;
        }
        self.player.is_playing = self.player_state == PlayerState::Playing;

        // Fin de pista: siguiente de la cola.
        if self.player_state == PlayerState::Ended {
            self.next_track();
        }
    }
}

fn ctx_account_placeholder() -> Option<Account> {
    None
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.pump_tasks(&ctx);
        self.update_frame(&ctx);

        // Repintado periódico: el vídeo necesita fotogramas y las posiciones
        // cambian aunque no haya interacción.
        if self.route_is_watch_or_shorts() || self.player.is_playing {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        // El buscador y los atajos globales se recogen antes de pintar para que
        // las pantallas ya vean el estado nuevo.
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.egui_wants_keyboard_input() {
            self.handle(&ctx, Intent::TogglePlay);
        }

        // El préstamo de `thumbs` es mutable y las pantallas lo piden prestado a
        // la vez que prestan otras partes de `self`, así que se usa una copia
        // local solo del cache y el resto por referencia inmutable.
        let lookup_ctx = ctx.clone();
        let thumb_cache = self.thumbs.clone();
        let thumb_tx = self.thumb_tx.clone();
        let http = self.http.clone();
        let runtime = self.runtime.clone();
        let mut lookup =
            move |thumbs: &crate::domain::Thumbs, want: u32| -> Option<egui::TextureId> {
                thumb_cache
                    .lock()
                    .get_or_request(&lookup_ctx, thumbs, want, &thumb_tx, &http, &runtime)
                    .map(|t| t.id())
            };

        let mut intent = Intent::None;

        let p = palette(&ctx);

        // ------------------------------------------- cabecera, barra lateral
        //
        // La estructura global vive en `ui::shell` porque se pinta en todas las
        // pantallas: así una corrección de densidad se revisa contra una sola
        // captura y se aplica en todas partes.

        // La barra lateral se oculta sola cuando la ventana es estrecha, como
        // la web (por debajo de ~792 px pasa a iconos).
        let window_w = ctx.content_rect().width();
        let compact = !self.sidebar_open || window_w < metrics::SIDEBAR_HIDE_UNDER;

        let mut shell_intent = ShellIntent::None;
        let mut search_focused = false;

        egui::Panel::top("top_bar")
            .exact_size(metrics::TOP_BAR)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(palette(&ctx).bg)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| {
                let mut header_state = crate::ui::shell::HeaderState {
                    search_text: &mut self.search_input,
                    account: self.account.as_ref(),
                    unread: self.notifications.iter().filter(|n| n.unread).count(),
                    sidebar_open: !compact,
                    search_focused: &mut search_focused,
                };
                shell_intent = crate::ui::shell::header(ui, &mut header_state);
            });

        if compact {
            egui::Panel::left("rail")
                .exact_size(metrics::SIDEBAR_MINI)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::NONE
                        .fill(palette(&ctx).bg)
                        .inner_margin(egui::Margin::symmetric(4, 0)),
                )
                .show(ui, |ui| {
                    let i = crate::ui::shell::sidebar_rail(ui, &self.route);
                    if !matches!(i, ShellIntent::None) {
                        shell_intent = i;
                    }
                });
        } else {
            egui::Panel::left("sidebar")
                .exact_size(metrics::SIDEBAR)
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    egui::Frame::NONE
                        .fill(palette(&ctx).bg)
                        .inner_margin(egui::Margin::symmetric(12, 0)),
                )
                .show(ui, |ui| {
                    let mut input = crate::ui::shell::SidebarInput {
                        route: &self.route,
                        subscriptions: &self.subscriptions,
                        state: &mut self.sidebar,
                        logged_in: self.ctx.session.is_logged_in(),
                        account_name: self.account.as_ref().map(|a| a.name.as_str()).unwrap_or(""),
                    };
                    let i = crate::ui::shell::sidebar(ui, &mut input, &mut lookup);
                    if !matches!(i, ShellIntent::None) {
                        shell_intent = i;
                    }
                });
        }

        // Overlays de la cabecera: notificaciones y menú de perfil.
        if self.show_notifications {
            let (close, reload) = crate::ui::shell::notifications_panel(
                &ctx,
                &self.notifications,
                self.notifications_load.is_loading(),
                self.notifications_load.error(),
                self.ctx.session.is_logged_in(),
                &mut lookup,
            );
            if close {
                self.show_notifications = false;
            }
            if reload && !self.notifications_load.is_loading() {
                self.load_notifications();
            }
        }
        if self.show_profile {
            let (close, i) = crate::ui::shell::profile_menu(&ctx, self.account.as_ref(), self.dark);
            if close {
                self.show_profile = false;
            }
            if !matches!(i, ShellIntent::None) {
                shell_intent = i;
            }
        }
        if let Some((card, pos)) = self.card_menu.clone() {
            let (close, action) = crate::ui::shell::card_menu(&ctx, pos, &card);
            if close {
                self.card_menu = None;
            }
            match action {
                crate::ui::shell::CardMenuAction::Enqueue => {
                    self.handle(&ctx, Intent::Enqueue(card));
                }
                crate::ui::shell::CardMenuAction::WatchLater => {
                    self.toast("Ver más tarde requiere iniciar sesión".into());
                }
                crate::ui::shell::CardMenuAction::Share => {
                    self.toast("Enlace copiado: youtu.be/".to_string() + &card.id);
                }
                crate::ui::shell::CardMenuAction::NotInterested => {
                    self.toast("No volveré a mostrarte este vídeo".into());
                }
                crate::ui::shell::CardMenuAction::Report => {
                    self.toast("La denuncia se hace en youtube.com".into());
                }
                crate::ui::shell::CardMenuAction::None => {}
            }
        }

        // Traducción de la estructura global a intenciones de la aplicación.
        let shell_intent = match shell_intent {
            ShellIntent::None => Intent::None,
            ShellIntent::Navigate(route) => Intent::Navigate(route),
            ShellIntent::SubmitSearch(query) => Intent::SubmitSearch(query),
            ShellIntent::SearchChanged(_) => Intent::None,
            ShellIntent::ToggleSidebar => Intent::ToggleSidebar,
            ShellIntent::ShowNotifications => Intent::ToggleNotifications,
            ShellIntent::ShowProfileMenu => Intent::ToggleProfileMenu,
            ShellIntent::Login => Intent::Login,
            ShellIntent::Logout => Intent::Logout,
            ShellIntent::OpenSettings => Intent::Navigate(Route::Settings),
            ShellIntent::ShowMoreSubscriptions => Intent::None,
            ShellIntent::ShowMoreDestinations => Intent::None,
            ShellIntent::OpenExternal(url) => Intent::OpenExternal(url),
        };

        // ------------------------------------------------------ barra inferior
        if !self.queue.is_empty() || self.player.stream.is_some() {
            egui::Panel::bottom("player_bar")
                .exact_size(metrics::MINI_PLAYER)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(p.raised)
                        .inner_margin(egui::Margin::symmetric(12, 8)),
                )
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        // Miniatura y título de lo que suena.
                        let title = self
                            .queue
                            .current()
                            .map(|c| c.title.clone())
                            .unwrap_or_else(|| "Nada sonando".into());
                        let author = self
                            .queue
                            .current()
                            .map(|c| c.author.clone())
                            .unwrap_or_default();
                        let thumbs = self
                            .queue
                            .current()
                            .map(|c| c.thumbs.clone())
                            .unwrap_or_default();
                        let (thumb_rect, _) =
                            ui.allocate_exact_size(Vec2::new(88.0, 50.0), Sense::hover());
                        if let Some(id) = lookup(&thumbs, 88) {
                            let shape = egui::epaint::RectShape::filled(
                                thumb_rect,
                                egui::CornerRadius::same(6),
                                Color32::WHITE,
                            )
                            .with_texture(
                                id,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                            );
                            ui.painter().add(shape);
                        } else {
                            ui.painter().rect_filled(thumb_rect, 6, p.skeleton);
                        }
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).size(13.0).strong().color(p.text));
                            ui.label(RichText::new(author).size(11.0).color(p.text2));
                        });

                        ui.add_space(16.0);
                        if icons_mod::icon_button(ui, Icon::Previous, 18.0, false, "Anterior")
                            .clicked()
                        {
                            intent = Intent::PreviousTrack;
                        }
                        let playing = self.player.is_playing;
                        if icons_mod::icon_button(
                            ui,
                            if playing { Icon::Pause } else { Icon::Play },
                            22.0,
                            false,
                            "Reproducir / Pausar",
                        )
                        .clicked()
                        {
                            intent = Intent::TogglePlay;
                        }
                        if icons_mod::icon_button(ui, Icon::Next, 18.0, false, "Siguiente")
                            .clicked()
                        {
                            intent = Intent::NextTrack;
                        }
                        if icons_mod::icon_button(
                            ui,
                            Icon::Shuffle,
                            16.0,
                            self.queue.shuffle(),
                            "Aleatorio",
                        )
                        .clicked()
                        {
                            intent = Intent::ToggleShuffle;
                        }
                        let repeat_icon = if self.queue.repeat() == RepeatMode::One {
                            Icon::RepeatOne
                        } else {
                            Icon::Repeat
                        };
                        if icons_mod::icon_button(
                            ui,
                            repeat_icon,
                            16.0,
                            self.queue.repeat() != RepeatMode::Off,
                            self.queue.repeat().label(),
                        )
                        .clicked()
                        {
                            intent = Intent::CycleRepeat;
                        }

                        ui.add_space(12.0);
                        let ratio = if self.player.duration_ms > 0 {
                            (self.player.position_ms as f32 / self.player.duration_ms as f32)
                                .clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        ui.label(
                            RichText::new(screens::watch::format_time(self.player.position_ms))
                                .size(11.0)
                                .color(p.text2),
                        );
                        let bar = widgets::progress_bar_ext(ui, ratio, 3.0, None);
                        if bar.clicked() || bar.dragged() {
                            if let Some(fraction) = widgets::pointer_ratio(&bar) {
                                intent = Intent::SeekFraction(fraction);
                            }
                        }
                        ui.label(
                            RichText::new(screens::watch::format_time(self.player.duration_ms))
                                .size(11.0)
                                .color(p.text2),
                        );

                        ui.add_space(8.0);
                        let mut volume = self.volume;
                        if ui
                            .add_sized(
                                Vec2::new(90.0, 18.0),
                                egui::Slider::new(&mut volume, 0.0..=100.0).show_value(false),
                            )
                            .changed()
                        {
                            intent = Intent::SetVolume(volume);
                        }
                        if icons_mod::icon_button(ui, Icon::Queue, 18.0, self.queue_open, "Cola")
                            .clicked()
                        {
                            intent = Intent::ToggleQueue;
                        }
                    });
                });
        }

        // ------------------------------------------------------- contenido
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.bg).inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 10,
                bottom: 10,
            }))
            .show(ui, |ui| {
                let screen_intent = match &self.route {
                    Route::Home => screens::home(
                        ui,
                        &self.home.feed,
                        &self.home.load,
                        &self.home_chip,
                        &self.home_shorts,
                        &mut lookup,
                    ),
                    Route::Search { query, .. } => screens::search(
                        ui,
                        query,
                        &self.search.feed,
                        &self.search.load,
                        self.search_filter,
                        &mut lookup,
                    ),
                    Route::MusicSearch { query } => screens::search(
                        ui,
                        query,
                        &self.search.feed,
                        &self.search.load,
                        self.search_filter,
                        &mut lookup,
                    ),
                    Route::Watch { video_id } => {
                        let _ = video_id;
                        let placeholder = WatchInfo::default();
                        let info = self.watch_info.as_ref().unwrap_or(&placeholder);
                        let mut input = crate::ui::screens::watch::WatchInput {
                            info,
                            load: &self.watch_load,
                            player: &self.player,
                            queue_len: self.queue.len(),
                            playlist: &self.playlist_panel,
                            playlist_title: self.playlist_panel_title.as_deref(),
                            playlist_open: self.playlist_open,
                            comments: &self.comments,
                            comments_load: &self.comments_load,
                            comments_open: self.comments_open,
                            comments_input: &mut self.comment_input,
                            description_open: self.description_open,
                        };
                        match crate::ui::screens::watch::watch(ui, &mut input, &mut lookup) {
                            crate::ui::screens::watch::WatchIntent::Action(i) => Some(i),
                            crate::ui::screens::watch::WatchIntent::None => None,
                        }
                    }
                    Route::Shorts { .. } => {
                        let mut input = crate::ui::screens::shorts::ShortsInput {
                            cards: &self.shorts_cards,
                            index: self.shorts_index,
                            load: &self.shorts_load,
                            player: &self.player,
                            comments: &self.comments,
                            comments_load: &self.comments_load,
                            comments_open: self.comments_open,
                            comments_input: &mut self.comment_input,
                            liked: self
                                .watch_info
                                .as_ref()
                                .map(|w| w.like_state == crate::domain::LikeState::Like)
                                .unwrap_or(false),
                            subscribed: self
                                .watch_info
                                .as_ref()
                                .map(|w| w.subscribed)
                                .unwrap_or(false),
                        };
                        crate::ui::screens::shorts::shorts(ui, &mut input, &mut lookup)
                    }
                    Route::MusicHome => {
                        screens::music_home(ui, &self.music.feed, &self.music.load, &mut lookup)
                    }
                    Route::Album { browse_id } => screens::album(
                        ui,
                        browse_id,
                        &self.browse.feed,
                        &self.browse.load,
                        &mut lookup,
                    ),
                    Route::Artist { browse_id } => screens::artist(
                        ui,
                        browse_id,
                        &self.browse.feed,
                        &self.browse.load,
                        &mut lookup,
                    ),
                    Route::Playlist { browse_id } => screens::playlist(
                        ui,
                        browse_id,
                        &self.browse.feed,
                        &self.browse.load,
                        &mut lookup,
                    ),
                    Route::Channel { browse_id } => {
                        let info = self.watch_info.as_ref().map(|w| &w.channel);
                        // El canal es "propio" cuando el `browse_id` es el del
                        // usuario (`me`) o cuando coincide con el canal de la
                        // cuenta cargada: cambia las acciones (Personalizar en
                        // lugar de Suscribirse).
                        let own = browse_id == "me"
                            || self
                                .account
                                .as_ref()
                                .and_then(|a| a.channel_id.clone())
                                .map(|id| id == *browse_id)
                                .unwrap_or(false);
                        screens::channel(
                            ui,
                            browse_id,
                            info,
                            &self.channel_state.feed,
                            &self.channel_state.load,
                            self.channel_tab,
                            self.watch_info
                                .as_ref()
                                .map(|w| w.subscribed)
                                .unwrap_or(false),
                            own,
                            &mut lookup,
                        )
                    }
                    Route::Library => screens::library(
                        ui,
                        &self.library_state.feed,
                        &self.library_state.load,
                        &mut lookup,
                    ),
                    Route::Subscriptions => screens::subscriptions(
                        ui,
                        &self.browse.feed,
                        &self.library_state.load,
                        &mut lookup,
                    ),
                    Route::History => screens::history(
                        ui,
                        &self.library_state.feed,
                        &self.library_state.load,
                        &mut lookup,
                    ),
                    Route::Settings => screens::settings(
                        ui,
                        self.settings_max_height,
                        self.volume,
                        self.dark,
                        &self.thumbs_bytes(),
                        self.ctx.store.cache_size(),
                        &self.ctx.paths.root.to_string_lossy(),
                        &crate::playback::engine::backend_info(),
                        self.ctx.session.is_logged_in(),
                        &self
                            .account
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_default(),
                        &mut self.login_paste,
                        self.login_error.as_deref(),
                    ),
                };
                if let Some(screen_intent) = screen_intent {
                    intent = screen_intent;
                }
            });

        // ------------------------------------------------- avisos y login
        if let Some((message, at)) = self.toast.clone() {
            if at.elapsed() < Duration::from_secs(4) {
                egui::Area::new(egui::Id::unique("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -100.0))
                    .show(&ctx, |ui| {
                        egui::Frame::NONE
                            .fill(Color32::from_rgba_unmultiplied(40, 40, 40, 240))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(RichText::new(message).size(13.0).color(Color32::WHITE));
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }

        if self.show_login {
            let mut open = self.show_login;
            egui::Window::new("Iniciar sesión")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(520.0)
                .show(&ctx, |ui| {
                    let p = palette(&ctx);
                    ui.label(
                        RichText::new(
                            "NeoTube usa tu sesión real de YouTube (no OAuth) porque las \
                             recomendaciones y la biblioteca solo existen dentro de la web.",
                        )
                        .size(13.0)
                        .color(p.text2),
                    );
                    ui.add_space(10.0);
                    if widgets::pill_button(ui, "Abrir ventana de Google", true, false).clicked() {
                        intent = Intent::Login;
                    }
                    if neotube2_webview_available() {
                        ui.label(
                            RichText::new("La ventana se cierra sola al terminar.")
                                .size(11.0)
                                .color(p.text3),
                        );
                    } else {
                        ui.label(
                            RichText::new(
                                "Si no se abre ninguna ventana, pega abajo la cabecera Cookie: \
                                 de una sesión de youtube.com.",
                            )
                            .size(11.0)
                            .color(p.text3),
                        );
                    }
                    ui.add_space(12.0);
                    widgets::divider(ui);
                    ui.add_space(8.0);
                    ui.label(RichText::new("O pega las cookies").size(14.0).strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut self.login_paste)
                            .hint_text("SAPISID=…; VISITOR_INFO1_LIVE=…; …")
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    if let Some(error) = &self.login_error {
                        ui.label(RichText::new(error).size(12.0).color(p.brand));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::pill_button(ui, "Usar estas cookies", false, false).clicked() {
                            intent = Intent::LoginWithCookies(self.login_paste.clone());
                        }
                        if self.ctx.session.is_logged_in()
                            && widgets::pill_button(ui, "Cerrar sesión", false, false).clicked()
                        {
                            intent = Intent::Logout;
                        }
                    });
                });
            self.show_login = open;
        }

        self.handle(&ctx, intent);
    }
}

impl App {
    fn route_is_watch_or_shorts(&self) -> bool {
        matches!(self.route, Route::Watch { .. } | Route::Shorts { .. })
    }

    fn thumbs_bytes(&self) -> String {
        format!("{:.1} MB", self.thumbs.lock().bytes() as f32 / 1_048_576.0)
    }
}

/// ¿Hay motor de WebView en esta compilación?
/// Abre una URL en el navegador del sistema.
///
/// Se usa para lo que **no** es interfaz propia y no se puede hacer de forma
/// nativa sin una integración que hoy no existe: subir vídeo, gestionar la
/// cuenta de Google, comprar membresías. Abrir el navegador es honesto; un
/// botón que no hace nada, no.
fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

fn neotube2_webview_available() -> bool {
    cfg!(feature = "webview-login")
}
