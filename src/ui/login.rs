//! Inicio de sesión: captura de la sesión del navegador.
//!
//! ## Por qué una WebView y no leer las cookies del navegador
//!
//! Se probaron las dos vías antes de elegir:
//!
//! | Vía | Resultado |
//! |---|---|
//! | Leer `Cookies` de Chrome/Edge desde disco | **No funciona.** Desde Chrome 127 las cookies se cifran con la clave `app_bound_encrypted_key` (`APPB`), ligada al binario firmado del navegador y al TPM. No se descifran desde fuera ni con DPAPI. Además el fichero está bloqueado mientras el navegador corre. |
//! | Leer `cookies.sqlite` de Firefox | Funcionaba, pero aquí no hay Firefox instalado y dependía del perfil del usuario. |
//! | **WebView2 propia** | **Sí.** El motor es nuestro, el perfil es nuestro (`%APPDATA%\NeoTube2\webview2`) y `WebView::cookies()` devuelve la lista **completa**, incluidas las `HttpOnly` como `SAPISID`, que son exactamente las que firman las peticiones. |
//!
//! Así que el login abre una ventana real de Google, deja que el usuario entre,
//! y **cuando la navegación llega a `music.youtube.com`** se queda con las
//! cookies de esa sesión y cierra. Es el mismo enfoque que NeoTube 1.x, pero sin
//! el parche de WebView2 que necesitaba su fork.
//!
//! ## Lo que no se hace
//!
//! - No se navega con la WebView: se abre **solo** para el login y se destruye
//!   al terminar (PLAN.md §6). Las pantallas siguen siendo nativas.
//! - No se copian cookies de `.google.com`. `session.rs` las filtra a
//!   `youtube.com` porque mezclarlas duplica nombres en la cabecera `Cookie:` y
//!   YouTube contesta 400.
//! - No se registra el valor de ninguna cookie (PLAN.md §6).

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::session::Cookies;

/// Mensaje que envía la ventana de login al terminar.
#[derive(Debug)]
pub enum LoginOutcome {
    /// Cookies capturadas de una sesión real: `(nombre, valor, dominio)`.
    Cookies(Vec<(String, String, String)>),
    /// El usuario cerró la ventana antes de entrar.
    Cancelled,
    /// Error de entorno o de red.
    Failed(String),
}

/// Abre la ventana de login en un hilo aparte.
///
/// La WebView necesita su propio bucle de eventos; hacerlo en el hilo de egui
/// congelaría la interfaz mientras el usuario escribe su contraseña.
pub fn spawn_login(sender: Sender<LoginOutcome>) {
    std::thread::spawn(move || {
        let outcome = run_webview_login();
        let _ = sender.send(outcome);
    });
}

/// URL de entrada del login.
///
/// Es la misma que usa InnerTune: entrar ya por `accounts.google.com` con
/// `ltmpl=music&service=youtube` evita que sea YouTube quien construya la
/// redirección al login, que es donde aparecía el «400: solicitud mal
/// formada».
pub const LOGIN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube\
&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue\
%26next%3Dhttps%3A%2F%2Fmusic.youtube.com%252F";

/// A dónde tiene que llegar la navegación para dar el login por terminado.
const SUCCESS_PREFIX: &str = "https://music.youtube.com";

/// Tiempo máximo que se espera al usuario.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Cada cuánto se comprueba la ubicación como respaldo del aviso de navegación.
const POLL: Duration = Duration::from_millis(400);

