//! Piezas reutilizables de la interfaz.
//!
//! Los iconos se dibujan con primitivas en vez de con glifos de fuente: así no
//! dependen de qué traiga la fuente del sistema, se ven nítidos con cualquier
//! escalado y no hay cuadros de «carácter no encontrado».

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, Ui, Vec2,
};

use crate::domain::{Card, MediaKind, Shelf, Thumbs};
use crate::ui::theme::{like_active, metrics, palette};

/// Iconos disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Shorts,
    Subscriptions,
    Music,
    Library,
    History,
    Settings,
    Search,
    Menu,
    Play,
    Pause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    RepeatOne,
    Volume,
    Mute,
    Queue,
    Fullscreen,
    FullscreenExit,
    Like,
    Dislike,
    Share,
    Close,
    Back,
    Forward,
    User,
    Clock,
    Plus,
    Check,
    Trash,
    Download,
    ChevronDown,
    ChevronRight,
}

/// Dibuja un icono centrado en `rect`.
pub fn draw_icon(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    // Lado de referencia del icono (los trazos se escalan con él).
    let s = rect.width().min(rect.height());
    let half = s * 0.5;
    let stroke = Stroke::new((s / 12.0).max(1.2), color);

    // Triángulo relleno (reproducir y derivados).
    let triangle = |dir: f32| {
        let h = half * 0.78;
        let w = half * 0.68;
        let tip = c + Vec2::new(dir * w, 0.0);
        let a = c + Vec2::new(-dir * w, -h);
        let b = c + Vec2::new(-dir * w, h);
        let mut shape = egui::epaint::PathShape::convex_polygon(
            vec![tip, a, b],
            color,
            Stroke::NONE,
        );
        shape.fill = color;
        p.add(shape);
    };

    match icon {
        Icon::Play => triangle(1.0),
        Icon::Pause => {
            let w = half * 0.28;
            let h = half * 0.78;
            p.rect_filled(
                Rect::from_center_size(c - Vec2::new(half * 0.34, 0.0), Vec2::new(w, h * 2.0)),
                CornerRadius::same(1),
                color,
            );
            p.rect_filled(
                Rect::from_center_size(c + Vec2::new(half * 0.34, 0.0), Vec2::new(w, h * 2.0)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Next => {
            triangle(1.0);
            p.rect_filled(
                Rect::from_center_size(c + Vec2::new(half * 0.62, 0.0), Vec2::new(s * 0.1, s * 0.62)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Previous => {
            triangle(-1.0);
            p.rect_filled(
                Rect::from_center_size(c - Vec2::new(half * 0.62, 0.0), Vec2::new(s * 0.1, s * 0.62)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Home => {
            // Tejado y cuerpo, como el icono de la web.
            let r = Rect::from_center_size(c + Vec2::new(0.0, half * 0.35), Vec2::new(s * 0.52, s * 0.42));
            p.rect_stroke(r, CornerRadius::same(1), stroke, egui::StrokeKind::Middle);
            p.line_segment(
                [c + Vec2::new(-half * 0.42, half * 0.15), c + Vec2::new(0.0, -half * 0.45)],
                stroke,
            );
            p.line_segment(
                [c + Vec2::new(half * 0.42, half * 0.15), c + Vec2::new(0.0, -half * 0.45)],
                stroke,
            );
        }
        Icon::Shorts => {
            let r = Rect::from_center_size(c, Vec2::new(s * 0.46, s * 0.72));
            p.rect_stroke(r, CornerRadius::same((s * 0.22) as u8), stroke, egui::StrokeKind::Middle);
            triangle(1.0);
        }
        Icon::Subscriptions => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.72, s * 0.56)),
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            p.line_segment([c + Vec2::new(-half * 0.18, -half * 0.03), c + Vec2::new(half * 0.18, -half * 0.03)], stroke);
            p.circle_filled(c + Vec2::new(0.0, half * 0.62), s * 0.1, color);
        }
        Icon::Music => {
            p.line_segment([c + Vec2::new(half * 0.3, -half * 0.42), c + Vec2::new(half * 0.3, half * 0.24)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.38, -half * 0.32), c + Vec2::new(-half * 0.38, half * 0.34)], stroke);
            p.line_segment(
                [c + Vec2::new(half * 0.3, -half * 0.42), c + Vec2::new(-half * 0.38, -half * 0.32)],
                stroke,
            );
            p.circle_filled(c + Vec2::new(half * 0.14, half * 0.3), s * 0.13, color);
            p.circle_filled(c + Vec2::new(-half * 0.54, half * 0.4), s * 0.13, color);
        }
        Icon::Library => {
            p.line_segment([c + Vec2::new(-half * 0.42, -half * 0.42), c + Vec2::new(-half * 0.42, half * 0.42)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.14, -half * 0.42), c + Vec2::new(-half * 0.14, half * 0.42)], stroke);
            p.line_segment([c + Vec2::new(half * 0.14, -half * 0.3), c + Vec2::new(half * 0.42, half * 0.4)], stroke);
        }
        Icon::History => {
            p.circle_stroke(c, half * 0.62, stroke);
            p.line_segment([c, c + Vec2::new(0.0, -half * 0.36)], stroke);
            p.line_segment([c, c + Vec2::new(half * 0.3, half * 0.1)], stroke);
        }
        Icon::Clock => {
            p.circle_stroke(c, half * 0.62, stroke);
            p.line_segment([c, c + Vec2::new(0.0, -half * 0.34)], stroke);
            p.line_segment([c, c + Vec2::new(half * 0.26, 0.0)], stroke);
        }
        Icon::Settings => {
            p.circle_stroke(c, half * 0.3, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let dir = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + dir * half * 0.52, c + dir * half * 0.82], stroke);
            }
        }
        Icon::Search => {
            p.circle_stroke(c - Vec2::new(half * 0.12, half * 0.12), half * 0.5, stroke);
            p.line_segment(
                [c + Vec2::new(half * 0.26, half * 0.26), c + Vec2::new(half * 0.7, half * 0.7)],
                Stroke::new(stroke.width * 1.3, color),
            );
        }
        Icon::Menu => {
            for i in [-1.0f32, 0.0, 1.0] {
                p.line_segment(
                    [c + Vec2::new(-half * 0.6, i * half * 0.38), c + Vec2::new(half * 0.6, i * half * 0.38)],
                    stroke,
                );
            }
        }
        Icon::Shuffle => {
            p.line_segment([c + Vec2::new(-half * 0.6, -half * 0.34), c + Vec2::new(half * 0.6, half * 0.34)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.6, half * 0.34), c + Vec2::new(half * 0.6, -half * 0.34)], stroke);
        }
        Icon::Repeat | Icon::RepeatOne => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.72, s * 0.48)),
                CornerRadius::same((s * 0.18) as u8),
                stroke,
                egui::StrokeKind::Middle,
            );
            triangle(1.0);
            let _ = metrics::CARD_RADIUS;
        }
        Icon::Volume => {
            p.rect_filled(
                Rect::from_center_size(c - Vec2::new(half * 0.32, 0.0), Vec2::new(s * 0.2, s * 0.3)),
                CornerRadius::same(1),
                color,
            );
            let mut tri = egui::epaint::PathShape::convex_polygon(
                vec![
                    c + Vec2::new(-half * 0.2, -half * 0.3),
                    c + Vec2::new(half * 0.16, -half * 0.56),
                    c + Vec2::new(half * 0.16, half * 0.56),
                    c + Vec2::new(-half * 0.2, half * 0.3),
                ],
                color,
                Stroke::NONE,
            );
            tri.fill = color;
            p.add(tri);
            p.circle_stroke(c + Vec2::new(half * 0.1, 0.0), half * 0.4, stroke);
        }
        Icon::Mute => {
            p.rect_filled(
                Rect::from_center_size(c - Vec2::new(half * 0.32, 0.0), Vec2::new(s * 0.2, s * 0.3)),
                CornerRadius::same(1),
                color,
            );
            p.line_segment(
                [c + Vec2::new(-half * 0.1, -half * 0.3), c + Vec2::new(half * 0.5, half * 0.5)],
                stroke,
            );
            p.line_segment(
                [c + Vec2::new(-half * 0.1, half * 0.3), c + Vec2::new(half * 0.5, -half * 0.5)],
                stroke,
            );
        }
        Icon::Queue => {
            for i in [-1.0f32, 0.0, 1.0] {
                p.line_segment(
                    [c + Vec2::new(-half * 0.6, i * half * 0.4), c + Vec2::new(half * 0.4, i * half * 0.4)],
                    stroke,
                );
            }
            p.circle_filled(c + Vec2::new(half * 0.55, half * 0.4), s * 0.08, color);
        }
        Icon::Fullscreen => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = c + Vec2::new(sx * half * 0.62, sy * half * 0.62);
                p.line_segment([corner, corner - Vec2::new(sx * half * 0.3, 0.0)], stroke);
                p.line_segment([corner, corner - Vec2::new(0.0, sy * half * 0.3)], stroke);
            }
        }
        Icon::FullscreenExit => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = c + Vec2::new(sx * half * 0.65, sy * half * 0.65);
                p.line_segment([corner, corner + Vec2::new(sx * half * 0.3, 0.0)], stroke);
                p.line_segment([corner, corner + Vec2::new(0.0, sy * half * 0.3)], stroke);
            }
        }
        Icon::Like => {
            p.circle_stroke(c, half * 0.62, stroke);
            // Pulgar estilizado: el círculo basta para leerse a 20 px.
            p.line_segment([c + Vec2::new(-half * 0.1, half * 0.16), c + Vec2::new(half * 0.3, -half * 0.28)], stroke);
        }
        Icon::Dislike => {
            p.circle_stroke(c, half * 0.62, stroke);
            p.line_segment([c + Vec2::new(-half * 0.1, -half * 0.16), c + Vec2::new(half * 0.3, half * 0.28)], stroke);
        }
        Icon::Share => {
            p.line_segment([c + Vec2::new(-half * 0.4, 0.0), c + Vec2::new(half * 0.5, -half * 0.45)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.4, 0.0), c + Vec2::new(half * 0.5, half * 0.45)], stroke);
            p.circle_filled(c + Vec2::new(-half * 0.45, 0.0), s * 0.09, color);
        }
        Icon::Close => {
            p.line_segment([c + Vec2::new(-half * 0.42, -half * 0.42), c + Vec2::new(half * 0.42, half * 0.42)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.42, half * 0.42), c + Vec2::new(half * 0.42, -half * 0.42)], stroke);
        }
        Icon::Back => {
            p.line_segment([c + Vec2::new(half * 0.35, -half * 0.5), c + Vec2::new(-half * 0.35, 0.0)], stroke);
            p.line_segment([c + Vec2::new(half * 0.35, half * 0.5), c + Vec2::new(-half * 0.35, 0.0)], stroke);
        }
        Icon::Forward => {
            p.line_segment([c + Vec2::new(-half * 0.35, -half * 0.5), c + Vec2::new(half * 0.35, 0.0)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.35, half * 0.5), c + Vec2::new(half * 0.35, 0.0)], stroke);
        }
        Icon::User => {
            p.circle_stroke(c + Vec2::new(0.0, -half * 0.28), half * 0.3, stroke);
            p.line_segment(
                [c + Vec2::new(-half * 0.5, half * 0.55), c + Vec2::new(half * 0.5, half * 0.55)],
                stroke,
            );
        }
        Icon::Plus => {
            p.line_segment([c + Vec2::new(-half * 0.45, 0.0), c + Vec2::new(half * 0.45, 0.0)], stroke);
            p.line_segment([c + Vec2::new(0.0, -half * 0.45), c + Vec2::new(0.0, half * 0.45)], stroke);
        }
        Icon::Check => {
            p.line_segment([c + Vec2::new(-half * 0.4, 0.0), c + Vec2::new(-half * 0.1, half * 0.35)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.1, half * 0.35), c + Vec2::new(half * 0.45, -half * 0.4)], stroke);
        }
        Icon::Trash => {
            p.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, half * 0.1), Vec2::new(s * 0.5, s * 0.6)),
                CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Middle,
            );
            p.line_segment([c + Vec2::new(-half * 0.36, -half * 0.24), c + Vec2::new(half * 0.36, -half * 0.24)], stroke);
        }
        Icon::Download => {
            p.line_segment([c + Vec2::new(0.0, -half * 0.5), c + Vec2::new(0.0, half * 0.15)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.3, -half * 0.1), c + Vec2::new(0.0, half * 0.2)], stroke);
            p.line_segment([c + Vec2::new(half * 0.3, -half * 0.1), c + Vec2::new(0.0, half * 0.2)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.45, half * 0.45), c + Vec2::new(half * 0.45, half * 0.45)], stroke);
        }
        Icon::ChevronDown => {
            p.line_segment([c + Vec2::new(-half * 0.4, -half * 0.2), c + Vec2::new(0.0, half * 0.25)], stroke);
            p.line_segment([c + Vec2::new(half * 0.4, -half * 0.2), c + Vec2::new(0.0, half * 0.25)], stroke);
        }
        Icon::ChevronRight => {
            p.line_segment([c + Vec2::new(-half * 0.2, -half * 0.4), c + Vec2::new(half * 0.25, 0.0)], stroke);
            p.line_segment([c + Vec2::new(-half * 0.2, half * 0.4), c + Vec2::new(half * 0.25, 0.0)], stroke);
        }
    }
}

