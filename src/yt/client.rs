//! Cliente HTTP de la API interna de YouTube (`youtubei/v1`).
//!
//! Aquí solo vive el **transporte**: qué cliente dice ser cada petición, qué
//! cabeceras lleva, cómo se firma y cómo se trata un fallo. La interpretación
//! del JSON está en [`crate::yt::parse`] y las pantallas en [`crate::yt::api`].
//!
//! Tres perfiles de cliente, cada uno para lo suyo (PORTING_CONTRACT):
//!
//! | Perfil | Host | Para qué |
//! |---|---|---|
//! | `WEB` | www.youtube.com | Inicio, búsqueda, canales, comentarios, acciones de cuenta |
//! | `WEB_REMIX` | music.youtube.com | Catálogo de YouTube Music |
//! | `ANDROID_VR` | www.youtube.com | **Solo** resolver streams: devuelve `url` directas sin cifrar |
//!
//! `ANDROID_VR` se usa siempre **anónimo** y con `visitorData`: con cookies la
//! URL llega envenenada y googlevideo contesta 403 al reproductor.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};

use crate::session::SessionStore;

/// Error de una llamada a la API, con la clase de fallo separada para poder
/// decidir si merece reintento, si hay que pedir sesión o si no hay nada que
/// hacer (PLAN.md §5: nada de botones que aparenten funcionar).
#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("sin conexión con YouTube: {0}")]
    Network(String),
    #[error("YouTube respondió {status}")]
    Http { status: u16, body: String },
    #[error("respuesta ilegible: {0}")]
    Parse(String),
    #[error("hace falta iniciar sesión")]
    NeedsLogin,
    #[error("YouTube pide confirmar que no eres un robot")]
    BotCheck,
    #[error("{0}")]
    Unavailable(String),
}

impl ApiError {
    /// ¿Merece la pena reintentar tal cual?
    pub fn is_retryable(&self) -> bool {
        match self {
            ApiError::Network(_) => true,
            ApiError::Http { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }

    /// Mensaje corto para pintar en la interfaz.
    pub fn user_message(&self) -> String {
        match self {
            ApiError::Network(_) => "No hay conexión con YouTube. Comprueba la red.".into(),
            ApiError::Http { status: 401 | 403, .. } => {
                "YouTube rechazó la petición. Vuelve a iniciar sesión.".into()
            }
            ApiError::Http { status: 429, .. } => {
                "YouTube está limitando las peticiones. Espera un momento.".into()
            }
            ApiError::Http { status, .. } => format!("YouTube respondió con un error {status}."),
            ApiError::Parse(_) => "La respuesta de YouTube no tiene el formato esperado.".into(),
            ApiError::NeedsLogin => "Necesitas iniciar sesión para esto.".into(),
            ApiError::BotCheck => {
                "YouTube pide confirmar que no eres un robot. Reintenta o resuelve la pista desde el navegador."
                    .into()
            }
            ApiError::Unavailable(m) => m.clone(),
        }
    }
}

/// Idioma y país de las peticiones. Se derivan del sistema para que los títulos
/// de sección lleguen ya traducidos por Google.
#[derive(Debug, Clone)]
pub struct Locale {
    pub hl: String,
    pub gl: String,
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            hl: "es".to_string(),
            gl: "ES".to_string(),
        }
    }
}

impl Locale {
    /// Del locale del sistema (`es-ES`, `en_US.UTF-8`…) al par `hl`/`gl`.
    pub fn from_system() -> Self {
        let raw = sys_locale::get_locale().unwrap_or_else(|| "es-ES".to_string());
        let mut parts = raw.split(['-', '_', '.']);
        let lang = parts.next().unwrap_or("es").to_ascii_lowercase();
        let region = parts
            .next()
            .map(|r| r.to_ascii_uppercase())
            .filter(|r| r.len() == 2)
            .unwrap_or_else(|| "US".to_string());
        Self {
            hl: if lang.is_empty() { "es".into() } else { lang },
            gl: region,
        }
    }
}

