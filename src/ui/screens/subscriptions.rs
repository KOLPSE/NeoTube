//! Suscripciones: lo último de los canales que sigue la cuenta.
//!
//! La referencia (youtube.com/feed/subscriptions) es una fila de canales con
//! «Todas» delante, las novedades en filas anchas y una columna a la derecha con
//! los canales. Sin sesión no hay suscripciones que enseñar: se dice y se para,
//! en vez de rellenar la pantalla con canales de mentira.

use egui::RichText;

use crate::domain::{Card, Feed, MediaKind, Route};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, row_card, ThumbLookup};

/// Ancho de la columna de canales de la derecha. Por debajo de
/// [`SIDE_COLUMN_MIN`] de ancho total no cabe y los canales van debajo de las
/// novedades, en lugar de comprimir las filas.
const SIDE_COLUMN: f32 = 300.0;
const SIDE_COLUMN_MIN: f32 = 1000.0;

/// Avatar de la fila superior de canales y ancho de su celda.
const STRIP_AVATAR: f32 = 48.0;
const STRIP_CELL: f32 = 88.0;

pub fn subscriptions(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    let (channels, videos) = split_cards(feed);

    // El `browse` de suscripciones sin sesión termina en `Load::Failed` con el
    // mensaje de sesión de `ApiError::user_message`, que es la única señal de
    // «falta sesión» que llega a esta pantalla: la firma no recibe la sesión.
    if let Load::Failed(message) = load {
        if needs_session(message) {
            widgets::empty_state(
                ui,
                "Necesitas una sesión",
                "Las suscripciones son de tu cuenta de YouTube: inicia sesión en Ajustes (o pegando tus cookies) y aquí aparecerá lo último de tus canales.",
            );
            return intent;
        }
        if channels.is_empty() && videos.is_empty() {
            if widgets::error_state(ui, message) {
                intent = Some(Intent::Reload);
            }
            return intent;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt("subscriptions_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                RichText::new("Suscripciones")
                    .size(font::PAGE_TITLE)
                    .strong()
                    .color(p.text),
            );
            ui.add_space(12.0);

            if let Some(i) = channel_strip(ui, &channels, &mut *lookup) {
                intent = Some(i);
            }
            ui.add_space(20.0);

            let total = ui.available_width();
            let side = if total >= SIDE_COLUMN_MIN {
                SIDE_COLUMN
            } else {
                0.0
            };
            let list_width = total - if side > 0.0 { side + 24.0 } else { 0.0 };

            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(list_width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if let Some(i) = video_rows(ui, &videos, load, &mut *lookup) {
                            intent = Some(i);
                        }
                    },
                );
                if side > 0.0 {
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(side, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(i) = channel_column(ui, &channels, &mut *lookup) {
                                intent = Some(i);
                            }
                        },
                    );
                }
            });

            ui.add_space(24.0);
        });

    intent
}

/// Separa canales y vídeos. El despachador devuelve los canales como tarjetas
/// de [`MediaKind::Channel`]; todo lo demás es la lista de novedades, en el
/// orden en que lo puso YouTube (lo más nuevo primero).
fn split_cards(feed: &Feed) -> (Vec<&Card>, Vec<&Card>) {
    let mut channels = Vec::new();
    let mut videos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for shelf in &feed.shelves {
        for card in &shelf.items {
            if card.kind == MediaKind::Channel {
                // Un mismo canal puede venir en dos estantes: se pinta una vez.
                if seen.insert(card.id.as_str()) {
                    channels.push(card);
                }
            } else {
                videos.push(card);
            }
        }
    }
    (channels, videos)
}

/// ¿El fallo significa «falta sesión»? Se comparan las frases de
/// [`crate::yt::client::ApiError::user_message`] porque la pantalla no recibe el
/// error tipado.
fn needs_session(message: &str) -> bool {
    let message = message.to_lowercase();
    message.contains("iniciar sesión") || message.contains("sesión")
}

/// Fila superior de canales, con la píldora «Todas» delante.
fn channel_strip(ui: &mut egui::Ui, channels: &[&Card], lookup: ThumbLookup<'_>) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    egui::ScrollArea::horizontal()
        .id_salt("subscriptions_strip")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // «Todas» no filtra: vuelve a pedir el feed completo, que es la
                // vista en la que ya estamos. Es la única entrada de la fila que
                // no abre un canal.
                if widgets::chip(ui, "Todas", true).clicked() {
                    intent = Some(Intent::Reload);
                }
                for card in channels {
                    let avatar = ui
                        .allocate_ui_with_layout(
                            egui::Vec2::new(STRIP_CELL, 0.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::Vec2::splat(STRIP_AVATAR),
                                    egui::Sense::click(),
                                );
                                widgets::avatar_image(ui, rect, &card.thumbs, &mut *lookup);
                                ui.add_space(6.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&card.title).size(font::META).color(p.text2),
                                    )
                                    .truncate(),
                                );
                                response
                            },
                        )
                        .inner;
                    if avatar.clicked() {
                        intent = Some(Intent::Navigate(Route::Channel {
                            browse_id: card.id.clone(),
                        }));
                    }
                }
            });
        });
    intent
}

/// Novedades de los canales: una fila ancha por vídeo.
fn video_rows(
    ui: &mut egui::Ui,
    videos: &[&Card],
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;
    if videos.is_empty() {
        match load {
            Load::Loading | Load::Idle => widgets::skeleton_grid(ui, 6),
            Load::Failed(message) => {
                if widgets::error_state(ui, message) {
                    intent = Some(Intent::Reload);
                }
            }
            Load::Ready => widgets::empty_state(
                ui,
                "Aquí todavía no hay novedades",
                "Cuando los canales a los que te suscribas publiquen algo, aparecerá en esta lista.",
            ),
        }
        return intent;
    }

    for card in videos {
        if let Some(action) = row_card(ui, card, &mut *lookup) {
            intent = Some(intent_from(action));
        }
        // Las filas van juntas: la separación la da el propio `row_card`, que
        // deja aire dentro de su alto.
        ui.add_space(4.0);
    }
    intent
}

/// Columna de la derecha: los canales, con su avatar.
fn channel_column(
    ui: &mut egui::Ui,
    channels: &[&Card],
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    if channels.is_empty() {
        return intent;
    }
    ui.label(
        RichText::new("Canales")
            .size(font::SECTION_TITLE)
            .strong()
            .color(p.text),
    );
    ui.add_space(8.0);
    for card in channels {
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(ui.available_width(), metrics::SIDEBAR_ROW),
            egui::Sense::click(),
        );
        if response.hovered() {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(metrics::SIDEBAR_RADIUS),
                p.hover,
            );
        }
        let avatar_rect = egui::Rect::from_center_size(
            egui::pos2(
                rect.left() + metrics::AVATAR_SMALL * 0.5 + 4.0,
                rect.center().y,
            ),
            egui::Vec2::splat(metrics::AVATAR_SMALL),
        );
        widgets::avatar_image(ui, avatar_rect, &card.thumbs, &mut *lookup);
        let galley = ui.painter().layout(
            card.title.clone(),
            egui::FontId::proportional(font::SIDEBAR),
            p.text,
            rect.right() - avatar_rect.right() - 16.0,
        );
        ui.painter().galley(
            egui::pos2(
                avatar_rect.right() + 12.0,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            p.text,
        );
        if response.clicked() {
            intent = Some(Intent::Navigate(Route::Channel {
                browse_id: card.id.clone(),
            }));
        }
    }
    intent
}
