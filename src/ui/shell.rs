//! Estructura global: cabecera, barra lateral, notificaciones y menú de perfil.
//!
//! Es la parte que más se compara con la referencia (capturas 02, 03, 04 y 07),
//! así que las medidas son literalmente las de youtube.com: 56 px de cabecera,
//! 240 de barra lateral desplegada, 72 de compacta, buscador de 40 px con radio
//! de píldora y botón integrado, chips de 32 con radio de 8.
//!
//! Separar esto de [`crate::ui::app`] tiene una razón práctica: la cabecera y la
//! barra lateral se pintan en **todas** las pantallas, así que un cambio de
//! densidad se nota en todas. Tenerlas en un fichero propio hace que ese cambio
//! se pueda revisar contra una sola captura.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Vec2};

use crate::domain::{Account, Route};
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, ThumbLookup};

/// Qué ha pedido el usuario en la cabecera o en la barra lateral.
#[derive(Debug, Clone)]
pub enum ShellIntent {
    None,
    Navigate(Route),
    SubmitSearch(String),
    ToggleSidebar,
    ShowNotifications,
    ShowProfileMenu,
    Login,
    Logout,
    OpenSettings,
    /// Escribir en el buscador (para que la app guarde el texto).
    SearchChanged(String),
    /// «Mostrar más» de las suscripciones.
    ShowMoreSubscriptions,
    /// «Mostrar más» de los destinos de la barra lateral.
    ShowMoreDestinations,
    /// Abrir un destino externo (Studio, Premium, YouTube Music web…).
    OpenExternal(String),
}

/// Entrada que necesita la cabecera para pintarse.
pub struct HeaderState<'a> {
    pub search_text: &'a mut String,
    pub account: Option<&'a Account>,
    pub unread: usize,
    pub sidebar_open: bool,
    /// El buscador tiene el foco (para que Escape lo suelte).
    pub search_focused: &'a mut bool,
}

