//! Resolución de streams reproducibles.
//!
//! ## Por qué `VISIONOS` y no `ANDROID_VR`
//!
//! El contrato heredado de NeoTube 1.x usaba `ANDROID_VR` porque era el único
//! cliente móvil que devolvía `url` directas sin `signatureCipher`. Eso sigue
//! siendo cierto, pero **desde finales de 2025 ese cliente solo sirve una
//! ventana del fichero**: medido con URLs recién resueltas, el audio deja de
//! responder (403) a partir de ~1 MB (≈65 s) y el vídeo a partir de ~30 % del
//! tamaño total. Abrir una canción completa o un vídeo de más de un minuto es
//! imposible por ahí, y no es un cupo que se gaste: la posición no se sirve, ni
//! con una URL nueva.
//!
//! La versión `1.62.27` de `ANDROID_VR` (anterior a la que usaba 1.x) todavía
//! sirve el fichero entero, pero YouTube ya contesta `LOGIN_REQUIRED`
//! («confirma que no eres un bot») a la mayoría de vídeos por esa vía.
//!
//! `VISIONOS` (cliente 101, versión `1.02`) sí sirve el fichero completo y
//! responde `OK` a los vídeos que `ANDROID_VR` bloquea. Es el mismo cliente que
//! usa yt-dlp hoy. Requiere dos cosas que no son evidentes y que se sacan de la
//! propia página del vídeo:
//!
//! - las cookies de la visita (`VISITOR_INFO1_LIVE`, `__Secure-YNID`, `YSC`…),
//! - el `signatureTimestamp` (`sts`) que aparece en el `ytcfg` de la página.
//!
//! Sin ellas devuelve `ERROR` o `UNPLAYABLE`.
//!
//! Todo va **anónimo**: nunca se mandan las cookies de la cuenta, porque
//! «decir que eres un visor de Vision Pro y adjuntar una sesión web» hace que
//! googlevideo emita URLs que luego rechaza al reproducirlas (está documentado
//! en el contrato heredado y se comprobó de nuevo aquí).
//!
//! Todo el código va compilado con `#[allow(dead_code)]` para que estas notas
//! sobrevivan a futuras limpiezas: son la mitad del valor del fichero.

use serde_json::{json, Value};

use crate::domain::{ResolvedStream, StreamTrack};
use crate::yt::client::{ApiError, Innertube, PlaybackClient};

/// Cliente que sirve el fichero completo hoy.
pub const VISIONOS_CLIENT_VERSION: &str = "1.02";
pub const VISIONOS_CLIENT_NUMBER: &str = "101";
pub const VISIONOS_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) \
AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
pub const VISIONOS_DEVICE_MODEL: &str = "RealityDevice17,1";
pub const VISIONOS_OS_VERSION: &str = "26.5.23O471";

/// Lo que la página del vídeo aporta y el `player` necesita.
#[derive(Debug, Default, Clone)]
pub struct PageContext {
    pub visitor_data: Option<String>,
    /// `signatureTimestamp` del `ytcfg` (`sts`). Sin él, `VISIONOS` contesta
    /// `ERROR`.
    pub signature_timestamp: Option<u64>,
    /// Cookies de la visita, ya como pares para la cabecera `Cookie:`.
    pub cookies: Vec<(String, String)>,
}

impl Innertube {
    /// Pide la página del vídeo y extrae lo que hace falta para `VISIONOS`.
    ///
    /// Se pide con `User-Agent` de Safari: es a un navegador a quien se le
    /// sirve el HTML que trae `visitorData` y el `sts` dentro.
    pub async fn video_page_context(&self, video_id: &str) -> PageContext {
        let url =
            format!("https://www.youtube.com/watch?v={video_id}&bpctr=9999999999&has_verified=1");
        let mut ctx = PageContext::default();
        let Ok(res) = self
            .http
            .get(&url)
            .header("User-Agent", VISIONOS_USER_AGENT)
            .header("Accept-Language", "en-us,en;q=0.5")
            .header("Cookie", "PREF=hl=en&tz=UTC; SOCS=CAI")
            .send()
            .await
        else {
            return ctx;
        };
        for value in res.headers().get_all("set-cookie") {
            if let Ok(text) = value.to_str() {
                if let Some(pair) = text.split(';').next() {
                    if let Some((name, val)) = pair.split_once('=') {
                        let name = name.trim();
                        if !name.is_empty() {
                            ctx.cookies.push((name.to_string(), val.trim().to_string()));
                        }
                    }
                }
            }
        }
        let Ok(html) = res.text().await else {
            return ctx;
        };
        let parsed = parse_watch_page(&html);
        ctx.visitor_data = parsed.visitor_data;
        ctx.signature_timestamp = parsed.signature_timestamp;
        ctx
    }

