//! Operaciones de alto nivel: una función por pantalla o acción.
//!
//! Cada función devuelve modelos de dominio (nunca JSON) para que la interfaz no
//! sepa nada del formato de YouTube, y todas comparten el mismo tratamiento de
//! errores: [`ApiError`] con una clase de fallo explicable al usuario.

use serde_json::{json, Value};

use crate::domain::*;
use crate::yt::client::{ApiError, Innertube};
use crate::yt::parse;

/// `browseId` fijos del catálogo musical. La biblioteca **no** es un solo id:
/// playlists, álbumes, canciones guardadas y artistas viven en sitios distintos
/// y pedir solo el primero deja media pantalla vacía.
pub mod music_ids {
    pub const HOME: &str = "FEmusic_home";
    pub const EXPLORE: &str = "FEmusic_explore";
    pub const LIKED_PLAYLISTS: &str = "FEmusic_liked_playlists";
    pub const LIKED_ALBUMS: &str = "FEmusic_liked_albums";
    pub const LIKED_VIDEOS: &str = "FEmusic_liked_videos";
    pub const LIBRARY_ARTISTS: &str = "FEmusic_library_corpus_track_artists";
    /// Pestaña "Tus me gusta": es una *lista* de pistas, no una rejilla.
    pub const LM: &str = "VLLM";
}

/// `params` de filtro de búsqueda musical (`EgWKAQI…`): lo que manda la web
/// cuando el usuario pulsa cada pestaña.
pub mod search_params {
    pub const SONGS: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
    pub const VIDEOS: &str = "EgWKAQIQAWoKEAkQChAFEAMQBA%3D%3D";
    pub const ALBUMS: &str = "EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D";
    pub const ARTISTS: &str = "EgWKAQIgAWoKEAkQChAFEAMQBA%3D%3D";
    pub const PLAYLISTS: &str = "EgWKAQIoAWoKEAkQChAFEAMQBA%3D%3D";
}

/// Extrae el `feed` de una respuesta `browse`/`search`.
///
/// Descarta entradas que no son contenido (anuncios, separadores,
/// continuaciones) y agrupa lo que aparece fuera de un estante en un estante
/// sin título, para que ninguna pantalla se quede vacía por un cambio de
/// envoltorio.
pub fn feed_from_json(json: &Value) -> Feed {
    let mut feed = Feed::default();

    // Estantes visibles. `itemSectionRenderer` se trata como contenedor, no
    // como estante: envolvería al estante real y saldría un título duplicado.
    const SHELVES: &[&str] = &[
        "shelfRenderer",
        "musicCarouselShelfRenderer",
        "musicShelfRenderer",
        "gridRenderer",
        "reelShelfRenderer",
        "richShelfRenderer",
        "richSectionRenderer",
        "horizontalListRenderer",
        "expandedShelfContentsRenderer",
        // Formato de la portada actual: una rejilla "rica" con
        // `richItemRenderer` dentro de `contents`. Sin esto, `browse
        // FEwhat_to_watch` devolvía cero estantes y la pantalla de Inicio salía
        // vacía aunque la respuesta viniera llena.
        "richGridRenderer",
        "richSectionRenderer",
        "gridRenderer",
    ];

    let mut shelves: Vec<(&str, &Value)> = Vec::new();
    collect_shelves(json, SHELVES, &mut shelves);

    // Se deduplica por (tipo, id) y no por puntero: el mismo vídeo puede venir
    // en el estante y otra vez suelto, y pintarlo dos veces es un fallo visible.
    let mut vistos: std::collections::HashSet<(MediaKind, String)> = std::collections::HashSet::new();
    for (renderer, node) in shelves {
        let mut cards: Vec<Card> = Vec::new();
        for (name, item) in parse::collect_vec(node, parse::CARD_RENDERERS) {
            if parse::is_placeholder(item) {
                continue;
            }
            if let Some(card) = parse::card_from(name, item) {
                cards.push(card);
            }
        }
        cards.retain(|c| vistos.insert((c.kind, c.id.clone())));
        if cards.is_empty() {
            continue;
        }
        let _ = node;
        let layout = match renderer {
            "gridRenderer" => ShelfLayout::Grid,
            "musicShelfRenderer" | "richShelfRenderer" => ShelfLayout::List,
            _ => ShelfLayout::Row,
        };
        feed.shelves.push(Shelf {
            title: parse::shelf_title(node),
            items: cards,
            more: parse::continuation_of(node),
            layout,
        });
    }

    // Tarjetas sueltas que no colgaron de ningún estante conocido.
    let mut sueltas: Vec<Card> = Vec::new();
    for (name, item) in parse::collect_vec(json, parse::CARD_RENDERERS) {
        if parse::is_placeholder(item) {
            continue;
        }
        if let Some(card) = parse::card_from(name, item) {
            if vistos.insert((card.kind, card.id.clone())) {
                sueltas.push(card);
            }
        }
    }
    if !sueltas.is_empty() {
        feed.shelves.push(Shelf {
            title: None,
            items: sueltas,
            more: None,
            layout: ShelfLayout::Grid,
        });
    }

    feed.chips = parse::chips(json);
    feed.continuation = parse::find_key(json, "continuationCommand")
        .and_then(|c| c.get("token"))
        .and_then(|t| t.as_str())
        .map(str::to_string);
    feed
}

