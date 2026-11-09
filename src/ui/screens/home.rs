//! Inicio: fila de chips, rejilla de recomendaciones y estante de Shorts.
//!
//! La referencia (youtube.com de escritorio) pinta **una** rejilla continua bajo
//! los chips y la sección de Shorts aparece intercalada tras la primera fila, no
//! al final. Por eso el contenido de los estantes se pinta aquí y no con un
//! widget por estante: la fila que corta la rejilla pertenece al mismo flujo.

use egui::RichText;

use crate::domain::{Card, Feed};
use crate::ui::actions::Load;
use crate::ui::app::Intent;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, card_grid, chip, CardAction, ThumbLookup};

/// Chips de la fila superior. «Todo» es el inicio sin filtrar; el resto pide una
/// consulta con ese tema, que es lo que hace la web al pulsarlos.
const CHIPS: &[&str] = &[
    "Todo",
    "Vídeos",
    "Música",
    "Directos",
    "Videojuegos",
    "Noticias",
    "Podcasts",
    "Ciencia",
    "Cocina",
    "Programación",
    "Viajes",
];

/// A qué distancia del fondo se pide la página siguiente. Se pide antes de tocar
/// el fondo para que la rejilla no llegue a quedarse corta mientras carga.
const LOAD_MORE_MARGIN: f32 = 320.0;

/// Claves del estado local de la pantalla en `Ui::data`. Van en un módulo para
/// que no se repitan literales y no choquen con las de otras pantallas.
mod ids {
    /// Salto horizontal pendiente de la fila de chips (lo pide una flecha).
    pub const CHIPS_JUMP: &str = "home:chips_jump";
    /// Token de continuación ya pedido, para no pedir la misma página dos veces.
    pub const MORE_REQUESTED: &str = "home:more_requested";
}