/// `visitorData` cacheado, con su hora.
#[derive(Debug, Default)]
struct VisitorCache {
    value: Option<String>,
    at: Option<Instant>,
}

/// Cuánto se reutiliza una identidad de visitante. Se renueva también cuando el
/// reproductor rechaza una URL (ver `ANDROID_VR` y PORTING_CONTRACT).
const VISITOR_TTL: Duration = Duration::from_secs(30 * 60);

const HOST_YOUTUBE: &str = "https://www.youtube.com";
const HOST_MUSIC: &str = "https://music.youtube.com";

/// Versión del cliente web. Si YouTube empieza a contestar 400 a todo a la vez,
/// esto es lo primero que hay que actualizar (ver docs/API_PROBE.md).
pub const WEB_CLIENT_VERSION: &str = "2.20250101.00.00";
pub const REMIX_CLIENT_VERSION: &str = "1.20241201.01.00";

/// Identidad del cliente `ANDROID_VR`, copiada campo a campo de lo que manda
/// yt-dlp. Con una descripción "parecida" la API contesta aviso de bot.
pub const VR_CLIENT_VERSION: &str = "1.65.10";
pub const VR_USER_AGENT: &str = "com.google.android.apps.youtube.vr.oculus/1.65.10 \
(Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A001) gzip";
pub const VR_CLIENT_NAME_NUMBER: &str = "28";

/// Identidad del cliente `VISIONOS`, el que sirve el fichero entero (ver
/// `yt::stream`).
pub const VISIONOS_CLIENT_VERSION: &str = "1.02";
pub const VISIONOS_CLIENT_NUMBER: &str = "101";
pub const VISIONOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

/// Cabeceras con las que hay que **abrir** la URL resuelta, no solo pedirla.
pub fn playback_headers() -> Vec<(String, String)> {
    playback_headers_for(PlaybackClient::VisionOs)
}

/// Qué cliente emitió la URL. Las cabeceras tienen que ser **las del cliente que
/// la pidió**: googlevideo valida el `User-Agent` contra el `c=` de la URL, y
/// abrir una URL de Vision Pro con el UA de Oculus es el mismo fallo que el
/// cliente `IOS` de la versión anterior (se descarga por HTTP y el reproductor
/// recibe 403).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackClient {
    VisionOs,
    AndroidVr,
}

pub fn playback_headers_for(client: PlaybackClient) -> Vec<(String, String)> {
    let ua = match client {
        PlaybackClient::VisionOs => VISIONOS_USER_AGENT,
        PlaybackClient::AndroidVr => VR_USER_AGENT,
    };
    vec![
        ("User-Agent".into(), ua.to_string()),
        ("Referer".into(), "https://www.youtube.com/".into()),
        ("Origin".into(), "https://www.youtube.com".into()),
    ]
}

/// Cliente de la API interna.
#[derive(Clone)]
pub struct Innertube {
    pub http: reqwest::Client,
    session: Arc<SessionStore>,
    pub locale: Locale,
    visitor: Arc<Mutex<VisitorCache>>,
}

impl Innertube {
    pub fn new(session: Arc<SessionStore>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(crate::yt::BROWSER_USER_AGENT)
            .build()?;
        Ok(Self {
            http,
            session,
            locale: Locale::from_system(),
            visitor: Arc::new(Mutex::new(VisitorCache::default())),
        })
    }

    pub fn session(&self) -> &Arc<SessionStore> {
        &self.session
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.is_logged_in()
    }

    /// Contexto `WEB`.
    fn context_web(&self) -> Value {
        json!({
            "client": {
                "clientName": "WEB",
                "clientVersion": WEB_CLIENT_VERSION,
                "hl": self.locale.hl,
                "gl": self.locale.gl,
                "userAgent": crate::yt::BROWSER_USER_AGENT,
                "originalUrl": format!("{HOST_YOUTUBE}/"),
                "platform": "DESKTOP",
            },
            "user": {"lockedSafetyMode": false},
        })
    }

