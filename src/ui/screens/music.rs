//! Música: portada, álbum, artista y lista.
//!
//! Comparte los modelos con el resto (`Feed`/`Shelf`), así que las pantallas son
//! casi la misma composición con cabeceras distintas. Lo que cambia de verdad es
//! la forma de la carátula: en Música es **cuadrada** (radio 8) y en el resto de
//! la aplicación las miniaturas son 16:9 (radio 12). Por eso aquí no se usa
//! [`crate::ui::widgets::grid_card`] salvo para lo que sí es vídeo.
//!
//! La identidad musical no trae barra propia: usa la misma barra superior, la
//! misma barra lateral y los mismos chips que el resto de destinos. Esta pantalla
//! solo pinta contenido.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Vec2};

use crate::domain::{Card, Feed, MediaKind, Shelf, ShelfLayout};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::icons::{self, Icon};
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, PillKind, ThumbLookup};

/// Lado de la carátula de la cabecera de ficha.
const HERO_COVER: f32 = 200.0;
/// Lado de las carátulas de los carruseles de la portada.
const COVER: f32 = 180.0;
/// Miniatura de una fila de pista.
const TRACK_THUMB: f32 = 56.0;
/// Alto de una fila de pista: la miniatura más 8 px de aire.
const TRACK_ROW_H: f32 = 64.0;
/// Ancho de la columna del número de pista.
const TRACK_NUMBER_W: f32 = 34.0;
/// Ancho de la columna de duración.
const TRACK_DURATION_W: f32 = 52.0;
/// Tamaño del título de la ficha. Es el número más grande de la aplicación y solo
/// se usa aquí; 34 px es la medida de la cabecera de Música.
const HERO_TITLE: f32 = 34.0;

// ------------------------------------------------------------------ portada

/// Portada de YouTube Music: un carrusel por sección, como la web.
pub fn music_home(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;

    // Un fallo que ya trae contenido detrás no borra la pantalla: se pinta lo que
    // hay y el aviso llega por el `toast` del despachador.
    if feed.shelves.is_empty() {
        if let Load::Failed(message) = load {
            if widgets::error_state(ui, message) {
                intent = Some(Intent::Reload);
            }
            return intent;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt("music_home")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            widgets::section_header(ui, "Música", Some("YouTube Music"));

            if feed.shelves.is_empty() {
                match load {
                    Load::Loading | Load::Idle => widgets::skeleton_grid(ui, 8),
                    Load::Failed(message) => {
                        if widgets::error_state(ui, message) {
                            intent = Some(Intent::Reload);
                        }
                    }
                    Load::Ready => widgets::empty_state(
                        ui,
                        "Nada por aquí",
                        "YouTube Music no devolvió contenido para la portada.",
                    ),
                }
                return;
            }

            for shelf in &feed.shelves {
                if shelf.items.is_empty() {
                    continue;
                }
                if let Some(title) = &shelf.title {
                    ui.label(
                        RichText::new(title)
                            .size(font::SECTION_TITLE)
                            .strong()
                            .color(p.text),
                    );
                    gap(ui, 8.0);
                }
                if let Some(i) = render_shelf(ui, shelf, &mut *lookup) {
                    intent = Some(i);
                }
                gap(ui, 20.0);
            }

            // Paginación real: solo se ofrece si la respuesta trajo continuación.
            if feed.continuation.is_some() {
                if load.is_loading() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Cargando más…")
                                .size(font::META)
                                .color(p.text2),
                        );
                    });
                } else if widgets::pill(ui, "Cargar más", None, PillKind::Filled).clicked() {
                    intent = Some(Intent::LoadMore);
                }
            }
        });

    intent
}

/// Pinta un estante musical con la forma que le corresponde:
///
/// - `List` (lo que YouTube devuelve como `musicShelfRenderer`): filas de pista
///   numeradas, que es el formato de un álbum y de una lista.
/// - carátulas cuadradas para audio (álbum, artista, lista, canción, mezcla).
/// - [`widgets::carousel`] cuando el estante sí es de vídeo: ahí la tarjeta 16:9
///   con canal y duración es la correcta.
fn render_shelf(ui: &mut egui::Ui, shelf: &Shelf, lookup: ThumbLookup<'_>) -> Option<Intent> {
    if matches!(shelf.layout, ShelfLayout::List) {
        return track_list(ui, &shelf.items, lookup);
    }
    if shelf.items.iter().all(|card| is_square(card.kind)) {
        return square_carousel(ui, shelf, lookup);
    }
    widgets::carousel(ui, shelf, lookup).map(intent_from)
}