/// Cabecera de 56 px.
pub fn header(ui: &mut egui::Ui, state: &mut HeaderState<'_>) -> ShellIntent {
    let p = palette(ui.ctx());
    let mut intent = ShellIntent::None;
    let full = ui.available_width();

    ui.horizontal_centered(|ui| {
        ui.add_space(4.0);
        if icons::icon_button(ui, Icon::Menu, 24.0, false, "Guía").clicked() {
            intent = ShellIntent::ToggleSidebar;
        }
        ui.add_space(8.0);

        // --- marca
        let (logo_rect, logo_resp) = ui.allocate_exact_size(Vec2::new(96.0, 20.0), Sense::click());
        // Marca: cuadrado rojo con el triángulo y el nombre del cliente. El
        // nombre cambia (NeoTube, no YouTube), la forma no: es lo que hace que
        // se reconozca la aplicación de un vistazo.
        let mark = Rect::from_min_size(
            egui::pos2(logo_rect.left(), logo_rect.center().y - 10.0),
            Vec2::new(28.0, 20.0),
        );
        ui.painter()
            .rect_filled(mark, CornerRadius::same(6), p.brand);
        let mut tri = egui::epaint::PathShape::convex_polygon(
            vec![
                mark.center() + Vec2::new(4.0, 0.0),
                mark.center() + Vec2::new(-2.0, -4.5),
                mark.center() + Vec2::new(-2.0, 4.5),
            ],
            Color32::WHITE,
            egui::Stroke::NONE,
        );
        tri.fill = Color32::WHITE;
        ui.painter().add(tri);
        ui.painter().text(
            egui::pos2(mark.right() + 6.0, mark.center().y),
            Align2::LEFT_CENTER,
            "NeoTube",
            FontId::proportional(18.0),
            p.text,
        );
        if logo_resp.clicked() {
            intent = ShellIntent::Navigate(Route::Home);
        }

        // --- buscador centrado
        // La web lo centra en la ventana, no en el espacio que queda: se calcula
        // su posición para que no se mueva al cambiar el contenido de la
        // izquierda o de la derecha.
        let search_w = (full * 0.36).clamp(240.0, metrics::SEARCH_MAX);
        let target_left = (full - search_w) * 0.5;
        let current = ui.cursor().left();
        if current < target_left {
            ui.add_space(target_left - current);
        }

        let (search_rect, _) =
            ui.allocate_exact_size(Vec2::new(search_w, metrics::SEARCH_HEIGHT), Sense::hover());
        // Cuerpo del buscador: fondo oscuro, borde de 1 px y radio de píldora.
        ui.painter().rect_filled(
            Rect::from_min_max(
                search_rect.left_top(),
                egui::pos2(search_rect.right() - 64.0, search_rect.bottom()),
            ),
            CornerRadius {
                nw: metrics::RADIUS_PILL,
                sw: metrics::RADIUS_PILL,
                ne: 0,
                se: 0,
            },
            p.bg,
        );
        ui.painter().rect_stroke(
            Rect::from_min_max(
                search_rect.left_top(),
                egui::pos2(search_rect.right() - 64.0, search_rect.bottom()),
            ),
            CornerRadius {
                nw: metrics::RADIUS_PILL,
                sw: metrics::RADIUS_PILL,
                ne: 0,
                se: 0,
            },
            egui::Stroke::new(1.0, p.divider),
            egui::StrokeKind::Inside,
        );
        let text_rect = Rect::from_min_max(
            search_rect.left_top() + Vec2::new(16.0, 0.0),
            egui::pos2(search_rect.right() - 72.0, search_rect.bottom()),
        );
        let edit = ui.put(
            text_rect,
            egui::TextEdit::singleline(state.search_text)
                .frame(egui::Frame::NONE)
                .hint_text(RichText::new("Buscar").size(font::BODY).color(p.text3))
                .margin(Vec2::new(0.0, 10.0)),
        );
        *state.search_focused = edit.has_focus();
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            intent = ShellIntent::SubmitSearch(state.search_text.clone());
        }
        if edit.changed() {
            intent = ShellIntent::SearchChanged(state.search_text.clone());
        }

        // Botón de búsqueda integrado en el extremo derecho de la píldora.
        let btn_rect = Rect::from_min_max(
            egui::pos2(search_rect.right() - 64.0, search_rect.top()),
            search_rect.right_bottom(),
        );
        ui.painter().rect_filled(
            btn_rect,
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: metrics::RADIUS_PILL,
                se: metrics::RADIUS_PILL,
            },
            p.chip,
        );
        let btn_resp = ui.allocate_rect(btn_rect, Sense::click());
        if btn_resp.hovered() {
            ui.painter().rect_filled(
                btn_rect,
                CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: metrics::RADIUS_PILL,
                    se: metrics::RADIUS_PILL,
                },
                p.hover,
            );
        }
        icons::draw(
            ui,
            Rect::from_center_size(btn_rect.center(), Vec2::splat(24.0)),
            Icon::Search,
            p.text,
        );
        if btn_resp.clicked() {
            intent = ShellIntent::SubmitSearch(state.search_text.clone());
        }
        if btn_resp.on_hover_text("Buscar").clicked() {
            intent = ShellIntent::SubmitSearch(state.search_text.clone());
        }

        // Micrófono, a la derecha del buscador.
        ui.add_space(8.0);
        if icons::icon_button(ui, Icon::Microphone, 20.0, false, "Buscar con voz").clicked() {
            // La búsqueda por voz usa el micrófono del sistema y el servicio de
            // Google; hoy no está integrada. Se avisa en lugar de fingir.
            intent = ShellIntent::OpenExternal(String::new());
        }

        // --- acciones de la derecha
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            // Avatar (con sesión) o botón de entrar (sin sesión).
            match state.account {
                Some(account) => {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
                    let mut lookup =
                        |_: &crate::domain::Thumbs, _: u32| -> Option<egui::TextureId> { None };
                    let _ = &mut lookup;
                    widgets::avatar_image(ui, rect, &account.avatar, &mut lookup);
                    if resp.clicked() {
                        intent = ShellIntent::ShowProfileMenu;
                    }
                }
                None => {
                    let resp = widgets::pill(
                        ui,
                        "Iniciar sesión",
                        Some(Icon::Account),
                        widgets::PillKind::Outline,
                    );
                    if resp.clicked() {
                        intent = ShellIntent::Login;
                    }
                    if icons::icon_button(ui, Icon::Account, 22.0, false, "Cuenta").clicked() {
                        intent = ShellIntent::Login;
                    }
                }
            }

            // Campana de notificaciones con globo de no leídas.
            ui.add_space(4.0);
            let (bell_rect, bell_resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
            if bell_resp.hovered() {
                ui.painter().circle_filled(bell_rect.center(), 20.0, p.chip);
            }
            icons::draw(
                ui,
                Rect::from_center_size(bell_rect.center(), Vec2::splat(24.0)),
                if state.unread > 0 {
                    Icon::BellFilled
                } else {
                    Icon::Bell
                },
                p.text,
            );
            if state.unread > 0 {
                let label = if state.unread > 9 {
                    "9+".to_string()
                } else {
                    state.unread.to_string()
                };
                let badge = Rect::from_center_size(
                    bell_rect.center() + Vec2::new(8.0, -9.0),
                    Vec2::new(16.0, 16.0),
                );
                ui.painter().circle_filled(badge.center(), 8.0, p.badge);
                ui.painter().text(
                    badge.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }
            if bell_resp.clicked() {
                intent = ShellIntent::ShowNotifications;
            }

            // Botón Crear.
            ui.add_space(4.0);
            let create = widgets::pill(ui, "Crear", Some(Icon::Create), widgets::PillKind::Filled);
            if create.clicked() {
                // Crear vídeo o emitir exige subir contenido a YouTube con una
                // API que no está documentada para clientes de terceros. Se
                // abre la web en lugar de simularlo.
                intent = ShellIntent::OpenExternal("https://studio.youtube.com".into());
            }
        });
    });

    intent
}