/// Botón circular con icono, como los de la barra superior de YouTube.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, active: bool, tooltip: &str) -> Response {
    let p = palette(ui.ctx());
    let side = size + 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let hovered = response.hovered();
    if hovered || active {
        let bg = if active { p.surface_active } else { p.surface_hover };
        ui.painter()
            .circle_filled(rect.center(), side * 0.5, bg);
    }
    let color = if active { p.text } else { p.text };
    draw_icon(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(size)),
        icon,
        color,
    );
    if !tooltip.is_empty() {
        response.clone().on_hover_text(tooltip)
    } else {
        response
    }
}

/// Botón con texto, opcionalmente relleno (como «Suscribirse»).
pub fn pill_button(
    ui: &mut Ui,
    label: &str,
    filled: bool,
    active: bool,
) -> Response {
    let p = palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    let height = 36.0;
    let width = galley.size().x + 32.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let fill = if filled {
        if response.hovered() {
            Color32::from_rgb(0xd9, 0xd9, 0xd9)
        } else {
            Color32::WHITE
        }
    } else if active {
        p.surface_active
    } else if response.hovered() {
        p.surface_hover
    } else {
        p.surface
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(height as u8 / 2), fill);
    let text_color = if filled { Color32::BLACK } else { p.text };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        text_color,
    );
    response
}