// -------------------------------------------------------------------- fichas

/// Página de álbum: cabecera con carátula y la lista de pistas.
pub fn album(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    browse(ui, HeroKind::Album, browse_id, feed, load, lookup)
}

/// Página de artista: cabecera y secciones (sus canciones, álbumes, similares).
pub fn artist(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    browse(ui, HeroKind::Artist, browse_id, feed, load, lookup)
}

/// Lista: cabecera y pistas en orden.
pub fn playlist(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    browse(ui, HeroKind::Playlist, browse_id, feed, load, lookup)
}

/// Qué entidad muestra la cabecera. Solo cambia la etiqueta del tipo.
#[derive(Clone, Copy)]
enum HeroKind {
    Album,
    Artist,
    Playlist,
}

impl HeroKind {
    fn label(self) -> &'static str {
        match self {
            HeroKind::Album => "Álbum",
            HeroKind::Artist => "Artista",
            HeroKind::Playlist => "Lista",
        }
    }
}

/// Composición común de álbum, artista y lista.
fn browse(
    ui: &mut egui::Ui,
    kind: HeroKind,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;

    if feed.shelves.is_empty() {
        if let Load::Failed(message) = load {
            if widgets::error_state(ui, message) {
                intent = Some(Intent::Reload);
            }
            return intent;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt(format!("music_browse:{browse_id}"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if feed.shelves.is_empty() {
                match load {
                    Load::Loading | Load::Idle => {
                        // Esqueleto con la forma de la ficha y de sus filas: si se
                        // pintara una rejilla de tarjetas, el contenido daría un
                        // salto de diseño al llegar.
                        hero_skeleton(ui);
                        track_skeleton(ui, 8);
                    }
                    Load::Failed(message) => {
                        if widgets::error_state(ui, message) {
                            intent = Some(Intent::Reload);
                        }
                    }
                    Load::Ready => widgets::empty_state(
                        ui,
                        "Esta ficha está vacía",
                        "YouTube Music no devolvió pistas para esta página.",
                    ),
                }
                return;
            }

            let (title, subtitle) = hero_text(feed, kind.label());
            if let Some(i) = hero(ui, kind, &title, &subtitle, cover_card(feed), &mut *lookup) {
                intent = Some(i);
            }

            for shelf in &feed.shelves {
                // La propia entidad puede venir como una tarjeta dentro del feed
                // (el `musicCardShelfRenderer` de la cabecera). Ya está pintada
                // arriba: repetirla aquí duplicaría la carátula.
                let items: Vec<Card> = shelf
                    .items
                    .iter()
                    .filter(|card| card.id != browse_id)
                    .cloned()
                    .collect();
                if items.is_empty() {
                    continue;
                }
                let view = Shelf {
                    title: shelf.title.clone(),
                    items,
                    more: shelf.more.clone(),
                    layout: shelf.layout,
                };

                // En una ficha el título del estante no repite el de la cabecera.
                if let Some(name) = view.title.as_ref().filter(|name| **name != title) {
                    ui.label(
                        RichText::new(name)
                            .size(font::SECTION_TITLE)
                            .strong()
                            .color(p.text),
                    );
                    gap(ui, 8.0);
                }
                if let Some(i) = render_shelf(ui, &view, &mut *lookup) {
                    intent = Some(i);
                }
                gap(ui, 20.0);
            }
        });

    intent
}

/// Carátula de la cabecera: la primera tarjeta que representa a la entidad y
/// tiene imagen.
///
/// En álbum y lista el catálogo no manda cabecera propia (`musicResponsiveHeader`
/// no se parsea), así que se usa la portada de la primera fila: YouTube Music
/// repite el arte del disco en todas las pistas.
fn cover_card(feed: &Feed) -> Option<&Card> {
    let cards = || feed.shelves.iter().flat_map(|shelf| shelf.items.iter());
    cards()
        .find(|card| is_entity(card.kind) && !card.thumbs.is_empty())
        .or_else(|| cards().find(|card| !card.thumbs.is_empty()))
}

/// Título y subtítulo de la cabecera, en orden de preferencia: la entidad de la
/// respuesta, el título del primer estante y, si no hay nada, la etiqueta del
/// tipo de página. Es puro (no pinta) para poder probarlo sin un contexto de
/// egui: el orden de preferencia es lo único que puede fallar.
fn hero_text(feed: &Feed, fallback: &str) -> (String, String) {
    let cover = cover_card(feed);
    let entity = cover.filter(|card| is_entity(card.kind) && !card.title.is_empty());
    let title = match entity {
        Some(card) => card.title.clone(),
        None => feed
            .shelves
            .iter()
            .find_map(|shelf| shelf.title.clone())
            .unwrap_or_else(|| fallback.to_string()),
    };
    let subtitle = cover
        .map(|card| {
            if card.meta.is_empty() {
                card.author.clone()
            } else {
                card.meta.clone()
            }
        })
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "YouTube Music".to_string());
    (title, subtitle)
}

/// Cabecera grande de álbum/artista/lista: carátula cuadrada, tipo, título,
/// subtítulo y las tres acciones de la referencia.
fn hero(
    ui: &mut egui::Ui,
    kind: HeroKind,
    title: &str,
    subtitle: &str,
    cover: Option<&Card>,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;

    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(HERO_COVER), Sense::hover());
        match cover {
            Some(card) => {
                widgets::thumb_image(ui, rect, &card.thumbs, metrics::RADIUS_CHIP, &mut *lookup)
            }
            None => {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(metrics::RADIUS_CHIP),
                    p.skeleton,
                );
            }
        }
        gap_x(ui, 24.0, HERO_COVER);

        ui.vertical(|ui| {
            ui.label(RichText::new(kind.label()).size(font::META).color(p.text2));
            gap(ui, 4.0);
            ui.add(
                egui::Label::new(RichText::new(title).size(HERO_TITLE).strong().color(p.text))
                    .truncate(),
            );
            if !subtitle.is_empty() {
                gap(ui, 6.0);
                ui.add(
                    egui::Label::new(RichText::new(subtitle).size(font::META).color(p.text2))
                        .truncate(),
                );
            }
            gap(ui, 18.0);

            // Cada botón emite su intención; no hay ninguno decorativo.
            ui.horizontal(|ui| {
                if widgets::pill(ui, "Reproducir", Some(Icon::Play), PillKind::Solid).clicked() {
                    intent = Some(Intent::PlayAll);
                }
                if widgets::pill(ui, "Aleatorio", Some(Icon::Shuffle), PillKind::Filled).clicked() {
                    intent = Some(Intent::PlayAllShuffled);
                }
                if widgets::pill(
                    ui,
                    "Añadir a la cola",
                    Some(Icon::Playlist),
                    PillKind::Filled,
                )
                .clicked()
                {
                    intent = Some(Intent::EnqueueAll);
                }
            });
        });
    });
    gap(ui, 20.0);
    intent
}