pub fn home(
    ui: &mut egui::Ui,
    feed: &Feed,
    load: &Load,
    active_chip: &str,
    shorts: &[Card],
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = chips_row(ui, active_chip);
    ui.add_space(10.0);

    // El estado de carga solo manda cuando no hay datos: volver a poner
    // esqueletos al paginar haría parpadear la rejilla que ya se está leyendo.
    let empty = !feed.shelves.iter().any(|s| !s.items.is_empty());

    if let Load::Failed(message) = load {
        if empty {
            if widgets::error_state(ui, message) {
                intent = Some(Intent::Reload);
            }
            return intent;
        }
    }

    if empty {
        match load {
            Load::Loading | Load::Idle => widgets::skeleton_grid(ui, 12),
            Load::Failed(_) => {}
            Load::Ready => widgets::empty_state(
                ui,
                "No hay nada que mostrar",
                "Prueba con otro chip o inicia sesión: el inicio anónimo llega casi vacío.",
            ),
        }
        return intent;
    }

    let output = egui::ScrollArea::vertical()
        .id_salt("home_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            draw_feed(ui, feed, shorts, &mut *lookup, &mut intent);
            if feed.continuation.is_some() && load.is_loading() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("Cargando más…")
                            .size(font::META)
                            .color(palette(ui.ctx()).text2),
                    );
                });
            }
            // Aire al final: sin él la última fila queda pegada al borde al
            // desplazarse hasta abajo.
            ui.add_space(24.0);
        });

    // --- paginación al llegar al fondo
    //
    // Se pide una sola vez por token: el token cambia cuando llega la página
    // nueva, así que la bandera guardada en `Ui::data` no hay que reiniciarla.
    // Mientras la petición anterior sigue en curso no se pide otra, o el
    // despachador encadenaría la misma continuación en cada fotograma.
    let offset = output.state.clamped_offset();
    let near_bottom =
        offset.y + output.inner_rect.height() >= output.content_size.y - LOAD_MORE_MARGIN;
    if near_bottom && !load.is_loading() {
        if let Some(token) = feed.continuation.clone() {
            let requested: Option<String> = ui.data(|d| {
                d.get_temp::<Option<String>>(egui::Id::unique(ids::MORE_REQUESTED))
                    .flatten()
            });
            if requested.as_deref() != Some(token.as_str()) {
                ui.data_mut(|d| {
                    d.insert_temp(egui::Id::unique(ids::MORE_REQUESTED), Some(token));
                });
                intent = Some(Intent::LoadMore);
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
        CardAction::Menu(card) => Intent::CardMenu(card),
    }
}

/// Fila de chips con desplazamiento horizontal.
fn chips_row(ui: &mut egui::Ui, active: &str) -> Option<Intent> {
    let p = palette(ui.ctx());

    // La rueda sobre esta fila desplaza en horizontal. egui solo hace eso con
    // Mayús pulsada (`always_scroll_the_only_direction` está desactivado en el
    // tema), así que el delta vertical se convierte aquí antes de que lo lea el
    // `ScrollArea`.
    let row_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        egui::Vec2::new(ui.available_width(), metrics::CHIP_HEIGHT),
    );
    if ui.rect_contains_pointer(row_rect) {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        if wheel != 0.0 {
            ui.input_mut(|i| {
                i.smooth_scroll_delta.y = 0.0;
                i.smooth_scroll_delta.x += wheel;
            });
        }
    }

    // Salto pedido por una flecha en el fotograma anterior: se aplica como
    // desplazamiento inicial y se consume, para no anular lo que arrastre la
    // rueda.
    let jump_id = egui::Id::unique(ids::CHIPS_JUMP);
    let jump: Option<f32> = ui.data(|d| d.get_temp(jump_id));
    if jump.is_some() {
        ui.data_mut(|d| d.remove_temp::<f32>(jump_id));
    }

    let mut area = egui::ScrollArea::horizontal()
        .id_salt("home_chips")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(x) = jump {
        area = area.horizontal_scroll_offset(x.max(0.0));
    }

    let mut intent = None;
    let output = area.show(ui, |ui| {
        ui.horizontal(|ui| {
            for label in CHIPS {
                if chip(ui, label, *label == active).clicked() {
                    intent = Some(Intent::HomeChip((*label).to_string()));
                }
            }
        });
    });

    // Flechas solo si la fila desborda: si no, serían controles decorativos.
    let overflow = output.content_size.x - output.inner_rect.width();
    if overflow > 1.0 {
        let offset = output.state.clamped_offset().x;
        let page = output.inner_rect.width() * 0.8;
        let mut jump_to = None;
        for (dir, icon) in [(-1.0_f32, Icon::ChevronLeft), (1.0, Icon::ChevronRight)] {
            let center = egui::pos2(
                if dir < 0.0 {
                    output.inner_rect.left() + 16.0
                } else {
                    output.inner_rect.right() - 16.0
                },
                output.inner_rect.center().y,
            );
            // En los extremos la flecha no lleva a ninguna parte: se atenúa y no
            // responde, como en la web.
            let usable = if dir < 0.0 {
                offset > 0.5
            } else {
                offset < overflow - 0.5
            };
            let hit = ui.interact(
                egui::Rect::from_center_size(center, egui::Vec2::splat(32.0)),
                egui::Id::unique(("home_chips_arrow", dir.to_bits())),
                egui::Sense::click(),
            );
            // Fondo del color de la página: si no, el chip que asoma por debajo
            // se lee encima de la flecha.
            ui.painter().circle_filled(center, 16.0, p.bg);
            let color = if !usable {
                p.text3
            } else if hit.hovered() {
                p.text
            } else {
                p.text2
            };
            icons::draw(
                ui,
                egui::Rect::from_center_size(center, egui::Vec2::splat(18.0)),
                icon,
                color,
            );
            if usable && hit.clicked() {
                jump_to = Some((offset + dir * page).clamp(0.0, overflow));
            }
        }
        if let Some(x) = jump_to {
            ui.data_mut(|d| d.insert_temp(jump_id, x));
        }
    }

    intent
}

