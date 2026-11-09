//! Música: portada, álbum, artista y lista.
//!
//! Comparte los modelos con el resto (`Feed`/`Shelf`), así que las pantallas son
//! casi la misma composición con cabeceras distintas. Lo que cambia de verdad es
//! el comportamiento: al pulsar una canción se reproduce **solo audio** (sin
//! decodificar vídeo), que es la mitad del consumo en este modo.

use egui::{Color32, CornerRadius, RichText, Sense, Vec2};

use crate::domain::{Card, Feed, MediaKind};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::screens::home::intent_from;
use crate::ui::theme::palette;
use crate::ui::widgets::{self, card_row, card_tile, pill_button, ThumbLookup};

/// Cabecera grande de álbum/artista/lista: carátula + título + acciones.
fn hero(
    ui: &mut egui::Ui,
    card: Option<&Card>,
    title: &str,
    subtitle: &str,
    lookup: ThumbLookup<'_>,
    play_all: &str,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    ui.horizontal_top(|ui| {
        let size = 200.0;
        match card {
            Some(card) => {
                // La carátula usa el mismo componente que las tarjetas.
                ui.allocate_ui_with_layout(
                    Vec2::new(size, size),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if let Some(id) = lookup(&card.thumbs, size as u32) {
                            let rect = egui::Rect::from_min_size(
                                ui.cursor().min,
                                Vec2::splat(size),
                            );
                            let mut shape = egui::epaint::RectShape::filled(
                                rect,
                                CornerRadius::same(8),
                                Color32::WHITE,
                            );
                            shape = shape.with_texture(id, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
                            ui.painter().add(shape);
                            ui.allocate_space(Vec2::splat(size));
                        } else {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::splat(size),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 8, p.skeleton);
                        }
                    },
                );
            }
            None => {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
                ui.painter().rect_filled(rect, 8, p.skeleton);
            }
        }
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(match card.map(|c| c.kind) {
                    Some(MediaKind::Album) => "Álbum",
                    Some(MediaKind::Artist) => "Artista",
                    Some(MediaKind::Playlist) => "Lista",
                    _ => "Música",
                })
                .size(12.0)
                .color(p.text_secondary),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(title).size(34.0).strong().color(p.text));
            if !subtitle.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(subtitle).size(13.0).color(p.text_secondary));
            }
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                if pill_button(ui, play_all, true, false).clicked() {
                    intent = Some(Intent::PlayAll);
                }
                if pill_button(ui, "Aleatorio", false, false).clicked() {
                    intent = Some(Intent::PlayAllShuffled);
                }
                if pill_button(ui, "Añadir a la cola", false, false).clicked() {
                    intent = Some(Intent::EnqueueAll);
                }
            });
        });
    });
    ui.add_space(20.0);
    intent
}

/// Portada de YouTube Music: carruseles por sección, como la web.
pub fn music_home(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    render_feed(ui, feed, load, lookup, true)
}

/// Página de álbum: cabecera con carátula y la lista de pistas.
pub fn album(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = render_browse(ui, browse_id, feed, load, lookup, "Reproducir")?;
    let _ = &mut intent;
    Some(intent)
}

/// Página de artista: cabecera y secciones (sus canciones, álbumes, similares).
pub fn artist(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    render_browse(ui, browse_id, feed, load, lookup, "Reproducir")
}

/// Lista: cabecera y pistas en orden.
pub fn playlist(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    render_browse(ui, browse_id, feed, load, lookup, "Reproducir todo")
}

/// Composición común de álbum/artista/lista.
fn render_browse(
    ui: &mut egui::Ui,
    browse_id: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
    play_label: &str,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent: Option<Intent> = None;

    if let Load::Failed(message) = load {
        if feed.shelves.is_empty() {
            if widgets::error_panel(ui, message, "Reintentar") {
                intent = Some(Intent::Reload);
            }
            return intent;
        }
    }

    egui::ScrollArea::vertical()
        .id_salt(format!("browse:{browse_id}"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if feed.shelves.is_empty() {
                // Esqueleto con la forma de la cabecera mientras carga.
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 200.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(rect, 12, p.skeleton);
                return;
            }

            // La primera tarjeta suele ser la propia entidad (álbum, artista o
            // lista) y sirve de carátula de la cabecera.
            let hero_card = feed
                .shelves
                .iter()
                .flat_map(|s| s.items.iter())
                .find(|c| {
                    matches!(
                        c.kind,
                        MediaKind::Album | MediaKind::Artist | MediaKind::Playlist
                    ) && !c.thumbs.is_empty()
                })
                .cloned();
            let title = hero_card
                .as_ref()
                .map(|c| c.title.clone())
                .or_else(|| feed.shelves.first().and_then(|s| s.title.clone()))
                .unwrap_or_else(|| "Cargando…".into());
            let subtitle = hero_card
                .as_ref()
                .map(|c| c.meta.clone())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "YouTube Music".into());

            if let Some(i) = hero(ui, hero_card.as_ref(), &title, &subtitle, lookup, play_label) {
                intent = Some(i);
            }

            if intent.is_none() {
                intent = render_feed(ui, feed, load, &mut *lookup, false);
            }
        });
    intent
}

/// Pinta los estantes de un feed musical. `carousels` decide si se pinta en
/// rejilla (portada) o en listas (álbum/lista de pistas).
fn render_feed(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
    carousels: bool,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    if feed.shelves.is_empty() {
        if load.is_loading() {
            widgets::skeleton_grid(ui, 6);
        } else if let Some(err) = load.error() {
            if widgets::error_panel(ui, err, "Reintentar") {
                intent = Some(Intent::Reload);
            }
        } else {
            widgets::empty_panel(
                ui,
                "Nada por aquí",
                "YouTube Music no devolvió contenido para esta sección.",
            );
        }
        return intent;
    }

    for shelf in &feed.shelves {
        if shelf.items.is_empty() {
            continue;
        }
        if let Some(title) = &shelf.title {
            ui.add_space(10.0);
            ui.label(RichText::new(title).size(22.0).strong().color(p.text));
            ui.add_space(8.0);
        }
        let has_audio = shelf.items.iter().any(|c| {
            matches!(c.kind, MediaKind::Song | MediaKind::Video)
        });
        if carousels && !shelf.items.iter().all(|c| matches!(c.kind, MediaKind::Song)) {
            // Carrusel horizontal de carátulas (álbumes, artistas, listas).
            egui::ScrollArea::horizontal()
                .id_salt(format!("music:{}", shelf.title.clone().unwrap_or_default()))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for card in &shelf.items {
                            let action = ui
                                .allocate_ui_with_layout(
                                    Vec2::new(180.0, 0.0),
                                    egui::Layout::top_down(egui::Align::LEFT),
                                    |ui| card_tile(ui, card, 180.0, &mut *lookup),
                                )
                                .inner;
                            if let Some(a) = action {
                                intent = Some(intent_from(a));
                            }
                        }
                    });
                });
        } else if has_audio || !carousels {
            // Lista de pistas: es el formato de álbum y de lista de reproducción.
            for (index, card) in shelf.items.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{}", index + 1))
                            .size(13.0)
                            .color(p.text_tertiary),
                    );
                    ui.add_space(4.0);
                    let width = ui.available_width();
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(a) = card_row(ui, card, &mut *lookup) {
                                intent = Some(intent_from(a));
                            }
                        },
                    );
                });
            }
        } else {
            if let Some(a) = widgets::card_grid(ui, &shelf.items, &mut *lookup) {
                intent = Some(intent_from(a));
            }
        }
        ui.add_space(16.0);
    }
    intent
}
