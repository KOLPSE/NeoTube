//! Página de vídeo con playlist lateral y comentarios (captura 11).
//!
//! Composición, de izquierda a derecha y de arriba abajo, igual que la
//! referencia:
//!
//! ```text
//! ┌──────────────────────────────┬────────────────────┐
//! │ reproductor (16:9)           │ panel de playlist  │
//! │ título                       │  cabecera          │
//! │ canal · suscribirse · acciones│  elemento activo   │
//! │ descripción                  │  lista con scroll  │
//! │ comentarios                  │                    │
//! └──────────────────────────────┴────────────────────┘
//! ```
//!
//! El panel de la derecha es **la playlist**, no la cola local: la referencia
//! muestra un mix de YouTube y la lista sigue al vídeo que suena. La cola local
//! tiene su propio panel en la barra inferior.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Vec2};

use crate::domain::{LikeState, WatchInfo};
use crate::ui::actions::Load;
use crate::ui::app::{Intent, PlayerView};
use crate::ui::icons::{self, Icon};
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, PillKind, ThumbLookup};

/// Lo que el usuario puede hacer en la página de vídeo.
pub enum WatchIntent {
    None,
    Action(Intent),
}

/// Entrada de la pantalla.
pub struct WatchInput<'a> {
    pub info: &'a WatchInfo,
    pub load: &'a Load,
    pub player: &'a PlayerView,
    pub queue_len: usize,
    /// Elementos de la playlist lateral (mix de YouTube o lista elegida).
    pub playlist: &'a [crate::domain::Card],
    pub playlist_title: Option<&'a str>,
    pub playlist_open: bool,
    pub comments: &'a [crate::yt::comments::Comment],
    pub comments_load: &'a Load,
    pub comments_open: bool,
    pub comments_input: &'a mut String,
    pub description_open: bool,
}

pub fn watch(
    ui: &mut egui::Ui,
    input: &mut WatchInput<'_>,
    lookup: ThumbLookup<'_>,
) -> WatchIntent {
    let p = palette(ui.ctx());
    let mut out = WatchIntent::None;
    let total_w = ui.available_width();

    // A partir de 1000 px caben vídeo y columna lateral; por debajo, la columna
    // pasa debajo, como la web.
    let side_w = if input.playlist_open {
        metrics::PLAYLIST_PANEL
    } else {
        400.0
    };
    let stacked = total_w < 1000.0;

    egui::ScrollArea::vertical()
        .id_salt("watch_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if stacked {
                if let Some(i) = main_column(ui, input, total_w, &mut *lookup) {
                    out = WatchIntent::Action(i);
                }
                ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
                if let Some(i) = side_column(ui, input, total_w, &mut *lookup) {
                    out = WatchIntent::Action(i);
                }
            } else {
                let main_w = total_w - side_w - 24.0;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(main_w, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(i) = main_column(ui, input, main_w, &mut *lookup) {
                                out = WatchIntent::Action(i);
                            }
                        },
                    );
                    ui.allocate_exact_size(Vec2::new(24.0, 1.0), Sense::hover());
                    ui.allocate_ui_with_layout(
                        Vec2::new(side_w, 0.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if let Some(i) = side_column(ui, input, side_w, &mut *lookup) {
                                out = WatchIntent::Action(i);
                            }
                        },
                    );
                });
            }
            ui.allocate_exact_size(Vec2::new(1.0, 40.0), Sense::hover());
        });

    let _ = p;
    out
}