/// Chip de filtro (los de la fila superior de Inicio y Búsqueda).
pub fn chip(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let p = palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0, 32.0), Sense::click());
    let fill = if selected {
        p.text
    } else if response.hovered() {
        p.surface_hover
    } else {
        p.chip
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::CHIP_RADIUS), fill);
    let color = if selected { p.chip_active_text } else { p.text };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(14.0), color);
    response
}

/// Marca de duración sobre la miniatura (`12:34`).
fn duration_badge(ui: &Ui, rect: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.0),
        Color32::WHITE,
    );
    let size = galley.size() + Vec2::new(8.0, 3.0);
    let badge = Rect::from_min_size(
        egui::pos2(rect.right() - size.x - 6.0, rect.bottom() - size.y - 6.0),
        size,
    );
    ui.painter().rect_filled(
        badge,
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(0, 0, 0, 200),
    );
    ui.painter().galley(
        badge.center() - galley.size() * 0.5,
        galley,
        Color32::WHITE,
    );
}

/// Función que devuelve la textura de una miniatura, si ya está cargada.
pub type ThumbLookup<'a> = &'a mut dyn FnMut(&Thumbs, u32) -> Option<egui::TextureId>;

/// Pinta la miniatura o un esqueleto si aún no está.
fn paint_thumb(
    ui: &Ui,
    rect: Rect,
    thumbs: &Thumbs,
    lookup: ThumbLookup<'_>,
    radius: u8,
) {
    let p = palette(ui.ctx());
    let texture = lookup(thumbs, rect.width().max(48.0) as u32);
    match texture {
        Some(id) => {
            let mut mesh = egui::epaint::RectShape::filled(
                rect,
                CornerRadius::same(radius),
                Color32::WHITE,
            );
            mesh = mesh.with_texture(id, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
            ui.painter().add(mesh);
        }
        None => {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(radius), p.skeleton);
        }
    }
}