/// User-Agent de un Chrome real.
///
/// El de WebView2 por defecto lleva `Edg/` y delata el motor embebido; Google
/// puede servir una pantalla de login distinta o rechazarla. Se fija uno de
/// escritorio corriente.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[cfg(feature = "webview-login")]
fn run_webview_login() -> LoginOutcome {
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::WindowId;

    /// Ventana de login y su WebView.
    struct LoginWindow {
        tx: Option<Sender<LoginOutcome>>,
        window: Option<winit::window::Window>,
        webview: Option<wry::WebView>,
        /// Última URL vista, puesta por el aviso de navegación. Se usa desde el
        /// bucle para pedir las cookies cuando el propio WebView ya existe: el
        /// aviso se construye *antes* que la WebView, así que no puede
        /// llamarla directamente.
        current_url: Arc<Mutex<Option<String>>>,
        started: Instant,
        finished: bool,
    }

    impl ApplicationHandler for LoginWindow {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let attrs = winit::window::Window::default_attributes()
                .with_title("Iniciar sesión en YouTube · NeoTube")
                .with_inner_size(winit::dpi::LogicalSize::new(520.0, 760.0));
            let window = match event_loop.create_window(attrs) {
                Ok(w) => w,
                Err(e) => {
                    self.finish(LoginOutcome::Failed(format!(
                        "no se pudo crear la ventana: {e}"
                    )));
                    event_loop.exit();
                    return;
                }
            };

            // Perfil propio en disco: la sesión sobrevive al cierre (no hay que
            // volver a entrar cada vez) y no se toca el perfil del navegador
            // del usuario. Tiene que ser escribible: el valor por defecto del
            // paquete falla si la app está instalada en Program Files.
            let data_dir = directories::ProjectDirs::from("xyz", "NeoTube", "NeoTube2")
                .map(|d| d.data_dir().join("webview2"))
                .unwrap_or_else(|| std::path::PathBuf::from("webview2"));
            if let Err(e) = std::fs::create_dir_all(&data_dir) {
                self.finish(LoginOutcome::Failed(format!(
                    "no se pudo crear el perfil de la WebView: {e}"
                )));
                event_loop.exit();
                return;
            }

            let url_slot = self.current_url.clone();
            let mut context = wry::WebContext::new(Some(data_dir));
            let builder = wry::WebViewBuilder::new_with_web_context(&mut context)
                .with_url(LOGIN_URL)
                .with_user_agent(USER_AGENT)
                .with_navigation_handler(move |url: String| {
                    // Se acepta la navegación y se anota la URL. La captura se
                    // hace desde el bucle, no aquí.
                    *url_slot.lock().unwrap() = Some(url);
                    true
                });

            match builder.build(&window) {
                Ok(webview) => {
                    self.webview = Some(webview);
                    self.window = Some(window);
                }
                Err(e) => {
                    self.finish(LoginOutcome::Failed(format!(
                        "no se pudo crear la WebView (¿falta el runtime de WebView2?): {e}"
                    )));
                    event_loop.exit();
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _id: WindowId,
            event: WindowEvent,
        ) {
            if let WindowEvent::CloseRequested = event {
                self.finish(LoginOutcome::Cancelled);
                event_loop.exit();
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if self.finished {
                event_loop.exit();
                return;
            }

            // Respaldo: si el aviso de navegación no llega (ocurre cuando Google
            // redirige dentro de un iframe o con historial ya existente), se
            // sondea la URL de la WebView.
            // La comprobación se hace antes de pedir prestada la WebView: si se
            // captura dentro del `if let`, `self` queda prestado en inmutable y
            // no se puede llamar a un método `&mut self`.
            let arrived = self
                .current_url
                .lock()
                .unwrap()
                .as_deref()
                .map(|url| url.starts_with(SUCCESS_PREFIX))
                .unwrap_or(false);
            if arrived {
                // Se saca la WebView del campo para liberar el préstamo antes de
                // capturar y cerrar.
                if let Some(webview) = self.webview.take() {
                    self.capture_and_finish(&webview, event_loop);
                    return;
                }
            }

            if self.started.elapsed() > LOGIN_TIMEOUT {
                self.finish(LoginOutcome::Failed(
                    "Se agotó el tiempo esperando el login.".into(),
                ));
                event_loop.exit();
                return;
            }

            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL));
        }
    }

    impl LoginWindow {
        fn finish(&mut self, outcome: LoginOutcome) {
            if self.finished {
                return;
            }
            self.finished = true;
            if let Some(tx) = self.tx.take() {
                let _ = tx.send(outcome);
            }
            // La WebView se destruye antes que la ventana, que es el orden que
            // espera WebView2.
            self.webview = None;
            self.window = None;
        }

        fn capture_and_finish(
            &mut self,
            webview: &wry::WebView,
            event_loop: &ActiveEventLoop,
        ) {
            match webview.cookies() {
                Ok(list) => {
                    let cookies: Vec<(String, String, String)> = list
                        .into_iter()
                        .map(|c| {
                            (
                                c.name().to_string(),
                                c.value().to_string(),
                                c.domain().unwrap_or("").to_string(),
                            )
                        })
                        .collect();
                    self.finish(LoginOutcome::Cookies(cookies));
                }
                Err(e) => {
                    self.finish(LoginOutcome::Failed(format!(
                        "no se pudieron leer las cookies de la sesión: {e}"
                    )));
                }
            }
            event_loop.exit();
        }
    }

    // El bucle de eventos tiene que construirse aquí, y aquí es un hilo
    // aparte (egui se queda el principal). winit lo prohíbe por defecto porque
    // en macOS es un problema real; en Windows se permite de forma explícita y
    // es la única manera de no congelar la interfaz mientras el usuario
    // escribe su contraseña.
    #[cfg(windows)]
    let event_loop = {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        let mut builder = EventLoop::builder();
        builder.with_any_thread(true);
        match builder.build() {
            Ok(el) => el,
            Err(e) => {
                return LoginOutcome::Failed(format!("no se pudo crear el bucle de eventos: {e}"))
            }
        }
    };
    #[cfg(not(windows))]
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            return LoginOutcome::Failed(format!("no se pudo crear el bucle de eventos: {e}"))
        }
    };
    // El canal se queda dentro del manejador; el resultado sale por él.
    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = LoginWindow {
        tx: Some(tx),
        window: None,
        webview: None,
        current_url: Arc::new(Mutex::new(None)),
        started: Instant::now(),
        finished: false,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        return LoginOutcome::Failed(format!("el bucle de eventos falló: {e}"));
    }
    rx.try_recv().unwrap_or(LoginOutcome::Cancelled)
}

