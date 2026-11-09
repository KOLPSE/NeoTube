//! Página de canal: banner, cabecera, pestañas y contenido.
//!
//! La misma pantalla sirve para el canal propio y para uno ajeno (capturas 09 y
//! 10). Lo único que cambia son las acciones de la cabecera: el canal propio
//! ofrece «Personalizar canal» y «Gestionar vídeos», el ajeno ofrece la
//! suscripción. Lo decide `own`, no una comparación de ids: el llamador sabe si
//! el canal es la cuenta activa.
//!
//! Los canales se piden por `browseId` (`UC…`). La pestaña activa **filtra** los
//! estantes que ya vinieron en el feed; no se pide nada nuevo al cambiarla.
//!
//! Lo que el modelo no trae no se pinta: `ChannelInfo` guarda id, nombre,
//! `@handle`, suscriptores, avatar y banner, así que la descripción, la fila de
//! enlaces, la insignia de verificado y el recuento de vídeos de la referencia
//! esperan a que el dominio los tenga. Inventarlos aquí sería mentir en la
//! cabecera de un canal real.

use egui::{CornerRadius, FontId, Rect, Response, RichText, Sense, Vec2};

use crate::domain::{Card, ChannelInfo, Feed, MediaKind, Shelf, ShelfLayout};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::icons::{self, Icon};
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, CardAction, PillKind, ThumbLookup};

/// Pestañas de la referencia (capturas 09 y 10). «Directos» no aparece en
/// ninguna de las dos y el feed de canal no distingue directos, así que no se
/// añade: una pestaña que no lleva a ninguna parte es peor que una que falta.
const TABS: &[&str] = &["Inicio", "Vídeos", "Shorts", "Listas", "Publicaciones"];

/// Tarjetas visibles en un carrusel. La captura 10 enseña exactamente seis
/// vídeos y la flecha de avance.
const ROW_VISIBLE: usize = 6;

/// Hueco entre el banner y la cabecera. En la captura 10 son ~24 px: el avatar
/// queda **debajo** del banner, no montado sobre él.
const BANNER_TO_HEADER: f32 = 24.0;

/// Hueco entre el avatar y la columna de texto de la cabecera.
const AVATAR_GAP: f32 = 24.0;

/// Hueco entre la línea de `@handle`/suscriptores y las acciones.
const ACTIONS_GAP: f32 = 16.0;

/// Alto de la fila de pestañas; el subrayado de la activa vive dentro.
const TAB_HEIGHT: f32 = 48.0;

/// Relleno lateral de cada pestaña (el subrayado mide lo que el texto).
const TAB_PAD_X: f32 = 16.0;

/// Grosor del subrayado de la pestaña activa, como la referencia.
const TAB_UNDERLINE: f32 = 2.0;

/// Hueco entre un título de sección y su fila de tarjetas.
const TITLE_GAP: f32 = 8.0;

/// Hueco entre secciones del contenido.
const SECTION_GAP: f32 = 32.0;

/// Radio de la flecha circular de los carruseles.
const ARROW_RADIUS: f32 = 20.0;

pub fn channel(
    ui: &mut egui::Ui,
    browse_id: &str,
    info: Option<&ChannelInfo>,
    feed: &Feed,
    load: &Load,
    active_tab: usize,
    subscribed: bool,
    own: bool,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;

    egui::ScrollArea::vertical()
        .id_salt(("channel", browse_id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            banner(ui, info, &mut *lookup);
            if let Some(i) = header(ui, browse_id, info, feed, subscribed, own, &mut *lookup) {
                intent = Some(i);
            }
            if let Some(i) = tabs(ui, browse_id, active_tab) {
                intent = Some(i);
            }
            if let Some(i) = content(ui, active_tab, feed, load, &mut *lookup) {
                intent = Some(i);
            }
            ui.allocate_exact_size(Vec2::new(1.0, SECTION_GAP), Sense::hover());
        });

    intent
}

// ------------------------------------------------------------------- banner