/// Tarjeta de la rejilla (Inicio, Búsqueda, relacionados).
///
/// Al pasar el ratón aparece un botón de reproducción y, si es una lista, la
/// superposición oscura — como en la web.
pub fn card_tile(ui: &mut Ui, card: &Card, width: f32, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let p = palette(ui.ctx());
    let thumb_height = width * 9.0 / 16.0;
    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.set_width(width);
            let (thumb_rect, thumb_resp) =
                ui.allocate_exact_size(Vec2::new(width, thumb_height), Sense::click());
            paint_thumb(ui, thumb_rect, &card.thumbs, lookup, metrics::CARD_RADIUS);
            if let Some(dur) = &card.duration {
                duration_badge(ui, thumb_rect, dur);
            }
            if let Some(badge) = &card.badge {
                let galley = ui.painter().layout_no_wrap(
                    badge.clone(),
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
                let size = galley.size() + Vec2::new(10.0, 3.0);
                let box_rect = Rect::from_min_size(
                    thumb_rect.left_top() + Vec2::new(6.0, 6.0),
                    size,
                );
                ui.painter().rect_filled(
                    box_rect,
                    CornerRadius::same(4),
                    Color32::from_rgba_unmultiplied(200, 0, 0, 220),
                );
                ui.painter().galley(
                    box_rect.center() - galley.size() * 0.5,
                    galley,
                    Color32::WHITE,
                );
            }

            if thumb_resp.hovered() {
                let circle = thumb_rect.right_bottom() - Vec2::new(24.0, 24.0);
                ui.painter().circle_filled(
                    circle,
                    18.0,
                    Color32::from_rgba_unmultiplied(0, 0, 0, 190),
                );
                draw_icon(
                    ui,
                    Rect::from_center_size(circle, Vec2::splat(18.0)),
                    if card.kind == MediaKind::Playlist || card.kind == MediaKind::Album || card.kind == MediaKind::Mix {
                        Icon::Queue
                    } else {
                        Icon::Play
                    },
                    Color32::WHITE,
                );
            }

            ui.add_space(10.0);
            let title_resp = ui.add(
                egui::Label::new(
                    RichText::new(&card.title)
                        .size(15.0)
                        .strong()
                        .color(p.text),
                )
                .truncate(),
            );
            if !card.author.is_empty() {
                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(&card.author)
                            .size(13.0)
                            .color(p.text_secondary),
                    )
                    .truncate(),
                );
                if resp.clicked() {
                    return Some(CardAction::OpenAuthor(card.author_id.clone()));
                }
            }
            if !card.meta.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(&card.meta).size(13.0).color(p.text_secondary))
                        .truncate(),
                );
            }

            if thumb_resp.clicked() || title_resp.clicked() {
                return Some(CardAction::Open(card.clone()));
            }
            if thumb_resp.double_clicked() {
                return Some(CardAction::PlayNow(card.clone()));
            }
            None
        },
    )
    .inner
}