/// Columna principal: reproductor, título, canal, acciones, descripción y
/// comentarios.
fn main_column(
    ui: &mut egui::Ui,
    input: &mut WatchInput<'_>,
    width: f32,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    let info = input.info;

    // ------------------------------------------------------- reproductor
    let video_h = width * 9.0 / 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, video_h), Sense::click());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(metrics::RADIUS_CARD),
        Color32::BLACK,
    );
    match (&input.player.texture, input.player.error.as_deref()) {
        (Some(texture), _) => {
            let shape = egui::epaint::RectShape::filled(
                rect,
                CornerRadius::same(metrics::RADIUS_CARD),
                Color32::WHITE,
            )
            .with_texture(
                texture.id(),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            );
            ui.painter().add(shape);
        }
        (None, Some(error)) => {
            ui.painter().add(egui::epaint::TextShape::new(
                rect.center(),
                ui.painter().layout_no_wrap(
                    error.to_string(),
                    FontId::proportional(font::BODY),
                    Color32::from_rgb(0xff, 0xaa, 0xaa),
                ),
                Color32::from_rgb(0xff, 0xaa, 0xaa),
            ));
        }
        (None, None) => {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Preparando el vídeo…",
                FontId::proportional(font::BODY),
                Color32::from_rgb(0xaa, 0xaa, 0xaa),
            );
            egui::Spinner::new()
                .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(24.0)));
        }
    }

    // Controles superpuestos: aparecen al pasar el ratón, como la referencia.
    if response.hovered() {
        overlay_controls(ui, rect, input.player, info, &mut intent);
    }

    ui.allocate_exact_size(Vec2::new(1.0, 12.0), Sense::hover());

    // ------------------------------------------------------------ título
    let title = if info.title.is_empty() {
        "Cargando…".to_string()
    } else {
        info.title.clone()
    };
    ui.add(
        egui::Label::new(
            RichText::new(title)
                .size(font::VIDEO_TITLE)
                .strong()
                .color(p.text),
        )
        .wrap(),
    );
    ui.allocate_exact_size(Vec2::new(1.0, 10.0), Sense::hover());

    // ------------------------------------- canal, suscripción y acciones
    ui.horizontal(|ui| {
        let (av, av_resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
        widgets::avatar_image(ui, av, &info.channel.avatar, &mut *lookup);
        if av_resp.clicked() {
            intent = Some(Intent::Open(crate::domain::Card::new(
                info.channel.id.clone(),
                crate::domain::MediaKind::Channel,
                info.channel.name.clone(),
            )));
        }
        ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(if info.channel.name.is_empty() {
                        "Canal"
                    } else {
                        &info.channel.name
                    })
                    .size(15.0)
                    .strong()
                    .color(p.text),
                )
                .sense(Sense::click()),
            );
            if let Some(subs) = &info.channel.subscribers {
                ui.label(RichText::new(subs).size(font::META).color(p.text2));
            }
        });
        ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());

        // Suscribirse / Suscrito con campana.
        let label = if info.subscribed {
            "Suscrito"
        } else {
            "Suscribirse"
        };
        if widgets::pill(
            ui,
            label,
            Some(if info.subscribed {
                Icon::Bell
            } else {
                Icon::Plus
            }),
            if info.subscribed {
                PillKind::Filled
            } else {
                PillKind::Solid
            },
        )
        .clicked()
        {
            intent = Some(Intent::ToggleSubscribe {
                channel_id: info.channel.id.clone(),
                on: !info.subscribed,
            });
        }

        // Acciones a la derecha: me gusta / no me gusta, compartir, guardar,
        // descargar y menú.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::icon_button(ui, Icon::More, 22.0, false, "Más").clicked() {
                intent = Some(Intent::Open(crate::domain::Card::new(
                    info.video_id.clone(),
                    crate::domain::MediaKind::Video,
                    info.title.clone(),
                )));
            }
            if icons::icon_button(ui, Icon::Save, 22.0, false, "Guardar").clicked() {
                intent = Some(Intent::EnqueueAll);
            }
            if icons::icon_button(ui, Icon::Share, 22.0, false, "Compartir").clicked() {
                intent = Some(Intent::None);
            }
            let liked = info.like_state == LikeState::Like;
            // Botón partido: pulgar arriba | número | pulgar abajo.
            let (like_rect, like_resp) =
                ui.allocate_exact_size(Vec2::new(96.0, 36.0), Sense::click());
            ui.painter()
                .rect_filled(like_rect, CornerRadius::same(metrics::RADIUS_PILL), p.chip);
            icons::draw(
                ui,
                Rect::from_center_size(
                    egui::pos2(like_rect.left() + 22.0, like_rect.center().y),
                    Vec2::splat(20.0),
                ),
                if liked {
                    Icon::ThumbsUpFilled
                } else {
                    Icon::ThumbsUp
                },
                if liked { p.like } else { p.text },
            );
            ui.painter().text(
                egui::pos2(like_rect.left() + 40.0, like_rect.center().y),
                Align2::LEFT_CENTER,
                info.likes_text.clone().unwrap_or_default(),
                FontId::proportional(font::META),
                p.text,
            );
            ui.painter().line_segment(
                [
                    egui::pos2(like_rect.right() - 36.0, like_rect.top() + 8.0),
                    egui::pos2(like_rect.right() - 36.0, like_rect.bottom() - 8.0),
                ],
                egui::Stroke::new(1.0, p.divider),
            );
            icons::draw(
                ui,
                Rect::from_center_size(
                    egui::pos2(like_rect.right() - 18.0, like_rect.center().y),
                    Vec2::splat(20.0),
                ),
                Icon::ThumbsDown,
                p.text,
            );
            if like_resp.clicked() {
                if let Some(pos) = like_resp.interact_pointer_pos() {
                    let on = pos.x < like_rect.right() - 36.0;
                    intent = Some(Intent::ToggleLike {
                        video_id: info.video_id.clone(),
                        on,
                    });
                }
            }
        });
    });

    ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());

    // Meta: visitas y fecha.
    let meta = [info.view_count.clone(), info.published.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    if !meta.is_empty() {
        ui.label(RichText::new(meta).size(font::BODY).strong().color(p.text));
    }
    ui.allocate_exact_size(Vec2::new(1.0, 10.0), Sense::hover());

    // ------------------------------------------------------- descripción
    if !info.description.is_empty() {
        egui::Frame::NONE
            .fill(p.chip)
            .corner_radius(CornerRadius::same(metrics::RADIUS_CARD))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(width - 24.0);
                let visible = if input.description_open {
                    info.description.clone()
                } else {
                    let mut preview: String = info
                        .description
                        .lines()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if info.description.lines().count() > 3 {
                        preview.push_str("…");
                    }
                    preview
                };
                ui.add(
                    egui::Label::new(RichText::new(visible).size(font::BODY).color(p.text)).wrap(),
                );
                if info.description.lines().count() > 3 {
                    let label = if input.description_open {
                        "Mostrar menos"
                    } else {
                        "más"
                    };
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new(label).size(font::BODY).strong().color(p.text),
                            )
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        intent = Some(Intent::ToggleDescription);
                    }
                }
                // Capítulos, si los hay: botones que saltan en el tiempo.
                if input.description_open && !info.chapters.is_empty() {
                    ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());
                    ui.label(
                        RichText::new("Capítulos")
                            .size(font::CARD_TITLE)
                            .strong()
                            .color(p.text),
                    );
                    for ch in &info.chapters {
                        let stamp = format_time((ch.start_seconds * 1000.0) as i64);
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new(format!("{stamp}  {}", ch.title))
                                        .size(font::BODY)
                                        .color(p.link),
                                )
                                .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            intent = Some(Intent::SeekSeconds(ch.start_seconds));
                        }
                    }
                }
            });
        ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
    }

    // -------------------------------------------------------- comentarios
    comments_section(ui, input, width, lookup, &mut intent);

    let _ = load_is_ok(input.load);
    intent
}