/// Entrada de la barra lateral: icono, etiqueta y destino.
struct NavItem {
    icon: Icon,
    filled: Icon,
    label: &'static str,
    route: Route,
    /// Punto azul de "con contenido nuevo" (directos).
    dot: bool,
}

fn primary_items() -> Vec<NavItem> {
    vec![
        NavItem {
            icon: Icon::Home,
            filled: Icon::HomeFilled,
            label: "Inicio",
            route: Route::Home,
            dot: false,
        },
        NavItem {
            icon: Icon::Shorts,
            filled: Icon::ShortsFilled,
            label: "Shorts",
            route: Route::Shorts { start_id: None },
            dot: false,
        },
        NavItem {
            icon: Icon::Subscriptions,
            filled: Icon::Subscriptions,
            label: "Suscripciones",
            route: Route::Subscriptions,
            dot: false,
        },
    ]
}

fn you_items() -> Vec<NavItem> {
    vec![
        NavItem {
            icon: Icon::Account,
            filled: Icon::Account,
            label: "Tu canal",
            route: Route::Channel {
                browse_id: "me".into(),
            },
            dot: false,
        },
        NavItem {
            icon: Icon::History,
            filled: Icon::History,
            label: "Historial",
            route: Route::History,
            dot: false,
        },
        NavItem {
            icon: Icon::Playlist,
            filled: Icon::Playlist,
            label: "Listas de reproducción",
            route: Route::Library,
            dot: false,
        },
        NavItem {
            icon: Icon::WatchLater,
            filled: Icon::WatchLater,
            label: "Ver más tarde",
            route: Route::Playlist {
                browse_id: "WL".into(),
            },
            dot: false,
        },
        NavItem {
            icon: Icon::ThumbsUp,
            filled: Icon::ThumbsUpFilled,
            label: "Vídeos que me gustan",
            route: Route::Playlist {
                browse_id: "LL".into(),
            },
            dot: false,
        },
        NavItem {
            icon: Icon::Videos,
            filled: Icon::Videos,
            label: "Mis vídeos",
            route: Route::Channel {
                browse_id: "me-videos".into(),
            },
            dot: false,
        },
        NavItem {
            icon: Icon::Downloads,
            filled: Icon::Downloads,
            label: "Descargas",
            route: Route::Library,
            dot: false,
        },
    ]
}

fn explore_items() -> Vec<NavItem> {
    vec![
        NavItem {
            icon: Icon::Music,
            filled: Icon::Music,
            label: "Música",
            route: Route::MusicHome,
            dot: false,
        },
        NavItem {
            icon: Icon::Films,
            filled: Icon::Films,
            label: "Películas",
            route: Route::Search {
                query: "películas".into(),
                chip: None,
            },
            dot: false,
        },
        NavItem {
            icon: Icon::Live,
            filled: Icon::Live,
            label: "En directo",
            route: Route::Search {
                query: "en directo".into(),
                chip: None,
            },
            dot: false,
        },
    ]
}

/// Estado de la barra lateral que sobrevive entre fotogramas.
pub struct SidebarState {
    pub show_all_subscriptions: bool,
    pub show_all_explore: bool,
}

/// Entrada de la barra lateral.
pub struct SidebarInput<'a> {
    pub route: &'a Route,
    pub subscriptions: &'a [(String, crate::domain::Thumbs, bool)],
    pub state: &'a mut SidebarState,
    pub logged_in: bool,
    pub account_name: &'a str,
}

