//! Parseo de respuestas de la API interna de YouTube.
//!
//! Estrategia: **no** se fijan rutas absolutas del tipo
//! `contents.singleColumnBrowseResultsRenderer...` porque YouTube inserta y
//! quita un nivel cada pocas semanas y una ruta rota deja la pantalla vacía sin
//! error. En su lugar:
//!
//! 1. Se recorren los nodos y se recogen los *renderers* por nombre, que es la
//!    parte estable del formato (`videoRenderer`, `musicResponsiveListItemRenderer`…).
//! 2. Cada renderer se convierte a [`Card`] con un lector tolerante: lo que no
//!    está, se deja vacío en vez de tumbar la respuesta entera.
//!
//! El conocimiento de qué renderer trae qué campo viene de
//! `docs/PORTING_CONTRACT.md` (código Flutter de NeoTube 1.x) y de las
//! respuestas reales guardadas en `tests/fixtures/`.

use serde_json::Value;

use crate::domain::{Card, MediaKind, Thumbnail, Thumbs};

/// Los renderers que sabemos convertir en tarjeta.
pub const CARD_RENDERERS: &[&str] = &[
    // YouTube "grande"
    "videoRenderer",
    "compactVideoRenderer",
    "gridVideoRenderer",
    "playlistVideoRenderer",
    "reelItemRenderer",
    "shortsLockupViewModel",
    "videoWithContextRenderer",
    "playlistRenderer",
    "gridPlaylistRenderer",
    "channelRenderer",
    "gridChannelRenderer",
    "lockupViewModel",
    "playlistPanelVideoRenderer",
    // YouTube Music
    "musicResponsiveListItemRenderer",
    "musicTwoRowItemRenderer",
    "musicCardShelfRenderer",
    "musicMultiRowListItemRenderer",
];

/// Renderers de estante/contenedor que aportan título propio.
pub const SHELF_RENDERERS: &[&str] = &[
    "shelfRenderer",
    "musicCarouselShelfRenderer",
    "musicShelfRenderer",
    "gridRenderer",
    "itemSectionRenderer",
    "musicCardShelfRenderer",
    "reelShelfRenderer",
    "richShelfRenderer",
    "richSectionRenderer",
    "horizontalListRenderer",
    "expandedShelfContentsRenderer",
];

fn as_array(v: Option<&Value>) -> &[Value] {
    v.and_then(|x| x.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// `node[key]` como array (vacío si no lo es).
pub fn arr<'a>(node: &'a Value, key: &str) -> &'a [Value] {
    as_array(node.get(key))
}

/// `node[key]` si no es null.
pub fn obj<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    node.get(key).filter(|v| !v.is_null())
}

/// Desciende por una ruta de claves.
pub fn dig<'a>(node: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = node;
    for key in path {
        cur = cur.get(key)?;
        if cur.is_null() {
            return None;
        }
    }
    Some(cur)
}

/// Texto de un nodo con cualquiera de las formas que usa YouTube:
/// `simpleText`, `runs[].text` o `content`.
pub fn text(node: &Value) -> String {
    if let Some(s) = node.as_str() {
        return s.to_string();
    }
    if let Some(s) = node.get("simpleText").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(s) = node.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(runs) = node.get("runs").and_then(|v| v.as_array()) {
        let mut out = String::new();
        for run in runs {
            if let Some(t) = run.get("text").and_then(|v| v.as_str()) {
                out.push_str(t);
            }
        }
        return out;
    }
    String::new()
}

/// [`text`] sobre una ruta.
pub fn text_at(node: &Value, path: &[&str]) -> String {
    dig(node, path).map(text).unwrap_or_default()
}