/// Pinta el contenido ya cargado: la rejilla con el estante de Shorts
/// intercalado tras la primera fila.
fn draw_feed(
    ui: &mut egui::Ui,
    feed: &Feed,
    shorts: &[Card],
    lookup: ThumbLookup<'_>,
    intent: &mut Option<Intent>,
) {
    let p = palette(ui.ctx());
    // Columnas reales del ancho disponible: a 1920 con la barra lateral abierta
    // salen cuatro, que es lo que muestra la referencia.
    let (cols, _) = widgets::grid_geometry(ui.available_width());
    let mut shorts_placed = shorts.is_empty();

    for (index, shelf) in feed.shelves.iter().enumerate() {
        if shelf.items.is_empty() {
            continue;
        }
        // El primer estante no lleva título: su encabezado es la propia página
        // de Inicio. Los demás («Continuar viendo»…) sí, como en la web.
        if index > 0 {
            if let Some(title) = &shelf.title {
                ui.label(
                    RichText::new(title)
                        .size(font::SECTION_TITLE)
                        .strong()
                        .color(p.text),
                );
                ui.add_space(10.0);
            }
        }

        if !shorts_placed {
            // La primera fila (las `cols` tarjetas que caben) va encima de
            // Shorts y el resto del estante sigue debajo: así la sección queda
            // intercalada, que es lo que hace la referencia.
            let cut = cols.min(shelf.items.len());
            let (head, tail) = shelf.items.split_at(cut);
            if let Some(action) = card_grid(ui, head, &mut *lookup) {
                *intent = Some(intent_from(action));
            }
            ui.add_space(metrics::GRID_GAP_Y);
            if let Some(action) = shorts_shelf(ui, shorts, &mut *lookup) {
                *intent = Some(intent_from(action));
            }
            ui.add_space(metrics::GRID_GAP_Y);
            shorts_placed = true;
            if !tail.is_empty() {
                if let Some(action) = card_grid(ui, tail, &mut *lookup) {
                    *intent = Some(intent_from(action));
                }
            }
        } else if let Some(action) = card_grid(ui, &shelf.items, &mut *lookup) {
            *intent = Some(intent_from(action));
        }
        ui.add_space(metrics::GRID_GAP_Y);
    }
}

/// Sección «Shorts»: icono rojo, título y fila de tarjetas 9:16. Cuatro caben
/// completas y la quinta se corta; ese recorte es lo que invita a desplazar la
/// fila, igual que en la referencia.
fn shorts_shelf(ui: &mut egui::Ui, shorts: &[Card], lookup: ThumbLookup<'_>) -> Option<CardAction> {
    let p = palette(ui.ctx());
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(egui::Vec2::splat(22.0), egui::Sense::hover());
        icons::draw(ui, icon_rect, Icon::ShortsFilled, p.brand);
        ui.add_space(8.0);
        ui.add(egui::Label::new(
            RichText::new("Shorts")
                .size(font::SECTION_TITLE)
                .strong()
                .color(p.text),
        ));
    });
    ui.add_space(12.0);

    let gap = metrics::GRID_GAP;
    // Cuatro visibles más el canto de la quinta: el ancho se calcula como si
    // cupieran 4.35 tarjetas, para que el corte caiga donde cae en la
    // referencia y quede claro que la fila sigue.
    let available = ui.available_width();
    let width = (available - gap * 3.0) / 4.35;

    let mut action = None;
    egui::ScrollArea::horizontal()
        .id_salt("home_shorts_row")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for card in shorts {
                    let found = ui
                        .allocate_ui_with_layout(
                            egui::Vec2::new(width, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| widgets::shorts_card(ui, card, width, &mut *lookup),
                        )
                        .inner;
                    if found.is_some() {
                        action = found;
                    }
                    ui.allocate_exact_size(egui::Vec2::new(gap, 1.0), egui::Sense::hover());
                }
            });
        });
    action
}