/// Banner del canal, a todo el ancho del contenido y con el radio de panel.
///
/// Sin banner **no** se reserva la franja: la captura 09 (canal propio sin
/// banner) empieza directamente en la cabecera, y 192 px vacíos ahí se leerían
/// como un fallo de carga.
fn banner(ui: &mut egui::Ui, info: Option<&ChannelInfo>, lookup: ThumbLookup<'_>) {
    let Some(thumbs) = info.map(|i| &i.banner).filter(|t| !t.is_empty()) else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), metrics::BANNER),
        Sense::hover(),
    );
    widgets::thumb_image(ui, rect, thumbs, metrics::RADIUS_PANEL, lookup);
    ui.allocate_exact_size(Vec2::new(1.0, BANNER_TO_HEADER), Sense::hover());
}

// ------------------------------------------------------------------ cabecera

/// Avatar, nombre, `@handle`, suscriptores y acciones del canal.
fn header(
    ui: &mut egui::Ui,
    browse_id: &str,
    info: Option<&ChannelInfo>,
    feed: &Feed,
    subscribed: bool,
    own: bool,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    ui.horizontal_top(|ui| {
        let (avatar_rect, _) =
            ui.allocate_exact_size(Vec2::splat(metrics::AVATAR_CHANNEL), Sense::hover());
        // Sin avatar, `avatar_image` pinta el círculo de esqueleto del tema: es
        // el estado de carga, no un avatar inventado.
        let avatar = info.map(|i| i.avatar.clone()).unwrap_or_default();
        widgets::avatar_image(ui, avatar_rect, &avatar, &mut *lookup);
        ui.allocate_exact_size(Vec2::new(AVATAR_GAP, 1.0), Sense::hover());

        ui.vertical(|ui| {
            ui.add(egui::Label::new(
                RichText::new(channel_name(info, feed))
                    .size(font::CHANNEL_NAME)
                    .strong()
                    .color(p.text),
            ));

            // `@handle` y suscriptores comparten línea, como la captura 10. El
            // recuento de vídeos se omite porque no está en `ChannelInfo`.
            let mut line: Vec<String> = Vec::new();
            if let Some(handle) = info
                .and_then(|i| i.handle.as_deref())
                .map(str::trim)
                .filter(|h| !h.is_empty())
            {
                line.push(if handle.starts_with('@') {
                    handle.to_string()
                } else {
                    format!("@{handle}")
                });
            }
            if let Some(subs) = info
                .and_then(|i| i.subscribers.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                line.push(subs.to_string());
            }
            if !line.is_empty() {
                ui.add(egui::Label::new(
                    RichText::new(line.join(" · "))
                        .size(font::BODY)
                        .color(p.text2),
                ));
            }

            ui.allocate_exact_size(Vec2::new(1.0, ACTIONS_GAP), Sense::hover());
            ui.horizontal(|ui| {
                if own {
                    // Canal propio: gestión, sin suscripción. Las dos acciones
                    // viven en YouTube Studio, que no está en el cliente (§9.2,
                    // «requiere web»): se abre la página correspondiente en el
                    // navegador en vez de dejar un botón que no hace nada.
                    if widgets::pill(ui, "Personalizar canal", None, PillKind::Filled).clicked() {
                        intent = Some(Intent::OpenExternal(format!(
                            "https://studio.youtube.com/channel/{browse_id}/editing"
                        )));
                    }
                    if widgets::pill(ui, "Gestionar vídeos", None, PillKind::Filled).clicked() {
                        intent = Some(Intent::OpenExternal(format!(
                            "https://studio.youtube.com/channel/{browse_id}/videos"
                        )));
                    }
                } else if subscribe_pill(ui, subscribed).clicked() {
                    intent = Some(Intent::ToggleSubscribe {
                        channel_id: browse_id.to_string(),
                        on: !subscribed,
                    });
                }
            });
        });
    });

    intent
}

