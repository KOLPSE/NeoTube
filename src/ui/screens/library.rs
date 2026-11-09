//! Biblioteca: lo que el usuario ha guardado en su cuenta.
//!
//! Se compone de tres peticiones distintas (listas, álbumes, artistas) porque
//! YouTube no las devuelve juntas; el despachador ya las une en un solo `Feed`,
//! así que aquí solo se pinta el resultado.

use crate::domain::Feed;
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::screens::home::intent_from;
use crate::ui::theme::palette;
use crate::ui::widgets::{self, card_grid, card_list, ThumbLookup};

pub fn library(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    render(ui, "Tu biblioteca", feed, load, lookup, true)
}

pub fn history(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    render(ui, "Historial", feed, load, lookup, false)
}

fn render(
    ui: &mut egui::Ui,
    title: &str,
    feed: &Feed,
    load: &Load,
    lookup: ThumbLookup<'_>,
    as_carousel: bool,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    if let Load::Failed(message) = load {
        if feed.shelves.is_empty() {
            widgets::error_panel(ui, message, "Reintentar");
            return Some(Intent::Reload);
        }
    }

    egui::ScrollArea::vertical()
        .id_salt(title)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            widgets::section_header(ui, title, None);

            if feed.shelves.is_empty() {
                match load {
                    Load::Loading | Load::Idle => widgets::skeleton_grid(ui, 6),
                    Load::Failed(message) => {
                        if widgets::error_panel(ui, message, "Reintentar") {
                            intent = Some(Intent::Reload);
                        }
                    }
                    Load::Ready => widgets::empty_panel(
                        ui,
                        "Aquí todavía no hay nada",
                        "Las listas, álbumes y artistas que guardes aparecerán en esta pantalla.",
                    ),
                }
                return;
            }

            for shelf in &feed.shelves {
                if shelf.items.is_empty() {
                    continue;
                }
                if let Some(name) = &shelf.title {
                    ui.label(egui::RichText::new(name).size(20.0).strong().color(p.text));
                    ui.add_space(8.0);
                }
                let action = if as_carousel {
                    egui::ScrollArea::horizontal()
                        .id_salt(format!("lib:{}", shelf.title.clone().unwrap_or_default()))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let mut local = None;
                                for card in &shelf.items {
                                    let a = ui
                                        .allocate_ui_with_layout(
                                            egui::vec2(180.0, 0.0),
                                            egui::Layout::top_down(egui::Align::LEFT),
                                            |ui| widgets::card_tile(ui, card, 180.0, &mut *lookup),
                                        )
                                        .inner;
                                    if a.is_some() {
                                        local = a;
                                    }
                                }
                                local
                            })
                            .inner
                        })
                        .inner
                } else {
                    card_list(ui, &shelf.items, &mut *lookup)
                };
                if let Some(action) = action {
                    intent = Some(intent_from(action));
                }
                ui.add_space(18.0);
            }

            // Los estantes largos se pintan también en rejilla al final, para
            // no obligar a desplazarse en horizontal por todo.
            if as_carousel && feed.shelves.iter().map(|s| s.items.len()).sum::<usize>() > 12 {
                let all: Vec<_> = feed.shelves.iter().flat_map(|s| s.items.clone()).collect();
                if let Some(action) = card_grid(ui, &all, &mut *lookup) {
                    intent = Some(intent_from(action));
                }
            }
        });
    intent
}