/// Barra lateral desplegada (240 px).
pub fn sidebar(
    ui: &mut egui::Ui,
    input: &mut SidebarInput<'_>,
    lookup: ThumbLookup<'_>,
) -> ShellIntent {
    let p = palette(ui.ctx());
    let mut intent = ShellIntent::None;
    // El scroll de la barra es independiente del contenido principal: la web
    // mueve solo la lista, no la página.
    egui::ScrollArea::vertical()
        .id_salt("sidebar_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(4.0);

            // --- destinos principales
            for item in primary_items() {
                let selected = same_route(input.route, &item.route);
                if widgets::sidebar_row(
                    ui,
                    Some(if selected { item.filled } else { item.icon }),
                    item.label,
                    selected,
                )
                .clicked()
                {
                    intent = ShellIntent::Navigate(item.route.clone());
                }
            }

            ui.add_space(8.0);
            widgets::divider(ui);
            ui.add_space(4.0);

            // --- suscripciones
            if input.logged_in {
                widgets::sidebar_group(ui, "Suscripciones", true);
                let limit = if input.state.show_all_subscriptions {
                    20
                } else {
                    7
                };
                let count = input.subscriptions.len();
                for (name, avatar, live) in input.subscriptions.iter().take(limit) {
                    let route = Route::Channel {
                        browse_id: format!("sub:{name}"),
                    };
                    let selected = same_route(input.route, &route);
                    if widgets::sidebar_channel_row(ui, name, avatar, *live, &mut *lookup).clicked()
                    {
                        intent = ShellIntent::Navigate(route);
                    }
                    let _ = selected;
                }
                if count > 7 {
                    let label = if input.state.show_all_subscriptions {
                        "Mostrar menos".to_string()
                    } else {
                        "Mostrar más".to_string()
                    };
                    if widgets::sidebar_row(ui, Some(Icon::ChevronDown), &label, false).clicked() {
                        input.state.show_all_subscriptions = !input.state.show_all_subscriptions;
                        intent = ShellIntent::ShowMoreSubscriptions;
                    }
                }
                ui.add_space(8.0);
                widgets::divider(ui);
                ui.add_space(4.0);
            }

            // --- Tú
            widgets::sidebar_group(ui, "Tú", true);
            for item in you_items() {
                let selected = same_route(input.route, &item.route);
                if widgets::sidebar_row(
                    ui,
                    Some(if selected { item.filled } else { item.icon }),
                    item.label,
                    selected,
                )
                .clicked()
                {
                    intent = ShellIntent::Navigate(item.route.clone());
                }
            }

            ui.add_space(8.0);
            widgets::divider(ui);
            ui.add_space(4.0);

            // --- Más de YouTube
            widgets::sidebar_group(ui, "Más de YouTube", false);
            for (label, url) in [
                ("YouTube Premium", "https://www.youtube.com/premium"),
                ("YouTube Music", "https://music.youtube.com"),
                ("YouTube Kids", "https://www.youtubekids.com"),
                ("YouTube Labs", "https://labs.youtube.com"),
            ] {
                if external_row(ui, label).clicked() {
                    intent = ShellIntent::OpenExternal(url.to_string());
                }
            }

            ui.add_space(8.0);
            widgets::divider(ui);
            ui.add_space(4.0);

            // --- Explorar
            widgets::sidebar_group(ui, "Explorar", false);
            let explore = explore_items();
            let limit = if input.state.show_all_explore {
                explore.len()
            } else {
                3
            };
            for item in explore.into_iter().take(limit) {
                let selected = same_route(input.route, &item.route);
                if widgets::sidebar_row(ui, Some(item.icon), item.label, selected).clicked() {
                    intent = ShellIntent::Navigate(item.route.clone());
                }
            }
            if widgets::sidebar_row(ui, Some(Icon::ChevronDown), "Mostrar más", false).clicked() {
                input.state.show_all_explore = !input.state.show_all_explore;
                intent = ShellIntent::ShowMoreDestinations;
            }

            ui.add_space(8.0);
            widgets::divider(ui);
            ui.add_space(4.0);

            if widgets::sidebar_row(ui, Some(Icon::Flag), "Historial de denuncias", false).clicked()
            {
                intent = ShellIntent::OpenExternal("https://www.youtube.com/report_history".into());
            }

            ui.add_space(8.0);
            widgets::divider(ui);
            ui.add_space(12.0);

            // --- pie con enlaces, como la captura 03
            let footer: [&[&str]; 4] = [
                &["Información", "Prensa", "Derechos de autor", "Contactar"],
                &["Creadores", "Publicidad", "Desarrolladores"],
                &["Términos", "Privacidad", "Política y seguridad"],
                &["Cómo funciona YouTube", "Probar funciones nuevas"],
            ];
            for group in footer {
                let mut line = String::new();
                for (i, word) in group.iter().enumerate() {
                    if i > 0 {
                        line.push_str("  ");
                    }
                    line.push_str(word);
                }
                let link_resp = ui.add(
                    egui::Label::new(
                        RichText::new(line)
                            .size(font::FOOTER)
                            .strong()
                            .color(p.text2),
                    )
                    .sense(Sense::click()),
                );
                if link_resp.clicked() {
                    intent = ShellIntent::OpenExternal("https://www.youtube.com/about".into());
                }
                ui.add_space(6.0);
            }
            ui.add_space(10.0);
            ui.add(egui::Label::new(
                RichText::new("© 2026 NeoTube · cliente independiente")
                    .size(font::FOOTER)
                    .color(p.text3),
            ));
            ui.add_space(16.0);
        });
    intent
}