/// Igual que [`parse::collect`], pero para estantes: no entra en un estante ya
/// recogido para no duplicar sus elementos.
fn collect_shelves<'a>(node: &'a Value, wanted: &[&'static str], out: &mut Vec<(&'static str, &'a Value)>) {
    match node {
        Value::Object(map) => {
            for key in wanted {
                if let Some(child) = map.get(*key) {
                    if !child.is_null() {
                        out.push((key, child));
                        return;
                    }
                }
            }
            for (_, v) in map {
                collect_shelves(v, wanted, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect_shelves(v, wanted, out);
            }
        }
        _ => {}
    }
}

/// Si la respuesta trae pantalla de error de YouTube, se convierte en error
/// explicable en vez de en un feed vacío.
fn check_errors(json: &Value) -> Result<(), ApiError> {
    if let Some(err) = parse::find_key(json, "alertRenderer") {
        let text = parse::text_at(err, &["text"]);
        if !text.trim().is_empty() {
            return Err(ApiError::Unavailable(text.trim().to_string()));
        }
    }
    if let Some(err) = json.get("error") {
        let msg = parse::text_at(err, &["message"]);
        let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        return Err(ApiError::Http {
            status: code as u16,
            body: msg,
        });
    }
    Ok(())
}

impl Innertube {
    // ------------------------------------------------------------- YouTube

    /// Inicio personalizado (`FEwhat_to_watch`). Con sesión, YouTube devuelve el
    /// feed de la cuenta; sin sesión, el genérico.
    pub async fn home(&self) -> Result<Feed, ApiError> {
        let json = self
            .post_web("browse", json!({ "browseId": "FEwhat_to_watch" }))
            .await?;
        check_errors(&json)?;
        let mut feed = feed_from_json(&json);
        let playable = feed
            .shelves
            .iter()
            .any(|s| s.items.iter().any(|c| !c.thumbs.is_empty() || c.duration.is_some()));
        if !playable {
            // **La portada anónima ya no trae vídeos.** Sin sesión, YouTube
            // responde con un `richGridRenderer` que solo contiene un
            // `feedNudgeRenderer` («inicia sesión para ver lo tuyo»): cero
            // `videoRenderer`. Antes se caía a `browse FEtrending`, que hoy
            // contesta 400.
            //
            // El respaldo es una búsqueda real de temas populares: son vídeos
            // de verdad, con datos de verdad, y se marcan como «Populares
            // ahora» para no hacer pasar una cosa por otra.
            let temas = ["tendencias", "música", "documental", "tecnología"];
            let semilla = temas[std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as usize)
                .unwrap_or(0)
                % temas.len()];
            if let Ok(trending) = self.search(semilla, Some("EgIQAQ%3D%3D")).await {
                let mut items: Vec<Card> = Vec::new();
                for shelf in trending.shelves {
                    items.extend(shelf.items);
                }
                // Solo vídeos con miniatura: es lo que se puede pintar.
                items.retain(|c| !c.thumbs.is_empty());
                let mut vistos = std::collections::HashSet::new();
                items.retain(|c| vistos.insert(c.id.clone()));
                if !items.is_empty() {
                    feed.shelves = vec![Shelf {
                        title: Some("Populares ahora".to_string()),
                        items,
                        more: None,
                        layout: ShelfLayout::Grid,
                    }];
                }
            }
        }
        Ok(feed)
    }

    /// `browse` por el host web. Se usa para canales cuando el host de música
    /// no devuelve nada.
    pub async fn browse_web(&self, browse_id: &str) -> Result<Feed, ApiError> {
        let json = self
            .post_web("browse", json!({ "browseId": browse_id }))
            .await?;
        check_errors(&json)?;
        Ok(feed_from_json(&json))
    }

    /// Búsqueda de YouTube. `params` es el filtro (vídeos, canales, listas…).
    pub async fn search(&self, query: &str, params: Option<&str>) -> Result<Feed, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Feed::default());
        }
        let mut body = json!({ "query": query });
        if let Some(p) = params.filter(|p| !p.is_empty()) {
            body["params"] = json!(p);
        }
        let json = self.post_web("search", body).await?;
        check_errors(&json)?;
        Ok(feed_from_json(&json))
    }

    /// Página de continuación: la misma llamada para cualquier pantalla, porque
    /// el token ya lleva el contexto.
    pub async fn continue_feed(&self, token: &str, music: bool) -> Result<Feed, ApiError> {
        let body = json!({ "continuation": token });
        let json = if music {
            self.post_music("browse", body).await?
        } else {
            self.post_web("browse", body).await?
        };
        check_errors(&json)?;
        let mut feed = feed_from_json(&json);
        if feed.shelves.is_empty() {
            // Respaldo documentado: algunas continuaciones de música solo
            // responden por query string.
            let endpoint = format!(
                "browse?ctoken={}&continuation={}&type=next",
                urlencoding::encode(token),
                urlencoding::encode(token)
            );
            let json = self.post_music(&endpoint, json!({})).await?;
            feed = feed_from_json(&json);
        }
        Ok(feed)
    }

    /// Página de un vídeo: metadatos, acciones y relacionados (`next`).
    pub async fn watch(&self, video_id: &str) -> Result<WatchInfo, ApiError> {
        let json = self
            .post_web(
                "next",
                json!({
                    "videoId": video_id,
                    "playbackContext": {"contentPlaybackContext": {"html5Preference": "HTML5_PREF_WANTS"}},
                }),
            )
            .await?;
        check_errors(&json)?;
        Ok(watch_from_json(video_id, &json))
    }

    /// Me gusta / quitar me gusta de un vídeo.
    pub async fn set_like(&self, video_id: &str, on: bool) -> Result<(), ApiError> {
        let endpoint = if on { "like/like" } else { "like/removelike" };
        let json = self
            .post_web(endpoint, json!({ "target": { "videoId": video_id } }))
            .await?;
        check_errors(&json)?;
        Ok(())
    }

    /// Suscribirse / darse de baja de un canal (`channelId` empieza por `UC…`).
    pub async fn set_subscribed(&self, channel_id: &str, on: bool) -> Result<(), ApiError> {
        let json = self
            .post_web("subscription/subscribe", json!({ "channelIds": [channel_id] }))
            .await;
        if json.is_err() && on {
            // El endpoint de suscripción a veces solo acepta el estado completo
            // en el cuerpo; se reintenta con la forma larga.
            let json = self
                .post_web(
                    "subscription/subscribe",
                    json!({ "channelIds": [channel_id], "params": "EgIIAg%3D%3D" }),
                )
                .await?;
            check_errors(&json)?;
            return Ok(());
        }
        if !on {
            let json = self
                .post_web("subscription/unsubscribe", json!({ "channelIds": [channel_id] }))
                .await?;
            check_errors(&json)?;
        }
        Ok(())
    }

    // --------------------------------------------------------------- Música

    pub async fn music_home(&self) -> Result<Feed, ApiError> {
        self.music_browse(music_ids::HOME).await
    }

    pub async fn music_explore(&self) -> Result<Feed, ApiError> {
        self.music_browse(music_ids::EXPLORE).await
    }

    /// `browse` genérico del host de música. Sirve para portada, biblioteca,
    /// álbum, artista, lista y canal.
    pub async fn music_browse(&self, browse_id: &str) -> Result<Feed, ApiError> {
        let json = self
            .post_music("browse", json!({ "browseId": browse_id }))
            .await?;
        check_errors(&json)?;
        Ok(feed_from_json(&json))
    }

    pub async fn music_search(&self, query: &str, params: Option<&str>) -> Result<Feed, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Feed::default());
        }
        let mut body = json!({ "query": query });
        if let Some(p) = params.filter(|p| !p.is_empty()) {
            body["params"] = json!(p);
        }
        let json = self.post_music("search", body).await?;
        check_errors(&json)?;
        Ok(feed_from_json(&json))
    }

    /// Biblioteca de la cuenta: listas, álbumes, artistas y canciones en un solo
    /// feed, en ese orden, para que la pantalla no dependa de un único id.
    pub async fn library(&self) -> Result<Feed, ApiError> {
        if !self.is_logged_in() {
            return Err(ApiError::NeedsLogin);
        }
        let mut feed = Feed::default();
        for (id, title) in [
            (music_ids::LIKED_PLAYLISTS, "Listas"),
            (music_ids::LIKED_ALBUMS, "Álbumes"),
            (music_ids::LIBRARY_ARTISTS, "Artistas"),
        ] {
            match self.music_browse(id).await {
                Ok(part) => {
                    let mut items: Vec<Card> = Vec::new();
                    for shelf in part.shelves {
                        items.extend(shelf.items);
                    }
                    if !items.is_empty() {
                        feed.shelves.push(Shelf {
                            title: Some(title.to_string()),
                            items,
                            more: None,
                            layout: ShelfLayout::Row,
                        });
                    }
                }
                Err(ApiError::NeedsLogin) => return Err(ApiError::NeedsLogin),
                Err(e) => log::warn!("biblioteca {id}: {e}"),
            }
        }
        if feed.shelves.is_empty() {
            return Err(ApiError::Unavailable(
                "Tu biblioteca está vacía o YouTube no la devolvió.".into(),
            ));
        }
        Ok(feed)
    }

    /// Historial de la cuenta (música). YouTube puede no devolverlo si el
    /// historial está desactivado: eso se explica, no se pinta vacío.
    pub async fn music_history(&self) -> Result<Feed, ApiError> {
        let json = self
            .post_music("browse", json!({ "browseId": "FEmusic_history" }))
            .await?;
        check_errors(&json)?;
        let mut feed = feed_from_json(&json);
        if feed.shelves.is_empty() {
            return Err(ApiError::Unavailable(
                "Tu historial está desactivado en YouTube o no hay actividad.".into(),
            ));
        }
        // El historial de música llega como carruseles por día: se aplanan en
        // un solo estante para poder pintarlo como lista.
        let mut items = Vec::new();
        for shelf in &feed.shelves {
            items.extend(shelf.items.clone());
        }
        feed.shelves = vec![Shelf {
            title: None,
            items,
            more: feed.continuation.clone(),
            layout: ShelfLayout::List,
        }];
        Ok(feed)
    }

    /// Cola de reproducción de YouTube para un vídeo/canción: lo que el
    /// reproductor web usa para "siguiente". Es la única vía para las radios
    /// (`RD…`), que no responden por `browse`.
    pub async fn music_queue(&self, video_id: &str, playlist_id: Option<&str>) -> Result<Vec<Card>, ApiError> {
        let mut body = json!({
            "videoId": video_id,
            "isAudioOnly": true,
            "params": "wAEB",
        });
        if let Some(pid) = playlist_id.filter(|p| !p.is_empty()) {
            body["playlistId"] = json!(pid);
        }
        let json = self.post_music("next", body).await?;
        check_errors(&json)?;
        let mut out = Vec::new();
        for (name, item) in parse::collect_vec(&json, &["playlistPanelVideoRenderer"]) {
            if let Some(card) = parse::card_from(name, item) {
                out.push(card);
            }
        }
        Ok(out)
    }

    /// Feed de Shorts. Se resuelve desde la búsqueda de Shorts de un tema
    /// cualquiera, porque no hay endpoint público de "shorts infinitos" sin
    /// un vídeo de partida.
    pub async fn shorts_feed(&self, seed_query: &str) -> Result<Vec<Card>, ApiError> {
        let json = self
            .post_web("search", json!({ "query": format!("#shorts {seed_query}") }))
            .await?;
        check_errors(&json)?;
        let mut out = Vec::new();
        for (name, item) in parse::collect_vec(&json, &["reelItemRenderer", "shortsLockupViewModel", "reelShelfRenderer"]) {
            if name == "reelShelfRenderer" {
                for (inner_name, inner) in parse::collect_vec(item, &["reelItemRenderer", "shortsLockupViewModel"]) {
                    if let Some(card) = parse::card_from(inner_name, inner) {
                        out.push(card);
                    }
                }
                continue;
            }
            if let Some(card) = parse::card_from(name, item) {
                out.push(card);
            }
        }
        // Deduplicar: la misma Short aparece en el estante y en la búsqueda.
        let mut vistos = std::collections::HashSet::new();
        out.retain(|c| vistos.insert(c.id.clone()));
        if out.is_empty() {
            return Err(ApiError::Unavailable(
                "YouTube no devolvió Shorts en este momento.".into(),
            ));
        }
        Ok(out)
    }
}