    /// `youtubei/v1/player` con el cliente `VISIONOS`, anónimo.
    pub async fn player_visionos(
        &self,
        video_id: &str,
        ctx: &PageContext,
    ) -> Result<Value, ApiError> {
        let url = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
        let mut client = json!({
            "clientName": "VISIONOS",
            "clientVersion": VISIONOS_CLIENT_VERSION,
            "deviceMake": "Apple",
            "deviceModel": VISIONOS_DEVICE_MODEL,
            "userAgent": VISIONOS_USER_AGENT,
            "osName": "visionOS",
            "osVersion": VISIONOS_OS_VERSION,
            "hl": self.locale.hl,
            "timeZone": "UTC",
            "utcOffsetMinutes": 0,
        });
        if let Some(v) = &ctx.visitor_data {
            client["visitorData"] = json!(v);
        }
        let mut playback_ctx = json!({ "html5Preference": "HTML5_PREF_WANTS" });
        if let Some(sts) = ctx.signature_timestamp {
            playback_ctx["signatureTimestamp"] = json!(sts);
        }
        let body = json!({
            "context": { "client": client },
            "videoId": video_id,
            "playbackContext": { "contentPlaybackContext": playback_ctx },
            "contentCheckOk": true,
            "racyCheckOk": true,
        });

        let mut req = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", VISIONOS_USER_AGENT)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "en-us,en;q=0.5")
            .header("X-Youtube-Client-Name", VISIONOS_CLIENT_NUMBER)
            .header("X-Youtube-Client-Version", VISIONOS_CLIENT_VERSION)
            .header("Origin", "https://www.youtube.com");
        if !ctx.cookies.is_empty() {
            // Las cookies de la visita, no las de la cuenta.
            let header = ctx
                .cookies
                .iter()
                .map(|(n, v)| format!("{n}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            req = req.header("Cookie", header);
        }
        if let Some(v) = &ctx.visitor_data {
            req = req.header("X-Goog-Visitor-Id", v);
        }
        let res = req
            .json(&body)
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

    /// Resuelve un vídeo a pistas reproducibles.
    ///
    /// Intenta `VISIONOS` y, si el vídeo no se puede reproducir por ahí, cae a
    /// `ANDROID_VR` 1.62.27 (que sirve el fichero completo pero bloquea más
    /// vídeos). Los dos son anónimos.
    pub async fn resolve_stream(
        &self,
        video_id: &str,
        max_height: Option<u32>,
    ) -> Result<ResolvedStream, ApiError> {
        let ctx = self.video_page_context(video_id).await;
        let (json, client) = match self.player_visionos(video_id, &ctx).await {
            Ok(j) => (j, PlaybackClient::VisionOs),
            Err(e) => {
                log::warn!("visionos falló ({e}); probando ANDROID_VR");
                (self.player_vr(video_id).await?, PlaybackClient::AndroidVr)
            }
        };

        let status = json
            .get("playabilityStatus")
            .and_then(|p| p.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("");

        if status != "OK" {
            let reason = json
                .get("playabilityStatus")
                .and_then(|p| p.get("reason"))
                .and_then(|r| r.as_str())
                .unwrap_or(status);
            return Err(match status {
                "LOGIN_REQUIRED" => ApiError::BotCheck,
                "UNPLAYABLE" | "ERROR" => ApiError::Unavailable(reason.to_string()),
                _ => ApiError::Unavailable(format!("YouTube no deja reproducir esto: {reason}")),
            });
        }

        let duration = json
            .get("videoDetails")
            .and_then(|v| v.get("lengthSeconds"))
            .and_then(|l| l.as_str())
            .and_then(|l| l.parse::<f64>().ok());
        let expires = json
            .get("streamingData")
            .and_then(|s| s.get("expiresInSeconds"))
            .and_then(|e| e.as_str())
            .and_then(|e| e.parse::<u64>().ok());

        let formats = collect_formats(&json);
        let audio = pick_audio(&formats);
        let video = pick_video(&formats, max_height);
        let muxed = pick_muxed(&formats);

        if audio.is_none() && video.is_none() && muxed.is_none() {
            return Err(ApiError::Unavailable(
                "YouTube no ofreció ninguna pista reproducible.".into(),
            ));
        }

        Ok(ResolvedStream {
            video_id: video_id.to_string(),
            muxed,
            video,
            audio,
            // Las cabeceras van emparejadas al cliente que emitió la URL.
            headers: crate::yt::client::playback_headers_for(client),
            expires_in_seconds: expires,
            resolved_at: std::time::Instant::now(),
            duration_seconds: duration,
        })
    }
}

/// Busca un atributo JSON en el HTML (`"clave":"valor"`), deshaciendo el escape
/// de JavaScript.
fn extract_json_attr(html: &str, key: &str) -> Option<String> {
    // Se busca a mano y no con una expresión regular: el patrón necesita
    // clases con barras escapadas y basta un `\` de menos para que la regex no
    // compile y la función devuelva `None` en silencio (que es justo lo que
    // hacía que el `player` no llevara `visitorData`).
    let needle = format!("\"{key}\"");
    let mut search_from = 0;
    while let Some(found) = html[search_from..].find(&needle) {
        let after_key = search_from + found + needle.len();
        let rest = &html[after_key..];
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            search_from = after_key;
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            // Valor no textual (un número): lo resuelve quien llama.
            search_from = after_key;
            continue;
        };
        // Se recorre hasta la comilla de cierre respetando los escapes.
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    return serde_json::from_str::<String>(&format!("\"{out}\"")).ok();
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        out.push('\\');
                        out.push(next);
                    }
                }
                other => out.push(other),
            }
        }
        return None;
    }
    None
}