/// Fila compacta (resultados de búsqueda, relacionados, playlists). Formato
/// horizontal con la miniatura a la izquierda, como `compactVideoRenderer`.
pub fn card_row(ui: &mut Ui, card: &Card, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let thumb_width = (width * 0.32).clamp(120.0, 220.0);
    let thumb_height = thumb_width * 9.0 / 16.0;

    let mut action = None;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, thumb_height + 8.0),
        Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(metrics::CARD_RADIUS),
        if response.hovered() {
            p.surface
        } else {
            Color32::TRANSPARENT
        },
    );
    let thumb_rect = Rect::from_min_size(
        rect.left_top() + Vec2::new(4.0, 4.0),
        Vec2::new(thumb_width, thumb_height),
    );
    paint_thumb(ui, thumb_rect, &card.thumbs, lookup, 8);
    if let Some(dur) = &card.duration {
        duration_badge(ui, thumb_rect, dur);
    }

    let text_left = thumb_rect.right() + 12.0;
    let text_width = rect.right() - text_left - 8.0;
    let mut y = rect.top() + 6.0;
    let mut paint_line = |text: &str, size: f32, color: Color32, strong: bool| {
        if text.is_empty() {
            return;
        }
        let g = ui.painter().layout(
            text.to_string(),
            FontId::proportional(size),
            color,
            text_width,
        );
        let height = g.size().y;
        ui.painter().galley(
            egui::pos2(text_left, y),
            g,
            color,
        );
        let _ = strong;
        y += height + 2.0;
    };
    paint_line(&card.title, 14.0, p.text, true);
    paint_line(&card.author, 12.0, p.text_secondary, false);
    paint_line(&card.meta, 12.0, p.text_secondary, false);

    if response.clicked() {
        action = Some(CardAction::Open(card.clone()));
    }
    if response.double_clicked() {
        action = Some(CardAction::PlayNow(card.clone()));
    }
    action
}