// -------------------------------------------------------------------- pistas

/// Filas de pista numeradas. Es el cuerpo de un álbum y de una lista.
fn track_list(ui: &mut egui::Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<Intent> {
    let mut intent: Option<Intent> = None;
    for (index, card) in cards.iter().enumerate() {
        if let Some(i) = track_row(ui, index + 1, card, &mut *lookup) {
            intent = Some(i);
        }
    }
    intent
}

/// Una fila de pista: número, miniatura, título, artista, álbum, duración y el
/// menú de tres puntos.
///
/// La fila se compone a mano (un rectángulo clicable y texto con el `painter`) en
/// vez de con widgets hijos: así el clic en cualquier punto de la fila es
/// «reproducir ya» y las columnas quedan alineadas aunque los títulos tengan
/// longitudes distintas. Tampoco se usa un `Grid` de egui, donde `add_space`
/// haría panic.
fn track_row(
    ui: &mut egui::Ui,
    number: usize,
    card: &Card,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, TRACK_ROW_H), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::RADIUS_CHIP), p.hover);
    }

    // Número de pista, alineado a la derecha de su columna como en la web.
    ui.painter().text(
        egui::pos2(rect.left() + TRACK_NUMBER_W - 8.0, rect.center().y),
        Align2::RIGHT_CENTER,
        number.to_string(),
        FontId::proportional(font::META),
        p.text3,
    );

    let thumb = Rect::from_center_size(
        egui::pos2(
            rect.left() + TRACK_NUMBER_W + 8.0 + TRACK_THUMB * 0.5,
            rect.center().y,
        ),
        Vec2::splat(TRACK_THUMB),
    );
    widgets::thumb_image(ui, thumb, &card.thumbs, metrics::RADIUS_CHIP, lookup);

    // Columnas: el ancho se reparte por proporciones para que todas las filas
    // alineen aunque cambie el ancho de la ventana.
    let menu = Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        Vec2::splat(28.0),
    );
    let text_left = thumb.right() + 12.0;
    let text_right = menu.left() - 12.0 - TRACK_DURATION_W - 12.0;
    let text_width = (text_right - text_left).max(0.0);
    let title_width = text_width * 0.46;
    let artist_width = text_width * 0.27;
    let album_width = text_width * 0.27;

    paint_line(
        ui,
        Rect::from_min_size(
            egui::pos2(text_left, rect.top()),
            Vec2::new(title_width, rect.height()),
        ),
        &card.title,
        font::CARD_TITLE,
        p.text,
    );
    paint_line(
        ui,
        Rect::from_min_size(
            egui::pos2(text_left + title_width, rect.top()),
            Vec2::new(artist_width, rect.height()),
        ),
        &card.author,
        font::META,
        p.text2,
    );
    // El parser guarda en `meta` las columnas flexibles que sobran: en una lista
    // es el álbum; en un álbum puede ser el año.
    paint_line(
        ui,
        Rect::from_min_size(
            egui::pos2(text_left + title_width + artist_width, rect.top()),
            Vec2::new(album_width, rect.height()),
        ),
        &card.meta,
        font::META,
        p.text2,
    );
    if let Some(duration) = &card.duration {
        ui.painter().text(
            egui::pos2(text_right + TRACK_DURATION_W, rect.center().y),
            Align2::RIGHT_CENTER,
            duration,
            FontId::proportional(font::META),
            p.text2,
        );
    }

    // El menú solo se dibuja al pasar por la fila, como en la referencia: es el
    // mismo clic de la fila, pero sobre la zona del icono emite `CardMenu`.
    if response.hovered() {
        icons::draw(
            ui,
            Rect::from_center_size(menu.center(), Vec2::splat(16.0)),
            Icon::More,
            p.text2,
        );
    }
    if response.clicked() {
        let over_menu = response
            .interact_pointer_pos()
            .is_some_and(|pos| menu.expand(4.0).contains(pos));
        intent = Some(if over_menu {
            Intent::CardMenu(card.clone())
        } else {
            Intent::PlayNow(card.clone())
        });
    }
    intent
}