/// Píldora de suscripción del canal ajeno: campana, texto y chevron en una sola
/// superficie, como la captura 10.
///
/// El chevron anuncia el menú de notificaciones, que la aplicación todavía no
/// tiene (§1.2): la píldora entera suscribe o cancela, y no se dibuja un menú
/// vacío detrás de la flecha.
fn subscribe_pill(ui: &mut egui::Ui, subscribed: bool) -> Response {
    let p = palette(ui.ctx());
    let label = if subscribed {
        "Suscrito"
    } else {
        "Suscribirse"
    };
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(font::BUTTON),
        p.text,
    );
    let icon_w = 24.0;
    let chevron_w = if subscribed { 20.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 32.0 + icon_w + chevron_w, 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let bg = if response.hovered() { p.hover } else { p.chip };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::RADIUS_PILL), bg);
    icons::draw(
        ui,
        Rect::from_center_size(
            egui::pos2(rect.left() + 22.0, rect.center().y),
            Vec2::splat(20.0),
        ),
        if subscribed { Icon::Bell } else { Icon::Plus },
        p.text,
    );
    let text_left = rect.left() + 16.0 + icon_w;
    let text_size = galley.size();
    ui.painter().galley(
        egui::pos2(text_left, rect.center().y - text_size.y * 0.5),
        galley,
        p.text,
    );
    if subscribed {
        icons::draw(
            ui,
            Rect::from_center_size(
                egui::pos2(rect.right() - 18.0, rect.center().y),
                Vec2::splat(16.0),
            ),
            Icon::ChevronDown,
            p.text,
        );
    }
    response
}

/// Nombre del canal. Si el feed no trae cabecera (`ChannelInfo`), se usa el
/// título de la única tarjeta de canal del estante, que sí es un dato; si
/// tampoco hay, un texto neutro.
fn channel_name(info: Option<&ChannelInfo>, feed: &Feed) -> String {
    if let Some(name) = info.map(|i| i.name.as_str()).filter(|n| !n.is_empty()) {
        return name.to_string();
    }
    feed.shelves
        .iter()
        .flat_map(|s| s.items.iter())
        .find(|c| c.kind == MediaKind::Channel)
        .map(|c| c.title.clone())
        .unwrap_or_else(|| "Canal".to_string())
}

// ------------------------------------------------------------------ pestañas

/// Fila de pestañas con lupa al final y separador a todo el ancho.
fn tabs(ui: &mut egui::Ui, browse_id: &str, active_tab: usize) -> Option<Intent> {
    let mut intent = None;
    ui.horizontal(|ui| {
        for (index, label) in TABS.iter().enumerate() {
            if tab(ui, label, index == active_tab).clicked() {
                intent = Some(Intent::SetChannelTab(index));
            }
        }
        // La lupa de la referencia busca dentro del canal; esa búsqueda no está
        // en el cliente, así que abre la del navegador en vez de fingir un campo
        // que no filtra.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button(ui, Icon::Search, 20.0, false, "Buscar en el canal").clicked() {
                intent = Some(Intent::OpenExternal(format!(
                    "https://www.youtube.com/channel/{browse_id}/search?query="
                )));
            }
        });
    });
    widgets::divider(ui);
    intent
}

/// Pestaña de texto con subrayado de 2 px en la activa.
///
/// No se usa `widgets::chip`: en la referencia la pestaña activa es texto
/// subrayado, no una píldora rellena como los filtros de la búsqueda.
fn tab(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let p = palette(ui.ctx());
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(font::CHIP), p.text);
    let size = Vec2::new(galley.size().x + TAB_PAD_X * 2.0, TAB_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::RADIUS_CHIP), p.hover);
    }
    let text_size = galley.size();
    ui.painter().galley(
        egui::pos2(
            rect.center().x - text_size.x * 0.5,
            rect.center().y - text_size.y * 0.5,
        ),
        galley,
        if selected { p.text } else { p.text2 },
    );
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(
                egui::pos2(rect.left() + TAB_PAD_X, rect.bottom() - TAB_UNDERLINE),
                Vec2::new(text_size.x, TAB_UNDERLINE),
            ),
            0,
            p.text,
        );
    }
    response
}

// ------------------------------------------------------------------ contenido

