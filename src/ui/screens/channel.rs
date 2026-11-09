//! Página de canal: banner, avatar, pestañas y contenido.
//!
//! Los canales se piden por `browseId` (`UC…`). La pestaña activa filtra los
//! estantes del mismo feed, sin volver a pedir nada: YouTube devuelve todas las
//! pestañas en la primera respuesta.

use egui::{Color32, CornerRadius, RichText, Sense, Vec2};

use crate::domain::{ChannelInfo, Feed};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::screens::home::intent_from;
use crate::ui::theme::{metrics, palette};
use crate::ui::widgets::{self, card_grid, card_list, pill_button, ThumbLookup};

/// Pestañas de la referencia.
const TABS: &[&str] = &["Inicio", "Vídeos", "Shorts", "Directos", "Listas"];

pub fn channel(
    ui: &mut egui::Ui,
    browse_id: &str,
    info: Option<&ChannelInfo>,
    feed: &Feed,
    load: &Load,
    active_tab: usize,
    subscribed: bool,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    egui::ScrollArea::vertical()
        .id_salt(format!("channel:{browse_id}"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = ui.available_width();

            // ---------------------------------------------------- banner
            let banner_height = 180.0;
            let (banner, _) = ui.allocate_exact_size(
                Vec2::new(width, banner_height),
                Sense::hover(),
            );
            match info.map(|i| &i.banner).filter(|t| !t.is_empty()) {
                Some(thumbs) => {
                    if let Some(id) = lookup(thumbs, width as u32) {
                        let mut shape = egui::epaint::RectShape::filled(
                            banner,
                            CornerRadius::same(metrics::CARD_RADIUS),
                            Color32::WHITE,
                        );
                        shape = shape.with_texture(id, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)));
                        ui.painter().add(shape);
                    } else {
                        ui.painter()
                            .rect_filled(banner, CornerRadius::same(metrics::CARD_RADIUS), p.skeleton);
                    }
                }
                None => {
                    ui.painter()
                        .rect_filled(banner, CornerRadius::same(metrics::CARD_RADIUS), p.skeleton);
                }
            }

            // ------------------------------------- avatar, nombre, acciones
            ui.horizontal_top(|ui| {
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.add_space(-36.0);
                    let thumbs = info.map(|i| &i.avatar).cloned().unwrap_or_default();
                    widgets::avatar(ui, &thumbs, 96.0, &mut *lookup);
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    let name = info
                        .map(|i| i.name.clone())
                        .filter(|n| !n.is_empty())
                        .or_else(|| {
                            feed.shelves
                                .iter()
                                .flat_map(|s| s.items.iter())
                                .find(|c| c.kind == crate::domain::MediaKind::Channel)
                                .map(|c| c.title.clone())
                        })
                        .unwrap_or_else(|| "Canal".to_string());
                    ui.label(RichText::new(name).size(28.0).strong().color(p.text));

                    let mut bits: Vec<String> = Vec::new();
                    if let Some(handle) = info.and_then(|i| i.handle.clone()) {
                        bits.push(handle);
                    }
                    if let Some(subs) = info.and_then(|i| i.subscribers.clone()) {
                        bits.push(subs);
                    }
                    if !bits.is_empty() {
                        ui.label(
                            RichText::new(bits.join(" · "))
                                .size(13.0)
                                .color(p.text_secondary),
                        );
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if pill_button(
                            ui,
                            if subscribed { "Suscrito" } else { "Suscribirse" },
                            !subscribed,
                            subscribed,
                        )
                        .clicked()
                        {
                            intent = Some(Intent::ToggleSubscribe {
                                channel_id: browse_id.to_string(),
                                on: !subscribed,
                            });
                        }
                    });
                });
            });

            ui.add_space(14.0);

            // ------------------------------------------------------ pestañas
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                for (index, label) in TABS.iter().enumerate() {
                    let selected = index == active_tab;
                    let response = widgets::chip(ui, label, selected);
                    if selected {
                        // Subrayado, como la web.
                        let rect = response.rect;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(rect.left(), rect.bottom() + 2.0),
                                Vec2::new(rect.width(), 2.0),
                            ),
                            1,
                            p.text,
                        );
                    }
                    if response.clicked() {
                        intent = Some(Intent::ChannelTab(index));
                    }
                }
            });
            ui.add_space(12.0);
            widgets::divider(ui);
            ui.add_space(12.0);

            // ------------------------------------------------------ contenido
            if let Load::Failed(message) = load {
                if widgets::error_panel(ui, message, "Reintentar") {
                    intent = Some(Intent::Reload);
                }
                return;
            }
            if feed.shelves.is_empty() {
                if load.is_loading() {
                    widgets::skeleton_grid(ui, 8);
                } else {
                    widgets::empty_panel(
                        ui,
                        "Este canal no tiene contenido visible",
                        "Puede que el contenido esté oculto o requiera sesión.",
                    );
                }
                return;
            }

            for shelf in &feed.shelves {
                // Filtro por pestaña: se decide por el tipo de las tarjetas.
                let kinds: Vec<crate::domain::MediaKind> =
                    shelf.items.iter().map(|c| c.kind).collect();
                let keep = match active_tab {
                    0 => true,
                    1 => kinds
                        .iter()
                        .any(|k| matches!(k, crate::domain::MediaKind::Video)),
                    2 => kinds
                        .iter()
                        .any(|k| matches!(k, crate::domain::MediaKind::Short)),
                    3 => kinds
                        .iter()
                        .any(|k| matches!(k, crate::domain::MediaKind::Live)),
                    4 => kinds
                        .iter()
                        .any(|k| matches!(k, crate::domain::MediaKind::Playlist)),
                    _ => true,
                };
                if !keep || shelf.items.is_empty() {
                    continue;
                }
                if let Some(title) = &shelf.title {
                    ui.label(RichText::new(title).size(18.0).strong().color(p.text));
                    ui.add_space(8.0);
                }
                let as_list = matches!(shelf.layout, crate::domain::ShelfLayout::List)
                    || shelf.items.len() < 5;
                let action = if as_list {
                    card_list(ui, &shelf.items, &mut *lookup)
                } else {
                    card_grid(ui, &shelf.items, &mut *lookup)
                };
                if let Some(a) = action {
                    intent = Some(intent_from(a));
                }
                ui.add_space(18.0);
            }
        });
    intent
}