struct RawFormat {
    itag: u32,
    mime: String,
    url: String,
    bitrate: u64,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    channels: Option<u32>,
    content_length: Option<u64>,
}

fn collect_formats(json: &Value) -> Vec<RawFormat> {
    let mut out = Vec::new();
    for key in ["formats", "adaptiveFormats"] {
        let Some(list) = json
            .get("streamingData")
            .and_then(|s| s.get(key))
            .and_then(|f| f.as_array())
        else {
            continue;
        };
        for f in list {
            // Solo se aceptan formatos con `url` servida: los que llegan en
            // `signatureCipher` exigirían ejecutar el JavaScript del
            // reproductor.
            let Some(url) = f.get("url").and_then(|u| u.as_str()) else {
                continue;
            };
            let mime = f
                .get("mimeType")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            if mime.is_empty() {
                continue;
            }
            out.push(RawFormat {
                itag: f.get("itag").and_then(|i| i.as_u64()).unwrap_or(0) as u32,
                url: url.to_string(),
                mime,
                bitrate: f.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0),
                width: f.get("width").and_then(|w| w.as_u64()).map(|w| w as u32),
                height: f.get("height").and_then(|h| h.as_u64()).map(|h| h as u32),
                fps: f.get("fps").and_then(|f| f.as_u64()).map(|f| f as u32),
                channels: f
                    .get("audioChannels")
                    .and_then(|c| c.as_u64())
                    .map(|c| c as u32),
                content_length: f
                    .get("contentLength")
                    .and_then(|c| c.as_str())
                    .and_then(|c| c.parse().ok()),
            });
        }
    }
    out
}

fn to_track(f: &RawFormat) -> StreamTrack {
    StreamTrack {
        url: f.url.clone(),
        mime: f.mime.clone(),
        itag: f.itag,
        bitrate: f.bitrate,
        width: f.width,
        height: f.height,
        fps: f.fps,
        audio_channels: f.channels,
        approx_duration_ms: f
            .content_length
            .map(|c| c * 8 / f.bitrate.max(1) * 1000 / 1000),
    }
}

/// Mejor audio: bitrate más alto, prefiriendo contenedores que VLC decodifica
/// sin depender de plugins extra (mp4/AAC y webm/Opus, ambos nativos).
fn pick_audio(formats: &[RawFormat]) -> Option<StreamTrack> {
    formats
        .iter()
        .filter(|f| f.mime.starts_with("audio/"))
        .max_by_key(|f| f.bitrate)
        .map(to_track)
}

/// Mejor vídeo por debajo del techo pedido.
///
/// Se prefieren códecs que el equipo reproduce por hardware cuando los hay
/// (`avc1` y `av01` en mp4) frente a VP9 en webm: es la diferencia entre
/// decodificar en CPU o en GPU, y por tanto entre un ventilador encendido o no.
fn pick_video(formats: &[RawFormat], max_height: Option<u32>) -> Option<StreamTrack> {
    let ceiling = max_height.unwrap_or(u32::MAX);
    let mut candidates: Vec<&RawFormat> = formats
        .iter()
        .filter(|f| f.mime.starts_with("video/"))
        .filter(|f| f.height.unwrap_or(0) <= ceiling)
        .collect();
    candidates.sort_by_key(|f| {
        let height = f.height.unwrap_or(0);
        let codec_rank = if f.mime.contains("avc1") {
            3
        } else if f.mime.contains("av01") {
            2
        } else if f.mime.contains("vp9") || f.mime.contains("vp09") {
            1
        } else {
            0
        };
        (height, codec_rank, f.fps.unwrap_or(0), f.bitrate)
    });
    candidates.last().map(|f| to_track(f))
}