// ---------------------------------------------------------------- carátulas

/// Carrusel horizontal de carátulas cuadradas.
fn square_carousel(ui: &mut egui::Ui, shelf: &Shelf, lookup: ThumbLookup<'_>) -> Option<Intent> {
    let mut intent: Option<Intent> = None;
    egui::ScrollArea::horizontal()
        .id_salt(("music_carousel", shelf.title.clone().unwrap_or_default()))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for card in &shelf.items {
                    let local = ui
                        .allocate_ui_with_layout(
                            Vec2::new(COVER, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| square_card(ui, card, COVER, &mut *lookup),
                        )
                        .inner;
                    if let Some(i) = local {
                        intent = Some(i);
                    }
                    gap_x(ui, metrics::GRID_GAP, 1.0);
                }
            });
        });
    intent
}

/// Tarjeta de carátula cuadrada (álbum, artista, lista o canción).
///
/// No se reutiliza [`crate::ui::widgets::grid_card`] porque esa es 16:9 con
/// avatar de canal: en Música la carátula es cuadrada y el radio es 8, no 12.
fn square_card(
    ui: &mut egui::Ui,
    card: &Card,
    size: f32,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;

    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    widgets::thumb_image(ui, rect, &card.thumbs, metrics::RADIUS_CHIP, lookup);

    // El botón de reproducción solo aparece si la tarjeta se puede reproducir:
    // un álbum o un artista no son una pista y `Intent::PlayNow` los rechazaría.
    let playable = is_playable(card.kind);
    if playable && response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 24.0, Color32::from_black_alpha(160));
        icons::draw(
            ui,
            Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
            Icon::Play,
            Color32::WHITE,
        );
    }
    if response.clicked() {
        let over_play = playable
            && response
                .interact_pointer_pos()
                .is_some_and(|pos| (rect.center() - pos).length() <= 26.0);
        intent = Some(if over_play {
            Intent::PlayNow(card.clone())
        } else {
            Intent::Open(card.clone())
        });
    }

    gap(ui, 8.0);
    ui.horizontal(|ui| {
        let title_width = size - 28.0;
        let title = ui.add_sized(
            [title_width, font::CARD_TITLE_LINE],
            egui::Label::new(
                RichText::new(&card.title)
                    .size(font::CARD_TITLE)
                    .strong()
                    .color(p.text),
            )
            .truncate()
            .sense(Sense::click()),
        );
        if title.clicked() {
            intent = Some(Intent::Open(card.clone()));
        }
        if icons::icon_button(ui, Icon::More, 16.0, false, "Más opciones").clicked() {
            intent = Some(Intent::CardMenu(card.clone()));
        }
    });
    if !card.author.is_empty() {
        ui.add(
            egui::Label::new(RichText::new(&card.author).size(font::META).color(p.text2))
                .truncate(),
        );
    }
    intent
}

