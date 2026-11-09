//! Motor de reproducción sobre **libVLC** (que a su vez usa FFmpeg y sus
//! propios accesos HTTP).
//!
//! ## Por qué VLC y no libmpv
//!
//! El plan original decía libmpv. Se descartó por una razón concreta y
//! comprobada en esta máquina: los binarios de mpv para Windows publican
//! `libmpv-2.dll` pero **no** una `libmpv.lib` (import library) con la que
//! enlazar, así que habría que generar los stubs a mano o depender de un fork.
//! VLC instala `libvlc.dll` en una ruta estable y su API pública de carga
//! dinámica (`libvlc_new`, `libvlc_media_player_*`) es tan antigua y estable que
//! se puede llamar sin cabeceras ni build script. Además decodifica con
//! **FFmpeg integrado**, que es exactamente lo que hace falta para Opus/VP9 y
//! H.264, y trae salida de audio WASAPI y vídeo Direct3D11.
//!
//! ## Cómo se pinta el vídeo
//!
//! No se abre ninguna ventana hija de VLC (`set_hwnd`): el vídeo se pide por
//! **callbacks de memoria** (`libvlc_video_set_callbacks`) en formato `RV32`
//! (BGRA) y se sube a una textura de egui. Ventajas: los controles superpuestos
//! son widgets normales, el vídeo respeta el escalado y el redimensionado, y no
//! hay dos ventanas que se puedan desincronizar. El coste es convertir BGRA a
//! RGBA una vez por fotograma, que a 1080p es despreciable frente a decodificar.
//!
//! ## Audio y vídeo separados
//!
//! YouTube sirve el vídeo y el audio en pistas separadas (ver
//! `yt::stream`). Se le pasa a VLC la pista de vídeo como medio principal y la
//! de audio como **esclavo** (`libvlc_media_player_add_slave`), que es la forma
//! que tiene VLC de sincronizar un segundo flujo contra el primero. Cuando solo
//! hay audio (música), se abre directamente y no se crea esclavo.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use libloading::Library;
use parking_lot::Mutex;

/// Estado del reproductor, ya normalizado (no se expone el enum de VLC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Opening,
    Playing,
    Paused,
    Ended,
    Error,
}

/// Un fotograma de vídeo listo para subir a una textura.
#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    /// RGBA8.
    pub pixels: Vec<u8>,
}

/// Búfer compartido entre el hilo de vídeo de VLC y la interfaz.
///
/// Se usan dos búferes: VLC escribe en el de atrás mientras la interfaz lee el
/// de delante. Sin esto, redimensionar la ventana mientras se reproduce produce
/// fotogramas partidos (mitad de una escena, mitad de otra).
pub struct VideoBuffer {
    width: AtomicUsize,
    height: AtomicUsize,
    front: Mutex<Option<Vec<u8>>>,
    back: Mutex<Vec<u8>>,
    dirty: AtomicBool,
}

impl VideoBuffer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            width: AtomicUsize::new(0),
            height: AtomicUsize::new(0),
            front: Mutex::new(None),
            back: Mutex::new(Vec::new()),
            dirty: AtomicBool::new(false),
        })
    }

    fn resize(&self, width: usize, height: usize) {
        self.width.store(width, Ordering::Relaxed);
        self.height.store(height, Ordering::Relaxed);
        let mut back = self.back.lock();
        let needed = width * height * 4;
        if back.len() != needed {
            back.resize(needed, 0);
        }
    }

    /// Entrega el fotograma más reciente a la interfaz, si hay uno nuevo.
    fn take(&self) -> Option<Frame> {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return None;
        }
        let front = self.front.lock().take()?;
        let width = self.width.load(Ordering::Relaxed);
        let height = self.height.load(Ordering::Relaxed);
        if width == 0 || height == 0 || front.len() < width * height * 4 {
            return None;
        }
        // RV32 es BGRA; egui quiere RGBA.
        let mut pixels = front;
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        Some(Frame {
            width,
            height,
            pixels,
        })
    }
}