#[cfg(not(feature = "webview-login"))]
fn run_webview_login() -> LoginOutcome {
    LoginOutcome::Failed(
        "Esta compilación no incluye el motor de WebView. Recompila con \
         `--features webview-login`, o pega la cabecera Cookie: de tu navegador aquí abajo."
            .into(),
    )
}

/// Interpreta la cabecera pegada a mano (`a=1; b=2`) o un `cookies.txt` de
/// extensión (formato Netscape).
pub fn cookies_from_paste(text: &str) -> Cookies {
    Cookies::from_header(text)
}

/// Lector de cookies de un navegador **del sistema**, para el modo sin WebView.
///
/// Solo se intenta con los que guardan las cookies sin cifrado ligado al
/// binario:
///
/// - **Firefox** (`cookies.sqlite`): en claro. Es el caso que cubre esto.
/// - **Chrome/Edge**: desde la versión 127 usan `app_bound_encrypted_key`
///   (`APPB`), que **no se puede descifrar** fuera del navegador. Se detecta y
///   se explica, en vez de devolver una lista vacía sin más.
///
/// Si no hay ninguno utilizable, la interfaz ofrece pegar la cabecera.
pub fn browser_login_cookies() -> Result<Vec<(String, String, String)>, String> {
    // 1. Firefox: el único que se puede leer sin descifrar nada.
    if let Some(cookies) = read_firefox_cookies() {
        return Ok(cookies);
    }
    // 2. Chromium: se comprueba si es descifrable y se informa con precisión.
    match chromium_status() {
        ChromiumStatus::AppBound => Err(
            "Tu navegador guarda las cookies cifradas con una clave ligada al propio \
             navegador (cifrado «app-bound»), que ninguna otra aplicación puede descifrar. \
             Usa la ventana de login de NeoTube (compilado con --features webview-login) o \
             pega la cabecera Cookie: a mano."
                .into(),
        ),
        ChromiumStatus::NotFound | ChromiumStatus::NoYoutubeCookies => Err(
            "No hay cookies de YouTube legibles en este equipo. Pega la cabecera Cookie: \
             de una sesión de youtube.com, o usa la ventana de login."
                .into(),
        ),
    }
}