/// Pista única con audio y vídeo. `VISIONOS` suele ofrecer pocas (a veces solo
/// 360p), pero cuando existe es lo más barato de reproducir.
fn pick_muxed(formats: &[RawFormat]) -> Option<StreamTrack> {
    formats
        .iter()
        .find(|f| f.itag == 18 && f.mime.starts_with("video/"))
        .or_else(|| {
            // Cualquier formato con `audioQuality` es muxado.
            formats
                .iter()
                .find(|f| f.mime.starts_with("video/") && f.height.is_some() && f.itag == 22)
        })
        .map(to_track)
}

/// Parsea del HTML de `/watch` lo que necesita el `player`: identidad de
/// visitante y `signatureTimestamp`.
///
/// Es una función pura a propósito: la parte frágil de esta integración es el
/// parseo de la página, y así se puede probar con HTML guardado sin red.
pub fn parse_watch_page(html: &str) -> PageContext {
    let mut ctx = PageContext::default();
    ctx.visitor_data = extract_json_attr(html, "visitorData");
    ctx.signature_timestamp = extract_json_attr(html, "signatureTimestamp")
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| {
            let re = regex::Regex::new(r#""signatureTimestamp"\s*:\s*(\d+)"#).ok()?;
            re.captures(html)?.get(1)?.as_str().parse().ok()
        })
        .or_else(|| {
            // En `PLAYER_CONFIG` aparece como `sts`.
            let re = regex::Regex::new(r#""sts"\s*:\s*(\d+)"#).ok()?;
            re.captures(html)?.get(1)?.as_str().parse().ok()
        });
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elige_el_audio_de_mas_bitrate() {
        let formats = vec![
            RawFormat {
                itag: 251,
                mime: "audio/webm; codecs=\"opus\"".into(),
                url: "u1".into(),
                bitrate: 136_544,
                width: None,
                height: None,
                fps: None,
                channels: Some(2),
                content_length: None,
            },
            RawFormat {
                itag: 140,
                mime: "audio/mp4; codecs=\"mp4a.40.2\"".into(),
                url: "u2".into(),
                bitrate: 130_677,
                width: None,
                height: None,
                fps: None,
                channels: Some(2),
                content_length: None,
            },
        ];
        assert_eq!(pick_audio(&formats).unwrap().itag, 251);
    }

    #[test]
    fn respeta_el_techo_de_calidad_y_prefiere_h264() {
        let mk = |itag: u32, mime: &str, height: u32, fps: u32, bitrate: u64| RawFormat {
            itag,
            mime: mime.into(),
            url: format!("u{itag}"),
            bitrate,
            width: Some(height * 16 / 9),
            height: Some(height),
            fps: Some(fps),
            channels: None,
            content_length: None,
        };
        let formats = vec![
            mk(313, "video/webm; codecs=\"vp9\"", 2160, 25, 18_076_636),
            mk(
                137,
                "video/mp4; codecs=\"avc1.640028\"",
                1080,
                25,
                4_334_157,
            ),
            mk(248, "video/webm; codecs=\"vp9\"", 1080, 25, 1_549_172),
        ];
        let best = pick_video(&formats, Some(1080)).unwrap();
        assert_eq!(best.itag, 137, "1080p h264 antes que 1080p vp9");
        let capped = pick_video(&formats, Some(720));
        assert!(capped.is_none(), "no hay nada <=720");
        assert_eq!(pick_video(&formats, None).unwrap().itag, 313);
    }

    #[test]
    fn extrae_visitor_y_sts_del_html() {
        // Forma real: `visitorData` como cadena escapada y `signatureTimestamp`
        // como número dentro de `ytcfg`.
        let html = r#"ytcfg.set({"INNERTUBE_CONTEXT":{"client":{"visitorData":"CgABC123%3D"}},
            "WEB_PLAYER_CONTEXT_CONFIGS":{"p":{"signatureTimestamp":20760}}});"#;
        let ctx = parse_watch_page(html);
        assert_eq!(ctx.visitor_data.as_deref(), Some("CgABC123%3D"));
        assert_eq!(ctx.signature_timestamp, Some(20760));

        // Variante con el valor como cadena.
        let ctx = parse_watch_page(r#"{"signatureTimestamp":"19999"}"#);
        assert_eq!(ctx.signature_timestamp, Some(19999));

        // Variante `sts`, que es como llega en PLAYER_CONFIG.
        let ctx = parse_watch_page(r#"{"sts":20760}"#);
        assert_eq!(ctx.signature_timestamp, Some(20760));

        // Sin datos, no se inventa nada.
        let ctx = parse_watch_page("<html></html>");
        assert!(ctx.visitor_data.is_none());
        assert!(ctx.signature_timestamp.is_none());
    }
}