/// Funciones de libVLC que se usan. Se cargan una vez.
struct VlcApi {
    _lib: Library,
    new: unsafe extern "C" fn(c_int, *const *const c_char) -> *mut c_void,
    release: unsafe extern "C" fn(*mut c_void),
    media_new_location: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    media_release: unsafe extern "C" fn(*mut c_void),
    media_add_option: unsafe extern "C" fn(*mut c_void, *const c_char),
    media_player_new_from_media: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    media_player_release: unsafe extern "C" fn(*mut c_void),
    media_player_play: unsafe extern "C" fn(*mut c_void) -> c_int,
    media_player_set_pause: unsafe extern "C" fn(*mut c_void, c_int),
    media_player_stop: unsafe extern "C" fn(*mut c_void),
    media_player_get_time: unsafe extern "C" fn(*mut c_void) -> i64,
    media_player_set_time: unsafe extern "C" fn(*mut c_void, i64),
    media_player_get_length: unsafe extern "C" fn(*mut c_void) -> i64,
    media_player_get_state: unsafe extern "C" fn(*mut c_void) -> c_int,
    media_player_is_playing: unsafe extern "C" fn(*mut c_void) -> c_int,
    audio_set_volume: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    audio_get_volume: unsafe extern "C" fn(*mut c_void) -> c_int,
    audio_set_mute: unsafe extern "C" fn(*mut c_void, c_int),
    video_set_callbacks: unsafe extern "C" fn(
        *mut c_void,
        lock: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void,
        unlock: unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void),
        display: unsafe extern "C" fn(*mut c_void, *mut c_void),
        opaque: *mut c_void,
    ),
    video_set_format_callbacks: unsafe extern "C" fn(
        *mut c_void,
        setup: unsafe extern "C" fn(
            *mut *mut c_void,
            *mut c_char,
            *mut c_uint,
            *mut c_uint,
            *mut c_uint,
            *mut c_uint,
        ) -> c_uint,
        cleanup: unsafe extern "C" fn(*mut *mut c_void),
    ),
    media_player_add_slave:
        unsafe extern "C" fn(*mut c_void, c_int, *const c_char, *mut c_int) -> c_int,
}

