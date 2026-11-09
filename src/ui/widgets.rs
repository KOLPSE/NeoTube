//! Piezas de interfaz reutilizables.
//!
//! Cada widget corresponde a un componente de la referencia y usa los tokens de
//! [`crate::ui::theme`]: nada de tamaños ni colores locales. La densidad, los
//! radios y los espacios son los de youtube.com de escritorio.

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, Ui, Vec2,
};

use crate::domain::{Card, MediaKind, Shelf, Thumbs};
use crate::ui::icons;
pub use crate::ui::icons::Icon;
use crate::ui::theme::{font, metrics, palette};

/// Función que devuelve la textura de una miniatura, pidiéndola si no está.
pub type ThumbLookup<'a> = &'a mut dyn FnMut(&Thumbs, u32) -> Option<egui::TextureId>;

// ------------------------------------------------------------------ primitivas

/// Chip de filtro. Seleccionado = fondo claro con texto oscuro; normal = gris
/// con texto claro. Radio 8, alto 32, 12 px de relleno lateral.
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let p = palette(ui.ctx());
    // El color con el que se mide **tiene que ser** el que se pinta: el pintor
    // no recolorea una galley ya creada, y el chip seleccionado salía como una
    // píldora vacía porque se medía en claro y se pintaba en oscuro.
    let fg = if selected { p.bg } else { p.text };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(font::CHIP), fg);
    let size = Vec2::new(
        galley.size().x + metrics::CHIP_PAD_X * 2.0,
        metrics::CHIP_HEIGHT,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let bg = if selected {
        p.text
    } else if response.hovered() {
        p.hover
    } else {
        p.chip
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::RADIUS_CHIP), bg);
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
    response
}

/// Botón de píldora. `kind` decide el estilo de la referencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillKind {
    /// Fondo blanco, texto negro («Suscribirse» sin sesión, «Reproducir»).
    Solid,
    /// Fondo gris, texto claro («Suscribirse» en oscuro, «Aleatorio»).
    Filled,
    /// Sin fondo hasta el hover («Compartir», «Guardar»).
    Ghost,
    /// Fondo blanco con texto oscuro y borde (el «Suscribirse» de la web).
    Outline,
}

pub fn pill(ui: &mut Ui, label: &str, icon: Option<Icon>, kind: PillKind) -> Response {
    let p = palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(font::BUTTON),
        p.text,
    );
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + 32.0 + icon_w, 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    let (bg, fg) = match kind {
        PillKind::Solid => (
            if response.hovered() {
                Color32::from_rgb(0xd9, 0xd9, 0xd9)
            } else {
                Color32::WHITE
            },
            Color32::BLACK,
        ),
        PillKind::Filled => (if response.hovered() { p.hover } else { p.chip }, p.text),
        PillKind::Ghost => (
            if response.hovered() {
                p.chip
            } else {
                Color32::TRANSPARENT
            },
            p.text,
        ),
        PillKind::Outline => (if response.hovered() { p.hover } else { p.chip }, p.text),
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::RADIUS_PILL), bg);
    if kind == PillKind::Outline {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(metrics::RADIUS_PILL),
            Stroke::new(1.0, p.divider),
            egui::StrokeKind::Inside,
        );
    }
    let mut text_left = rect.left() + 16.0;
    if let Some(icon) = icon {
        icons::draw(
            ui,
            Rect::from_center_size(
                egui::pos2(rect.left() + 22.0, rect.center().y),
                Vec2::splat(20.0),
            ),
            icon,
            fg,
        );
        text_left += icon_w;
    }
    let text_rect = Rect::from_min_size(
        egui::pos2(text_left, rect.center().y - galley.size().y * 0.5),
        galley.size(),
    );
    ui.painter().galley(text_rect.min, galley, fg);
    response
}

