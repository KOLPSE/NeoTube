//! Comentarios, respuestas y notificaciones.
//!
//! Los comentarios **no** tienen endpoint propio: viajan en el mismo `next` que
//! la página de vídeo, dentro de `engagementPanels[]`, y las páginas siguientes
//! se piden reenviando su token de continuación al mismo `next`
//! (`docs/API_PROBE.md` §3 y §8c). Por eso este módulo no añade transporte: usa
//! el perfil `WEB` de [`crate::yt::client`] tal cual.
//!
//! YouTube cambió el formato de los hilos en 2025: el `commentThreadRenderer`
//! nuevo ya no lleva `comment.commentRenderer`, sino un `commentViewModel` con
//! el texto, el autor y los «me gusta» dentro. Aquí se aceptan **las dos**
//! formas, y en el formato antiguo el texto se completa desde
//! `frameworkUpdates.entityBatchUpdate` (API_PROBE §8c: el texto no siempre está
//! en el renderer). Lo que no se pueda leer se deja vacío o se descarta; este
//! módulo no revienta por un campo que falte.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::domain::{Thumbnail, Thumbs};
use crate::yt::client::{ApiError, Innertube};
use crate::yt::parse;

/// Un comentario (o una respuesta, que es el mismo tipo un nivel más abajo).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comment {
    /// Id del comentario: con él se da «me gusta» (`rate_comment`).
    pub id: String,
    pub author: String,
    pub author_avatar: Thumbs,
    /// Canal del autor, si YouTube lo manda (`UC…`).
    pub author_channel_id: Option<String>,
    pub text: String,
    pub published: String,
    /// Texto ya formateado por YouTube ("221", "1,2 mil"), no un número: cada
    /// país lo abrevia a su manera y reinterpretarlo daría cifras falsas.
    pub likes: String,
    pub pinned: bool,
    /// Lo escribió el dueño del canal.
    pub is_owner: bool,
    /// El creador le dio al corazón.
    pub hearted: bool,
    /// Solo si YouTube lo dice; `None` no es «cero respuestas».
    pub reply_count: Option<u32>,
    /// Respuestas ya incluidas en esta respuesta (solo vienen las primeras).
    pub replies: Vec<Comment>,
}

/// Una página de comentarios con su continuación.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommentsPage {
    pub comments: Vec<Comment>,
    /// Token para pedir la página siguiente con [`Innertube::comment_replies`].
    pub continuation: Option<String>,
    /// «1.234 comentarios», tal y como lo cuenta YouTube.
    pub total: Option<String>,
}

/// Una entrada de la bandeja de notificaciones.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Notification {
    pub id: String,
    /// Primera parte del aviso (el canal que lo provoca), si se puede separar.
    pub title: String,
    /// `shortMessage` completo.
    pub text: String,
    pub thumbnail: Thumbs,
    pub published: String,
    pub unread: bool,
    /// `videoId` o `browseId` a donde lleva la notificación.
    pub target: Option<String>,
}

/// `panelIdentifier` del panel de comentarios del `next` del vídeo.
const COMMENT_PANEL_HINT: &str = "comment";

// ---------------------------------------------------------------- lectura

/// Nodo que contiene la sección de comentarios.
///
/// En la respuesta del vídeo es el panel `engagement-panel-comments-section`;
/// el `sectionIdentifier` del `itemSectionRenderer` de dentro es
/// `comment-item-section` (API_PROBE §3). En las continuaciones **no hay
/// panel**: los hilos cuelgan de `onResponseReceivedEndpoints`, así que se
/// devuelve la respuesta entera y la búsqueda por nombre de renderer del módulo
/// [`parse`] encuentra los hilos igual.
fn comment_scope(json: &Value) -> &Value {
    for (_, panel) in parse::collect_vec(json, &["engagementPanelSectionListRenderer"]) {
        let id = panel
            .get("panelIdentifier")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if id.contains(COMMENT_PANEL_HINT) {
            return panel;
        }
    }
    json
}