/// Barra lateral compacta (72 px): solo iconos y etiquetas cortas.
pub fn sidebar_rail(ui: &mut egui::Ui, route: &Route) -> ShellIntent {
    let mut intent = ShellIntent::None;
    ui.add_space(4.0);
    let items = [
        (Icon::Home, Icon::HomeFilled, "Inicio", Route::Home),
        (
            Icon::Shorts,
            Icon::ShortsFilled,
            "Shorts",
            Route::Shorts { start_id: None },
        ),
        (
            Icon::Subscriptions,
            Icon::Subscriptions,
            "Suscrip.",
            Route::Channel {
                browse_id: "subscriptions".into(),
            },
        ),
        (Icon::Library, Icon::Library, "Tú", Route::Library),
    ];
    for (icon, filled, label, item_route) in items {
        let selected = same_route(route, &item_route);
        if icons::rail_item(ui, if selected { filled } else { icon }, label, selected).clicked() {
            intent = ShellIntent::Navigate(item_route);
        }
    }
    intent
}

/// Fila de enlace externo: icono rojo de marca y etiqueta.
fn external_row(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), metrics::SIDEBAR_ROW),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(metrics::SIDEBAR_RADIUS), p.hover);
    }
    let icon_rect = Rect::from_center_size(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        Vec2::new(20.0, 14.0),
    );
    ui.painter()
        .rect_filled(icon_rect, CornerRadius::same(4), p.brand);
    let mut tri = egui::epaint::PathShape::convex_polygon(
        vec![
            icon_rect.center() + Vec2::new(2.5, 0.0),
            icon_rect.center() + Vec2::new(-1.5, -3.0),
            icon_rect.center() + Vec2::new(-1.5, 3.0),
        ],
        Color32::WHITE,
        egui::Stroke::NONE,
    );
    tri.fill = Color32::WHITE;
    ui.painter().add(tri);
    ui.painter().text(
        egui::pos2(rect.left() + 60.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(font::SIDEBAR),
        p.text,
    );
    response
}

/// ¿Las dos rutas representan la misma pantalla? La barra lateral marca la
/// sección, no el contenido concreto.
fn same_route(a: &Route, b: &Route) -> bool {
    match (a, b) {
        (Route::Home, Route::Home) => true,
        (Route::Shorts { .. }, Route::Shorts { .. }) => true,
        (Route::MusicHome, Route::MusicHome) => true,
        (Route::Library, Route::Library) => true,
        (Route::History, Route::History) => true,
        (Route::Subscriptions, Route::Subscriptions) => true,
        (Route::Channel { browse_id: x }, Route::Channel { browse_id: y }) => x == y,
        _ => false,
    }
}

/// Entrada del panel de notificaciones.
pub struct NotificationView {
    pub title: String,
    pub text: String,
    pub thumbnail: crate::domain::Thumbs,
    pub avatar: crate::domain::Thumbs,
    pub published: String,
    pub unread: bool,
}