/// Zona clicable de la barra lateral. `selected` pinta el fondo de la fila;
/// `hover` solo cuando se pasa por encima.
pub fn sidebar_row(ui: &mut Ui, icon: Option<Icon>, label: &str, selected: bool) -> Response {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, metrics::SIDEBAR_ROW), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::SIDEBAR_RADIUS), p.chip);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::SIDEBAR_RADIUS), p.hover);
    }
    let mut left = rect.left() + 12.0;
    if let Some(icon) = icon {
        icons::draw(
            ui,
            Rect::from_center_size(egui::pos2(left + 12.0, rect.center().y), Vec2::splat(24.0)),
            icon,
            p.text,
        );
        left += 24.0 + 24.0;
    }
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(font::SIDEBAR),
        p.text,
    );
    // Se recorta el texto en vez de desbordar la barra.
    let max = rect.right() - left - 8.0;
    let galley = if galley.size().x > max {
        ui.painter().layout(
            label.to_string(),
            FontId::proportional(font::SIDEBAR),
            p.text,
            max,
        )
    } else {
        galley
    };
    ui.painter().galley(
        egui::pos2(left, rect.center().y - galley.size().y * 0.5),
        galley,
        p.text,
    );
    response
}

/// Fila de la barra lateral con avatar (suscripciones).
pub fn sidebar_channel_row(
    ui: &mut Ui,
    name: &str,
    avatar: &Thumbs,
    live: bool,
    lookup: ThumbLookup<'_>,
) -> Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), metrics::SIDEBAR_ROW),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::SIDEBAR_RADIUS), p.hover);
    }
    let avatar_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        Vec2::splat(metrics::AVATAR_SMALL),
    );
    avatar_image(ui, avatar_rect, avatar, lookup);
    let galley = ui.painter().layout(
        name.to_string(),
        FontId::proportional(font::SIDEBAR),
        p.text,
        rect.right() - avatar_rect.right() - 24.0,
    );
    ui.painter().galley(
        egui::pos2(
            avatar_rect.right() + 16.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        p.text,
    );
    if live {
        // Punto azul de "en directo".
        ui.painter().circle_filled(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            3.0,
            Color32::from_rgb(0x3e, 0xa6, 0xff),
        );
    }
    response
}

/// Cabecera de grupo de la barra lateral («Suscripciones», «Tú», «Más de YouTube»).
pub fn sidebar_group(ui: &mut Ui, title: &str, with_chevron: bool) {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), metrics::SIDEBAR_ROW),
        if with_chevron {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if response.hovered() && with_chevron {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::SIDEBAR_RADIUS), p.hover);
    }
    let galley = ui.painter().layout_no_wrap(
        title.to_string(),
        FontId::proportional(font::SIDEBAR_GROUP),
        p.text,
    );
    // La galley es un `Arc`: se mide **antes** de entregarla al painter, porque
    // entregarla la mueve.
    let text_size = galley.size();
    ui.painter().galley(
        egui::pos2(rect.left() + 12.0, rect.center().y - text_size.y * 0.5),
        galley,
        p.text,
    );
    if with_chevron {
        icons::draw(
            ui,
            Rect::from_center_size(
                egui::pos2(rect.left() + 24.0 + text_size.x + 12.0, rect.center().y),
                Vec2::splat(16.0),
            ),
            Icon::ChevronRight,
            p.text,
        );
    }
}

/// Separador de 1 px que ocupa el ancho disponible.
pub fn divider(ui: &mut Ui) {
    let p = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_min_size(
            egui::pos2(rect.left(), rect.center().y),
            Vec2::new(rect.width(), 1.0),
        ),
        0,
        p.divider,
    );
}

// ------------------------------------------------------------------- imágenes

/// Pinta una miniatura 16:9 con radio, o su marcador de carga.
pub fn thumb_image(ui: &Ui, rect: Rect, thumbs: &Thumbs, radius: u8, lookup: ThumbLookup<'_>) {
    let p = palette(ui.ctx());
    match lookup(thumbs, rect.width() as u32) {
        Some(id) => {
            let shape =
                egui::epaint::RectShape::filled(rect, CornerRadius::same(radius), Color32::WHITE)
                    .with_texture(
                        id,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    );
            ui.painter().add(shape);
        }
        None => {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(radius), p.skeleton);
        }
    }
}