/// Recorre el árbol en profundidad y devuelve el primer nodo que tenga `key`.
pub fn find_key<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    match node {
        Value::Object(map) => {
            if let Some(v) = map.get(key) {
                if !v.is_null() {
                    return Some(v);
                }
            }
            for v in map.values() {
                if let Some(found) = find_key(v, key) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// Recoge `(nombre_del_renderer, nodo)` de todos los descendientes cuyo nombre
/// esté en `wanted`, **sin** bajar dentro de un renderer ya recogido: así un
/// `gridVideoRenderer` anidado no se cuenta dos veces.
pub fn collect<'a>(
    node: &'a Value,
    wanted: &[&'static str],
    out: &mut Vec<(&'static str, &'a Value)>,
) {
    match node {
        Value::Object(map) => {
            for key in wanted {
                if let Some(child) = map.get(*key) {
                    if !child.is_null() {
                        out.push((key, child));
                        return; // no se desciende dentro
                    }
                }
            }
            for (_, v) in map {
                collect(v, wanted, out);
            }
        }
        Value::Array(items) => {
            for v in items {
                collect(v, wanted, out);
            }
        }
        _ => {}
    }
}

pub fn collect_vec<'a>(node: &'a Value, wanted: &[&'static str]) -> Vec<(&'static str, &'a Value)> {
    let mut out = Vec::new();
    collect(node, wanted, &mut out);
    out
}

/// Miniaturas de un nodo, buscándolas en las formas habituales.
pub fn thumbs_of(node: &Value) -> Thumbs {
    if let Some(t) = obj(node, "thumbnail").and_then(|t| obj(t, "thumbnails")) {
        return thumbs_from_array(t);
    }
    // Formato nuevo (`lockupViewModel`, `shortsLockupViewModel`): las imágenes
    // vienen en `sources` con la misma forma `{url,width,height}`. Sin esto, los
    // relacionados de la página de vídeo llegaban sin miniatura.
    if let Some(t) = find_key(node, "sources").and_then(|t| t.as_array()) {
        let thumbs = thumbs_from_array(&Value::Array(t.clone()));
        if !thumbs.is_empty() {
            return thumbs;
        }
    }
    if let Some(t) = find_key(node, "thumbnails").and_then(|t| t.as_array()) {
        return thumbs_from_array(&Value::Array(t.clone()));
    }
    Thumbs::default()
}

/// Título legible de `accessibilityText`, que en los Shorts trae
/// "Título del vídeo, 40 mil visualizaciones - reproducir Short".
fn title_from_accessibility(node: &Value) -> String {
    let text = node
        .get("accessibilityText")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if text.is_empty() {
        return String::new();
    }
    let mut parts = text.splitn(2, ',');
    let title = parts.next().unwrap_or("").trim().to_string();
    // El sufijo de accesibilidad nunca es parte del título.
    title
        .trim_end_matches(" - reproducir Short")
        .trim()
        .to_string()
}

/// Metadatos del `lockupViewModel`: filas del `contentMetadataViewModel`.
fn lockup_meta(node: &Value) -> String {
    let Some(meta) = find_key(node, "lockupMetadataViewModel") else {
        return String::new();
    };
    let mut bits: Vec<String> = Vec::new();
    if let Some(rows) = find_key(meta, "metadataRows").and_then(|r| r.as_array()) {
        for row in rows {
            for part in arr(row, "metadataParts") {
                let t = text_at(part, &["text", "content"]);
                let t = if t.trim().is_empty() {
                    text_at(part, &["text"])
                } else {
                    t
                };
                let t = t.trim();
                if !t.is_empty() {
                    bits.push(t.to_string());
                }
            }
        }
    }
    bits.join(" · ")
}

pub fn thumbs_from_array(arr: &Value) -> Thumbs {
    Thumbs::new(
        as_array(Some(arr))
            .iter()
            .filter_map(|t| {
                let url = t.get("url").and_then(|u| u.as_str())?;
                let width = t.get("width").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
                let height = t.get("height").and_then(|h| h.as_u64()).unwrap_or(0) as u32;
                if url.is_empty() {
                    return None;
                }
                Some(Thumbnail {
                    url: url.to_string(),
                    width,
                    height,
                })
            })
            .collect::<Vec<_>>(),
    )
}

/// Si la miniatura no trae dimensiones (algunos `lockupViewModel`), se
/// reconstruyen por convención de YouTube: `.../hq720.jpg` es 1280x720.
fn guess_size(url: &str) -> (u32, u32) {
    for (marker, size) in [
        ("maxresdefault", (1280, 720)),
        ("sddefault", (640, 480)),
        ("hq720", (1280, 720)),
        ("hqdefault", (480, 360)),
        ("mqdefault", (320, 180)),
        ("default.jpg", (120, 90)),
    ] {
        if url.contains(marker) {
            return size;
        }
    }
    (480, 270)
}

/// Ids de navegación que puede llevar un nodo.
#[derive(Debug, Default, Clone)]
pub struct Endpoints {
    pub video_id: Option<String>,
    pub browse_id: Option<String>,
    pub playlist_id: Option<String>,
    pub channel_id: Option<String>,
    pub url: Option<String>,
}

pub fn endpoints(node: &Value) -> Endpoints {
    let mut out = Endpoints::default();
    if let Some(nav) = find_key(node, "navigationEndpoint") {
        if let Some(we) = obj(nav, "watchEndpoint") {
            out.video_id = we
                .get("videoId")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            out.playlist_id = we
                .get("playlistId")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if let Some(re) = obj(nav, "reelWatchEndpoint") {
            out.video_id = out.video_id.or_else(|| {
                re.get("videoId")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
        }
        if let Some(be) = obj(nav, "browseEndpoint") {
            out.browse_id = be
                .get("browseId")
                .and_then(|v| v.as_str())
                .map(str::to_string);
        }
        if let Some(ce) = obj(nav, "commandMetadata") {
            if let Some(web) = text_at(ce, &["webCommandMetadata", "url"])
                .strip_prefix('/')
                .map(str::to_string)
            {
                out.url = Some(web);
            }
        }
    }
    if let Some(vid) = node.get("videoId").and_then(|v| v.as_str()) {
        out.video_id.get_or_insert_with(|| vid.to_string());
    }
    if let Some(bid) = node.get("playlistId").and_then(|v| v.as_str()) {
        out.playlist_id.get_or_insert_with(|| bid.to_string());
    }
    if out.channel_id.is_none() {
        out.channel_id = find_key(node, "browseEndpoint")
            .and_then(|be| be.get("browseId"))
            .and_then(|v| v.as_str())
            .filter(|s| s.starts_with("UC"))
            .map(str::to_string);
    }
    out
}

/// Keywords internas (p. ej. `children` de `richItemRenderer`).
fn has_key(node: &Value, key: &str) -> bool {
    node.get(key).is_some_and(|v| !v.is_null())
}

/// Duración en texto, de `lengthText` o del overlay de tiempo.
fn duration_of(node: &Value) -> Option<String> {
    let direct = text_at(node, &["lengthText"]);
    if !direct.is_empty() {
        return Some(direct);
    }
    for overlay in arr(node, "thumbnailOverlays") {
        if let Some(status) = find_key(overlay, "thumbnailOverlayTimeStatusRenderer") {
            let t = text_at(status, &["text"]);
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    let t = text_at(node, &["timeStatus", "lengthText"]);
    if !t.is_empty() {
        return Some(t);
    }
    None
}

/// Duración en un `lockupViewModel`: el tiempo viene como texto de un
/// `thumbnailBadgeViewModel` dentro de los overlays.
fn lockup_duration(node: &Value) -> Option<String> {
    let start = find_key(node, "thumbnailBottomOverlayViewModel")
        .or_else(|| find_key(node, "thumbnailOverlayBadgeViewModel"))?;
    let mut found: Vec<String> = Vec::new();
    for (_, badge) in collect_vec(start, &["thumbnailBadgeViewModel"]) {
        let t = text_at(badge, &["text"]);
        if t.contains(':') {
            found.push(t);
        }
    }
    found.into_iter().next()
}

/// "EN VIVO" / "LIVE" si el overlay lo dice.
fn live_badge(node: &Value) -> Option<String> {
    for badge in arr(node, "badges") {
        let label = text_at(badge, &["metadataBadgeRenderer", "label"]);
        if label.to_uppercase().contains("LIVE") || label.to_uppercase().contains("VIVO") {
            return Some(label);
        }
    }
    for overlay in arr(node, "thumbnailOverlays") {
        if let Some(status) = find_key(overlay, "thumbnailOverlayTimeStatusRenderer") {
            let style = status.get("style").and_then(|s| s.as_str()).unwrap_or("");
            if style.contains("LIVE") {
                let label = text_at(status, &["text"]);
                return Some(if label.is_empty() {
                    "EN VIVO".to_string()
                } else {
                    label
                });
            }
        }
    }
    if dig(node, &["thumbnailOverlayTimeStatusRenderer", "style"])
        .and_then(|s| s.as_str())
        .is_some_and(|s| s.contains("LIVE"))
    {
        return Some("EN VIVO".to_string());
    }
    None
}

/// Convierte un renderer de tarjeta en [`Card`]. Devuelve `None` para entradas
/// que no son contenido navegable (anuncios, separadores, continuaciones).
pub fn card_from(renderer: &str, node: &Value) -> Option<Card> {
    match renderer {
        "videoRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let title = text_at(node, &["title"]);
            let mut card = Card::new(id, MediaKind::Video, title);
            card.author = text_at(node, &["ownerText"]);
            card.author_id = endpoints(node).channel_id;
            card.meta = join_meta(&[
                text_at(node, &["viewCountText"]),
                text_at(node, &["publishedTimeText"]),
            ]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            card.badge = live_badge(node);
            if card.badge.is_some() {
                card.kind = MediaKind::Live;
            }
            Some(card)
        }
        "compactVideoRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let mut card = Card::new(id, MediaKind::Video, text_at(node, &["title"]));
            card.author = text_at(node, &["shortBylineText"]);
            card.author_id = endpoints(node).channel_id;
            card.meta = join_meta(&[
                text_at(node, &["shortViewCountText"]),
                text_at(node, &["publishedTimeText"]),
            ]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            card.badge = live_badge(node);
            if card.badge.is_some() {
                card.kind = MediaKind::Live;
            }
            Some(card)
        }
        "gridVideoRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let mut card = Card::new(id, MediaKind::Video, text_at(node, &["title"]));
            card.author = text_at(node, &["shortBylineText"]);
            card.meta = join_meta(&[
                text_at(node, &["viewCountText"]),
                text_at(node, &["publishedTimeText"]),
            ]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            card.badge = live_badge(node);
            Some(card)
        }
        "playlistVideoRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let mut card = Card::new(id, MediaKind::Video, text_at(node, &["title"]));
            card.author = text_at(node, &["shortBylineText"]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            card.playlist_id = node
                .get("playlistId")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Some(card)
        }
        "reelItemRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let mut card = Card::new(id, MediaKind::Short, text_at(node, &["headline"]));
            card.author = text_at(
                node,
                &["navigationEndpoint", "reelWatchEndpoint", "owner", "name"],
            );
            card.meta = text_at(node, &["viewCountText"]);
            card.thumbs = thumbs_of(node);
            card.badge = Some("SHORTS".into());
            Some(card)
        }
        "shortsLockupViewModel" => {
            // El id no está en `videoId` sino en `entityId`, con la forma
            // "shorts-shelf-item-<videoId>". También se acepta el vídeo del
            // endpoint de watch como respaldo.
            let id = node
                .get("entityId")
                .and_then(|e| e.as_str())
                .and_then(|e| e.strip_prefix("shorts-shelf-item-"))
                .map(str::to_string)
                .or_else(|| endpoints(node).video_id)?;
            let mut card = Card::new(id, MediaKind::Short, title_from_accessibility(node));
            if card.title.is_empty() {
                card.title = text_at(node, &["overlayMetadata", "primaryText"]);
            }
            card.author = text_at(node, &["overlayMetadata", "secondaryText"]);
            card.meta = text_at(node, &["overlayMetadata", "secondaryText"]);
            card.thumbs = thumbs_of(node);
            card.badge = Some("SHORTS".into());
            if card.title.is_empty() {
                return None;
            }
            Some(card)
        }
        "videoWithContextRenderer" => {
            // Renderer que usan los Shorts y los "feed" experimentales: el id
            // vive en `videoId` o en el endpoint de watch.
            let ep = endpoints(node);
            let id = ep.video_id?;
            let mut card = Card::new(id, MediaKind::Video, text_at(node, &["headline"]));
            card.author = text_at(node, &["shortBylineText"]);
            card.meta = text_at(node, &["shortViewCountText"]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            Some(card)
        }
        "playlistRenderer" | "gridPlaylistRenderer" => {
            let id = node
                .get("playlistId")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| endpoints(node).playlist_id)?;
            let title = text_at(node, &["title"]);
            let mut card = Card::new(id.clone(), MediaKind::Playlist, title);
            card.playlist_id = Some(id);
            card.author = text_at(node, &["shortBylineText"]);
            let count = text_at(node, &["videoCountText"]);
            card.meta = join_meta(&[count, text_at(node, &["shortViewCountText"])]);
            card.thumbs = thumbs_of(node);
            Some(card)
        }
        "channelRenderer" | "gridChannelRenderer" => {
            let id = node
                .get("channelId")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| endpoints(node).browse_id)?;
            let title = text_at(node, &["title"]);
            let mut card = Card::new(id.clone(), MediaKind::Channel, title);
            card.author_id = Some(id);
            card.meta = join_meta(&[
                text_at(node, &["videoCountText"]),
                text_at(node, &["subscriberCountText"]),
            ]);
            card.thumbs = thumbs_of(node);
            Some(card)
        }
        // ---------------------------------------------------------- Música
        "musicResponsiveListItemRenderer" | "musicMultiRowListItemRenderer" => {
            let ep = endpoints(node);
            let video_id = ep.video_id.clone();
            let browse_id = ep.browse_id.clone();
            let (id, kind) = match (&browse_id, &video_id) {
                (Some(b), _) if b.starts_with("MPRE") => (b.clone(), MediaKind::Album),
                (Some(b), _) if b.starts_with("MPLA") || b.starts_with("UC") => {
                    (b.clone(), MediaKind::Artist)
                }
                (Some(b), _) if b.starts_with("VL") || b.starts_with("PL") => {
                    (b.clone(), MediaKind::Playlist)
                }
                (Some(b), Some(v)) => {
                    let _ = b;
                    (v.clone(), MediaKind::Song)
                }
                (None, Some(v)) => (v.clone(), MediaKind::Song),
                (Some(b), None) => (b.clone(), MediaKind::Playlist),
                (None, None) => return None,
            };
            let title = item_title(node);
            if title.is_empty() {
                return None;
            }
            let mut card = Card::new(id, kind, title);
            card.playlist_id = ep.playlist_id.clone();
            card.author_id = ep.browse_id.clone().filter(|b| b.starts_with("UC"));
            card.thumbs = thumbs_of(node);
            // La fila de música es una lista de columnas flexibles: la primera
            // lleva título, el resto artistas/álbum/duración según el contexto.
            let columns = flex_columns(node);
            if columns.len() >= 2 {
                card.author = columns[1].clone();
                card.meta = columns[2..].join(" · ");
            }
            card.duration = columns.last().filter(|c| c.contains(':')).cloned();
            Some(card)
        }
        "musicTwoRowItemRenderer" => {
            let ep = endpoints(node);
            let (id, kind) = match (ep.browse_id.clone(), ep.video_id.clone()) {
                (Some(b), _) if b.starts_with("MPRE") => (b, MediaKind::Album),
                (Some(b), _) if b.starts_with("MPLA") || b.starts_with("UC") => {
                    (b, MediaKind::Artist)
                }
                (Some(b), _) => (b, MediaKind::Playlist),
                (None, Some(v)) => (v, MediaKind::Song),
                (None, None) => return None,
            };
            let title = text_at(node, &["title"]);
            if title.is_empty() {
                return None;
            }
            let mut card = Card::new(id, kind, title);
            card.thumbs = thumbs_of(node);
            card.meta = join_meta(&[text_at(node, &["subtitle"]), text_at(node, &["subtitle2"])]);
            card.playlist_id = ep.playlist_id;
            Some(card)
        }
        "musicCardShelfRenderer" => {
            let ep = endpoints(node);
            let title = text_at(node, &["title"]);
            if title.is_empty() {
                return None;
            }
            let kind = if ep.video_id.is_some() {
                MediaKind::Song
            } else {
                MediaKind::Playlist
            };
            let mut card = Card::new(ep.video_id.or(ep.browse_id)?, kind, title);
            card.thumbs = thumbs_of(node);
            card.meta = text_at(node, &["subtitle"]);
            Some(card)
        }
        "lockupViewModel" => {
            // Formato nuevo (2025+) que **sustituye** a `videoRenderer` en los
            // relacionados y en varias rejillas. El id está en `contentId`, no
            // en `videoId`: leerlo del endpoint dejaba la lista de
            // "A continuación" vacía.
            let id = node
                .get("contentId")
                .and_then(|c| c.as_str())
                .map(str::to_string)
                .or_else(|| endpoints(node).video_id)?;
            let meta = find_key(node, "lockupMetadataViewModel");
            let title = meta.map(|m| text_at(m, &["title"])).unwrap_or_default();
            if title.is_empty() {
                return None;
            }
            let mut card = Card::new(id, MediaKind::Video, title);
            card.meta = lockup_meta(node);
            card.thumbs = thumbs_of(node);
            card.duration = lockup_duration(node);
            Some(card)
        }
        "playlistPanelVideoRenderer" => {
            let id = node.get("videoId").and_then(|v| v.as_str())?.to_string();
            let mut card = Card::new(id, MediaKind::Video, text_at(node, &["title"]));
            card.author = text_at(node, &["shortBylineText"]);
            card.duration = duration_of(node);
            card.thumbs = thumbs_of(node);
            Some(card)
        }
        _ => None,
    }
}

/// Columnas de texto de una fila de YouTube Music, en orden.
fn flex_columns(node: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for col in arr(node, "flexColumns") {
        if let Some(r) = find_key(col, "musicResponsiveListItemFlexColumnRenderer") {
            let t = text_at(r, &["text"]);
            if !t.trim().is_empty() {
                out.push(t.trim().to_string());
            }
        }
    }
    if out.is_empty() {
        // musicMultiRowListItemRenderer usa `title` + `subtitle`.
        for path in [vec!["title"], vec!["subtitle"], vec!["secondSubtitle"]] {
            let t = text_at(node, &path);
            if !t.trim().is_empty() {
                out.push(t.trim().to_string());
            }
        }
    }
    out
}

fn item_title(node: &Value) -> String {
    let direct = text_at(node, &["title"]);
    if !direct.trim().is_empty() {
        return direct.trim().to_string();
    }
    flex_columns(node).first().cloned().unwrap_or_default()
}

fn join_meta(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Título de estante de un contenedor, en cualquiera de sus formas.
pub fn shelf_title(node: &Value) -> Option<String> {
    for path in [
        vec!["title", "simpleText"],
        vec!["header", "title", "simpleText"],
        vec!["header", "musicCarouselShelfBasicHeaderRenderer", "title"],
        vec!["header", "musicShelfRenderer", "title"],
        vec!["header", "richShelfRenderer", "title"],
        vec![
            "header",
            "richSectionRenderer",
            "content",
            "richShelfRenderer",
            "title",
        ],
        vec!["header", "gridHeaderRenderer", "title"],
        vec!["title"],
    ] {
        if let Some(v) = dig(node, &path) {
            let t = text(v);
            if !t.trim().is_empty() {
                return Some(t.trim().to_string());
            }
        }
    }
    // Algunos contenedores llevan el título como runs dentro de `title`.
    let t = text_at(node, &["header", "title"]);
    if !t.trim().is_empty() {
        return Some(t.trim().to_string());
    }
    None
}

/// Token de continuación que cuelga de un nodo, si hay.
pub fn continuation_of(node: &Value) -> Option<String> {
    if let Some(c) = find_key(node, "continuationCommand") {
        if let Some(token) = c.get("token").and_then(|t| t.as_str()) {
            return Some(token.to_string());
        }
    }
    find_key(node, "continuationItemRenderer")
        .and_then(|n| find_key(n, "token"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

/// Texto de un chip de filtro (`chipCloudChipRenderer`), con los params que
/// hay que reenviar para activarlo.
pub fn chips(node: &Value) -> Vec<Card> {
    let mut out = Vec::new();
    for (_, chip) in collect_vec(node, &["chipCloudChipRenderer", "chipViewModel"]) {
        let label = text_at(chip, &["text"]).trim().to_string();
        let label = if label.is_empty() {
            text_at(chip, &["text", "simpleText"]).trim().to_string()
        } else {
            label
        };
        if label.is_empty() {
            continue;
        }
        let params = find_key(chip, "searchEndpoint")
            .and_then(|e| e.get("params"))
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        let selected = chip
            .get("isSelected")
            .and_then(|s| s.as_bool())
            .unwrap_or(false);
        let mut card = Card::new(params, MediaKind::Video, label);
        if selected {
            card.badge = Some("seleccionado".into());
        }
        out.push(card);
    }
    out
}

/// Convierte un número romano/localizado de "1,2 M de visualizaciones" a algo
/// comparable. Se usa para ordenar y para nada más: nunca se muestra.
#[allow(dead_code)]
pub fn approximate_views(meta: &str) -> u64 {
    let lower = meta.to_lowercase();
    let cleaned: String = lower
        .chars()
        .filter(|c| {
            c.is_ascii_digit() || *c == '.' || *c == ',' || *c == 'k' || *c == 'm' || *c == 'b'
        })
        .collect();
    let number: String = cleaned
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    let value: f64 = number.replace(',', ".").parse().unwrap_or(0.0);
    if cleaned.contains('m') || lower.contains(" mill") {
        (value * 1_000_000.0) as u64
    } else if cleaned.contains('k') || lower.contains(" mil") {
        (value * 1_000.0) as u64
    } else if cleaned.contains('b') {
        (value * 1_000_000_000.0) as u64
    } else {
        value as u64
    }
}

/// Corrige miniaturas que llegan sin tamaño declarado.
pub fn normalize_thumbs(thumbs: Thumbs) -> Thumbs {
    if thumbs.0.iter().all(|t| t.width > 0) {
        return thumbs;
    }
    Thumbs::new(thumbs.0.into_iter().map(|t| {
        if t.width > 0 {
            t
        } else {
            let (w, h) = guess_size(&t.url);
            Thumbnail {
                url: t.url,
                width: w,
                height: h,
            }
        }
    }))
}

/// `true` si el renderer está marcado como continuidad/relleno.
pub fn is_placeholder(node: &Value) -> bool {
    has_key(node, "continuationItemRenderer") || has_key(node, "adSlotRenderer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encuentra_renderers_sin_ruta_fija() {
        let json: Value = serde_json::json!({
            "a": {"b": [{"c": {"videoRenderer": {"videoId": "abc", "title": {"simpleText": "Hola"}}}}]}
        });
        let found = collect_vec(&json, CARD_RENDERERS);
        assert_eq!(found.len(), 1);
        let card = card_from(found[0].0, found[0].1).unwrap();
        assert_eq!(card.id, "abc");
        assert_eq!(card.title, "Hola");
    }

    #[test]
    fn no_desciende_dentro_de_un_renderer_recogido() {
        let json: Value = serde_json::json!({
            "videoRenderer": {
                "videoId": "outer",
                "thumbnailOverlays": [{"thumbnailOverlayTimeStatusRenderer": {"text": {"simpleText": "3:21"}}}]
            }
        });
        let found = collect_vec(&json, CARD_RENDERERS);
        assert_eq!(found.len(), 1, "el overlay no debe contar como tarjeta");
    }

    #[test]
    fn lee_duracion_y_directo() {
        let node = serde_json::json!({
            "videoId": "x",
            "title": {"runs": [{"text": "En "}, {"text": "directo"}]},
            "viewCountText": {"simpleText": "1.234 espectadores"},
            "lengthText": {"simpleText": "1:02:03"},
            "thumbnailOverlays": [{"thumbnailOverlayTimeStatusRenderer": {"text": {"simpleText": "EN VIVO"}, "style": "LIVE"}}]
        });
        let card = card_from("videoRenderer", &node).unwrap();
        assert_eq!(card.title, "En directo");
        assert_eq!(card.duration.as_deref(), Some("1:02:03"));
        assert_eq!(card.kind, MediaKind::Live);
    }

    #[test]
    fn distingue_album_de_cancion_en_musica() {
        let album = serde_json::json!({
            "flexColumns": [{"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Discovery"}]}}}],
            "navigationEndpoint": {"browseEndpoint": {"browseId": "MPREb_123"}}
        });
        let card = card_from("musicResponsiveListItemRenderer", &album).unwrap();
        assert_eq!(card.kind, MediaKind::Album);
        assert_eq!(card.id, "MPREb_123");
    }
}
