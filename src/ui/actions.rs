//! Tareas asíncronas y su resultado.
//!
//! La interfaz nunca espera a la red: pide una tarea, se queda pintando el
//! estado de carga y recibe el resultado como mensaje. Cada tarea lleva dos
//! números de generación para poder **descartar respuestas obsoletas**: si el
//! usuario cambia de pantalla mientras carga, lo que llega tarde se tira
//! (PLAN.md §5: «cancelar tareas al abandonar pantallas y descartar respuestas
//! obsoletas»).

use crate::domain::{Account, Card, Feed, ResolvedStream, WatchInfo};

/// Qué se ha pedido. Sirve para las trazas y para decidir qué hacer con el
/// resultado.
#[derive(Debug, Clone)]
pub enum TaskKind {
    Home,
    Search { query: String, music: bool, params: Option<String> },
    Watch { video_id: String },
    Resolve { video_id: String, max_height: Option<u32> },
    Shorts { seed: String },
    MusicHome,
    Browse { browse_id: String, music: bool },
    Library,
    History,
    More { token: String, music: bool, key: String },
    Account,
    Like { video_id: String, on: bool },
    Subscribe { channel_id: String, on: bool },
    Login,
}

/// Lo que devuelve una tarea terminada.
#[derive(Debug)]
pub enum Payload {
    /// Feed paginado. `key` es la clave de caché y `append` indica si se añade
    /// al feed ya visible (paginación) o lo sustituye.
    Feed {
        key: String,
        feed: Feed,
        append: bool,
        music: bool,
    },
    Watch(Box<WatchInfo>),
    Stream(Box<ResolvedStream>),
    /// Cola de YouTube (radios) o lista de Shorts.
    Cards(Vec<Card>),
    Account(Option<Account>),
    /// Acción de cuenta completada, con el estado resultante.
    Done { what: &'static str, ok: bool },
}

/// Una tarea terminada, con su identidad.
pub struct TaskDone {
    pub id: u64,
    pub kind: TaskKind,
    pub result: Result<Payload, String>,
}

impl TaskDone {
    /// ¿La tarea era de esta pantalla?
    pub fn matches(&self, want: &TaskKind) -> bool {
        std::mem::discriminant(&self.kind) == std::mem::discriminant(want)
    }
}

/// Genera identificadores de tarea.
#[derive(Default)]
pub struct TaskIds {
    next: u64,
}

impl TaskIds {
    pub fn take(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

/// Estados de carga posibles de una sección de la interfaz.
#[derive(Debug, Clone, PartialEq)]
pub enum Load {
    /// Todavía no se ha pedido nada.
    Idle,
    Loading,
    Ready,
    Failed(String),
}

impl Load {
    pub fn is_loading(&self) -> bool {
        matches!(self, Load::Loading)
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Load::Failed(e) => Some(e),
            _ => None,
        }
    }
}

impl Default for Load {
    fn default() -> Self {
        Load::Idle
    }
}