/// Barra de progreso + controles superpuestos del reproductor.
fn overlay_controls(
    ui: &mut egui::Ui,
    rect: Rect,
    player: &PlayerView,
    info: &WatchInfo,
    intent: &mut Option<Intent>,
) {
    let p = palette(ui.ctx());
    let controls_h = 40.0;
    let bar = Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - controls_h - 12.0),
        Vec2::new(rect.width(), controls_h + 12.0),
    );
    // Degradado inferior: se simula con varias franjas para no depender de
    // soporte de degradados en el pintor.
    for i in 0..8 {
        let alpha = (i as f32 / 8.0 * 200.0) as u8;
        let h = bar.height() / 8.0;
        ui.painter().rect_filled(
            Rect::from_min_size(
                egui::pos2(bar.left(), bar.top() + h * i as f32),
                Vec2::new(bar.width(), h),
            ),
            0,
            Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
        );
    }
    let ratio = if player.duration_ms > 0 {
        (player.position_ms as f32 / player.duration_ms as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = Rect::from_min_size(
        egui::pos2(bar.left() + 12.0, bar.top() + 4.0),
        Vec2::new(bar.width() - 24.0, 3.0),
    );
    ui.painter().rect_filled(
        track,
        CornerRadius::same(2),
        Color32::from_rgba_unmultiplied(255, 255, 255, 80),
    );
    ui.painter().rect_filled(
        Rect::from_min_size(track.left_top(), Vec2::new(track.width() * ratio, 3.0)),
        CornerRadius::same(2),
        p.brand,
    );
    ui.painter().circle_filled(
        egui::pos2(track.left() + track.width() * ratio, track.center().y),
        5.0,
        p.brand,
    );

    let cy = bar.bottom() - 18.0;
    let mut x = bar.left() + 20.0;
    // Reproducir / pausar.
    let play_rect = Rect::from_center_size(egui::pos2(x, cy), Vec2::splat(22.0));
    icons::draw(
        ui,
        play_rect,
        if player.is_playing {
            Icon::Pause
        } else {
            Icon::Play
        },
        Color32::WHITE,
    );
    x += 34.0;
    let next_rect = Rect::from_center_size(egui::pos2(x, cy), Vec2::splat(20.0));
    icons::draw(ui, next_rect, Icon::Next, Color32::WHITE);
    x += 30.0;
    // Tiempo.
    ui.painter().text(
        egui::pos2(x, cy),
        Align2::LEFT_CENTER,
        format!(
            "{} / {}",
            format_time(player.position_ms),
            format_time(if player.duration_ms > 0 {
                player.duration_ms
            } else {
                (info.duration_seconds.unwrap_or(0.0) * 1000.0) as i64
            })
        ),
        FontId::proportional(font::META),
        Color32::WHITE,
    );

    // A la derecha: calidad, subtítulos, ajustes, mini y pantalla completa.
    let mut rx = bar.right() - 20.0;
    for icon in [
        Icon::Fullscreen,
        Icon::Miniplayer,
        Icon::Settings,
        Icon::Subtitles,
    ] {
        icons::draw(
            ui,
            Rect::from_center_size(egui::pos2(rx - 10.0, cy), Vec2::splat(18.0)),
            icon,
            Color32::WHITE,
        );
        rx -= 32.0;
    }
    let quality = player
        .stream
        .as_ref()
        .map(|s| s.best_quality_label())
        .unwrap_or_default();
    if !quality.is_empty() {
        ui.painter().text(
            egui::pos2(rx - 4.0, cy),
            Align2::RIGHT_CENTER,
            quality,
            FontId::proportional(font::META),
            Color32::WHITE,
        );
    }

    // Zonas interactivas.
    let resp = ui.interact(
        Rect::from_min_size(
            egui::pos2(bar.left(), bar.top()),
            Vec2::new(bar.width(), bar.height()),
        ),
        egui::Id::unique("watch_controls"),
        Sense::click(),
    );
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if play_rect.expand(8.0).contains(pos) {
                *intent = Some(Intent::TogglePlay);
            } else if next_rect.expand(8.0).contains(pos) {
                *intent = Some(Intent::NextTrack);
            } else if track.expand(10.0).contains(pos) {
                let f = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
                *intent = Some(Intent::SeekFraction(f));
            } else {
                *intent = Some(Intent::TogglePlay);
            }
        }
    }
}