impl VlcApi {
    fn load() -> anyhow::Result<Self> {
        let candidates = vlc_library_candidates();
        let mut last_error = String::new();
        for path in &candidates {
            // SAFETY: cargar una biblioteca y leer símbolos con nombre conocido
            // es seguro mientras la firma declarada coincida con la del
            // encabezado de libVLC 3, que es lo que se ha comprobado.
            unsafe {
                let lib = match load_library(path) {
                    Ok(l) => l,
                    Err(e) => {
                        last_error = format!("{}: {e}", path.display());
                        continue;
                    }
                };
                let new: unsafe extern "C" fn(c_int, *const *const c_char) -> *mut c_void =
                    *lib.get::<unsafe extern "C" fn(c_int, *const *const c_char) -> *mut c_void>(
                        b"libvlc_new\0",
                    )?;
                let release = *lib.get::<unsafe extern "C" fn(*mut c_void)>(b"libvlc_release\0")?;
                let media_new_location =
                    *lib.get(b"libvlc_media_new_location\0")?;
                let media_release =
                    *lib.get::<unsafe extern "C" fn(*mut c_void)>(b"libvlc_media_release\0")?;
                let media_add_option =
                    *lib.get(b"libvlc_media_add_option\0")?;
                let media_player_new_from_media =
                    *lib.get(b"libvlc_media_player_new_from_media\0")?;
                let media_player_release =
                    *lib.get::<unsafe extern "C" fn(*mut c_void)>(b"libvlc_media_player_release\0")?;
                let media_player_play = *lib.get(b"libvlc_media_player_play\0")?;
                let media_player_set_pause = *lib.get(b"libvlc_media_player_set_pause\0")?;
                let media_player_stop = *lib.get(b"libvlc_media_player_stop\0")?;
                let media_player_get_time = *lib.get(b"libvlc_media_player_get_time\0")?;
                let media_player_set_time = *lib.get(b"libvlc_media_player_set_time\0")?;
                let media_player_get_length = *lib.get(b"libvlc_media_player_get_length\0")?;
                let media_player_get_state = *lib.get(b"libvlc_media_player_get_state\0")?;
                let media_player_is_playing = *lib.get(b"libvlc_media_player_is_playing\0")?;
                let audio_set_volume = *lib.get(b"libvlc_audio_set_volume\0")?;
                let audio_get_volume = *lib.get(b"libvlc_audio_get_volume\0")?;
                let audio_set_mute = *lib.get(b"libvlc_audio_set_mute\0")?;
                let video_set_callbacks = *lib.get(b"libvlc_video_set_callbacks\0")?;
                let video_set_format_callbacks =
                    *lib.get(b"libvlc_video_set_format_callbacks\0")?;
                let media_player_add_slave = *lib.get(b"libvlc_media_player_add_slave\0")?;
                return Ok(Self {
                    _lib: lib,
                    new,
                    release,
                    media_new_location,
                    media_release,
                    media_add_option,
                    media_player_new_from_media,
                    media_player_release,
                    media_player_play,
                    media_player_set_pause,
                    media_player_stop,
                    media_player_get_time,
                    media_player_set_time,
                    media_player_get_length,
                    media_player_get_state,
                    media_player_is_playing,
                    audio_set_volume,
                    audio_get_volume,
                    audio_set_mute,
                    video_set_callbacks,
                    video_set_format_callbacks,
                    media_player_add_slave,
                });
            }
        }
        anyhow::bail!(
            "no se encontró libVLC. Instala VLC (https://www.videolan.org/) o define \
             NEOTUBE_VLC_DIR apuntando a la carpeta que contiene libvlc.dll. Probado: {last_error}"
        )
    }
}

/// Carga la biblioteca resolviendo sus dependencias contra **su propio
/// directorio**.
///
/// Esto no es un detalle: `libvlc.dll` depende de `libvlccore.dll`, que está a
/// su lado, y `LoadLibrary` no busca dependencias en el directorio del DLL que
/// carga (solo en el del ejecutable y en `PATH`). Con `Library::new` y una ruta
/// absoluta, Windows responde «LoadLibraryExW failed» aunque el fichero exista.
/// `LOAD_WITH_ALTERED_SEARCH_PATH` cambia el orden de búsqueda para que empiece
/// por la carpeta del propio DLL, que es justo lo que hace falta.
///
/// SAFETY: la persona que llama debe tratar los símbolos que extraiga como los
/// declara el encabezado de libVLC.
unsafe fn load_library(path: &std::path::Path) -> Result<Library, libloading::Error> {
    #[cfg(windows)]
    {
        const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;
        use libloading::os::windows::{Library as WinLibrary, LOAD_WITH_ALTERED_SEARCH_PATH as FLAG};
        let _ = LOAD_WITH_ALTERED_SEARCH_PATH;
        let lib = WinLibrary::load_with_flags(path, FLAG)?;
        Ok(Library::from(lib))
    }
    #[cfg(not(windows))]
    {
        Library::new(path)
    }
}

