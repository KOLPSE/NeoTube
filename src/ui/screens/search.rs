//! Búsqueda: filtros, resultados mezclados y paginación.

use crate::domain::Feed;
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::screens::home::intent_from;
use crate::ui::theme::palette;
use crate::ui::widgets::{self, card_list, chip, ThumbLookup};

/// Filtros de la barra superior. `params` es el valor que espera la API para
/// ese filtro (`EgIQAQ%3D%3D` = solo vídeos, etc.).
pub const FILTERS: &[(&str, Option<&str>)] = &[
    ("Todo", None),
    ("Vídeos", Some("EgIQAQ%3D%3D")),
    ("Canales", Some("EgIQAg%3D%3D")),
    ("Listas", Some("EgIQAw%3D%3D")),
    ("Directos", Some("EgJAAQ%3D%3D")),
];

pub fn search(
    ui: &mut egui::Ui,
    query: &str,
    feed: &Feed,
    load: &Load,
    active_filter: usize,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    ui.horizontal(|ui| {
        for (index, (label, params)) in FILTERS.iter().enumerate() {
            if chip(ui, label, index == active_filter).clicked() {
                intent = Some(Intent::SearchFilter {
                    params: params.map(str::to_string),
                });
            }
        }
    });
    ui.add_space(10.0);

    if query.trim().is_empty() {
        widgets::empty_panel(ui, "Busca en YouTube", "Escribe algo y pulsa Intro.");
        return intent;
    }

    match load {
        Load::Failed(message) => {
            if widgets::error_panel(ui, message, "Reintentar") {
                intent = Some(Intent::Reload);
            }
        }
        Load::Loading | Load::Idle if feed.shelves.is_empty() => {
            for _ in 0..6 {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 130.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 8, p.skeleton);
            }
        }
        _ => {
            let mut any = false;
            for shelf in &feed.shelves {
                if shelf.items.is_empty() {
                    continue;
                }
                any = true;
                if let Some(title) = &shelf.title {
                    ui.label(egui::RichText::new(title).size(16.0).strong().color(p.text));
                }
                if let Some(action) = card_list(ui, &shelf.items, &mut *lookup) {
                    intent = Some(intent_from(action));
                }
                ui.add_space(12.0);
            }
            if !any {
                widgets::empty_panel(
                    ui,
                    "No hay resultados",
                    "Prueba con otras palabras o quita el filtro.",
                );
            }
            if load.is_loading() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Cargando más resultados…")
                            .size(13.0)
                            .color(p.text2),
                    );
                });
            }
        }
    }
    intent
}