/// Avatar circular.
pub fn avatar_image(ui: &Ui, rect: Rect, thumbs: &Thumbs, lookup: ThumbLookup<'_>) {
    let p = palette(ui.ctx());
    match lookup(thumbs, rect.width() as u32) {
        Some(id) => {
            let shape = egui::epaint::RectShape::filled(
                rect,
                CornerRadius::same((rect.width() * 0.5) as u8),
                Color32::WHITE,
            )
            .with_texture(
                id,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            );
            ui.painter().add(shape);
        }
        None => {
            ui.painter()
                .circle_filled(rect.center(), rect.width() * 0.5, p.skeleton);
        }
    }
}

// -------------------------------------------------------------------- tarjetas

/// Tarjeta de la rejilla de Inicio, igual que la referencia:
/// miniatura 16:9 con duración, y debajo una fila con avatar de canal y dos
/// líneas de texto (título y metadatos), más el menú de tres puntos.
pub fn grid_card(
    ui: &mut Ui,
    card: &Card,
    width: f32,
    lookup: ThumbLookup<'_>,
) -> Option<CardAction> {
    let p = palette(ui.ctx());
    let mut action = None;
    let thumb_h = width * 9.0 / 16.0;

    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_width(width);

            // --- miniatura
            let (thumb, thumb_resp) =
                ui.allocate_exact_size(Vec2::new(width, thumb_h), Sense::click());
            thumb_image(ui, thumb, &card.thumbs, metrics::RADIUS_CARD, lookup);
            if let Some(badge) = &card.badge {
                let live = card.kind == MediaKind::Live;
                icons::overlay_badge(ui, thumb, badge, live);
            }
            if let Some(dur) = &card.duration {
                icons::duration_badge(ui, thumb, dur);
            }
            if thumb_resp.hovered() {
                // Botón de reproducción en la esquina, como la web al pasar.
                let center = thumb.right_bottom() - Vec2::new(24.0, 24.0);
                ui.painter().circle_filled(
                    center,
                    18.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 190),
                );
                icons::draw(
                    ui,
                    Rect::from_center_size(center, Vec2::splat(18.0)),
                    if matches!(
                        card.kind,
                        MediaKind::Playlist | MediaKind::Mix | MediaKind::Album
                    ) {
                        Icon::Playlist
                    } else {
                        Icon::Play
                    },
                    Color32::WHITE,
                );
            }

            // `add_space` no está permitido dentro de un `Grid` (egui lo
            // prohíbe expresamente): se reserva la franja con un allocate, que
            // sí es válido y da el mismo resultado.
            ui.allocate_exact_size(Vec2::new(width, 12.0), Sense::hover());

            // --- fila de texto con avatar
            let mut go_author = false;
            ui.horizontal_top(|ui| {
                let avatar_rect =
                    Rect::from_min_size(ui.cursor().min, Vec2::splat(metrics::AVATAR_CARD));
                let avatar_resp = ui.allocate_rect(avatar_rect, Sense::click());
                avatar_image(ui, avatar_rect, &card.thumbs, lookup);
                if avatar_resp.clicked() {
                    go_author = true;
                }
                ui.allocate_exact_size(Vec2::new(12.0, metrics::AVATAR_CARD), Sense::hover());
                let text_width = ui.available_width() - 28.0;
                ui.allocate_ui_with_layout(
                    Vec2::new(text_width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        // Título: máximo 2 líneas, como la referencia.
                        let title_resp = ui.add(
                            egui::Label::new(
                                RichText::new(&card.title)
                                    .size(font::CARD_TITLE)
                                    .strong()
                                    .color(p.text),
                            )
                            .truncate(),
                        );
                        let mut line = String::new();
                        if !card.author.is_empty() {
                            line.push_str(&card.author);
                        }
                        if !card.meta.is_empty() {
                            if !line.is_empty() {
                                line.push_str(" · ");
                            }
                            line.push_str(&card.meta);
                        }
                        let meta_resp = ui.add(
                            egui::Label::new(RichText::new(line).size(font::META).color(p.text2))
                                .truncate()
                                .wrap_mode(egui::TextWrapMode::Truncate),
                        );
                        if title_resp.clicked() || meta_resp.clicked() {
                            action = Some(CardAction::Open(card.clone()));
                        }
                    },
                );
                // Menú de tres puntos.
                let more = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                if more.1.hovered() {
                    ui.painter().circle_filled(more.0.center(), 12.0, p.hover);
                }
                icons::draw(ui, more.0, Icon::More, p.text);
                if more.1.clicked() {
                    action = Some(CardAction::Menu(card.clone()));
                }
            });

            if thumb_resp.clicked() {
                action = Some(CardAction::Open(card.clone()));
            }
            if thumb_resp.double_clicked() {
                action = Some(CardAction::PlayNow(card.clone()));
            }
            if go_author {
                action = Some(CardAction::OpenAuthor(card.author_id.clone()));
            }
        },
    );
    action
}

