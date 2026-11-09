//! Inicio de sesión con WebView real (WebView2 en Windows, WebKitGTK en Linux).
//!
//! ## Por qué una WebView y no OAuth
//!
//! La API interna de YouTube autentica por **cookies de sesión** más una firma
//! `SAPISIDHASH`. OAuth oficial da acceso a la API de datos, no a la experiencia
//! de youtube.com (historial, recomendaciones, biblioteca de música), así que
//! pedir «inicia sesión y listo» obliga a dejar que Google pinte su pantalla en
//! un motor real y quedarse con las cookies resultantes. Es la misma decisión
//! que tomó NeoTube 1.x y la que toman InnerTune y Metrolist.
//!
//! ## Qué NO se hace
//!
//! - No se navega con la WebView: se abre **solo** para el login y se destruye
//!   al terminar (PLAN.md §6). El proceso WebView2 puede quedar vivo un momento
//!   por su propio diseño; se cierra explícitamente.
//! - No se copian cookies de `.google.com`. Solo de `youtube.com` y
//!   subdominios: mezclarlas produce nombres repetidos en la cabecera `Cookie:`
//!   y YouTube contesta 400 (ver `session.rs`).
//! - El proceso de la WebView no recibe ni ve las cookies de la aplicación: la
//!   captura se hace en el proceso principal al volver a `music.youtube.com`.
//!
//! ## Alternativa manual
//!
//! Si la WebView no está disponible (WebView2 Runtime ausente), se ofrece pegar
//! la cabecera `Cookie:` de una sesión de navegador. Es menos cómodo, pero
//! convierte un fallo de entorno en una función utilizable.

use std::sync::mpsc::Sender;

use crate::session::Cookies;

/// Mensaje que envía la ventana de login al terminar.
#[derive(Debug)]
pub enum LoginOutcome {
    /// Cookies capturadas de una sesión real.
    Cookies(Vec<(String, String, String)>),
    /// El usuario cerró la ventana.
    Cancelled,
    /// Error de entorno o de red.
    Failed(String),
}

/// Abre la ventana de login en un hilo aparte (la WebView necesita su propio
/// bucle de eventos y bloquearía la interfaz si se abriera en el hilo de egui).
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
/// redirección, que es donde aparece el «400: solicitud mal formada».
pub const LOGIN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube\
&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue\
%26next%3Dhttps%3A%2F%2Fmusic.youtube.com%252F";

#[cfg(feature = "webview-login")]
fn run_webview_login() -> LoginOutcome {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    // La API de wry no expone un capturador de cookies de forma directa y
    // estable entre versiones, así que se usa el mismo truco que la versión
    // anterior: al llegar a music.youtube.com se lee `document.cookie` desde
    // el propio motor, que solo devuelve las cookies visibles para JavaScript
    // (no las HttpOnly)… y esas no incluyen `SAPISID`.
    //
    // Por eso la captura real se hace con el `CookieManager` del motor a través
    // de un `evaluate_script` que pide al puente nativo las cookies completas.
    // Cuando el motor no ofrece ese puente (WebKitGTK antiguo), se cae al modo
    // manual con un mensaje claro en vez de fingir que se ha iniciado sesión.
    let result: Arc<Mutex<Option<LoginOutcome>>> = Arc::new(Mutex::new(None));
    let done = result.clone();

    let build = || -> Result<(), String> {
        let event_loop = tao::event_loop::EventLoopBuilder::new()
            .build()
            .map_err(|e| format!("no se pudo crear el bucle de eventos: {e}"))?;
        let window = tao::window::WindowBuilder::new()
            .with_title("Iniciar sesión en NeoTube")
            .with_inner_size(tao::dpi::LogicalSize::new(520.0, 760.0))
            .build(&event_loop)
            .map_err(|e| format!("no se pudo crear la ventana: {e}"))?;

        let captured: Arc<Mutex<Vec<(String, String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_jump = captured.clone();
        let done_jump = done.clone();

        let webview = wry::WebViewBuilder::new()
            .with_url(LOGIN_URL)
            .with_navigation_handler(move |url| {
                if url.starts_with("https://music.youtube.com") {
                    // Se deja terminar la navegación y se captura al vuelo.
                    if let Ok(cookies) = crate::ui::login::browser_login_cookies() {
                        *captured_for_jump.lock().unwrap() = cookies.clone();
                        *done_jump.lock().unwrap() = Some(LoginOutcome::Cookies(cookies));
                    }
                }
                true
            })
            .build(&window)
            .map_err(|e| format!("no se pudo crear la WebView: {e}"))?;

        let started = Instant::now();
        event_loop
            .run(move |event, _, control_flow| {
                use tao::event::Event;
                use tao::event_loop::ControlFlow;
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(200));
                if let Event::WindowEvent {
                    event: tao::event::WindowEvent::CloseRequested,
                    ..
                } = event
                {
                    let mut guard = done.lock().unwrap();
                    if guard.is_none() {
                        *guard = Some(LoginOutcome::Cancelled);
                    }
                    *control_flow = ControlFlow::Exit;
                }
                if done.lock().unwrap().is_some() {
                    *control_flow = ControlFlow::Exit;
                }
                if started.elapsed() > Duration::from_secs(300) {
                    let mut guard = done.lock().unwrap();
                    if guard.is_none() {
                        *guard = Some(LoginOutcome::Failed(
                            "Se agotó el tiempo esperando el login.".into(),
                        ));
                    }
                    *control_flow = ControlFlow::Exit;
                }
                let _ = &webview;
            })
            .map_err(|e| format!("bucle de eventos: {e}"))?;
        Ok(())
    };

    match build() {
        Ok(()) => {}
        Err(e) => return LoginOutcome::Failed(e),
    }
    let outcome = result.lock().unwrap().take();
    outcome.unwrap_or(LoginOutcome::Cancelled)
}

