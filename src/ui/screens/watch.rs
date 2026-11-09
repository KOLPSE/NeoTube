//! Página de vídeo: reproductor, cabecera, acciones, descripción y relacionados.
//!
//! Es la pantalla con más partes móviles, así que se separa en bloques con la
//! misma jerarquía que la web: reproductor → título → canal y acciones →
//! descripción → relacionados.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Vec2};

use crate::domain::{LikeState, WatchInfo};
use crate::ui::actions::Load;
use crate::ui::app::{Intent, PlayerView};
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{metrics, palette};
use crate::ui::widgets::{
    self, card_row, draw_icon, icon_button, like_color, pill_button, progress_bar, Icon,
    ThumbLookup,
};

pub fn watch(
    ui: &mut egui::Ui,
    video_id: &str,
    info: &WatchInfo,
    load: &Load,
    player: &PlayerView,
    queue_len: usize,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;
    let wide = ui.available_width() > 1100.0;

    egui::ScrollArea::vertical()
        .id_salt("watch_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if wide {
                ui.horizontal_top(|ui| {
                    let related_width = 400.0;
                    let main_width = ui.available_width() - related_width - 24.0;
                    ui.allocate_ui_with_layout(
                        Vec2::new(main_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(i) = main_column(ui, video_id, info, load, player, queue_len, &mut *lookup) {
                                intent = Some(i);
                            }
                        },
                    );
                    ui.add_space(12.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(related_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(i) = related_column(ui, info, load, &mut *lookup) {
                                intent = Some(i);
                            }
                        },
                    );
                });
            } else {
                if let Some(i) = main_column(ui, video_id, info, load, player, queue_len, &mut *lookup) {
                    intent = Some(i);
                }
                ui.add_space(20.0);
                widgets::divider(ui);
                ui.add_space(12.0);
                if let Some(i) = related_column(ui, info, load, &mut *lookup) {
                    intent = Some(i);
                }
            }
            ui.add_space(40.0);
        });
    intent
}