/// Sección de comentarios: cabecera con recuento y orden, y lista.
fn comments_section(
    ui: &mut egui::Ui,
    input: &mut WatchInput<'_>,
    width: f32,
    lookup: ThumbLookup<'_>,
    intent: &mut Option<Intent>,
) {
    let p = palette(ui.ctx());
    ui.horizontal(|ui| {
        let count = input
            .comments
            .first()
            .map(|_| input.comments.len().to_string())
            .unwrap_or_default();
        let label = if count.is_empty() {
            "Comentarios".to_string()
        } else {
            format!("{} comentarios", count)
        };
        ui.add(
            egui::Label::new(
                RichText::new(label)
                    .size(font::SECTION_TITLE)
                    .strong()
                    .color(p.text),
            )
            .sense(Sense::click()),
        );
        ui.allocate_exact_size(Vec2::new(16.0, 1.0), Sense::hover());
        if ui
            .add(
                egui::Label::new(
                    RichText::new("Ordenar por")
                        .size(font::BODY)
                        .strong()
                        .color(p.text),
                )
                .sense(Sense::click()),
            )
            .clicked()
        {
            *intent = Some(Intent::LoadMore);
        }
        icons::draw(
            ui,
            Rect::from_center_size(ui.cursor().min + Vec2::new(8.0, 8.0), Vec2::splat(16.0)),
            Icon::Sort,
            p.text,
        );
        ui.allocate_exact_size(Vec2::new(20.0, 1.0), Sense::hover());
    });
    ui.allocate_exact_size(Vec2::new(1.0, 12.0), Sense::hover());

    // Caja para escribir.
    ui.horizontal(|ui| {
        let (av, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
        ui.painter().circle_filled(av.center(), 20.0, p.chip);
        ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
        let w = width - 60.0;
        let edit = ui.add_sized(
            Vec2::new(w, 32.0),
            egui::TextEdit::singleline(input.comments_input)
                .frame(egui::Frame::NONE)
                .hint_text(
                    RichText::new("Añade un comentario…")
                        .size(font::BODY)
                        .color(p.text3),
                ),
        );
        // Subrayado, como la web.
        ui.painter().line_segment(
            [
                egui::pos2(edit.rect.left(), edit.rect.bottom()),
                egui::pos2(edit.rect.right(), edit.rect.bottom()),
            ],
            egui::Stroke::new(1.0, p.divider),
        );
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            *intent = Some(Intent::PostComment(input.comments_input.clone()));
        }
    });
    ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());

    match input.comments_load {
        Load::Loading if input.comments.is_empty() => {
            for _ in 0..3 {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 20.0, p.skeleton);
                    ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                    ui.vertical(|ui| {
                        for w in [200.0f32, 320.0] {
                            let (l, _) = ui.allocate_exact_size(Vec2::new(w, 12.0), Sense::hover());
                            ui.painter().rect_filled(l, 4, p.skeleton);
                        }
                    });
                });
                ui.allocate_exact_size(Vec2::new(1.0, 14.0), Sense::hover());
            }
        }
        Load::Failed(message) => {
            ui.label(
                RichText::new(message.as_str())
                    .size(font::BODY)
                    .color(p.text2),
            );
        }
        _ => {
            let limit = if input.comments_open {
                input.comments.len()
            } else {
                6.min(input.comments.len())
            };
            for comment in input.comments.iter().take(limit) {
                comment_row(ui, comment, width, &mut *lookup, intent);
                ui.allocate_exact_size(Vec2::new(1.0, 14.0), Sense::hover());
            }
            if !input.comments_open && input.comments.len() > limit {
                if widgets::pill(ui, "Más respuestas", None, PillKind::Filled).clicked() {
                    *intent = Some(Intent::ToggleComments);
                }
            }
        }
    }
}

