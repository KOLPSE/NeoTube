//! Shorts: feed vertical de una pieza por pantalla.
//!
//! Decisión de rendimiento (PLAN.md §8): se mantiene **un solo** reproductor y
//! solo existe un fotograma en memoria. Al cambiar de Short se resuelve el
//! siguiente y se sustituye; no se precargan tres vídeos ni se deja el anterior
//! en memoria. La navegación es con rueda, flechas o botones laterales.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Vec2};

use crate::domain::Card;
use crate::ui::actions::Load;
use crate::ui::app::{Intent, PlayerView};
use crate::ui::theme::{metrics, palette};
use crate::ui::widgets::{self, draw_icon, Icon, ThumbLookup};

pub fn shorts(
    ui: &mut egui::Ui,
    cards: &[Card],
    index: usize,
    load: &Load,
    player: &PlayerView,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    if let Load::Failed(message) = load {
        if widgets::error_panel(ui, message, "Reintentar") {
            return Some(Intent::Reload);
        }
        return None;
    }

    if cards.is_empty() {
        if load.is_loading() {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.spinner();
                ui.label(RichText::new("Buscando Shorts…").size(14.0).color(p.text_secondary));
            });
        } else {
            widgets::empty_panel(ui, "No hay Shorts", "Prueba a recargar en un momento.");
        }
        return None;
    }

    let card = cards.get(index.min(cards.len() - 1))?;
    let available = ui.available_size();
    // Marco vertical centrado, con la proporción 9:16 de los Shorts.
    let frame_height = available.y.min(720.0);
    let frame_width = (frame_height * 9.0 / 16.0).min(available.x * 0.7);
    let (outer, response) = ui.allocate_exact_size(available, Sense::click());
    let frame = Rect::from_center_size(outer.center(), Vec2::new(frame_width, frame_height));

    ui.painter()
        .rect_filled(frame, CornerRadius::same(metrics::CARD_RADIUS), Color32::BLACK);

    match &player.texture {
        Some(texture) => {
            let mut shape = egui::epaint::RectShape::filled(
                frame,
                CornerRadius::same(metrics::CARD_RADIUS),
                Color32::WHITE,
            );
            shape = shape.with_texture(texture.id(), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
            ui.painter().add(shape);
        }
        None => {
            // Miniatura mientras se resuelve el stream: así el cambio se ve
            // inmediato aunque la red tarde.
            if let Some(id) = lookup(&card.thumbs, frame_width as u32) {
                let mut shape = egui::epaint::RectShape::filled(
                    frame,
                    CornerRadius::same(metrics::CARD_RADIUS),
                    Color32::WHITE,
                );
                shape = shape.with_texture(id, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
                ui.painter().add(shape);
            }
            egui::Spinner::new().paint_at(ui, frame.shrink(40.0));
        }
    }

    // Degradado inferior con título y canal, como la web.
    let overlay = Rect::from_min_size(
        egui::pos2(frame.left(), frame.bottom() - 120.0),
        Vec2::new(frame.width(), 120.0),
    );
    ui.painter().rect_filled(
        overlay,
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: metrics::CARD_RADIUS,
            se: metrics::CARD_RADIUS,
        },
        Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    );
    ui.painter().text(
        egui::pos2(overlay.left() + 16.0, overlay.top() + 24.0),
        Align2::LEFT_TOP,
        &card.author,
        FontId::proportional(15.0),
        Color32::WHITE,
    );
    let title = ui.painter().layout(
        card.title.clone(),
        FontId::proportional(13.0),
        Color32::from_rgb(0xe5, 0xe5, 0xe5),
        frame_width - 90.0,
    );
    ui.painter().galley(
        egui::pos2(overlay.left() + 16.0, overlay.top() + 52.0),
        title,
        Color32::WHITE,
    );

    // Acciones laterales (me gusta, comentarios, compartir, canal).
    let actions_x = frame.right() - 36.0;
    let mut y = frame.bottom() - 220.0;
    for (icon, label) in [
        (Icon::Like, "Me gusta"),
        (Icon::Dislike, "No me gusta"),
        (Icon::Queue, "Cola"),
        (Icon::Share, "Compartir"),
    ] {
        let rect = Rect::from_center_size(egui::pos2(actions_x, y), Vec2::splat(28.0));
        ui.painter()
            .circle_filled(rect.center(), 20.0, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
        draw_icon(ui, rect, icon, Color32::WHITE);
        let hit = Rect::from_center_size(rect.center(), Vec2::splat(44.0));
        if hit.contains(response.hover_pos().unwrap_or_default()) && response.clicked() {
            intent = match label {
                "Me gusta" => Some(Intent::ToggleLike {
                    video_id: card.id.clone(),
                    on: true,
                }),
                "Cola" => Some(Intent::Enqueue(card.clone())),
                _ => Some(Intent::None),
            };
        }
        y += 52.0;
    }

    // Navegación vertical.
    let up = Rect::from_center_size(
        egui::pos2(frame.left() - 40.0, frame.center().y - 60.0),
        Vec2::splat(36.0),
    );
    let down = Rect::from_center_size(
        egui::pos2(frame.left() - 40.0, frame.center().y + 60.0),
        Vec2::splat(36.0),
    );
    ui.painter()
        .circle_filled(up.center(), 18.0, p.surface);
    ui.painter()
        .circle_filled(down.center(), 18.0, p.surface);
    draw_icon(
        ui,
        up,
        Icon::ChevronDown,
        p.text,
    );
    draw_icon(ui, down, Icon::ChevronRight, p.text);

    // Rueda del ratón y flechas como en la web.
    let scrolled = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scrolled.abs() > 20.0 {
        intent = Some(if scrolled < 0.0 {
            Intent::ShortsNext
        } else {
            Intent::ShortsPrevious
        });
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
        intent = Some(Intent::ShortsNext);
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
        intent = Some(Intent::ShortsPrevious);
    }
    if up.contains(response.hover_pos().unwrap_or_default()) && response.clicked() {
        intent = Some(Intent::ShortsPrevious);
    }
    if down.contains(response.hover_pos().unwrap_or_default()) && response.clicked() {
        intent = Some(Intent::ShortsNext);
    }
    if response.double_clicked() {
        intent = Some(Intent::TogglePlay);
    }

    // Contador de posición, útil para saber que hay más.
    ui.painter().text(
        egui::pos2(frame.left() + 16.0, frame.top() + 16.0),
        Align2::LEFT_TOP,
        format!("{} / {}", index + 1, cards.len()),
        FontId::proportional(12.0),
        Color32::WHITE,
    );
    intent
}