/// Panel de notificaciones anclado a la campana (captura 04).
///
/// Es un `Area` superpuesto: no desplaza la página, se cierra con Escape, con
/// clic fuera o volviendo a pulsar la campana, y su scroll es interno.
pub fn notifications_panel(
    ctx: &egui::Context,
    items: &[NotificationView],
    loading: bool,
    error: Option<&str>,
    logged_in: bool,
    lookup: ThumbLookup<'_>,
) -> (bool, bool) {
    let p = palette(ctx);
    let mut open = true;
    let mut close = false;
    let mut reload = false;
    let width = metrics::NOTIFICATIONS_PANEL;
    // Anclado bajo la campana, alineado a la derecha con 16 px de margen.
    let screen = ctx.content_rect();
    let pos = egui::pos2(screen.right() - width - 24.0, 64.0);

    egui::Area::new(egui::Id::unique("notifications"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(p.menu)
                .corner_radius(CornerRadius::same(metrics::RADIUS_PANEL))
                .stroke(egui::Stroke::new(1.0, p.divider))
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_width(width);
                    // Cabecera del panel.
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Notificaciones")
                                .size(font::MENU_HEADER)
                                .strong()
                                .color(p.text),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            if icons::icon_button(ui, Icon::Settings, 20.0, false, "Ajustes")
                                .clicked()
                            {
                                reload = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    widgets::divider(ui);

                    egui::ScrollArea::vertical()
                        .id_salt("notifications_scroll")
                        .max_height(metrics::NOTIFICATIONS_MAX_H)
                        .show(ui, |ui| {
                            if !logged_in {
                                ui.add_space(24.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("Inicia sesión para ver tus notificaciones")
                                            .size(font::BODY)
                                            .color(p.text2),
                                    );
                                });
                                ui.add_space(24.0);
                            } else if let Some(message) = error {
                                ui.add_space(20.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(message).size(font::BODY).color(p.text2),
                                    );
                                });
                                ui.add_space(20.0);
                            } else if loading && items.is_empty() {
                                for _ in 0..4 {
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui
                                            .allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                                        ui.painter().circle_filled(r.center(), 18.0, p.skeleton);
                                        ui.add_space(12.0);
                                        ui.vertical(|ui| {
                                            for w in [220.0f32, 160.0] {
                                                let (l, _) = ui.allocate_exact_size(
                                                    Vec2::new(w, 12.0),
                                                    Sense::hover(),
                                                );
                                                ui.painter().rect_filled(l, 4, p.skeleton);
                                                ui.add_space(6.0);
                                            }
                                        });
                                    });
                                    ui.add_space(16.0);
                                }
                            } else if items.is_empty() {
                                ui.add_space(24.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No hay notificaciones nuevas")
                                            .size(font::BODY)
                                            .color(p.text2),
                                    );
                                });
                                ui.add_space(24.0);
                            } else {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Importante").size(font::META).color(p.text2),
                                );
                                for item in items {
                                    notification_row(ui, item, &mut *lookup);
                                }
                            }
                        });
                    widgets::divider(ui);
                    let foot = ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click());
                    if foot.1.hovered() {
                        ui.painter().rect_filled(foot.0, 0, p.hover);
                    }
                    ui.painter().text(
                        egui::pos2(foot.0.left() + 16.0, foot.0.center().y),
                        Align2::LEFT_CENTER,
                        "Más notificaciones",
                        FontId::proportional(font::MENU),
                        p.text,
                    );
                    if foot.1.clicked() {
                        reload = true;
                    }
                });
        });

    // Cierre: Escape o clic fuera del panel.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if ctx.input(|i| i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            let panel = Rect::from_min_size(pos, Vec2::new(width, 400.0));
            let _ = panel;
            if pos.y > 64.0 + metrics::NOTIFICATIONS_MAX_H || pos.x < screen.right() - width - 40.0
            {
                // Clic claramente fuera del panel: se cierra.
                if pos.y > 60.0 {
                    close = true;
                }
            }
        }
    }
    let _ = open;
    (close, reload)
}

/// Fila de notificación: avatar, texto, miniatura, antigüedad y menú.
fn notification_row(ui: &mut egui::Ui, item: &NotificationView, lookup: ThumbLookup<'_>) {
    let p = palette(ui.ctx());
    let width = ui.available_width();
    let text_w = width - 36.0 - 12.0 - 96.0 - 24.0 - 32.0;
    let galley = ui.painter().layout(
        item.text.clone(),
        FontId::proportional(font::NOTIFICATION),
        p.text,
        text_w,
    );
    let height = (galley.size().y + 60.0).max(84.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 0, p.hover);
    }
    // Indicador de no leída.
    if item.unread {
        ui.painter().circle_filled(
            egui::pos2(rect.left() + 8.0, rect.top() + 28.0),
            3.0,
            Color32::from_rgb(0x3e, 0xa6, 0xff),
        );
    }
    // Avatar del canal.
    let avatar = Rect::from_min_size(
        egui::pos2(rect.left() + 20.0, rect.top() + 12.0),
        Vec2::splat(36.0),
    );
    widgets::avatar_image(ui, avatar, &item.avatar, &mut *lookup);
    // Texto.
    ui.painter().galley(
        egui::pos2(avatar.right() + 12.0, rect.top() + 12.0),
        galley,
        p.text,
    );
    // Antigüedad.
    ui.painter().text(
        egui::pos2(avatar.right() + 12.0, rect.bottom() - 18.0),
        Align2::LEFT_CENTER,
        &item.published,
        FontId::proportional(font::META),
        p.text2,
    );
    // Miniatura del contenido.
    let thumb = Rect::from_min_size(
        egui::pos2(rect.right() - 96.0 - 32.0, rect.top() + 12.0),
        Vec2::new(96.0, 54.0),
    );
    widgets::thumb_image(ui, thumb, &item.thumbnail, 8, &mut *lookup);
    // Menú de tres puntos.
    icons::draw(
        ui,
        Rect::from_center_size(
            egui::pos2(rect.right() - 16.0, rect.top() + 28.0),
            Vec2::splat(20.0),
        ),
        Icon::More,
        p.text2,
    );
    let _ = &item.title;
}