    /// Contexto `WEB_REMIX`.
    fn context_remix(&self) -> Value {
        json!({
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": REMIX_CLIENT_VERSION,
                "hl": self.locale.hl,
                "gl": self.locale.gl,
                "originalUrl": format!("{HOST_MUSIC}/"),
            },
            "user": {"lockedSafetyMode": false},
        })
    }

    fn auth_headers(&self, origin: &str) -> Vec<(String, String)> {
        if !self.session.is_logged_in() {
            return Vec::new();
        }
        let cookies = self.session.cookies();
        let mut headers = vec![
            ("Cookie".to_string(), cookies.header()),
            ("X-Goog-AuthUser".to_string(), "0".to_string()),
            ("X-Origin".to_string(), origin.to_string()),
            ("Origin".to_string(), origin.to_string()),
        ];
        if let Some(auth) = cookies.authorization(origin) {
            headers.push(("Authorization".to_string(), auth));
        }
        headers
    }

    /// POST a un endpoint de la API interna.
    async fn post(
        &self,
        host: &str,
        origin: &str,
        context: Value,
        endpoint: &str,
        body: Value,
        authenticated: bool,
    ) -> Result<Value, ApiError> {
        let url = format!("{host}/youtubei/v1/{endpoint}?prettyPrint=false");
        let mut payload = body;
        if let Value::Object(ref mut map) = payload {
            map.insert("context".into(), context);
        }

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Accept-Language", format!("{}-{},{};q=0.9", self.locale.hl, self.locale.gl, self.locale.hl));

        if authenticated {
            for (k, v) in self.auth_headers(origin) {
                req = req.header(k, v);
            }
            if let Some(v) = self.visitor_data().await {
                req = req.header("X-Goog-Visitor-Id", v);
            }
        }

        let res = req
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let status = res.status().as_u16();
        let bytes = res
            .bytes()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if status != 200 {
            let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(400)])
                .replace(['\n', '\r'], " ");
            return Err(match status {
                401 | 403 => ApiError::NeedsLogin,
                _ => ApiError::Http {
                    status,
                    body: snippet,
                },
            });
        }

        serde_json::from_slice(&bytes).map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// Petición anónima al host de YouTube con el perfil web.
    pub async fn post_web(&self, endpoint: &str, body: Value) -> Result<Value, ApiError> {
        let ctx = self.context_web();
        self.post(HOST_YOUTUBE, HOST_YOUTUBE, ctx, endpoint, body, true)
            .await
    }

    /// Petición al host de música con el perfil `WEB_REMIX`.
    pub async fn post_music(&self, endpoint: &str, body: Value) -> Result<Value, ApiError> {
        let ctx = self.context_remix();
        self.post(HOST_MUSIC, HOST_MUSIC, ctx, endpoint, body, true)
            .await
    }

    /// GET simple al host de YouTube (páginas HTML: solo para `visitorData`).
    async fn get_text(&self, url: &str, headers: Vec<(String, String)>) -> Result<String, ApiError> {
        let mut req = self.http.get(url);
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let res = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if !res.status().is_success() {
            return Err(ApiError::Http {
                status: res.status().as_u16(),
                body: String::new(),
            });
        }
        res.text().await.map_err(|e| ApiError::Network(e.to_string()))
    }

    /// Identidad de visitante, cacheada 30 min. Sin ella `ANDROID_VR` contesta
    /// `LOGIN_REQUIRED` aunque el vídeo sea público.
    pub async fn visitor_data(&self) -> Option<String> {
        {
            let cache = self.visitor.lock();
            if let (Some(v), Some(at)) = (cache.value.clone(), cache.at) {
                if at.elapsed() < VISITOR_TTL {
                    return Some(v);
                }
            }
        }
        let url = format!("{HOST_YOUTUBE}/watch?v=dQw4w9WgXcQ&bpctr=9999999999&has_verified=1");
        let html = self
            .get_text(
                &url,
                vec![
                    (
                        "User-Agent".into(),
                        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
(KHTML, like Gecko) Version/15.5 Safari/605.1.15"
                            .into(),
                    ),
                    ("Accept-Language".into(), "en-us,en;q=0.5".into()),
                    ("Cookie".into(), "PREF=hl=en&tz=UTC; SOCS=CAI".into()),
                ],
            )
            .await
            .ok()?;
        let value = extract_visitor_data(&html)?;
        let mut cache = self.visitor.lock();
        cache.value = Some(value.clone());
        cache.at = Some(Instant::now());
        Some(value)
    }

    /// Tira la identidad de visitante actual: la siguiente petición pedirá otra.
    pub fn invalidate_visitor(&self) {
        *self.visitor.lock() = VisitorCache::default();
    }

    /// `youtubei/v1/player` con el cliente `ANDROID_VR`, **anónimo**.
    ///
    /// Devuelve el JSON crudo: la selección de pistas la hace
    /// [`crate::yt::stream`] porque depende de lo que ofrezca el vídeo.
    pub async fn player_vr(&self, video_id: &str) -> Result<Value, ApiError> {
        let visitor = self.visitor_data().await;
        let url = format!("{HOST_YOUTUBE}/youtubei/v1/player?prettyPrint=false");
        let mut client_ctx = json!({
            "clientName": "ANDROID_VR",
            "clientVersion": VR_CLIENT_VERSION,
            "deviceMake": "Oculus",
            "deviceModel": "Quest 3",
            "androidSdkVersion": 32,
            "userAgent": VR_USER_AGENT,
            "osName": "Android",
            "osVersion": "12L",
            "timeZone": "UTC",
            "utcOffsetMinutes": 0,
            "hl": self.locale.hl,
            "gl": self.locale.gl,
        });
        if let Some(v) = &visitor {
            client_ctx["visitorData"] = json!(v);
        }
        let payload = json!({
            "context": {
                "client": client_ctx,
                "user": {"lockedSafetyMode": false},
            },
            "videoId": video_id,
            "playbackContext": {"contentPlaybackContext": {"html5Preference": "HTML5_PREF_WANTS"}},
            "contentCheckOk": true,
            "racyCheckOk": true,
        });

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("User-Agent", VR_USER_AGENT)
            .header("X-Youtube-Client-Name", VR_CLIENT_NAME_NUMBER)
            .header("X-Youtube-Client-Version", VR_CLIENT_VERSION)
            .header("Origin", HOST_YOUTUBE);
        if let Some(v) = &visitor {
            req = req.header("X-Goog-Visitor-Id", v);
        }
        // Sin `Cookie` ni `Authorization`: la sesión aquí envenena la URL.
        let res = req
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = res.status().as_u16();
        let bytes = res
            .bytes()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if status != 200 {
            return Err(ApiError::Http {
                status,
                body: String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).to_string(),
            });
        }
        serde_json::from_slice(&bytes).map_err(|e| ApiError::Parse(e.to_string()))
    }
}

/// Del HTML de `/watch` saca `visitorData`, que viene escapado a la manera de
/// JavaScript (`=` → `\u003d`): hay que deshacerlo o la cabecera viaja con las
/// barras invertidas dentro.
fn extract_visitor_data(html: &str) -> Option<String> {
    let re = regex::Regex::new(r#""visitorData"\s*:\s*"([^"]+)""#).ok()?;
    let raw = re.captures(html)?.get(1)?.as_str();
    serde_json::from_str::<String>(&format!("\"{raw}\""))
        .ok()
        .or_else(|| Some(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desescapa_visitor_data() {
        let html = r#"...,"visitorData":"CgtvYWtleQ%3D%3D",..."#;
        // %3D no es escape JS: se devuelve tal cual.
        assert_eq!(extract_visitor_data(html).unwrap(), "CgtvYWtleQ%3D%3D");
        let html = r#""visitorData":"Cgt\u003dbad\u003d""#;
        assert_eq!(extract_visitor_data(html).unwrap(), "Cgt=bad=");
    }

    #[test]
    fn locale_desde_sistema_produce_dos_letras() {
        let l = Locale::from_system();
        assert!(l.hl.len() >= 2);
        assert_eq!(l.gl.len(), 2);
    }
}