/// Tarjeta vertical de Shorts (9:16), con título debajo.
pub fn shorts_card(
    ui: &mut Ui,
    card: &Card,
    width: f32,
    lookup: ThumbLookup<'_>,
) -> Option<CardAction> {
    let p = palette(ui.ctx());
    let mut action = None;
    let height = width * 16.0 / 9.0;
    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
            thumb_image(ui, rect, &card.thumbs, metrics::RADIUS_CARD, lookup);
            if let Some(badge) = &card.badge {
                if badge != "SHORTS" {
                    icons::overlay_badge(ui, rect, badge, false);
                }
            }
            if response.hovered() {
                let c = rect.center();
                ui.painter()
                    .circle_filled(c, 22.0, Color32::from_rgba_unmultiplied(0, 0, 0, 170));
                icons::draw(
                    ui,
                    Rect::from_center_size(c, Vec2::splat(20.0)),
                    Icon::Play,
                    Color32::WHITE,
                );
            }
            ui.add_space(10.0);
            let resp = ui.add(
                egui::Label::new(
                    RichText::new(&card.title)
                        .size(font::CARD_TITLE)
                        .strong()
                        .color(p.text),
                )
                .truncate(),
            );
            if !card.meta.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(&card.meta).size(font::META).color(p.text2))
                        .truncate(),
                );
            }
            if response.clicked() || resp.clicked() {
                action = Some(CardAction::Open(card.clone()));
            }
        },
    );
    action
}

/// Fila horizontal (búsqueda, relacionados, listas): miniatura a la izquierda.
pub fn row_card(ui: &mut Ui, card: &Card, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let thumb_w = (width * 0.36).clamp(160.0, 360.0);
    let thumb_h = thumb_w * 9.0 / 16.0;
    let row_h = thumb_h + 8.0;

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::RADIUS_CARD), p.chip);
    }
    let thumb = Rect::from_min_size(
        rect.left_top() + Vec2::new(4.0, 4.0),
        Vec2::new(thumb_w, thumb_h),
    );
    thumb_image(ui, thumb, &card.thumbs, 8, lookup);
    if let Some(dur) = &card.duration {
        icons::duration_badge(ui, thumb, dur);
    }
    if let Some(badge) = &card.badge {
        icons::overlay_badge(ui, thumb, badge, card.kind == MediaKind::Live);
    }

    let left = thumb.right() + 16.0;
    let text_w = rect.right() - left - 8.0;
    let mut y = rect.top() + 4.0;
    let mut paint = |text: &str, size: f32, color: Color32| {
        if text.is_empty() {
            return;
        }
        let g = ui
            .painter()
            .layout(text.to_string(), FontId::proportional(size), color, text_w);
        y += g.size().y;
        ui.painter()
            .galley(egui::pos2(left, y - g.size().y), g, color);
        y += 2.0;
    };
    paint(&card.title, font::CARD_TITLE, p.text);
    paint(&card.author, font::META, p.text2);
    paint(&card.meta, font::META, p.text2);

    let mut action = None;
    // Menú de tres puntos a la derecha.
    let more = Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.top() + 20.0),
        Vec2::splat(24.0),
    );
    if response.hovered() {
        icons::draw(ui, more, Icon::More, p.text);
    }
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            action = Some(if more.expand(6.0).contains(pos) {
                CardAction::Menu(card.clone())
            } else {
                CardAction::Open(card.clone())
            });
        } else {
            action = Some(CardAction::Open(card.clone()));
        }
    }
    if response.double_clicked() {
        action = Some(CardAction::PlayNow(card.clone()));
    }
    action
}