/// Convierte la respuesta de `next` en [`WatchInfo`].
pub fn watch_from_json(video_id: &str, json: &Value) -> WatchInfo {
    let mut info = WatchInfo {
        video_id: video_id.to_string(),
        ..Default::default()
    };

    if let Some(primary) = parse::find_key(json, "videoPrimaryInfoRenderer") {
        info.title = parse::text_at(primary, &["title"]);
        info.view_count = non_empty(parse::text_at(primary, &["viewCount", "videoViewCountRenderer", "viewCount"]));
        if info.view_count.is_none() {
            info.view_count = non_empty(parse::text_at(primary, &["viewCount", "videoViewCountRenderer", "shortViewCount"]));
        }
        info.published = non_empty(parse::text_at(primary, &["dateText"]));
        let badge = parse::text_at(primary, &["badges"]);
        let _ = badge;
        for b in primary.get("badges").and_then(|b| b.as_array()).into_iter().flatten() {
            let label = parse::text_at(b, &["metadataBadgeRenderer", "label"]);
            if label.to_uppercase().contains("LIVE") || label.to_uppercase().contains("VIVO") {
                info.is_live = true;
            }
        }
    }

    if let Some(secondary) = parse::find_key(json, "videoSecondaryInfoRenderer") {
        if let Some(owner) = secondary.get("owner").and_then(|o| parse::find_key(o, "videoOwnerRenderer")) {
            info.channel.name = parse::text_at(owner, &["title"]);
            info.channel.avatar = parse::thumbs_of(owner);
            info.channel.subscribers = non_empty(parse::text_at(owner, &["subscriberCountText"]));
            info.channel.id = parse::find_key(owner, "browseEndpoint")
                .and_then(|b| b.get("browseId"))
                .and_then(|b| b.as_str())
                .unwrap_or_default()
                .to_string();
            if info.channel.avatar.is_empty() {
                if let Some(t) = owner.get("thumbnail") {
                    info.channel.avatar = parse::thumbs_from_array(
                        t.get("thumbnails").unwrap_or(&Value::Null),
                    );
                }
            }
        }
        // Descripción: puede venir como `attributedDescription.content` (nuevo)
        // o como `description.runs` (antiguo).
        let attributed = parse::text_at(secondary, &["attributedDescription", "content"]);
        let runs = parse::text_at(secondary, &["description"]);
        info.description = if attributed.trim().is_empty() { runs } else { attributed };
        if info.description.is_empty() {
            if let Some(d) = parse::find_key(secondary, "attributedDescription") {
                info.description = parse::text_at(d, &["content"]);
            }
        }
        info.chapters = chapters_from(&info.description);
    }

    // Estado de "me gusta" y suscripción: los dos viven en botones con forma
    // de `toggleButtonRenderer`.
    if let Some(toggle) = parse::find_key(json, "likeButtonViewModel") {
        for t in parse::collect_vec(toggle, &["toggleButtonRenderer"]) {
            let is_toggled = parse::find_key(t.1, "isToggled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_toggled {
                info.like_state = LikeState::Like;
            }
        }
        info.likes_text = non_empty(parse::text_at(toggle, &["defaultText", "accessibility", "accessibilityData", "label"]));
    }
    for b in parse::collect_vec(json, &["subscribeButtonRenderer"]) {
        for sub in parse::collect_vec(b.1, &["subscribeButtonRenderer"]) {
            let subscribed = sub
                .1
                .get("subscribed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if subscribed {
                info.subscribed = true;
            }
            if info.channel.id.is_empty() {
                info.channel.id = sub
                    .1
                    .get("channelId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
            }
        }
    }
    if let Some(toggle) = parse::find_key(json, "toggleButtonRenderer") {
        if toggle.get("isToggled").and_then(|v| v.as_bool()).unwrap_or(false) {
            info.like_state = LikeState::Like;
        }
    }

    // Relacionados. La lista cambió de formato: antes eran
    // `compactVideoRenderer` y hoy llegan como `lockupViewModel` (con el id en
    // `contentId`). Se recogen todos los tipos conocidos en vez de uno solo,
    // deduplicando, porque YouTube mezcla los dos formatos en la misma página.
    let mut relacionados = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    for (name, item) in parse::collect_vec(json, parse::CARD_RENDERERS) {
        if parse::is_placeholder(item) {
            continue;
        }
        if let Some(card) = parse::card_from(name, item) {
            if matches!(
                card.kind,
                MediaKind::Video | MediaKind::Short | MediaKind::Live | MediaKind::Song
            ) && card.id != video_id
                && vistos.insert(card.id.clone())
            {
                relacionados.push(card);
            }
        }
    }
    info.related = relacionados;
    info.continuation = parse::find_key(json, "continuationCommand")
        .and_then(|c| c.get("token"))
        .and_then(|t| t.as_str())
        .map(str::to_string);
    info
}

/// Capítulos a partir de marcas de tiempo al principio de línea de la
/// descripción (`0:00 Intro`). Es lo que hace la propia web cuando no hay
/// capítulos oficiales.
fn chapters_from(description: &str) -> Vec<Chapter> {
    let mut out = Vec::new();
    for line in description.lines().take(80) {
        let line = line.trim();
        let mut parts = line.splitn(2, char::is_whitespace);
        let stamp = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("").trim();
        if rest.is_empty() {
            continue;
        }
        let secs = parse_timestamp(stamp);
        if let Some(s) = secs {
            out.push(Chapter {
                title: rest.to_string(),
                start_seconds: s,
            });
        }
    }
    // Sin al menos dos capítulos no merece la pena pintarlos.
    if out.len() < 2 {
        out.clear();
    }
    out
}

/// `1:02:03` / `02:03` / `0:00` → segundos.
pub fn parse_timestamp(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for p in &parts {
        let v: f64 = p.parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_capitulos_de_la_descripcion() {
        let desc = "Un pódcast muy largo\n\n0:00 Introducción\n3:45 Tema uno\n1:02:03 Cierre";
        let caps = chapters_from(desc);
        assert_eq!(caps.len(), 3);
        assert_eq!(caps[1].start_seconds, 225.0);
        assert_eq!(caps[2].start_seconds, 3723.0);
    }

    #[test]
    fn menos_de_dos_capitulos_no_se_pintan() {
        assert!(chapters_from("0:00 solo uno").is_empty());
        assert!(chapters_from("texto sin marcas").is_empty());
    }

    #[test]
    fn feed_agrupa_estantes_y_no_duplica() {
        let json = serde_json::json!({
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "itemSectionRenderer": {
                                            "contents": [{
                                                "shelfRenderer": {
                                                    "title": {"runs": [{"text": "Recomendado"}]},
                                                    "content": {
                                                        "horizontalListRenderer": {
                                                            "items": [
                                                                {"gridVideoRenderer": {"videoId": "a", "title": {"simpleText": "A"}}},
                                                                {"gridVideoRenderer": {"videoId": "b", "title": {"simpleText": "B"}}}
                                                            ]
                                                        }
                                                    }
                                                }
                                            }]
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });
        let feed = feed_from_json(&json);
        let total: usize = feed.shelves.iter().map(|s| s.items.len()).sum();
        assert_eq!(total, 2, "cada vídeo una sola vez: {feed:?}");
        assert_eq!(feed.shelves[0].title.as_deref(), Some("Recomendado"));
    }

    #[test]
    fn lee_metadatos_de_un_video() {
        let json = serde_json::json!({
            "contents": {"twoColumnWatchNextResults": {"results": {"results": {"contents": [
                {"videoPrimaryInfoRenderer": {
                    "title": {"runs": [{"text": "Un título"}]},
                    "viewCount": {"videoViewCountRenderer": {"viewCount": {"simpleText": "1.234.567 visualizaciones"}}},
                    "dateText": {"simpleText": "hace 2 días"}
                }},
                {"videoSecondaryInfoRenderer": {
                    "owner": {"videoOwnerRenderer": {
                        "title": {"runs": [{"text": "Canal"}]},
                        "subscriberCountText": {"runs": [{"text": "12 mil suscriptores"}]},
                        "navigationEndpoint": {"browseEndpoint": {"browseId": "UC123"}}
                    }},
                    "attributedDescription": {"content": "0:00 Inicio\n1:00 Siguiente"}
                }}
            ]}}}}
        });
        let info = watch_from_json("abc", &json);
        assert_eq!(info.title, "Un título");
        assert_eq!(info.channel.name, "Canal");
        assert_eq!(info.channel.id, "UC123");
        assert_eq!(info.view_count.as_deref(), Some("1.234.567 visualizaciones"));
        assert_eq!(info.chapters.len(), 2);
    }
}