// ------------------------------------------------------------------ estados

/// Marcador de carga con la forma de la cabecera de ficha.
fn hero_skeleton(ui: &mut egui::Ui) {
    let p = palette(ui.ctx());
    ui.horizontal_top(|ui| {
        let (cover, _) = ui.allocate_exact_size(Vec2::splat(HERO_COVER), Sense::hover());
        ui.painter()
            .rect_filled(cover, CornerRadius::same(metrics::RADIUS_CHIP), p.skeleton);
        gap_x(ui, 24.0, HERO_COVER);
        ui.vertical(|ui| {
            for (width, height) in [(80.0, 12.0), (320.0, 34.0), (200.0, 12.0), (420.0, 36.0)] {
                let (line, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
                ui.painter().rect_filled(line, 4, p.skeleton);
                gap(ui, 10.0);
            }
        });
    });
    gap(ui, 20.0);
}

/// Marcador de carga con la forma de una fila de pista: así el contenido no
/// salta cuando llega.
fn track_skeleton(ui: &mut egui::Ui, count: usize) {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    for _ in 0..count {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, TRACK_ROW_H), Sense::hover());
        let thumb = Rect::from_center_size(
            egui::pos2(
                rect.left() + TRACK_NUMBER_W + 8.0 + TRACK_THUMB * 0.5,
                rect.center().y,
            ),
            Vec2::splat(TRACK_THUMB),
        );
        ui.painter()
            .rect_filled(thumb, CornerRadius::same(metrics::RADIUS_CHIP), p.skeleton);
        // Las barras se pintan, no se reservan: reservarlas dentro de la fila
        // sumaría altura al cursor del layout y separaría los marcadores.
        let left = thumb.right() + 12.0;
        for (offset, factor, height) in [(0.0, 0.32, 12.0), (0.34, 0.18, 10.0), (0.54, 0.16, 10.0)]
        {
            let bar = Rect::from_min_size(
                egui::pos2(left + width * offset, rect.center().y - height * 0.5),
                Vec2::new(width * factor, height),
            );
            ui.painter().rect_filled(bar, 4, p.skeleton);
        }
    }
}

// ------------------------------------------------------------------ auxilio

/// ¿La tarjeta representa la propia ficha (álbum, artista o lista) y no una de
/// sus pistas?
fn is_entity(kind: MediaKind) -> bool {
    matches!(
        kind,
        MediaKind::Album | MediaKind::Artist | MediaKind::Playlist
    )
}