/// Estado del almacén de cookies de Chromium/Edge.
enum ChromiumStatus {
    /// Cifrado ligado al navegador: no descifrable desde fuera.
    AppBound,
    /// Hay perfil pero sin cookies de YouTube.
    NoYoutubeCookies,
    /// No hay perfil de Chromium instalado.
    NotFound,
}

fn chromium_status() -> ChromiumStatus {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        for name in ["Google/Chrome/User Data", "Microsoft/Edge/User Data"] {
            roots.push(std::path::PathBuf::from(&local).join(name));
        }
    }
    let mut found_profile = false;
    for root in roots {
        let local_state = root.join("Local State");
        if let Ok(text) = std::fs::read_to_string(&local_state) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if json
                    .get("os_crypt")
                    .and_then(|o| o.get("app_bound_encrypted_key"))
                    .is_some()
                {
                    return ChromiumStatus::AppBound;
                }
            }
        }
        if root.exists() {
            found_profile = true;
        }
    }
    if found_profile {
        ChromiumStatus::NoYoutubeCookies
    } else {
        ChromiumStatus::NotFound
    }
}

/// Lee `moz_cookies` del perfil de Firefox que tenga sesión de YouTube.
fn read_firefox_cookies() -> Option<Vec<(String, String, String)>> {
    let mut bases: Vec<std::path::PathBuf> = Vec::new();
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        bases.push(std::path::PathBuf::from(&appdata).join("Mozilla/Firefox/Profiles"));
    }
    #[cfg(unix)]
    if let Some(dirs) = directories::BaseDirs::new() {
        bases.push(dirs.home_dir().join(".mozilla/firefox"));
    }

    for base in bases {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let db = entry.path().join("cookies.sqlite");
            if !db.exists() {
                continue;
            }
            if let Ok(cookies) = read_firefox_db(&db) {
                // Solo vale si trae la cookie que firma.
                let parsed = Cookies::from_raw(cookies.clone());
                if parsed.signing_value().is_some() {
                    return Some(cookies);
                }
            }
        }
    }
    None
}

/// Copia el `cookies.sqlite` antes de abrirlo: el navegador lo bloquea si está
/// abierto, y una base bloqueada no se puede leer ni en solo lectura.
fn read_firefox_db(path: &std::path::Path) -> Result<Vec<(String, String, String)>, String> {
    let temp = std::env::temp_dir().join("neotube-firefox-cookies.sqlite");
    std::fs::copy(path, &temp).map_err(|e| e.to_string())?;
    let conn = rusqlite::Connection::open_with_flags(
        &temp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT name, value, host FROM moz_cookies \
             WHERE host LIKE '%youtube.com' OR host LIKE '%google.com'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let out: Vec<_> = rows.flatten().collect();
    let _ = std::fs::remove_file(&temp);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pegar_cabecera_extrae_sapisid() {
        let cookies = cookies_from_paste("SAPISID=abc123; VISITOR_INFO1_LIVE=xyz; PREF=hl=es");
        assert_eq!(cookies.signing_value(), Some("abc123"));
    }

    #[test]
    fn pegar_formato_netscape() {
        // domain \t flag \t path \t secure \t expiry \t name \t value
        let text = ".youtube.com\tTRUE\t/\tTRUE\t1800000000\tSAPISID\tsapisid-value\n\
                    .google.com\tTRUE\t/\tTRUE\t1800000000\tSAPISID\totro";
        let cookies = cookies_from_paste(text);
        assert_eq!(cookies.signing_value(), Some("sapisid-value"));
        assert_eq!(cookies.len(), 1, "las de google.com no entran");
    }

    #[test]
    fn la_url_de_login_apunta_al_final_correcto() {
        // Si se cambia la URL, el aviso de navegación dejaría de disparar la
        // captura y el login se quedaría esperando hasta agotar el tiempo.
        assert!(LOGIN_URL.contains("music.youtube.com"));
        assert!(SUCCESS_PREFIX.starts_with("https://music.youtube.com"));
    }
}