/// Lo que el usuario ha hecho con una tarjeta.
#[derive(Debug, Clone)]
pub enum CardAction {
    Open(Card),
    PlayNow(Card),
    OpenAuthor(Option<String>),
    /// Abrir el menú contextual de la tarjeta.
    Menu(Card),
    Enqueue(Card),
}

// -------------------------------------------------------------------- rejillas

/// Columnas y ancho de tarjeta para el ancho disponible, como la web:
/// se quita una columna cuando la tarjeta baja del mínimo.
pub fn grid_geometry(available: f32) -> (usize, f32) {
    let gap = metrics::GRID_GAP;
    let cols = (((available + gap) / (metrics::CARD_MIN + gap)).floor() as usize).max(1);
    let width = (available - gap * (cols.saturating_sub(1)) as f32) / cols as f32;
    (cols, width)
}

/// Rejilla de tarjetas de vídeo.
pub fn card_grid(ui: &mut Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let (cols, width) = grid_geometry(ui.available_width());
    let mut action = None;
    // Composición manual por filas en lugar de `egui::Grid`.
    //
    // Un `Grid` exige que la cuadrícula se recorra entera en el mismo
    // fotograma y que el número de columnas no cambie a mitad; aquí las
    // tarjetas llevan menús y texturas que se piden de forma asíncrona, así que
    // al llegar miniaturas nuevas el `Grid` veía «primera vez» otra vez y
    // dejaba su aviso de error pintado **en la interfaz**. Fila a fila no hay
    // estado que mantener y el resultado visual es idéntico.
    let mut index = 0;
    while index < cards.len() {
        let end = (index + cols).min(cards.len());
        ui.horizontal_top(|ui| {
            for card in &cards[index..end] {
                let a = ui
                    .allocate_ui_with_layout(
                        Vec2::new(width, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| grid_card(ui, card, width, &mut *lookup),
                    )
                    .inner;
                if a.is_some() {
                    action = a;
                }
                // Separación entre columnas (no `add_space`: dentro de un
                // `horizontal` está permitido, pero se reserva explícito para
                // que el ancho cuadre al píxel).
                if !std::ptr::eq(card, &cards[end - 1]) {
                    ui.allocate_exact_size(Vec2::new(metrics::GRID_GAP, 1.0), Sense::hover());
                }
            }
        });
        index = end;
        ui.allocate_exact_size(Vec2::new(1.0, metrics::GRID_GAP_Y), Sense::hover());
    }
    action
}

/// Rejilla de tarjetas de Shorts.
pub fn shorts_grid(ui: &mut Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let available = ui.available_width();
    let gap = metrics::GRID_GAP;
    let cols = (((available + gap) / (180.0 + gap)).floor() as usize).clamp(2, 8);
    let width = (available - gap * (cols - 1) as f32) / cols as f32;
    let mut action = None;
    // Se compone fila a fila en lugar de con `Grid`: las tarjetas de Shorts
    // tienen altura variable (título de una o dos líneas) y un `Grid` que no
    // cierra todas sus filas deja el aviso «First use of Grid ID» pintado en la
    // interfaz, además de desalinear las columnas.
    let mut index = 0;
    while index < cards.len() {
        ui.horizontal_top(|ui| {
            for card in cards[index..(index + cols).min(cards.len())].iter() {
                let a = ui
                    .allocate_ui_with_layout(
                        Vec2::new(width, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| shorts_card(ui, card, width, &mut *lookup),
                    )
                    .inner;
                if a.is_some() {
                    action = a;
                }
                ui.allocate_exact_size(Vec2::new(gap, 1.0), Sense::hover());
            }
        });
        index += cols;
        ui.allocate_exact_size(Vec2::new(1.0, metrics::GRID_GAP_Y), Sense::hover());
    }
    action
}

/// Lista vertical de filas.
pub fn card_list(ui: &mut Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let mut action = None;
    for card in cards {
        if let Some(a) = row_card(ui, card, &mut *lookup) {
            action = Some(a);
        }
    }
    action
}

// ------------------------------------------------------------------ estados

/// Marcadores de carga con la forma del contenido: evita el salto de diseño y
/// comunica que se está cargando sin una barra girando en medio de la nada.
pub fn skeleton_grid(ui: &mut Ui, count: usize) {
    let p = palette(ui.ctx());
    let (cols, width) = grid_geometry(ui.available_width());
    // Igual que `card_grid`: fila a fila, sin `Grid`. El marcador de carga se
    // repinta mientras llegan datos, y un `Grid` cuyo número de columnas
    // depende del ancho deja de reconocer su propio id al redimensionar.
    let mut index = 0;
    while index < count {
        let end = (index + cols).min(count);
        ui.horizontal_top(|ui| {
            for _ in index..end {
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let (thumb, _) = ui.allocate_exact_size(
                            Vec2::new(width, width * 9.0 / 16.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            thumb,
                            CornerRadius::same(metrics::RADIUS_CARD),
                            p.skeleton,
                        );
                        ui.allocate_exact_size(Vec2::new(width, 12.0), Sense::hover());
                        ui.horizontal(|ui| {
                            let (av, _) = ui
                                .allocate_exact_size(Vec2::splat(metrics::AVATAR_CARD), Sense::hover());
                            ui.painter().circle_filled(av.center(), 18.0, p.skeleton);
                            ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                            ui.vertical(|ui| {
                                for (w, h) in [(0.7f32, 14.0f32), (0.45, 12.0)] {
                                    let (r, _) = ui
                                        .allocate_exact_size(Vec2::new(width * w, h), Sense::hover());
                                    ui.painter().rect_filled(r, 4, p.skeleton);
                                }
                            });
                        });
                    },
                );
                if end - index > 1 {
                    ui.allocate_exact_size(Vec2::new(metrics::GRID_GAP, 1.0), Sense::hover());
                }
            }
        });
        index = end;
        ui.allocate_exact_size(Vec2::new(1.0, metrics::GRID_GAP_Y), Sense::hover());
    }
}

/// Mensaje de error con botón de reintento. Devuelve `true` si se pidió
/// reintentar.
pub fn error_state(ui: &mut Ui, message: &str) -> bool {
    let p = palette(ui.ctx());
    let mut retry = false;
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        icons::draw(
            ui,
            Rect::from_center_size(
                ui.cursor().center_top() + Vec2::new(0.0, 16.0),
                Vec2::splat(28.0),
            ),
            Icon::Warning,
            p.text2,
        );
        ui.add_space(36.0);
        ui.label(RichText::new("Algo ha ido mal").size(18.0).color(p.text));
        ui.add_space(6.0);
        ui.add(egui::Label::new(
            RichText::new(message).size(font::BODY).color(p.text2),
        ));
        ui.add_space(16.0);
        if pill(ui, "Reintentar", None, PillKind::Filled).clicked() {
            retry = true;
        }
        ui.add_space(48.0);
    });
    retry
}