/// Una fila de comentario con avatar, autor, fecha, texto, acciones y respuestas.
fn comment_row(
    ui: &mut egui::Ui,
    comment: &crate::yt::comments::Comment,
    width: f32,
    lookup: ThumbLookup<'_>,
    intent: &mut Option<Intent>,
) {
    let p = palette(ui.ctx());
    ui.horizontal_top(|ui| {
        let (av, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
        widgets::avatar_image(ui, av, &comment.author_avatar, &mut *lookup);
        ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
        ui.allocate_ui_with_layout(
            Vec2::new(width - 60.0, 0.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.horizontal(|ui| {
                    let handle = if comment.author.starts_with('@') {
                        comment.author.clone()
                    } else {
                        format!("@{}", comment.author)
                    };
                    ui.add(
                        egui::Label::new(
                            RichText::new(handle)
                                .size(font::META)
                                .strong()
                                .color(p.text),
                        )
                        .sense(Sense::click()),
                    );
                    ui.label(
                        RichText::new(&comment.published)
                            .size(font::META)
                            .color(p.text2),
                    );
                    if comment.pinned {
                        ui.label(RichText::new("Fijado").size(font::META).color(p.text2));
                    }
                });
                ui.add(
                    egui::Label::new(
                        RichText::new(&comment.text)
                            .size(font::COMMENT)
                            .color(p.text),
                    )
                    .wrap(),
                );
                ui.allocate_exact_size(Vec2::new(1.0, 6.0), Sense::hover());
                ui.horizontal(|ui| {
                    // Pulgar arriba + número.
                    let (like, like_resp) =
                        ui.allocate_exact_size(Vec2::new(52.0, 26.0), Sense::click());
                    icons::draw(
                        ui,
                        Rect::from_center_size(
                            egui::pos2(like.left() + 12.0, like.center().y),
                            Vec2::splat(16.0),
                        ),
                        Icon::ThumbsUp,
                        p.text2,
                    );
                    if !comment.likes.is_empty() {
                        ui.painter().text(
                            egui::pos2(like.left() + 24.0, like.center().y),
                            Align2::LEFT_CENTER,
                            &comment.likes,
                            FontId::proportional(font::META),
                            p.text2,
                        );
                    }
                    if like_resp.clicked() {
                        *intent = Some(Intent::RateComment {
                            comment_id: comment.id.clone(),
                            up: true,
                        });
                    }
                    icons::draw(
                        ui,
                        Rect::from_center_size(
                            ui.cursor().min + Vec2::new(10.0, 13.0),
                            Vec2::splat(16.0),
                        ),
                        Icon::ThumbsDown,
                        p.text2,
                    );
                    ui.allocate_exact_size(Vec2::new(24.0, 1.0), Sense::hover());
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new("Responder")
                                    .size(font::META)
                                    .strong()
                                    .color(p.text2),
                            )
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        *intent = Some(Intent::ToggleComments);
                    }
                });
                // Respuestas ya cargadas.
                for reply in &comment.replies {
                    ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());
                    ui.horizontal_top(|ui| {
                        let (rav, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                        widgets::avatar_image(ui, rav, &reply.author_avatar, &mut *lookup);
                        ui.allocate_exact_size(Vec2::new(8.0, 1.0), Sense::hover());
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("@{}", reply.author))
                                    .size(font::META)
                                    .strong()
                                    .color(p.text),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&reply.text).size(font::COMMENT).color(p.text),
                                )
                                .wrap(),
                            );
                        });
                    });
                }
            },
        );
    });
}