/// Arrays cuyos elementos son hilos de comentarios, en el orden en que
/// aparecen.
///
/// Se buscan así y no por ruta fija porque la lista de hilos cambia de
/// envoltorio: `itemSectionRenderer.contents[]` en el vídeo,
/// `reloadContinuationItemsCommand.continuationItems[]` en las continuaciones.
/// Tampoco se desciende dentro de un hilo ya encontrado: sus respuestas son
/// otra lista y su token de continuación no es el de la página.
fn thread_lists<'a>(node: &'a Value, out: &mut Vec<&'a [Value]>) {
    match node {
        Value::Object(map) => {
            if map.contains_key("commentThreadRenderer") {
                return;
            }
            for value in map.values() {
                if let Value::Array(items) = value {
                    if items
                        .iter()
                        .any(|i| i.get("commentThreadRenderer").is_some())
                    {
                        out.push(items.as_slice());
                        continue;
                    }
                }
                thread_lists(value, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                thread_lists(value, out);
            }
        }
        _ => {}
    }
}

fn continuation_token_of(item: &Value) -> Option<String> {
    let node = item.get("continuationItemRenderer")?;
    parse::find_key(node, "continuationCommand")
        .and_then(|c| c.get("token"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

/// Token de la página siguiente: el del último `continuationItemRenderer` de la
/// última lista de hilos (API_PROBE §8c). Sin hilos (sección vacía o recién
/// cargada) se acepta cualquier continuación del panel.
fn page_continuation(scope: &Value) -> Option<String> {
    let mut lists: Vec<&[Value]> = Vec::new();
    thread_lists(scope, &mut lists);
    for list in lists.iter().rev() {
        if let Some(token) = list.iter().rev().find_map(continuation_token_of) {
            return Some(token);
        }
    }
    parse::continuation_of(scope)
}

/// «1.234 comentarios» de `commentsHeaderRenderer.countText`.
fn page_total(scope: &Value) -> Option<String> {
    let header = parse::find_key(scope, "commentsHeaderRenderer")?;
    let text = parse::text_at(header, &["countText"]);
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Entidades de `frameworkUpdates`, indexadas por id de comentario.
///
/// En el formato antiguo el texto, la fecha y los «me gusta» viven aquí y no en
/// `commentThreadRenderer` (API_PROBE §8c), así que el índice es obligatorio.
/// Se guarda también la lista en orden para poder emparejar por posición cuando
/// el payload no repita el `commentId`, pero solo si los recuentos cuadran.
fn entity_payloads<'a>(json: &'a Value) -> (HashMap<String, &'a Value>, Vec<&'a Value>) {
    let mut by_id: HashMap<String, &'a Value> = HashMap::new();
    let mut ordered: Vec<&'a Value> = Vec::new();
    for (_, entity) in parse::collect_vec(json, &["commentEntityPayload"]) {
        ordered.push(entity);
        if let Some(id) = parse::dig(entity, &["properties", "commentId"]).and_then(|v| v.as_str())
        {
            by_id.insert(id.to_string(), entity);
        }
    }
    (by_id, ordered)
}

/// Primer número que aparezca en un texto o un nodo numérico. Las cifras de
/// YouTube llegan abreviadas y con separadores de miles del país del usuario.
fn digits(text: &str) -> Option<u32> {
    let only: String = text.chars().filter(char::is_ascii_digit).collect();
    only.parse().ok()
}

fn number_or_digits(node: &Value) -> Option<u32> {
    node.as_u64()
        .map(|n| n as u32)
        .or_else(|| digits(&parse::text(node)))
}

/// Canal del autor: `authorEndpoint.browseEndpoint.browseId` en el formato
/// nuevo y dentro de `navigationEndpoint` en el antiguo.
fn author_channel_id(node: &Value) -> Option<String> {
    for key in ["authorEndpoint", "navigationEndpoint"] {
        let browse = node
            .get(key)
            .and_then(|e| e.get("browseEndpoint"))
            .and_then(|b| b.get("browseId"))
            .and_then(|v| v.as_str())
            .filter(|id| id.starts_with("UC"));
        if let Some(id) = browse {
            return Some(id.to_string());
        }
    }
    parse::endpoints(node).channel_id
}

/// ¿Alguna insignia dice que el autor es el dueño del canal? YouTube lo pone en
/// `tooltip`/`label` de `authorBadges`, traducido al idioma de la sesión.
fn mentions_owner(node: &Value) -> bool {
    for (_, badge) in parse::collect_vec(
        node,
        &["liveChatAuthorBadgeRenderer", "metadataBadgeRenderer"],
    ) {
        for path in [&["tooltip"][..], &["label"][..]] {
            let label = parse::text_at(badge, path).to_lowercase();
            if label.contains("owner") || label.contains("propietario") || label.contains("creador")
            {
                return true;
            }
        }
    }
    false
}

/// Número de respuestas, en cualquiera de las formas conocidas. Sin dato se
/// devuelve `None`: un cero pintaría «0 respuestas» sobre un hilo con 628.
fn reply_count_of(thread: &Value) -> Option<u32> {
    for path in [
        &[
            "replies",
            "commentRepliesRenderer",
            "viewReplies",
            "buttonRenderer",
            "text",
        ][..],
        &[
            "replies",
            "commentRepliesRenderer",
            "moreReplies",
            "buttonRenderer",
            "text",
        ][..],
        &["replyCount"][..],
    ] {
        let text = parse::text_at(thread, path);
        if let Some(n) = digits(&text) {
            return Some(n);
        }
    }
    parse::find_key(thread, "repliesCount").and_then(number_or_digits)
}

/// Respuestas ya cargadas dentro del hilo (las que YouTube incluye antes de
/// pedir la continuación).
fn replies_of(thread: &Value) -> Vec<Comment> {
    let Some(replies) = thread.get("replies") else {
        return Vec::new();
    };
    parse::collect_vec(replies, &["commentThreadRenderer"])
        .into_iter()
        .filter_map(|(_, node)| comment_from_thread(node))
        .collect()
}

/// Comentario de un `commentThreadRenderer`, sin mirar `frameworkUpdates`.
///
/// Formatos:
/// - nuevo: `commentViewModel.commentViewModel` (a veces `commentViewModel` a
///   secas) con `contentText`, `authorText`, `likeCount`…;
/// - antiguo: `comment.commentRenderer` con los mismos datos en otros nombres.
fn comment_from_thread(thread: &Value) -> Option<Comment> {
    if let Some(vm_root) = thread.get("commentViewModel") {
        let vm = vm_root.get("commentViewModel").unwrap_or(vm_root);
        let id = vm
            .get("commentId")
            .or_else(|| vm.get("commentKey"))
            .and_then(|v| v.as_str())?;
        let out = Comment {
            id: id.to_string(),
            author: parse::text_at(vm, &["authorText"]),
            author_avatar: match vm.get("authorThumbnail") {
                Some(t) => parse::thumbs_of(t),
                None => Thumbs::default(),
            },
            author_channel_id: author_channel_id(vm),
            text: parse::text_at(vm, &["contentText"]),
            published: parse::text_at(vm, &["publishedTimeText"]),
            likes: {
                let direct = parse::text_at(vm, &["likeCount"]);
                if direct.is_empty() {
                    parse::text_at(vm, &["likeCountNotliked"])
                } else {
                    direct
                }
            },
            pinned: !parse::text_at(vm, &["pinnedText"]).trim().is_empty(),
            is_owner: vm
                .get("authorIsOwner")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || mentions_owner(vm),
            // El indicador de corazón no se pudo comprobar contra YouTube en
            // vivo: se aceptan los nombres que usa la web y, si no viene, se
            // queda en `false` en vez de inventarlo.
            hearted: vm
                .get("hearted")
                .or_else(|| vm.get("isHearted"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_count: reply_count_of(thread),
            replies: replies_of(thread),
        };
        return non_empty(out);
    }

    let renderer = thread
        .get("comment")
        .and_then(|c| c.get("commentRenderer"))
        .or_else(|| thread.get("commentRenderer"))?;
    let id = renderer.get("commentId").and_then(|v| v.as_str())?;
    let out = Comment {
        id: id.to_string(),
        author: parse::text_at(renderer, &["authorText"]),
        author_avatar: match renderer.get("authorThumbnail") {
            Some(t) => parse::thumbs_of(t),
            None => Thumbs::default(),
        },
        author_channel_id: author_channel_id(renderer),
        text: parse::text_at(renderer, &["contentText"]),
        published: parse::text_at(renderer, &["publishedTimeText"]),
        likes: parse::text_at(renderer, &["voteCount"]),
        pinned: !parse::text_at(renderer, &["pinnedText"]).trim().is_empty(),
        is_owner: renderer
            .get("authorIsOwner")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || mentions_owner(renderer),
        hearted: renderer
            .get("isHearted")
            .or_else(|| renderer.get("hearted"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        reply_count: reply_count_of(thread),
        replies: replies_of(thread),
    };
    non_empty(out)
}

/// Descarta el hilo si no quedó ni autor ni texto: un hilo así no es un
/// comentario, es un formato que no supimos leer.
fn non_empty(comment: Comment) -> Option<Comment> {
    (!comment.text.is_empty() || !comment.author.is_empty()).then_some(comment)
}

/// Completa con la entidad de `frameworkUpdates` lo que el renderer no traiga.
fn fill_from_entity(comment: &mut Comment, entity: &Value) {
    if comment.author.is_empty() {
        comment.author = parse::text_at(entity, &["author", "displayName"]);
    }
    if comment.author_avatar.is_empty() {
        if let Some(url) = parse::dig(entity, &["author", "avatarThumbnailUrl"])
            .and_then(|v| v.as_str())
            .filter(|u| !u.is_empty())
        {
            // La entidad solo da una URL suelta, sin tamaño declarado: se deja a
            // cero porque `Thumbs::best_for` acepta igual una única miniatura.
            comment.author_avatar = Thumbs::new([Thumbnail {
                url: url.to_string(),
                width: 0,
                height: 0,
            }]);
        }
    }
    if comment.author_channel_id.is_none() {
        comment.author_channel_id = parse::dig(entity, &["author", "channelId"])
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty())
            .map(str::to_string);
    }
    if comment.text.is_empty() {
        comment.text = parse::text_at(entity, &["properties", "content", "content"]);
    }
    if comment.published.is_empty() {
        comment.published = parse::text_at(entity, &["properties", "publishedTime"]);
    }
    if comment.likes.is_empty() {
        comment.likes = parse::text_at(entity, &["toolbar", "likeCountNotliked"]);
        if comment.likes.is_empty() {
            comment.likes = parse::text_at(entity, &["toolbar", "likeCountLiked"]);
        }
    }
}

/// Convierte la respuesta de `next` en una página de comentarios.
pub fn comments_from_json(json: &Value) -> CommentsPage {
    let scope = comment_scope(json);
    let (by_id, ordered) = entity_payloads(json);
    let mut lists: Vec<&[Value]> = Vec::new();
    thread_lists(scope, &mut lists);

    let threads: Vec<&Value> = lists
        .iter()
        .flat_map(|list| list.iter())
        .filter_map(|item| item.get("commentThreadRenderer"))
        .collect();

    let mut page = CommentsPage {
        comments: Vec::new(),
        continuation: page_continuation(scope),
        total: page_total(scope),
    };
    let thread_count = threads.len();
    // El mismo hilo puede venir repetido cuando la respuesta trae varios
    // endpoints; pintarlo dos veces sería un fallo visible.
    let mut seen: HashSet<String> = HashSet::new();
    for (index, thread) in threads.into_iter().enumerate() {
        let Some(mut comment) = comment_from_thread(thread) else {
            continue;
        };
        // Emparejar la entidad por id es lo correcto; por posición solo se
        // acepta si el número de entidades y de hilos coincide, porque de lo
        // contrario se pondría el texto de un comentario en otro.
        let entity = by_id.get(&comment.id).copied().or_else(|| {
            ordered
                .get(index)
                .filter(|_| ordered.len() == thread_count)
                .copied()
        });
        if let Some(entity) = entity {
            fill_from_entity(&mut comment, entity);
        }
        if seen.insert(comment.id.clone()) {
            page.comments.push(comment);
        }
    }
    page
}

/// Convierte la respuesta de `notification` en la lista de la bandeja.
///
/// Los nodos se recogen por nombre (`notificationRenderer`) y no por ruta: la
/// bandeja cambia de envoltorio (`multiPageMenuRenderer`, secciones, menús por
/// pestañas) sin avisar.
pub fn notifications_from_json(json: &Value) -> Vec<Notification> {
    let mut out = Vec::new();
    for (index, (_, node)) in parse::collect_vec(json, &["notificationRenderer"])
        .into_iter()
        .enumerate()
    {
        let target = {
            let ep = parse::endpoints(node);
            ep.video_id.or(ep.browse_id)
        };
        let id = node
            .get("notificationId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| target.clone())
            .unwrap_or_else(|| format!("notificacion-{index}"));
        out.push(Notification {
            id,
            title: notification_title(node),
            text: parse::text_at(node, &["shortMessage"]),
            thumbnail: parse::thumbs_of(node),
            published: parse::text_at(node, &["sentTimeText"]),
            // `read` ausente se trata como leída: marcar todo como no leído es
            // peor que no marcar ninguna.
            unread: !node.get("read").and_then(|v| v.as_bool()).unwrap_or(true),
            target,
        });
    }
    out
}

/// Título de la notificación. El aviso llega entero en `shortMessage`
/// («Canal: …»); se separa el primer `run` **solo** cuando hay más texto detrás,
/// para no quedarse con el título y perder el cuerpo.
fn notification_title(node: &Value) -> String {
    let direct = parse::text_at(node, &["title"]);
    if !direct.trim().is_empty() {
        return direct.trim().to_string();
    }
    let Some(runs) = parse::dig(node, &["shortMessage", "runs"]).and_then(|r| r.as_array()) else {
        return String::new();
    };
    if runs.len() < 2 {
        return String::new();
    }
    // Cada `run` lleva `text`, no `simpleText`/`content`: `parse::text` sobre el
    // run suelto devolvería la cadena vacía.
    runs.first()
        .and_then(|run| run.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_string()
}

/// Mismo criterio que `api::check_errors` (privado): un `alertRenderer` es un
/// texto para el usuario, no un resultado válido.
fn check_errors(json: &Value) -> Result<(), ApiError> {
    if let Some(err) = parse::find_key(json, "alertRenderer") {
        let text = parse::text_at(err, &["text"]);
        if !text.trim().is_empty() {
            return Err(ApiError::Unavailable(text.trim().to_string()));
        }
    }
    if let Some(err) = json.get("error") {
        let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        return Err(ApiError::Http {
            status: code as u16,
            body: parse::text_at(err, &["message"]),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------- peticiones

impl Innertube {
    /// Comentarios de un vídeo o Short: el panel viene en el mismo `next` que la
    /// página de vídeo.
    ///
    /// Sin sesión también funciona; solo se lee. Si el panel no aparece (vídeo
    /// con los comentarios desactivados) se devuelve una página vacía: no hay
    /// texto de error que YouTube dé en ese caso.
    pub async fn comments(&self, video_id: &str) -> Result<CommentsPage, ApiError> {
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
        Ok(comments_from_json(&json))
    }

    /// Página siguiente de comentarios o de respuestas: el token ya lleva el
    /// contexto, así que la petición es la misma (`next` + `continuation`,
    /// API_PROBE §8c).
    pub async fn comment_replies(
        &self,
        continuation_token: &str,
    ) -> Result<CommentsPage, ApiError> {
        let json = self
            .post_web("next", json!({ "continuation": continuation_token }))
            .await?;
        check_errors(&json)?;
        Ok(comments_from_json(&json))
    }

    /// Publica un comentario de primer nivel.
    ///
    /// **Limitación documentada**: `createCommentParams` no lo genera el
    /// cliente, lo publica YouTube en el `next` del vídeo; si esa respuesta no
    /// lo trae, no se puede comentar por esta vía y se devuelve
    /// [`ApiError::Unavailable`] con el motivo. La forma del cuerpo
    /// (`createCommentParams` + `commentText.runs`) **no** se pudo verificar
    /// contra YouTube en vivo desde este entorno: si falla, el error será el de
    /// YouTube, no un éxito fingido.
    pub async fn post_comment(&self, video_id: &str, text: &str) -> Result<(), ApiError> {
        if !self.is_logged_in() {
            return Err(ApiError::NeedsLogin);
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(ApiError::Unavailable("El comentario está vacío.".into()));
        }
        let json = self
            .post_web("next", json!({ "videoId": video_id }))
            .await?;
        check_errors(&json)?;
        let Some(params) = parse::find_key(&json, "createCommentParams").and_then(|p| p.as_str())
        else {
            return Err(ApiError::Unavailable(
                "comentar requiere un parámetro que YouTube no devolvió".into(),
            ));
        };
        let json = self
            .post_web(
                "comment/create_comment",
                json!({
                    "createCommentParams": params,
                    "commentText": {"runs": [{"text": text}]},
                }),
            )
            .await?;
        check_errors(&json)?;
        Ok(())
    }

    /// «Me gusta» o «no me gusta» de un comentario.
    ///
    /// `ACTION_LIKE_COMMENT` es la acción observada; la de «no me gusta» se
    /// deduce por simetría del nombre y **no** se pudo verificar en vivo, así
    /// que un fallo se devuelve tal cual en vez de darlo por hecho.
    pub async fn rate_comment(&self, comment_id: &str, up: bool) -> Result<(), ApiError> {
        if !self.is_logged_in() {
            return Err(ApiError::NeedsLogin);
        }
        let action = if up {
            "ACTION_LIKE_COMMENT"
        } else {
            "ACTION_DISLIKE_COMMENT"
        };
        let json = self
            .post_web(
                "comment/perform_comment_action",
                json!({
                    "target": {"commentId": comment_id},
                    "actions": [action],
                }),
            )
            .await?;
        check_errors(&json)?;
        Ok(())
    }

    /// Bandeja de notificaciones de la cuenta.
    ///
    /// `notification` con el perfil `WEB` de la sesión; sin sesión no hay
    /// bandeja que pedir, así que se pide iniciar sesión en vez de devolver una
    /// lista vacía que parecería «no tienes notificaciones».
    pub async fn notifications(&self) -> Result<Vec<Notification>, ApiError> {
        if !self.is_logged_in() {
            return Err(ApiError::NeedsLogin);
        }
        let json = self
            .post_web(
                "notification",
                json!({
                    "notificationsMenuRequestType": "NOTIFICATIONS_MENU_REQUEST_TYPE_INBOX",
                }),
            )
            .await?;
        check_errors(&json)?;
        Ok(notifications_from_json(&json))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Formato nuevo (2025+): el hilo trae `commentViewModel` y el panel cuelga
    /// de `engagementPanels`.
    #[test]
    fn lee_comentario_en_formato_nuevo() {
        let json = json!({
            "engagementPanels": [{
                "engagementPanelSectionListRenderer": {
                    "panelIdentifier": "engagement-panel-comments-section",
                    "content": {"sectionListRenderer": {"contents": [{
                        "itemSectionRenderer": {
                            "sectionIdentifier": "comment-item-section",
                            "contents": [
                                {"commentsHeaderRenderer": {
                                    "countText": {"runs": [{"text": "1.234 comentarios"}]}
                                }},
                                {"commentThreadRenderer": {
                                    "commentViewModel": {"commentViewModel": {
                                        "commentId": "Ugx1",
                                        "commentKey": "Ugx1",
                                        "authorText": {"simpleText": "@ana"},
                                        "authorThumbnail": {"thumbnails": [
                                            {"url": "https://i.ytimg.com/vi/a/default.jpg", "width": 48, "height": 48}
                                        ]},
                                        "contentText": {"runs": [{"text": "Buen "}, {"text": "vídeo"}]},
                                        "publishedTimeText": {"runs": [{"text": "hace 2 días"}]},
                                        "likeCount": {"simpleText": "12"},
                                        "pinnedText": {"runs": [{"text": "Fijado por el creador"}]},
                                        "authorEndpoint": {"browseEndpoint": {"browseId": "UCana"}}
                                    }},
                                    "replies": {"commentRepliesRenderer": {
                                        "viewReplies": {"buttonRenderer": {"text": {"runs": [{"text": "3 respuestas"}]}}}
                                    }}
                                }}
                            ]
                        }
                    }, {
                        "continuationItemRenderer": {"continuationEndpoint": {"continuationCommand": {"token": "TOKEN-P1"}}}
                    }]}}
                }
            }]
        });
        let page = comments_from_json(&json);
        assert_eq!(page.comments.len(), 1);
        let comment = &page.comments[0];
        assert_eq!(comment.id, "Ugx1");
        assert_eq!(comment.author, "@ana");
        assert_eq!(comment.text, "Buen vídeo");
        assert_eq!(comment.published, "hace 2 días");
        assert_eq!(comment.likes, "12");
        assert!(comment.pinned);
        assert_eq!(comment.reply_count, Some(3));
        assert_eq!(comment.author_channel_id.as_deref(), Some("UCana"));
        assert_eq!(
            comment.author_avatar.first().unwrap().url,
            "https://i.ytimg.com/vi/a/default.jpg"
        );
        assert_eq!(page.total.as_deref(), Some("1.234 comentarios"));
        assert_eq!(page.continuation.as_deref(), Some("TOKEN-P1"));
    }

    /// Formato antiguo: el renderer no trae texto; hay que sacarlo de
    /// `frameworkUpdates.entityBatchUpdate` (API_PROBE §8c).
    #[test]
    fn lee_comentario_antiguo_con_framework_updates() {
        let json = json!({
            "frameworkUpdates": {"entityBatchUpdate": {"mutations": [
                {"payload": {"commentEntityPayload": {
                    "properties": {
                        "commentId": "Ugx2",
                        "content": {"content": "Texto desde las entidades"},
                        "publishedTime": "hace 6 años"
                    },
                    "author": {"displayName": "@leo", "channelId": "UCleo"},
                    "toolbar": {"likeCountNotliked": "221"}
                }}}
            ]}},
            "onResponseReceivedEndpoints": [
                {"reloadContinuationItemsCommand": {"continuationItems": [
                    {"commentsHeaderRenderer": {"countText": {"simpleText": "20 comentarios"}}}
                ]}},
                {"reloadContinuationItemsCommand": {"continuationItems": [
                    {"commentThreadRenderer": {"comment": {"commentRenderer": {
                        "commentId": "Ugx2",
                        "authorText": {"simpleText": "@leo"}
                    }}}},
                    {"continuationItemRenderer": {"continuationEndpoint": {"continuationCommand": {"token": "TOKEN-P2"}}}}
                ]}}
            ]
        });
        let page = comments_from_json(&json);
        assert_eq!(page.comments.len(), 1);
        let comment = &page.comments[0];
        assert_eq!(comment.id, "Ugx2");
        assert_eq!(comment.author, "@leo");
        assert_eq!(comment.text, "Texto desde las entidades");
        assert_eq!(comment.published, "hace 6 años");
        assert_eq!(comment.likes, "221");
        assert_eq!(comment.author_channel_id.as_deref(), Some("UCleo"));
        assert_eq!(page.total.as_deref(), Some("20 comentarios"));
        assert_eq!(page.continuation.as_deref(), Some("TOKEN-P2"));
    }

    /// El token de la página es el de la lista que trae hilos, no el que va
    /// dentro de un hilo (respuestas) ni el de la cabecera.
    #[test]
    fn saca_el_token_de_continuacion_de_la_lista_de_hilos() {
        let json = json!({
            "onResponseReceivedEndpoints": [
                {"reloadContinuationItemsCommand": {"continuationItems": [
                    {"commentsHeaderRenderer": {}},
                    {"continuationItemRenderer": {"continuationEndpoint": {"continuationCommand": {"token": "CABECERA"}}}}
                ]}},
                {"reloadContinuationItemsCommand": {"continuationItems": [
                    {"commentThreadRenderer": {"commentViewModel": {
                        "commentId": "a", "contentText": {"runs": [{"text": "uno"}]}
                    }}},
                    {"commentThreadRenderer": {"commentViewModel": {
                        "commentId": "b", "contentText": {"runs": [{"text": "dos"}]}
                    }}},
                    {"continuationItemRenderer": {"continuationEndpoint": {"continuationCommand": {"token": "PAGINA-2"}}}}
                ]}}
            ]
        });
        let page = comments_from_json(&json);
        assert_eq!(page.continuation.as_deref(), Some("PAGINA-2"));
        assert_eq!(page.comments.len(), 2);
    }

    #[test]
    fn lee_una_notificacion() {
        let json = json!({
            "contents": [{"notificationRenderer": {
                "notificationId": "n1",
                "shortMessage": {"runs": [{"text": "@ana"}, {"text": ": vídeo nuevo"}]},
                "sentTimeText": {"simpleText": "hace 2 horas"},
                "read": false,
                "thumbnail": {"thumbnails": [
                    {"url": "https://i.ytimg.com/vi/abc123/default.jpg", "width": 120, "height": 90}
                ]},
                "navigationEndpoint": {"watchEndpoint": {"videoId": "abc123"}}
            }}]
        });
        let list = notifications_from_json(&json);
        assert_eq!(list.len(), 1);
        let notification = &list[0];
        assert_eq!(notification.id, "n1");
        assert_eq!(notification.title, "@ana");
        assert_eq!(notification.text, "@ana: vídeo nuevo");
        assert_eq!(notification.published, "hace 2 horas");
        assert!(notification.unread);
        assert_eq!(notification.target.as_deref(), Some("abc123"));
        assert_eq!(
            notification.thumbnail.first().unwrap().url,
            "https://i.ytimg.com/vi/abc123/default.jpg"
        );
    }

    /// Ni una respuesta vacía ni un hilo sin id deben tumbar el parseo: se
    /// descarta lo ilegible y se sigue.
    #[test]
    fn no_revienta_con_nodos_incompletos() {
        let empty = json!({});
        assert_eq!(comments_from_json(&empty), CommentsPage::default());
        assert!(notifications_from_json(&empty).is_empty());

        let roto = json!({
            "itemSectionRenderer": {"contents": [
                {"commentThreadRenderer": {"commentViewModel": {"commentViewModel": {"authorText": {"simpleText": "@nadie"}}}}},
                {"commentThreadRenderer": {"comment": {"commentRenderer": {"authorText": {"simpleText": "@sin-id"}}}}}
            ]}
        });
        assert!(comments_from_json(&roto).comments.is_empty());
    }
}