/// Estado vacío.
pub fn empty_state(ui: &mut Ui, title: &str, detail: &str) {
    let p = palette(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(64.0);
        ui.label(RichText::new(title).size(18.0).color(p.text));
        ui.add_space(6.0);
        ui.add(egui::Label::new(
            RichText::new(detail).size(font::BODY).color(p.text2),
        ));
        ui.add_space(64.0);
    });
}

/// Título de sección, con acción opcional a la derecha.
pub fn section_title(ui: &mut Ui, title: &str) -> Response {
    let p = palette(ui.ctx());
    ui.add(
        egui::Label::new(
            RichText::new(title)
                .size(font::SECTION_TITLE)
                .strong()
                .color(p.text),
        )
        .sense(Sense::click()),
    )
}

/// Título de sección con el botón de reproducción todo de la referencia.
pub fn shelf_header(ui: &mut Ui, title: &str) -> bool {
    let p = palette(ui.ctx());
    let mut play_all = false;
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                RichText::new(title)
                    .size(font::SECTION_TITLE)
                    .strong()
                    .color(p.text),
            )
            .sense(Sense::click()),
        );
        ui.add_space(8.0);
        let resp = ui.allocate_exact_size(Vec2::new(120.0, 28.0), Sense::click());
        if resp.1.hovered() {
            ui.painter()
                .rect_filled(resp.0, CornerRadius::same(metrics::RADIUS_PILL), p.chip);
        }
        icons::draw(
            ui,
            Rect::from_center_size(
                egui::pos2(resp.0.left() + 16.0, resp.0.center().y),
                Vec2::splat(18.0),
            ),
            Icon::Play,
            p.text,
        );
        ui.painter().text(
            egui::pos2(resp.0.left() + 32.0, resp.0.center().y),
            Align2::LEFT_CENTER,
            "Reproducir todo",
            FontId::proportional(font::BODY),
            p.text,
        );
        if resp.1.clicked() {
            play_all = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icons::draw(
                ui,
                Rect::from_center_size(
                    ui.cursor().center_top() + Vec2::new(-12.0, 12.0),
                    Vec2::splat(20.0),
                ),
                Icon::ChevronRight,
                p.text2,
            );
        });
    });
    play_all
}