fn load_is_ok(_load: &Load) {}

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

/// Columna lateral: panel de playlist o, si está cerrado, recomendaciones.
fn side_column(
    ui: &mut egui::Ui,
    input: &mut WatchInput<'_>,
    width: f32,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    if input.playlist_open && !input.playlist.is_empty() {
        // Panel de playlist: cabecera con título y controles, y lista con
        // scroll propio.
        egui::Frame::NONE
            .fill(p.raised)
            .corner_radius(CornerRadius::same(metrics::RADIUS_PANEL))
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    input.playlist_title.unwrap_or("Lista de reproducción"),
                                )
                                .size(font::MENU_HEADER)
                                .strong()
                                .color(p.text),
                            )
                            .wrap(),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} · {} de {}",
                                if input.playlist_title.is_some() {
                                    "Mezcla"
                                } else {
                                    "Lista"
                                },
                                input
                                    .playlist
                                    .iter()
                                    .position(|c| c.id == input.info.video_id)
                                    .map(|i| i + 1)
                                    .unwrap_or(1),
                                input.playlist.len()
                            ))
                            .size(font::META)
                            .color(p.text2),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.allocate_exact_size(Vec2::new(8.0, 1.0), Sense::hover());
                        if icons::icon_button(ui, Icon::Close, 18.0, false, "Cerrar").clicked() {
                            intent = Some(Intent::TogglePlaylistPanel);
                        }
                        if icons::icon_button(ui, Icon::More, 18.0, false, "Más").clicked() {
                            intent = Some(Intent::EnqueueAll);
                        }
                        if icons::icon_button(ui, Icon::Shuffle, 18.0, false, "Aleatorio").clicked()
                        {
                            intent = Some(Intent::ToggleShuffle);
                        }
                    });
                });
                widgets::divider(ui);
                egui::ScrollArea::vertical()
                    .id_salt("playlist_scroll")
                    .max_height(560.0)
                    .show(ui, |ui| {
                        for (i, card) in input.playlist.iter().enumerate() {
                            let active = card.id == input.info.video_id;
                            let (rect, resp) = ui
                                .allocate_exact_size(Vec2::new(width - 8.0, 56.0), Sense::click());
                            if active {
                                ui.painter().rect_filled(rect, 6, p.chip);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, 6, p.hover);
                            }
                            // Miniatura con el número si es la activa.
                            let thumb = Rect::from_min_size(
                                rect.left_top() + Vec2::new(4.0, 4.0),
                                Vec2::new(84.0, 47.0),
                            );
                            widgets::thumb_image(ui, thumb, &card.thumbs, 4, &mut *lookup);
                            icons::duration_badge(
                                ui,
                                thumb,
                                card.duration.as_deref().unwrap_or(""),
                            );
                            if active {
                                // Marca de reproducción sobre la miniatura.
                                ui.painter().rect_filled(
                                    thumb,
                                    CornerRadius::same(4),
                                    Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                                );
                                icons::draw(
                                    ui,
                                    Rect::from_center_size(thumb.center(), Vec2::splat(18.0)),
                                    Icon::Play,
                                    p.brand,
                                );
                            }
                            let tx = thumb.right() + 8.0;
                            ui.painter().text(
                                egui::pos2(tx, rect.top() + 14.0),
                                Align2::LEFT_TOP,
                                &card.title,
                                FontId::proportional(font::CARD_TITLE),
                                p.text,
                            );
                            ui.painter().text(
                                egui::pos2(tx, rect.top() + 34.0),
                                Align2::LEFT_TOP,
                                &card.author,
                                FontId::proportional(font::META),
                                p.text2,
                            );
                            icons::draw(
                                ui,
                                Rect::from_center_size(
                                    egui::pos2(rect.right() - 14.0, rect.center().y),
                                    Vec2::splat(18.0),
                                ),
                                Icon::More,
                                p.text2,
                            );
                            if resp.clicked() && !active {
                                intent = Some(Intent::PlayNow(card.clone()));
                            }
                            let _ = i;
                        }
                    });
            });
    } else {
        // Filtros y recomendaciones, como la captura: chips arriba y lista.
        egui::ScrollArea::horizontal()
            .id_salt("watch_chips")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, label) in ["Todos", "Música electrónica", "Relevancia"]
                        .iter()
                        .enumerate()
                    {
                        if widgets::chip(ui, label, i == 0).clicked() {
                            intent = Some(Intent::None);
                        }
                    }
                });
            });
        ui.allocate_exact_size(Vec2::new(1.0, 10.0), Sense::hover());
        for card in input.info.related.iter().take(20) {
            if let Some(a) = widgets::row_card(ui, card, &mut *lookup) {
                intent = Some(intent_from(a));
            }
        }
    }
    intent
}