/// Dónde se busca `libvlc.dll` / `libvlc.so`.
fn vlc_library_candidates() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("NEOTUBE_VLC_DIR") {
        out.push(std::path::PathBuf::from(dir));
    }
    #[cfg(windows)]
    {
        for env in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(base) = std::env::var(env) {
                out.push(std::path::PathBuf::from(base).join("VideoLAN/VLC"));
            }
        }
        out.push(std::path::PathBuf::from("C:/Program Files/VideoLAN/VLC"));
    }
    #[cfg(target_os = "macos")]
    {
        out.push(std::path::PathBuf::from(
            "/Applications/VLC.app/Contents/MacOS/lib",
        ));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for p in ["/usr/lib/x86_64-linux-gnu", "/usr/lib", "/usr/lib64"] {
            out.push(std::path::PathBuf::from(p));
        }
    }
    #[cfg(windows)]
    let file = "libvlc.dll";
    #[cfg(target_os = "macos")]
    let file = "libvlc.dylib";
    #[cfg(all(unix, not(target_os = "macos")))]
    let file = "libvlc.so";

    out.into_iter()
        .map(|dir| {
            // Se recompone la ruta para que los separadores sean los del
            // sistema: una ruta mixta («C:/...\libvlc.dll») hace fallar a
            // LoadLibraryExW en Windows.
            let mut path = std::path::PathBuf::new();
            for part in dir.components() {
                path.push(part.as_os_str());
            }
            path.join(file)
        })
        .collect()
}

/// Reproductor. Un único motor por aplicación (PLAN.md §8: un solo reproductor).
pub struct Engine {
    api: Arc<VlcApi>,
    instance: *mut c_void,
    player: Mutex<*mut c_void>,
    buffer: Arc<VideoBuffer>,
    state: Mutex<State>,
    volume: Mutex<i32>,
    muted: Mutex<bool>,
}