/// Menú del perfil (captura 07): identidad, cuenta, Studio, datos, aspecto,
/// idioma, restringido, ubicación, atajos y configuración.
pub fn profile_menu(
    ctx: &egui::Context,
    account: Option<&Account>,
    dark: bool,
) -> (bool, ShellIntent) {
    let p = palette(ctx);
    let mut close = false;
    let mut intent = ShellIntent::None;
    let width = 300.0;
    let screen = ctx.content_rect();
    let pos = egui::pos2(screen.right() - width - 16.0, 56.0);

    egui::Area::new(egui::Id::unique("profile_menu"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(p.menu)
                .corner_radius(CornerRadius::same(metrics::RADIUS_PANEL))
                .stroke(egui::Stroke::new(1.0, p.divider))
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.set_width(width);
                    // --- identidad
                    let (head, _) = ui.allocate_exact_size(
                        Vec2::new(width, if account.is_some() { 96.0 } else { 64.0 }),
                        Sense::hover(),
                    );
                    match account {
                        Some(account) => {
                            let avatar = Rect::from_min_size(
                                head.left_top() + Vec2::new(16.0, 16.0),
                                Vec2::splat(40.0),
                            );
                            let mut lookup =
                                |_: &crate::domain::Thumbs, _: u32| -> Option<egui::TextureId> {
                                    None
                                };
                            widgets::avatar_image(ui, avatar, &account.avatar, &mut lookup);
                            ui.painter().text(
                                egui::pos2(avatar.right() + 12.0, avatar.top() + 8.0),
                                Align2::LEFT_TOP,
                                &account.name,
                                FontId::proportional(16.0),
                                p.text,
                            );
                            let handle = account
                                .handle
                                .clone()
                                .unwrap_or_else(|| "@neotube".to_string());
                            ui.painter().text(
                                egui::pos2(avatar.right() + 12.0, avatar.top() + 30.0),
                                Align2::LEFT_TOP,
                                &handle,
                                FontId::proportional(font::META),
                                p.text2,
                            );
                            let link = ui.allocate_rect(
                                Rect::from_min_size(
                                    egui::pos2(avatar.right() + 12.0, avatar.bottom() + 4.0),
                                    Vec2::new(120.0, 20.0),
                                ),
                                Sense::click(),
                            );
                            ui.painter().text(
                                egui::pos2(avatar.right() + 12.0, avatar.bottom() + 14.0),
                                Align2::LEFT_CENTER,
                                "Ver tu canal",
                                FontId::proportional(font::BODY),
                                p.link,
                            );
                            if link.clicked() {
                                intent = ShellIntent::Navigate(Route::Channel {
                                    browse_id: "me".into(),
                                });
                                close = true;
                            }
                        }
                        None => {
                            ui.painter().text(
                                head.left_top() + Vec2::new(16.0, 24.0),
                                Align2::LEFT_TOP,
                                "Sin sesión iniciada",
                                FontId::proportional(font::BODY),
                                p.text2,
                            );
                        }
                    }
                    widgets::divider(ui);

                    if account.is_some() {
                        if widgets::menu_row(ui, Some(Icon::Google), "Cuenta de Google", None)
                            .clicked()
                        {
                            intent =
                                ShellIntent::OpenExternal("https://myaccount.google.com".into());
                            close = true;
                        }
                        if widgets::menu_row(ui, Some(Icon::Account), "Cambiar de cuenta", Some(""))
                            .clicked()
                        {
                            intent = ShellIntent::Login;
                            close = true;
                        }
                        if widgets::menu_row(ui, Some(Icon::ArrowRight), "Cerrar sesión", None)
                            .clicked()
                        {
                            intent = ShellIntent::Logout;
                            close = true;
                        }
                        widgets::divider(ui);
                        if widgets::menu_row(ui, Some(Icon::Studio), "YouTube Studio", None)
                            .clicked()
                        {
                            intent = ShellIntent::OpenExternal("https://studio.youtube.com".into());
                            close = true;
                        }
                        if widgets::menu_row(
                            ui,
                            Some(Icon::Shopping),
                            "Compras y suscripciones",
                            None,
                        )
                        .clicked()
                        {
                            intent = ShellIntent::OpenExternal(
                                "https://www.youtube.com/paid_memberships".into(),
                            );
                            close = true;
                        }
                        widgets::divider(ui);
                        if widgets::menu_row(ui, Some(Icon::Shield), "Tus datos en YouTube", None)
                            .clicked()
                        {
                            intent = ShellIntent::OpenExternal(
                                "https://myactivity.google.com/product/youtube".into(),
                            );
                            close = true;
                        }
                    } else {
                        if widgets::menu_row(ui, Some(Icon::Account), "Iniciar sesión", None)
                            .clicked()
                        {
                            intent = ShellIntent::Login;
                            close = true;
                        }
                        widgets::divider(ui);
                    }

                    // Aspecto: se aplica de verdad, y se refleja en la etiqueta.
                    let theme_label = if dark {
                        "Aspecto: oscuro"
                    } else {
                        "Aspecto: claro"
                    };
                    if widgets::menu_row(ui, Some(Icon::Star), theme_label, Some("")).clicked() {
                        // Cambiar de tema se hace desde Ajustes para no repetir
                        // la lógica en dos sitios.
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    if widgets::menu_row(
                        ui,
                        Some(Icon::Globe),
                        "Idioma de visualización",
                        Some("es"),
                    )
                    .clicked()
                    {
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    if widgets::menu_row(
                        ui,
                        Some(Icon::Lock),
                        "Modo restringido: desactivado",
                        Some(""),
                    )
                    .clicked()
                    {
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    if widgets::menu_row(ui, Some(Icon::Globe), "Ubicación: España", Some(""))
                        .clicked()
                    {
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    if widgets::menu_row(ui, Some(Icon::Keyboard), "Combinaciones de teclas", None)
                        .clicked()
                    {
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    widgets::divider(ui);
                    if widgets::menu_row(ui, Some(Icon::Settings), "Configuración", None).clicked()
                    {
                        intent = ShellIntent::OpenSettings;
                        close = true;
                    }
                    widgets::divider(ui);
                    if widgets::menu_row(ui, Some(Icon::Help), "Ayuda", None).clicked() {
                        intent =
                            ShellIntent::OpenExternal("https://support.google.com/youtube".into());
                        close = true;
                    }
                    if widgets::menu_row(ui, Some(Icon::Feedback), "Enviar sugerencias", None)
                        .clicked()
                    {
                        intent =
                            ShellIntent::OpenExternal("https://www.youtube.com/t/feedback".into());
                        close = true;
                    }
                });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    // Clic fuera del menú.
    if ctx.input(|i| i.pointer.any_click()) {
        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
            if pos.x < screen.right() - width - 24.0 || pos.y > screen.height() * 0.85 {
                close = true;
            }
        }
    }
    (close, intent)
}

/// Menú contextual de una tarjeta (los tres puntos).
pub fn card_menu(
    ctx: &egui::Context,
    at: egui::Pos2,
    card: &crate::domain::Card,
) -> (bool, CardMenuAction) {
    let p = palette(ctx);
    let mut close = false;
    let mut action = CardMenuAction::None;
    let width = 260.0;
    let screen = ctx.content_rect();
    // El menú no puede salirse de la ventana.
    let pos = egui::pos2(
        at.x.min(screen.right() - width - 8.0),
        at.y.min(screen.bottom() - 320.0),
    );

    egui::Area::new(egui::Id::unique("card_menu"))
        .fixed_pos(pos)
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(p.raised)
                .corner_radius(CornerRadius::same(8))
                .stroke(egui::Stroke::new(1.0, p.divider))
                .inner_margin(egui::Margin::symmetric(0, 8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    if widgets::menu_row(ui, Some(Icon::Queue), "Añadir a la cola", None).clicked()
                    {
                        action = CardMenuAction::Enqueue;
                        close = true;
                    }
                    if widgets::menu_row(
                        ui,
                        Some(Icon::WatchLater),
                        "Guardar en Ver más tarde",
                        None,
                    )
                    .clicked()
                    {
                        action = CardMenuAction::WatchLater;
                        close = true;
                    }
                    if widgets::menu_row(ui, Some(Icon::Share), "Compartir", None).clicked() {
                        action = CardMenuAction::Share;
                        close = true;
                    }
                    widgets::divider(ui);
                    if widgets::menu_row(ui, Some(Icon::ThumbsDown), "No me interesa", None)
                        .clicked()
                    {
                        action = CardMenuAction::NotInterested;
                        close = true;
                    }
                    if widgets::menu_row(ui, Some(Icon::Flag), "Denunciar", None).clicked() {
                        action = CardMenuAction::Report;
                        close = true;
                    }
                    let _ = &card.title;
                });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) || ctx.input(|i| i.pointer.any_click()) {
        // Un clic dentro del menú ya lo cierra por sí mismo; el clic fuera
        // también. Se deja que el propio menú consuma el clic interior.
        // `Context::rect_contains_pointer` pide el `LayerId` del área, que aquí
        // no hace falta: el menú vive en una capa propia y basta comparar la
        // posición del puntero con su rectángulo.
        let menu_rect = Rect::from_min_size(pos, Vec2::new(width, 320.0));
        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
            if !menu_rect.contains(p) {
                close = true;
            }
        }
    }
    (close, action)
}

/// Acción elegida en el menú de una tarjeta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardMenuAction {
    None,
    Enqueue,
    WatchLater,
    Share,
    NotInterested,
    Report,
}