/// Lo que el usuario ha hecho con una tarjeta.
#[derive(Debug, Clone)]
pub enum CardAction {
    /// Abrir la página del contenido.
    Open(Card),
    /// Reproducir ya, sin salir de la pantalla.
    PlayNow(Card),
    /// Ir al canal o artista.
    OpenAuthor(Option<String>),
    /// Poner en cola (botón explícito).
    Enqueue(Card),
}

/// Estante horizontal con título y desplazamiento lateral.
pub fn shelf_row(ui: &mut Ui, shelf: &Shelf, lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let p = palette(ui.ctx());
    if let Some(title) = &shelf.title {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(title).size(20.0).strong().color(p.text));
        });
    }
    let card_width = 320.0f32;
    egui::ScrollArea::horizontal()
        .id_salt(shelf.title.clone().unwrap_or_default())
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for card in &shelf.items {
                    let action = ui
                        .allocate_ui_with_layout(
                            Vec2::new(card_width, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| card_tile(ui, card, card_width, &mut *lookup),
                        )
                        .inner;
                    if action.is_some() {
                        return;
                    }
                }
            });
        });
    ui.add_space(8.0);
    None
}

/// Rejilla adaptable de tarjetas. Devuelve la acción si alguna se activó.
pub fn card_grid(ui: &mut Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let spacing = metrics::GAP;
    let avail = ui.available_width();
    let columns = ((avail + spacing) / (metrics::CARD_MIN_WIDTH + spacing))
        .floor()
        .max(1.0) as usize;
    let width = (avail - spacing * (columns as f32 - 1.0)) / columns as f32;
    let mut action = None;
    egui::Grid::new("card_grid")
        .num_columns(columns)
        .spacing(Vec2::new(spacing, 24.0))
        .show(ui, |ui| {
            for (i, card) in cards.iter().enumerate() {
                if let Some(a) = card_tile(ui, card, width, &mut *lookup) {
                    action = Some(a);
                }
                if (i + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
    action
}

/// Lista vertical de tarjetas compactas.
pub fn card_list(ui: &mut Ui, cards: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let mut action = None;
    for card in cards {
        if let Some(a) = card_row(ui, card, &mut *lookup) {
            action = Some(a);
        }
    }
    action
}

/// Esqueleto de carga con la forma del contenido (evita el salto de diseño).
pub fn skeleton_grid(ui: &mut Ui, count: usize) {
    let p = palette(ui.ctx());
    let spacing = metrics::GAP;
    let avail = ui.available_width();
    let columns = ((avail + spacing) / (metrics::CARD_MIN_WIDTH + spacing))
        .floor()
        .max(1.0) as usize;
    let width = (avail - spacing * (columns as f32 - 1.0)) / columns as f32;
    let mut drawn = 0;
    egui::Grid::new("skeleton_grid")
        .num_columns(columns)
        .spacing(Vec2::new(spacing, 24.0))
        .show(ui, |ui| {
            while drawn < count {
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(width, width * 9.0 / 16.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(metrics::CARD_RADIUS),
                    p.skeleton,
                );
                ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
                drawn += 1;
                if drawn % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

/// Mensaje de error con botón de reintento.
pub fn error_panel(ui: &mut Ui, message: &str, retry: &str) -> bool {
    let p = palette(ui.ctx());
    let mut retry_clicked = false;
    egui::Frame::NONE
        .fill(p.surface)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(20))
        .show(ui, |ui| {
            ui.set_max_width(560.0);
            ui.label(RichText::new(message).size(14.0).color(p.text));
            ui.add_space(10.0);
            if pill_button(ui, retry, false, false).clicked() {
                retry_clicked = true;
            }
        });
    retry_clicked
}

/// Contenedor de estado vacío, con el texto de YouTube («No hay resultados»).
pub fn empty_panel(ui: &mut Ui, title: &str, detail: &str) {
    let p = palette(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(RichText::new(title).size(18.0).color(p.text));
        ui.add_space(6.0);
        ui.label(RichText::new(detail).size(14.0).color(p.text_secondary));
    });
}

/// Cabecera de sección con acción a la derecha.
pub fn section_header(ui: &mut Ui, title: &str, subtitle: Option<&str>) {
    let p = palette(ui.ctx());
    ui.add_space(8.0);
    ui.label(RichText::new(title).size(22.0).strong().color(p.text));
    if let Some(sub) = subtitle {
        ui.label(RichText::new(sub).size(13.0).color(p.text_secondary));
    }
    ui.add_space(10.0);
}

/// Separador de 1 px como los de la web.
pub fn divider(ui: &mut Ui) {
    let p = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0, p.divider);
}

/// Barra de progreso del reproductor (roja, como la web).
pub fn progress_bar(ui: &mut Ui, ratio: f32, height: f32, hovered_ratio: Option<f32>) -> Response {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, height.max(3.0) + 6.0), Sense::click_and_drag());
    let bar = Rect::from_center_size(rect.center(), Vec2::new(width, height.max(3.0)));
    ui.painter()
        .rect_filled(bar, CornerRadius::same(2), p.surface_hover);
    let filled = (width * ratio.clamp(0.0, 1.0)).max(0.0);
    ui.painter().rect_filled(
        Rect::from_min_size(bar.left_top(), Vec2::new(filled, bar.height())),
        CornerRadius::same(2),
        p.brand,
    );
    if let Some(ratio) = hovered_ratio {
        let x = bar.left() + width * ratio.clamp(0.0, 1.0);
        ui.painter()
            .circle_filled(egui::pos2(x, bar.center().y), height.max(3.0) + 1.0, p.brand);
    }
    response
}

/// Ratio (0..1) de una posición del puntero dentro de un `Response`.
pub fn pointer_ratio(response: &Response) -> Option<f32> {
    let pos = response.hover_pos()?;
    let rect = response.rect;
    if rect.width() <= 0.0 {
        return None;
    }
    Some(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

/// Miniatura circular (avatar de canal).
pub fn avatar(ui: &mut Ui, thumbs: &Thumbs, size: f32, lookup: ThumbLookup<'_>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let texture = lookup(thumbs, size as u32);
    match texture {
        Some(id) => {
            // Se recorta a círculo con una máscara de 4 esquinas redondeadas al
            // máximo: es lo más parecido sin shaders propios.
            let mut shape = egui::epaint::RectShape::filled(
                rect,
                CornerRadius::same((size * 0.5) as u8),
                Color32::WHITE,
            );
            shape = shape.with_texture(id, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
            ui.painter().add(shape);
        }
        None => {
            let p = palette(ui.ctx());
            ui.painter()
                .circle_filled(rect.center(), size * 0.5, p.skeleton);
        }
    }
}

/// Texto con el color de "me gusta" cuando está activo.
pub fn like_color(active: bool, normal: Color32) -> Color32 {
    if active {
        like_active()
    } else {
        normal
    }
}