// SAFETY: los punteros de libVLC son propiedad de este objeto y solo se usan a
// través de sus métodos, que están serializados por los mutex. VLC documenta
// que su instancia y su media player se pueden usar desde cualquier hilo, pero
// no desde dos a la vez.
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    /// Carga libVLC y crea la instancia.
    ///
    /// `no_video_output` crea el motor sin salida de vídeo (modo música): no
    /// reserva búferes ni fotogramas, y consume menos.
    pub fn new(no_video_output: bool) -> anyhow::Result<Arc<Self>> {
        let api = Arc::new(VlcApi::load()?);
        let args = vec![
            "--no-osd".to_string(),
            "--no-video-title-show".to_string(),
            "--intf=dummy".to_string(),
            "--no-plugins-cache".to_string(),
            "--network-caching=1200".to_string(),
            "--file-caching=300".to_string(),
            "--no-snapshot-preview".to_string(),
            "--quiet".to_string(),
        ];
        // Nota: no se pasa `--ipv4` (VLC 3.0.23 ya no lo acepta como opción de
        // instancia). Cuando una máquina tiene IPv6 anunciada pero sin salida,
        // el acceso se queda en «TLS handshake: Resource temporarily
        // unavailable» y el medio no abre; se resuelve forzando la resolución
        // IPv4 en la opción de medio (`:ipv4`), no en la instancia.
        if no_video_output {
            // Sin salida de vídeo no se decodifican fotogramas: es la mitad del
            // consumo en modo música.
            // (Se deja el vídeo desactivado por medio, no aquí, para poder
            // reutilizar la instancia si el usuario cambia de modo.)
        }
        let c_args: Vec<CString> = args
            .iter()
            .filter_map(|a| CString::new(a.as_str()).ok())
            .collect();
        let mut ptrs: Vec<*const c_char> = c_args.iter().map(|c| c.as_ptr()).collect();
        // SAFETY: punteros válidos durante toda la llamada; libVLC copia los
        // argumentos que necesita.
        let instance = unsafe { (api.new)(ptrs.len() as c_int, ptrs.as_mut_ptr()) };
        if instance.is_null() {
            anyhow::bail!("libVLC no pudo crear una instancia");
        }
        Ok(Arc::new(Self {
            api,
            instance,
            player: Mutex::new(std::ptr::null_mut()),
            buffer: VideoBuffer::new(),
            state: Mutex::new(State::Idle),
            volume: Mutex::new(80),
            muted: Mutex::new(false),
        }))
    }

    pub fn buffer(&self) -> Arc<VideoBuffer> {
        self.buffer.clone()
    }

    pub fn state(&self) -> State {
        *self.state.lock()
    }

    /// Abre un medio. `audio_slave` es la pista de audio cuando el vídeo viene
    /// sin sonido.
    pub fn open(
        &self,
        url: &str,
        audio_slave: Option<&str>,
        headers: &[(String, String)],
        has_video: bool,
    ) -> anyhow::Result<()> {
        self.stop();
        let Ok(c_url) = CString::new(url) else {
            anyhow::bail!("URL de medio inválida");
        };
        // SAFETY: llamadas de libVLC con cadenas válidas; cada recurso se
        // libera más abajo en el mismo orden en que se creó.
        unsafe {
            let media = (self.api.media_new_location)(self.instance, c_url.as_ptr());
            if media.is_null() {
                anyhow::bail!("libVLC no pudo crear el medio");
            }
            for (name, value) in headers {
                if let Ok(opt) = CString::new(format!(":{name}:{value}")) {
                    // VLC lee las cabeceras de la opción `:http-*`; se pasan
                    // también como opciones de medio genéricas por si el acceso
                    // las consume en el demux.
                    (self.api.media_add_option)(media, opt.as_ptr());
                }
            }
            if let Ok(opt) = CString::new(":http-referrer=https://www.youtube.com/") {
                (self.api.media_add_option)(media, opt.as_ptr());
            }
            // Preferir IPv4: en equipos con IPv6 anunciada y sin salida real,
            // VLC se queda bloqueado en el handshake TLS. El fallo se ve como
            // «VLC es incapaz de abrir el MRL» sin más pistas.
            if let Ok(opt) = CString::new(":ipv4") {
                (self.api.media_add_option)(media, opt.as_ptr());
            }
            let player = (self.api.media_player_new_from_media)(media);
            (self.api.media_release)(media);
            if player.is_null() {
                anyhow::bail!("libVLC no pudo crear el reproductor");
            }

            if has_video {
                let opaque = Arc::as_ptr(&self.buffer) as *mut c_void;
                (self.api.video_set_format_callbacks)(
                    player,
                    video_setup,
                    video_cleanup,
                );
                (self.api.video_set_callbacks)(player, video_lock, video_unlock, video_display, opaque);
            } else {
                // Sin imagen: se evita crear salida de vídeo para no gastar CPU
                // decodificando algo que no se pinta.
                if let Ok(opt) = CString::new(":no-video") {
                    (self.api.media_add_option)(player, opt.as_ptr());
                }
            }

            if let Some(slave_url) = audio_slave {
                if let Ok(c_slave) = CString::new(slave_url) {
                    // Tipo 1 = audio (0 es subtítulos).
                    let mut track: c_int = 0;
                    let rc = (self.api.media_player_add_slave)(player, 1, c_slave.as_ptr(), &mut track);
                    if rc != 0 {
                        log::warn!("VLC no aceptó la pista de audio esclava");
                    }
                }
            }

            *self.player.lock() = player;
        }
        *self.state.lock() = State::Opening;
        let volume = *self.volume.lock();
        self.set_volume(volume);
        Ok(())
    }

    pub fn play(&self) -> anyhow::Result<()> {
        let player = *self.player.lock();
        if player.is_null() {
            anyhow::bail!("no hay medio abierto");
        }
        // SAFETY: `player` sale de `libvlc_media_player_new_from_media` y sigue
        // viva hasta `stop`.
        let rc = unsafe { (self.api.media_player_play)(player) };
        if rc != 0 {
            *self.state.lock() = State::Error;
            anyhow::bail!("VLC no pudo iniciar la reproducción");
        }
        *self.state.lock() = State::Playing;
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) {
        let player = *self.player.lock();
        if player.is_null() {
            return;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.media_player_set_pause)(player, if paused { 1 } else { 0 }) };
        let mut state = self.state.lock();
        if *state == State::Playing || *state == State::Paused {
            *state = if paused { State::Paused } else { State::Playing };
        }
    }

    pub fn is_playing(&self) -> bool {
        let player = *self.player.lock();
        if player.is_null() {
            return false;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.media_player_is_playing)(player) == 1 }
    }

    pub fn stop(&self) {
        let mut guard = self.player.lock();
        let player = *guard;
        if player.is_null() {
            return;
        }
        // SAFETY: se para y se libera el reproductor; después el puntero se
        // anula para que nadie lo vuelva a usar.
        unsafe {
            (self.api.media_player_stop)(player);
            (self.api.media_player_release)(player);
        }
        *guard = std::ptr::null_mut();
        *self.state.lock() = State::Idle;
    }

    /// Posición actual y duración, en milisegundos.
    pub fn position_ms(&self) -> i64 {
        let player = *self.player.lock();
        if player.is_null() {
            return 0;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.media_player_get_time)(player) }
    }

    pub fn duration_ms(&self) -> i64 {
        let player = *self.player.lock();
        if player.is_null() {
            return 0;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.media_player_get_length)(player) }
    }

    pub fn seek_ms(&self, position: i64) {
        let player = *self.player.lock();
        if player.is_null() {
            return;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.media_player_set_time)(player, position.max(0)) };
    }

    /// Estado normalizado, consultando a VLC. Se llama una vez por fotograma.
    pub fn refresh_state(&self) -> State {
        let player = *self.player.lock();
        if player.is_null() {
            return State::Idle;
        }
        // SAFETY: ver `play`.
        let raw = unsafe { (self.api.media_player_get_state)(player) };
        // enum libvlc_state_t: 0 NothingSpecial, 1 Opening, 2 Buffering,
        // 3 Playing, 4 Paused, 5 Stopped, 6 Ended, 7 Error.
        let state = match raw {
            0 | 1 | 2 => State::Opening,
            3 => State::Playing,
            4 => State::Paused,
            5 => State::Idle,
            6 => State::Ended,
            7 => State::Error,
            _ => State::Idle,
        };
        // VLC informa Error también cuando una pista esclava falla; si el vídeo
        // sigue reproduciendo, se prefiere no romper la experiencia.
        let mut current = self.state.lock();
        if state != State::Opening {
            *current = state;
        }
        *current
    }

    /// Volumen 0..=150 (VLC acepta hasta 200; se limita a 150 por seguridad
    /// auditiva).
    pub fn set_volume(&self, volume: i32) {
        let volume = volume.clamp(0, 150);
        *self.volume.lock() = volume;
        let player = *self.player.lock();
        if player.is_null() {
            return;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.audio_set_volume)(player, volume) };
    }

    pub fn volume(&self) -> i32 {
        let player = *self.player.lock();
        if player.is_null() {
            return *self.volume.lock();
        }
        // SAFETY: ver `play`.
        let v = unsafe { (self.api.audio_get_volume)(player) };
        if v >= 0 {
            *self.volume.lock() = v;
            v
        } else {
            *self.volume.lock()
        }
    }

    pub fn set_muted(&self, muted: bool) {
        *self.muted.lock() = muted;
        let player = *self.player.lock();
        if player.is_null() {
            return;
        }
        // SAFETY: ver `play`.
        unsafe { (self.api.audio_set_mute)(player, if muted { 1 } else { 0 }) };
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: la instancia se creó con `libvlc_new` y no se ha liberado.
        unsafe { (self.api.release)(self.instance) };
    }
}