/// Secciones del canal filtradas por la pestaña activa.
fn content(
    ui: &mut egui::Ui,
    active_tab: usize,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;

    if let Load::Failed(message) = load {
        if widgets::error_state(ui, message) {
            intent = Some(Intent::Reload);
        }
        return intent;
    }

    let visible: Vec<&Shelf> = feed
        .shelves
        .iter()
        .filter(|shelf| {
            !shelf.items.is_empty() && shelf.items.iter().any(|c| tab_accepts(active_tab, c.kind))
        })
        .collect();

    if visible.is_empty() {
        if load.is_loading() {
            widgets::skeleton_grid(ui, 12);
        } else if active_tab > 3 {
            widgets::empty_state(
                ui,
                "Sin publicaciones",
                "Las publicaciones de la comunidad no forman parte del modelo de datos todavía.",
            );
        } else {
            widgets::empty_state(
                ui,
                "Este canal no tiene contenido visible",
                "Puede que el contenido esté oculto o requiera sesión.",
            );
        }
        return intent;
    }

    for (index, shelf) in visible.iter().enumerate() {
        if let Some(title) = &shelf.title {
            widgets::section_title(ui, title);
            ui.allocate_exact_size(Vec2::new(1.0, TITLE_GAP), Sense::hover());
        }

        // Estantes largos, en rejilla: doce tarjetas no caben en un carrusel sin
        // obligar a desplazarse para ver lo que sí entra en dos filas.
        let as_grid =
            matches!(shelf.layout, ShelfLayout::Grid) || shelf.items.len() > ROW_VISIBLE * 2;
        let action = if matches!(shelf.layout, ShelfLayout::List) {
            widgets::card_list(ui, &shelf.items, &mut *lookup)
        } else if as_grid {
            widgets::card_grid(ui, &shelf.items, &mut *lookup)
        } else {
            paged_row(ui, index, &shelf.items, &mut *lookup)
        };
        if let Some(a) = action {
            intent = Some(intent_from(a));
        }
        ui.allocate_exact_size(Vec2::new(1.0, SECTION_GAP), Sense::hover());
    }

    intent
}

/// ¿Esta pestaña enseña tarjetas de este tipo?
///
/// «Vídeos» incluye los directos ya emitidos, como en la web. «Publicaciones»
/// no tiene equivalente en `MediaKind`, así que no acepta nada: no se inventa.
fn tab_accepts(tab: usize, kind: MediaKind) -> bool {
    match tab {
        0 => true,
        1 => matches!(kind, MediaKind::Video | MediaKind::Live),
        2 => kind == MediaKind::Short,
        3 => kind == MediaKind::Playlist,
        _ => false,
    }
}