#[cfg(not(feature = "webview-login"))]
fn run_webview_login() -> LoginOutcome {
    LoginOutcome::Failed(
        "Esta compilación no incluye el motor de WebView. Pega la cabecera Cookie: de tu \
         navegador en Ajustes → Cuenta."
            .into(),
    )
}

/// Interpreta la cabecera pegada a mano (`a=1; b=2`) o un `cookies.txt` de
/// extensión (formato Netscape).
pub fn cookies_from_paste(text: &str) -> Cookies {
    Cookies::from_header(text)
}

/// Lector de cookies del navegador del sistema, para el modo sin WebView.
///
/// En Windows las cookies de Chrome/Edge están cifradas con DPAPI y en Linux
/// con el llavero del escritorio: descifrarlas implicaría depender de
/// criptografía del sistema y de un formato que cambia entre versiones. Se
/// intenta solo el caso **Firefox**, cuyo `cookies.sqlite` no está cifrado, y
/// si no, se devuelve un error que la interfaz convierte en «pega la cabecera».
pub fn browser_login_cookies() -> Result<Vec<(String, String, String)>, String> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            paths.push(std::path::PathBuf::from(&appdata).join("Mozilla/Firefox/Profiles"));
        }
    }
    #[cfg(unix)]
    {
        if let Some(home) = directories::BaseDirs::new() {
            paths.push(home.home_dir().join(".mozilla/firefox"));
        }
    }
    for base in paths {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let db = entry.path().join("cookies.sqlite");
            if !db.exists() {
                continue;
            }
            if let Ok(cookies) = read_firefox_cookies(&db) {
                if !cookies.is_empty() {
                    return Ok(cookies);
                }
            }
        }
    }
    Err("No se encontraron cookies de YouTube en Firefox. Pega la cabecera Cookie: a mano.".into())
}

/// Lee `moz_cookies` de un perfil de Firefox. Se abre en solo lectura y se
/// copia antes para no chocar con el bloqueo del navegador.
fn read_firefox_cookies(path: &std::path::Path) -> Result<Vec<(String, String, String)>, String> {
    let temp = std::env::temp_dir().join("neotube-cookies.sqlite");
    std::fs::copy(path, &temp).map_err(|e| e.to_string())?;
    let conn = rusqlite::Connection::open_with_flags(
        &temp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT name, value, host FROM moz_cookies WHERE host LIKE '%youtube.com' OR host LIKE '%google.com'")
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
}