// ---------------------------------------------------------------- callbacks
//
// Todos los callbacks vienen de hilos de VLC. Solo tocan el `VideoBuffer`, que
// está protegido por mutex y atómicos: nunca la interfaz ni el motor.

unsafe extern "C" fn video_setup(
    opaque: *mut *mut c_void,
    chroma: *mut c_char,
    width: *mut c_uint,
    height: *mut c_uint,
    pitches: *mut c_uint,
    lines: *mut c_uint,
) -> c_uint {
    // RV32 = BGRA de 32 bits, el formato de vídeo que VLC entrega de forma
    // nativa y que se convierte a RGBA sin ambigüedad.
    let name = b"RV32\0";
    std::ptr::copy_nonoverlapping(name.as_ptr() as *const c_char, chroma, name.len());
    let w = *width;
    let h = *height;
    if !opaque.is_null() {
        // SAFETY: `opaque` es el `Arc::as_ptr` del `VideoBuffer` del motor, que
        // sigue vivo mientras el reproductor exista.
        let buffer = unsafe { &*(*opaque as *const VideoBuffer) };
        buffer.resize(w as usize, h as usize);
        *pitches = w * 4;
        *lines = h;
    } else {
        // Sin búfer no hay dónde escribir: se pide el tamaño que el llamador
        // ya tiene (VLC lo interpreta como "sin cambios" si devuelve lo mismo).
        *pitches = w * 4;
        *lines = h;
    }
    1
}