/// Carrusel de tarjetas con flechas, seis por página.
///
/// Se pagina en vez de desplazarse para que la flecha sepa cuándo ha llegado al
/// final: en los canales `Shelf::more` llega vacío, así que no hay paginación
/// del servidor que aprovechar y la página se guarda en la memoria de egui.
fn paged_row(
    ui: &mut egui::Ui,
    key: usize,
    cards: &[Card],
    lookup: ThumbLookup<'_>,
) -> Option<CardAction> {
    // Un estante sin tarjetas no tiene flechas que colocar: sin esta salida, la
    // fila vacía dejaría el rectángulo del carrusel en `Rect::NOTHING` y las
    // flechas se pintarían en una posición infinita.
    if cards.is_empty() {
        return None;
    }
    let p = palette(ui.ctx());
    let gap = metrics::GRID_GAP;
    let width =
        ((ui.available_width() - gap * (ROW_VISIBLE - 1) as f32) / ROW_VISIBLE as f32).max(120.0);
    let pages = cards.len().div_ceil(ROW_VISIBLE);
    let id = ui.scope_id().with(("channel_row", key));
    let page = ui
        .data_mut(|d| *d.get_temp_mut_or_default::<usize>(id))
        .min(pages.saturating_sub(1));

    let mut action = None;
    let mut row = Rect::NOTHING;
    ui.horizontal_top(|ui| {
        for (i, card) in cards
            .iter()
            .skip(page * ROW_VISIBLE)
            .take(ROW_VISIBLE)
            .enumerate()
        {
            // El hueco va **antes** de cada tarjeta menos la primera: así el
            // borde derecho de la fila es el de la última tarjeta y la flecha se
            // ancla ahí sin un margen de más.
            if i > 0 {
                ui.allocate_exact_size(Vec2::new(gap, 1.0), Sense::hover());
            }
            let a = ui
                .allocate_ui_with_layout(
                    Vec2::new(width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| widgets::grid_card(ui, card, width, &mut *lookup),
                )
                .inner;
            if a.is_some() {
                action = a;
            }
        }
        row = ui.min_rect();
    });

    // Flechas sobre el borde de la fila, a la altura de las miniaturas.
    let arrow_y = row.top() + width * 9.0 / 16.0 * 0.5;
    if page + 1 < pages {
        let center = egui::pos2(row.right() - ARROW_RADIUS * 0.5, arrow_y);
        let hit = Rect::from_center_size(center, Vec2::splat(ARROW_RADIUS * 2.0));
        let response = ui.interact(hit, id.with("next"), Sense::click());
        ui.painter().circle_filled(center, ARROW_RADIUS, p.scrim);
        icons::draw(
            ui,
            Rect::from_center_size(center, Vec2::splat(18.0)),
            Icon::ChevronRight,
            p.on_brand,
        );
        if response.clicked() {
            ui.data_mut(|d| *d.get_temp_mut_or_default::<usize>(id) = page + 1);
        }
    }
    if page > 0 {
        let center = egui::pos2(row.left() + ARROW_RADIUS * 0.5, arrow_y);
        let hit = Rect::from_center_size(center, Vec2::splat(ARROW_RADIUS * 2.0));
        let response = ui.interact(hit, id.with("previous"), Sense::click());
        ui.painter().circle_filled(center, ARROW_RADIUS, p.scrim);
        icons::draw(
            ui,
            Rect::from_center_size(center, Vec2::splat(18.0)),
            Icon::ChevronLeft,
            p.on_brand,
        );
        if response.clicked() {
            ui.data_mut(|d| *d.get_temp_mut_or_default::<usize>(id) = page - 1);
        }
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Thumbs;

    fn card(id: &str, kind: MediaKind, title: &str) -> Card {
        let mut card = Card::new(id, kind, title);
        card.thumbs = Thumbs::default();
        card
    }

    fn feed_with(items: Vec<Card>) -> Feed {
        Feed {
            shelves: vec![Shelf {
                title: Some("Vídeos".to_string()),
                items,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn info() -> ChannelInfo {
        ChannelInfo {
            id: "UC1".to_string(),
            name: "Canal de prueba".to_string(),
            handle: Some("canal-de-prueba".to_string()),
            subscribers: Some("1,2 K suscriptores".to_string()),
            avatar: Thumbs::default(),
            banner: Thumbs::default(),
        }
    }

    /// Pinta la pantalla en un contexto de egui sin ventana. Es la única forma
    /// de recorrer cabecera, pestañas y contenido (incluidos los estantes, que
    /// piden texturas que aquí no existen) sin abrir una ventana.
    fn render(own: bool, active_tab: usize) -> usize {
        let ctx = egui::Context::default();
        let info = info();
        let feed = feed_with(vec![card("v1", MediaKind::Video, "Un vídeo")]);
        let load = Load::Ready;
        let mut lookup = |_t: &Thumbs, _w: u32| -> Option<egui::TextureId> { None };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                Vec2::new(1280.0, 800.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let intent = channel(
                    ui,
                    "UC1",
                    Some(&info),
                    &feed,
                    &load,
                    active_tab,
                    false,
                    own,
                    &mut lookup,
                );
                // Sin interacción no se emite ninguna intención: nada de
                // acciones implícitas al abrir la página.
                assert!(intent.is_none());
            });
        });
        // El atlas de fuentes deja un delta de textura pendiente; sin manejarlo,
        // `epaint` entra en pánico al soltar la salida.
        output.textures_delta.clear();
        output.shapes.len()
    }

    #[test]
    fn las_dos_variantes_se_pintan_sin_panico() {
        assert!(render(true, 0) > 0);
        assert!(render(false, 0) > 0);
        // «Publicaciones» no tiene datos en el modelo: solo estado vacío.
        assert!(render(false, 4) > 0);
    }

    #[test]
    fn publicaciones_no_acepta_ningun_tipo_conocido() {
        for kind in [
            MediaKind::Video,
            MediaKind::Short,
            MediaKind::Live,
            MediaKind::Playlist,
            MediaKind::Channel,
        ] {
            assert!(!tab_accepts(4, kind));
        }
        // «Inicio» no filtra y «Vídeos» incluye los directos ya emitidos.
        assert!(tab_accepts(0, MediaKind::Channel));
        assert!(tab_accepts(1, MediaKind::Live));
        assert!(!tab_accepts(3, MediaKind::Video));
    }

    #[test]
    fn nombre_cae_al_titulo_de_la_tarjeta_de_canal() {
        let feed = feed_with(vec![card("UC9", MediaKind::Channel, "Otro canal")]);
        assert_eq!(channel_name(None, &feed), "Otro canal");
        assert_eq!(channel_name(None, &Feed::default()), "Canal");
        assert_eq!(channel_name(Some(&info()), &feed), "Canal de prueba");
    }
}