/// Carrusel horizontal de tarjetas con flechas, como los estantes de la web.
pub fn carousel(ui: &mut Ui, shelf: &Shelf, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let mut action = None;
    let card_w = 320.0;
    egui::ScrollArea::horizontal()
        .id_salt(("carousel", shelf.title.clone().unwrap_or_default()))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for card in &shelf.items {
                    let a = ui
                        .allocate_ui_with_layout(
                            Vec2::new(card_w, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| grid_card(ui, card, card_w, &mut *lookup),
                        )
                        .inner;
                    if a.is_some() {
                        action = a;
                    }
                }
            });
        });
    action
}

// ------------------------------------------------------------ controles varios

/// Barra de progreso. Devuelve el `Response` para poder buscar en el tiempo.
pub fn progress_bar(ui: &mut Ui, ratio: f32, height: f32) -> Response {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height + 8.0), Sense::click_and_drag());
    let bar = Rect::from_center_size(rect.center(), Vec2::new(width, height));
    ui.painter()
        .rect_filled(bar, CornerRadius::same(2), p.hover);
    let filled = width * ratio.clamp(0.0, 1.0);
    ui.painter().rect_filled(
        Rect::from_min_size(bar.left_top(), Vec2::new(filled, bar.height())),
        CornerRadius::same(2),
        p.brand,
    );
    if response.hovered() {
        let knob = egui::pos2(bar.left() + filled, bar.center().y);
        ui.painter().circle_filled(knob, height + 2.0, p.brand);
    }
    response
}

/// Proporción (0..1) del puntero dentro de un `Response` horizontal.
pub fn pointer_ratio(response: &Response) -> Option<f32> {
    let pos = response.hover_pos()?;
    let rect = response.rect;
    if rect.width() <= 0.0 {
        return None;
    }
    Some(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

/// Fila de menú contextual (los de tres puntos y el de perfil).
pub fn menu_row(ui: &mut Ui, icon: Option<Icon>, label: &str, trailing: Option<&str>) -> Response {
    let p = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(0), p.hover);
    }
    let mut left = rect.left() + 16.0;
    if let Some(icon) = icon {
        icons::draw(
            ui,
            Rect::from_center_size(egui::pos2(left + 12.0, rect.center().y), Vec2::splat(24.0)),
            icon,
            p.text,
        );
        left += 24.0 + 16.0;
    }
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(font::MENU), p.text);
    ui.painter().galley(
        egui::pos2(left, rect.center().y - galley.size().y * 0.5),
        galley,
        p.text,
    );
    if let Some(trailing) = trailing {
        let g = ui.painter().layout_no_wrap(
            trailing.to_string(),
            FontId::proportional(font::MENU),
            p.text2,
        );
        ui.painter().galley(
            egui::pos2(
                rect.right() - g.size().x - 40.0,
                rect.center().y - g.size().y * 0.5,
            ),
            g,
            p.text2,
        );
        icons::draw(
            ui,
            Rect::from_center_size(
                egui::pos2(rect.right() - 20.0, rect.center().y),
                Vec2::splat(16.0),
            ),
            Icon::ChevronRight,
            p.text2,
        );
    }
    response
}