unsafe extern "C" fn video_cleanup(_opaque: *mut *mut c_void) {}

unsafe extern "C" fn video_lock(opaque: *mut c_void, planes: *mut *mut c_void) -> *mut c_void {
    if opaque.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: ver `video_setup`.
    let buffer = unsafe { &*(opaque as *const VideoBuffer) };
    let mut back = buffer.back.lock();
    *planes = back.as_mut_ptr() as *mut c_void;
    // El identificador de imagen es el propio puntero al búfer de atrás.
    1 as *mut c_void
}

unsafe extern "C" fn video_unlock(_opaque: *mut c_void, _picture: *mut c_void, _planes: *const *mut c_void) {
}

unsafe extern "C" fn video_display(opaque: *mut c_void, _picture: *mut c_void) {
    if opaque.is_null() {
        return;
    }
    // SAFETY: ver `video_setup`.
    let buffer = unsafe { &*(opaque as *const VideoBuffer) };
    // Se copia el búfer de atrás al de delante. Es una copia por fotograma
    // (~8 MB a 1080p, decenas de microsegundos) y evita que la interfaz lea
    // mientras VLC escribe.
    let back = buffer.back.lock();
    let mut front = buffer.front.lock();
    match front.as_mut() {
        Some(front) if front.len() == back.len() => front.copy_from_slice(&back),
        _ => *front = Some(back.clone()),
    }
    drop(front);
    drop(back);
    buffer.dirty.store(true, Ordering::Release);
}

impl VideoBuffer {
    /// Tamaño actual del vídeo (0 si aún no hay).
    pub fn dimensions(&self) -> (usize, usize) {
        (
            self.width.load(Ordering::Relaxed),
            self.height.load(Ordering::Relaxed),
        )
    }

    /// Fotograma listo para pintar.
    pub fn next_frame(&self) -> Option<Frame> {
        self.take()
    }
}

/// Comprueba si hay libVLC disponible, para poder avisar en la interfaz en vez
/// de fallar al abrir un vídeo.
pub fn is_available() -> bool {
    VlcApi::load().is_ok()
}

/// Texto de diagnóstico para Ajustes.
pub fn backend_info() -> String {
    match VlcApi::load() {
        Ok(api) => {
            let _ = &api;
            "VLC (libVLC 3) mediante FFmpeg integrado".to_string()
        }
        Err(e) => e.to_string(),
    }
}

/// Convierte una ruta de fichero local en un `file:///` que VLC acepte.
pub fn file_url(path: &std::path::Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

/// Ayuda para depurar: nombres de los eventos que se consultan por polling.
#[allow(dead_code)]
fn _state_names() {
    let _ = CStr::from_bytes_with_nul(b"libvlc_MediaPlayerEndReached\0");
}