/// ¿La carátula de este tipo es cuadrada? En Música todo lo que suena lleva arte
/// cuadrado; lo que es vídeo mantiene la miniatura 16:9.
fn is_square(kind: MediaKind) -> bool {
    matches!(
        kind,
        MediaKind::Album
            | MediaKind::Artist
            | MediaKind::Playlist
            | MediaKind::Song
            | MediaKind::Mix
    )
}

/// ¿Se puede enviar a la cola o al reproductor tal cual?
fn is_playable(kind: MediaKind) -> bool {
    matches!(
        kind,
        MediaKind::Song | MediaKind::Video | MediaKind::Short | MediaKind::Live
    )
}

/// Hueco vertical. Se reserva con `allocate_exact_size` y no con `add_space`
/// porque la misma función se usa dentro de composiciones donde `add_space`
/// dispararía el panic de egui («add_space makes no sense in a grid layout»).
fn gap(ui: &mut egui::Ui, height: f32) {
    ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
}

/// Hueco horizontal.
fn gap_x(ui: &mut egui::Ui, width: f32, height: f32) {
    ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
}

/// Pinta una línea de texto recortada al ancho de su celda y centrada en
/// vertical.
///
/// Se pinta con el `painter` y no con un `egui::Label`: la fila ya reservó su
/// rectángulo clicable y añadir widgets hijos movería el cursor del layout y
/// descuadraría las columnas. El recorte se hace con el `LayoutJob` de egui
/// (`max_rows = 1` + puntos suspensivos) para quedarse en una sola línea.
fn paint_line(ui: &egui::Ui, rect: Rect, text: &str, size: f32, color: Color32) {
    if text.is_empty() || rect.width() <= 8.0 {
        return;
    }
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = rect.width();
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Thumbnail, Thumbs};

    fn card(id: &str, kind: MediaKind, title: &str, author: &str, meta: &str) -> Card {
        let mut card = Card::new(id, kind, title);
        card.author = author.to_string();
        card.meta = meta.to_string();
        card.thumbs = Thumbs::new([Thumbnail {
            url: "https://example.invalid/cover.jpg".into(),
            width: 544,
            height: 544,
        }]);
        card
    }

    fn feed_with(title: Option<&str>, items: Vec<Card>) -> Feed {
        Feed {
            shelves: vec![Shelf {
                title: title.map(str::to_string),
                items,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn cabecera_usa_la_entidad_cuando_la_respuesta_la_trae() {
        let feed = feed_with(
            Some("Pistas"),
            vec![card(
                "MPREb_1",
                MediaKind::Album,
                "Disco",
                "Artista",
                "2020 · 10 canciones",
            )],
        );
        let (title, subtitle) = hero_text(&feed, "Álbum");
        assert_eq!(title, "Disco");
        assert_eq!(subtitle, "2020 · 10 canciones");
    }

    #[test]
    fn cabecera_cae_al_titulo_del_estante_si_solo_hay_pistas() {
        let feed = feed_with(
            Some("Artista"),
            vec![card("v1", MediaKind::Song, "Canción", "Artista", "")],
        );
        let (title, subtitle) = hero_text(&feed, "Lista");
        assert_eq!(title, "Artista");
        // Sin `meta`, el subtítulo es el artista de la primera fila.
        assert_eq!(subtitle, "Artista");
    }

    #[test]
    fn cabecera_sin_contenido_usa_la_etiqueta_del_tipo() {
        let (title, subtitle) = hero_text(&Feed::default(), "Artista");
        assert_eq!(title, "Artista");
        assert_eq!(subtitle, "YouTube Music");
    }

    #[test]
    fn solo_lo_que_suena_es_reproducible_y_cuadrado() {
        assert!(is_playable(MediaKind::Song));
        assert!(!is_playable(MediaKind::Album));
        assert!(is_square(MediaKind::Album));
        assert!(!is_square(MediaKind::Video));
    }

    #[test]
    fn la_caratula_prefiere_la_entidad_a_una_pista() {
        let feed = feed_with(
            None,
            vec![
                card("v1", MediaKind::Song, "Canción", "Artista", "Disco"),
                card("MPREb_1", MediaKind::Album, "Disco", "Artista", "2020"),
            ],
        );
        assert_eq!(
            cover_card(&feed).map(|card| card.id.as_str()),
            Some("MPREb_1")
        );
    }
}