// ------------------------------------------------------- alias de transición
//
// La interfaz se está reescribiendo pantalla a pantalla contra las capturas de
// referencia. Mientras quede alguna sin migrar, estos alias mantienen el árbol
// compilando; desaparecen cuando la última pantalla use los nombres nuevos.

/// Alias de [`icons::draw`].
pub fn draw_icon(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    icons::draw(ui, rect, icon, color)
}

/// Alias de [`pill`].
pub fn pill_button(ui: &mut Ui, label: &str, solid: bool, active: bool) -> Response {
    let kind = if solid {
        PillKind::Solid
    } else if active {
        PillKind::Filled
    } else {
        PillKind::Filled
    };
    pill(ui, label, None, kind)
}

/// Alias de [`row_card`].
pub fn card_row(ui: &mut Ui, card: &Card, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    row_card(ui, card, lookup)
}

/// Alias de [`grid_card`].
pub fn card_tile(
    ui: &mut Ui,
    card: &Card,
    width: f32,
    lookup: ThumbLookup<'_>,
) -> Option<CardAction> {
    grid_card(ui, card, width, lookup)
}

/// Alias de [`error_state`].
pub fn error_panel(ui: &mut Ui, message: &str, _retry: &str) -> bool {
    error_state(ui, message)
}

/// Alias de [`empty_state`].
pub fn empty_panel(ui: &mut Ui, title: &str, detail: &str) {
    empty_state(ui, title, detail)
}

/// Alias de [`section_title`], con subtítulo opcional.
pub fn section_header(ui: &mut Ui, title: &str, subtitle: Option<&str>) {
    let p = palette(ui.ctx());
    ui.add_space(8.0);
    section_title(ui, title);
    if let Some(sub) = subtitle {
        ui.label(RichText::new(sub).size(font::META).color(p.text2));
    }
    ui.add_space(10.0);
}

/// Versión de [`progress_bar`] que acepta el ratio de hover (las pantallas
/// antiguas pasaban un cuarto argumento).
pub fn progress_bar_ext(ui: &mut Ui, ratio: f32, height: f32, _hovered: Option<f32>) -> Response {
    progress_bar(ui, ratio, height)
}

/// Alias de [`icons::icon_button`] con el mismo nombre que usaban las
/// pantallas antes de separar iconos y widgets.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, active: bool, tooltip: &str) -> Response {
    icons::icon_button(ui, icon, size, active, tooltip)
}

/// Alias de [`icons::icon_button`].
pub fn icon_button2(ui: &mut Ui, icon: Icon, size: f32, active: bool, tooltip: &str) -> Response {
    icons::icon_button(ui, icon, size, active, tooltip)
}

/// Color del "me gusta" según estado.
pub fn like_color(active: bool, normal: Color32) -> Color32 {
    if active {
        palette(&egui::Context::default()).like
    } else {
        normal
    }
}

/// Miniatura redonda usada por las pantallas antiguas.
pub fn avatar(ui: &mut Ui, thumbs: &Thumbs, size: f32, lookup: ThumbLookup<'_>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    avatar_image(ui, rect, thumbs, lookup);
}

/// Alias de [`carousel`] con estante completo (compatibilidad).
pub fn shelf_row(ui: &mut Ui, shelf: &Shelf, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    carousel(ui, shelf, lookup)
}