fn main_column(
    ui: &mut egui::Ui,
    _video_id: &str,
    info: &WatchInfo,
    load: &Load,
    player: &PlayerView,
    queue_len: usize,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    // ------------------------------------------------------- reproductor
    let available = ui.available_width();
    let height = available * 9.0 / 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(available, height), Sense::click());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::CARD_RADIUS), Color32::BLACK);
    match (&player.texture, player.error.as_deref()) {
        (Some(texture), _) => {
            let mut shape = egui::epaint::RectShape::filled(
                rect,
                CornerRadius::same(metrics::CARD_RADIUS),
                Color32::WHITE,
            );
            shape = shape.with_texture(texture.id(), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
            ui.painter().add(shape);
        }
        (None, Some(error)) => {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                error,
                FontId::proportional(15.0),
                Color32::from_rgb(0xff, 0xaa, 0xaa),
            );
        }
        (None, None) => {
            egui::Spinner::new().paint_at(ui, rect.shrink(200.0));
            ui.painter().text(
                rect.center() + Vec2::new(0.0, 40.0),
                Align2::CENTER_CENTER,
                "Preparando el vídeo…",
                FontId::proportional(14.0),
                Color32::from_rgb(0xaa, 0xaa, 0xaa),
            );
        }
    }

    // Controles superpuestos: aparecen al pasar el ratón por encima, como en la
    // web, y no ocupan espacio del contenido.
    if response.hovered() {
        let bar_height = 56.0;
        let bar = Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom() - bar_height),
            Vec2::new(rect.width(), bar_height),
        );
        ui.painter().rect_filled(
            bar,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: metrics::CARD_RADIUS,
                se: metrics::CARD_RADIUS,
            },
            Color32::from_rgba_unmultiplied(0, 0, 0, 180),
        );
        let ratio = if player.duration_ms > 0 {
            player.position_ms as f32 / player.duration_ms as f32
        } else {
            0.0
        };
        let progress_rect = Rect::from_min_size(
            egui::pos2(bar.left() + 12.0, bar.top() + 6.0),
            Vec2::new(bar.width() - 24.0, 6.0),
        );
        let filled = progress_rect.width() * ratio.clamp(0.0, 1.0);
        ui.painter().rect_filled(
            progress_rect,
            CornerRadius::same(3),
            Color32::from_rgba_unmultiplied(255, 255, 255, 70),
        );
        ui.painter().rect_filled(
            Rect::from_min_size(progress_rect.left_top(), Vec2::new(filled, 6.0)),
            CornerRadius::same(3),
            p.brand,
        );
        ui.painter().circle_filled(
            egui::pos2(progress_rect.left() + filled, progress_rect.center().y),
            6.0,
            p.brand,
        );

        // Botones: reproducir/pausar, siguiente, tiempo y calidad.
        let controls_y = bar.top() + 30.0;
        let mut x = bar.left() + 16.0;
        let icon_size = 26.0;
        let play_rect = Rect::from_min_size(egui::pos2(x, controls_y - 13.0), Vec2::splat(icon_size));
        ui.painter().circle_filled(play_rect.center(), 16.0, Color32::from_rgba_unmultiplied(255,255,255,30));
        draw_icon(
            ui,
            play_rect,
            if player.is_playing { Icon::Pause } else { Icon::Play },
            Color32::WHITE,
        );
        x += 44.0;
        let next_rect = Rect::from_min_size(egui::pos2(x, controls_y - 11.0), Vec2::splat(22.0));
        draw_icon(ui, next_rect, Icon::Next, Color32::WHITE);
        x += 34.0;

        let time = format!(
            "{} / {}",
            format_time(player.position_ms),
            format_time(if player.duration_ms > 0 {
                player.duration_ms
            } else {
                info.duration_seconds.unwrap_or(0.0) as i64 * 1000
            })
        );
        ui.painter().text(
            egui::pos2(x, controls_y),
            Align2::LEFT_CENTER,
            time,
            FontId::proportional(12.0),
            Color32::WHITE,
        );

        let quality = player
            .stream
            .as_ref()
            .map(|s| s.best_quality_label())
            .unwrap_or_default();
        ui.painter().text(
            egui::pos2(bar.right() - 16.0, controls_y),
            Align2::RIGHT_CENTER,
            quality,
            FontId::proportional(12.0),
            Color32::WHITE,
        );

        // Zonas interactivas (se comprueban después de pintar para no
        // interrumpir el dibujo).
        if response.clicked() {
            if play_rect.contains(response.interact_pointer_pos().unwrap_or_default()) {
                intent = Some(Intent::TogglePlay);
            } else if next_rect.contains(response.interact_pointer_pos().unwrap_or_default()) {
                intent = Some(Intent::NextTrack);
            } else if progress_rect.expand(8.0).contains(response.interact_pointer_pos().unwrap_or_default()) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let fraction = ((pos.x - progress_rect.left()) / progress_rect.width()).clamp(0.0, 1.0);
                    intent = Some(Intent::SeekFraction(fraction));
                }
            } else {
                intent = Some(Intent::TogglePlay);
            }
        }
    }

    ui.add_space(12.0);

    // ----------------------------------------------------------- título
    let title = if info.title.is_empty() {
        "Cargando…".to_string()
    } else {
        info.title.clone()
    };
    ui.label(RichText::new(title).size(20.0).strong().color(p.text));

    ui.add_space(8.0);

    // ------------------------------------------------- canal y acciones
    ui.horizontal(|ui| {
        widgets::avatar(ui, &info.channel.avatar, 40.0, &mut *lookup);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(if info.channel.name.is_empty() {
                    "Canal"
                } else {
                    &info.channel.name
                })
                .size(15.0)
                .strong()
                .color(p.text),
            );
            if let Some(subs) = &info.channel.subscribers {
                ui.label(RichText::new(subs).size(12.0).color(p.text_secondary));
            }
        });
        ui.add_space(12.0);
        if pill_button(ui, if info.subscribed { "Suscrito" } else { "Suscribirse" }, !info.subscribed, info.subscribed).clicked() {
            if let Some(id) = info.channel.id.strip_prefix("UC") {
                intent = Some(Intent::ToggleSubscribe {
                    channel_id: format!("UC{id}"),
                    on: !info.subscribed,
                });
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, Icon::Queue, 20.0, false, "Cola").clicked() {
                intent = Some(Intent::ToggleQueue);
            }
            if queue_len > 0 {
                ui.label(
                    RichText::new(queue_len.to_string())
                        .size(11.0)
                        .color(p.text_secondary),
                );
            }
            let (like_rect, like_response) = ui
                .allocate_exact_size(Vec2::new(76.0, 36.0), Sense::click());
            let liked = info.like_state == LikeState::Like;
            let (fill, fg) = if liked {
                (like_color(true, p.text), Color32::WHITE)
            } else {
                (p.surface, p.text)
            };
            ui.painter().rect_filled(like_rect, CornerRadius::same(18), fill);
            draw_icon(
                ui,
                Rect::from_center_size(like_rect.left_center() + Vec2::new(24.0, 0.0), Vec2::splat(18.0)),
                Icon::Like,
                fg,
            );
            if let Some(likes) = &info.likes_text {
                ui.painter().text(
                    like_rect.right_center() - Vec2::new(14.0, 0.0),
                    Align2::RIGHT_CENTER,
                    likes,
                    FontId::proportional(13.0),
                    fg,
                );
            }
            if like_response.clicked() {
                intent = Some(Intent::ToggleLike {
                    video_id: info.video_id.clone(),
                    on: !liked,
                });
            }
            if icon_button(ui, Icon::Dislike, 20.0, false, "No me gusta").clicked() {
                intent = Some(Intent::ToggleLike {
                    video_id: info.video_id.clone(),
                    on: false,
                });
            }
        });
    });

    ui.add_space(10.0);

    // Meta: visualizaciones y fecha, como la web.
    let meta = [info.view_count.clone(), info.published.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    if !meta.is_empty() {
        ui.label(RichText::new(meta).size(13.0).strong().color(p.text));
    }

    ui.add_space(10.0);

    // ------------------------------------------------------ descripción
    if !info.description.is_empty() {
        let mut expanded = ui.data(|d| d.get_temp::<bool>(egui::Id::unique("desc_expanded")).unwrap_or(false));
        egui::Frame::NONE
            .fill(p.surface)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                let text = if expanded {
                    info.description.clone()
                } else {
                    // Cuatro líneas de vista previa, como la web.
                    let mut preview: String = info
                        .description
                        .lines()
                        .take(4)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if info.description.lines().count() > 4 {
                        preview.push('…');
                    }
                    preview
                };
                ui.label(RichText::new(text).size(14.0).color(p.text));
                if info.description.lines().count() > 4 {
                    let label = if expanded { "Mostrar menos" } else { "Más información" };
                    if ui
                        .add(egui::Label::new(RichText::new(label).size(13.0).strong().color(p.text)).sense(Sense::click()))
                        .clicked()
                    {
                        expanded = !expanded;
                        ui.data_mut(|d| d.insert_temp(egui::Id::unique("desc_expanded"), expanded));
                    }
                }

                // Capítulos: se pintan como botones que saltan en el tiempo.
                if expanded && !info.chapters.is_empty() {
                    ui.add_space(10.0);
                    widgets::divider(ui);
                    ui.add_space(8.0);
                    ui.label(RichText::new("Capítulos").size(14.0).strong().color(p.text));
                    for chapter in &info.chapters {
                        let stamp = format_time((chapter.start_seconds * 1000.0) as i64);
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new(format!("{stamp}  {}", chapter.title))
                                        .size(13.0)
                                        .color(p.link),
                                )
                                .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            intent = Some(Intent::SeekSeconds(chapter.start_seconds));
                        }
                    }
                }
            });
    }

    ui.add_space(8.0);

    // Barra de progreso grande bajo el vídeo: en la web es una línea fina que
    // se puede arrastrar.
    let ratio = if player.duration_ms > 0 {
        (player.position_ms as f32 / player.duration_ms as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let response = progress_bar(ui, ratio, 3.0, None);
    if response.clicked() || response.dragged() {
        if let Some(fraction) = widgets::pointer_ratio(&response) {
            intent = Some(Intent::SeekFraction(fraction));
        }
    }

    if load.is_loading() && info.title.is_empty() {
        ui.add_space(20.0);
        ui.spinner();
    }
    intent
}

/// `1:02:03` / `2:03` a partir de milisegundos.
pub fn format_time(ms: i64) -> String {
    let total = (ms.max(0) / 1000) as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Columna de "A continuación". Es una función, no un cierre, para no retener
/// el préstamo mutable del buscador de miniaturas mientras se pinta el vídeo.
fn related_column(
    ui: &mut egui::Ui,
    info: &WatchInfo,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut local = None;
    if info.related.is_empty() {
        if load.is_loading() {
            for _ in 0..5 {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 90.0), Sense::hover());
                ui.painter().rect_filled(rect, 8, p.skeleton);
            }
        }
        return local;
    }
    ui.label(RichText::new("A continuación").size(16.0).strong().color(p.text));
    ui.add_space(6.0);
    for card in info.related.iter().take(20) {
        if let Some(action) = card_row(ui, card, &mut *lookup) {
            local = Some(intent_from(action));
        }
    }
    local
}
