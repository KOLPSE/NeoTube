//! Inicio: cabecera de chips y rejilla de recomendaciones.
//!
//! Comportamiento de la referencia: los chips filtran **sin** recargar la
//! página (se pinta una consulta de búsqueda con ese filtro), la rejilla es
//! adaptable y cada tarjeta se ve como en la web.

use crate::domain::{Card, Feed};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::widgets::{self, card_grid, chip, CardAction, ThumbLookup};

/// Chips de la fila superior. Los cuatro primeros son fijos en la web; el resto
/// son temas que se pueden pedir directamente.
const CHIPS: &[&str] = &[
    "Todo", "Música", "Directos", "Videojuegos", "Noticias", "Podcasts", "Ciencia",
    "Cocina", "Programación", "Viajes",
];

pub fn home(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    active_chip: &str,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = crate::ui::theme::palette(ui.ctx());
    let mut intent = None;

    // Fila de chips con desplazamiento horizontal, como la web.
    egui::ScrollArea::horizontal()
        .id_salt("home_chips")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for label in CHIPS {
                    if chip(ui, label, *label == active_chip).clicked() {
                        intent = Some(Intent::HomeChip((*label).to_string()));
                    }
                }
            });
        });
    ui.add_space(8.0);

    match load {
        Load::Failed(message) => {
            if widgets::error_panel(ui, message, "Reintentar") {
                intent = Some(Intent::Reload);
            }
        }
        Load::Loading | Load::Idle if feed.shelves.is_empty() => {
            widgets::skeleton_grid(ui, 12);
        }
        _ => {
            if feed.shelves.is_empty() {
                widgets::empty_panel(ui, "No hay nada que mostrar", "Prueba a recargar o a buscar.");
                return intent;
            }
            // El primer estante (destacados) va en rejilla; los demás, en filas
            // horizontales como los carruseles de la web.
            for (index, shelf) in feed.shelves.iter().enumerate() {
                let mut cards: Vec<Card> = Vec::new();
                for card in &shelf.items {
                    cards.push(card.clone());
                }
                if shelf.items.len() >= 6 || index == 0 {
                    if let Some(title) = &shelf.title {
                        ui.label(egui::RichText::new(title).size(18.0).strong().color(p.text));
                        ui.add_space(6.0);
                    }
                    if let Some(action) = card_grid(ui, &cards, &mut *lookup) {
                        intent = Some(intent_from(action));
                    }
                } else {
                    if let Some(title) = &shelf.title {
                        ui.label(egui::RichText::new(title).size(18.0).strong().color(p.text));
                    }
                    if let Some(action) = widgets::card_list(ui, &cards, &mut *lookup) {
                        intent = Some(intent_from(action));
                    }
                }
                ui.add_space(20.0);
            }
            if feed.continuation.is_some() && load.is_loading() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(egui::RichText::new("Cargando más…").size(13.0).color(p.text_secondary));
                });
            }
        }
    }
    intent
}

/// Traduce una acción de tarjeta a una intención de la aplicación.
pub fn intent_from(action: CardAction) -> Intent {
    match action {
        CardAction::Open(card) => Intent::Open(card),
        CardAction::PlayNow(card) => Intent::PlayNow(card),
        CardAction::OpenAuthor(id) => match id {
            Some(id) => Intent::Open(crate::domain::Card::new(
                id,
                crate::domain::MediaKind::Channel,
                String::new(),
            )),
            None => Intent::None,
        },
        CardAction::Enqueue(card) => Intent::Enqueue(card),
    }
}
