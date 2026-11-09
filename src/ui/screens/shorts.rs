//! Shorts: un reproductor vertical, dos estados del mismo reparto del espacio.
//!
//! Composiciones de las capturas de referencia (`plan-y-referencias/imagenes`):
//!
//! ```text
//! 06 · comentarios cerrados         05 · comentarios abiertos
//! ┌──────────────────────────────┐  ┌───────────────────┬────────────┐
//! │        │ marco 9:16 │ acciones│  │  │ marco │ acc. │   │ panel de   │
//! │        │            │ + flecha│  │  │ 9:16  │      │   │ comentarios│
//! │        │ progreso   │        │  │  │ barra ───────  │   │ 480 px     │
//! └──────────────────────────────┘  └───────────────────┴────────────┘
//! ```
//!
//! Dos reglas que explican todo lo de abajo:
//!
//! - **El reparto se calcula solo con el área disponible y el estado del panel**
//!   ([`layout`]). No hay memoria de la composición anterior, así que cerrar el
//!   panel devuelve exactamente la de la captura 06: no se «recompone» nada,
//!   se vuelve a calcular con los mismos números.
//! - **Solo hay un vídeo.** La pantalla pinta el fotograma de `player.texture`,
//!   que ya es único, y nada más: no pinta la miniatura de los siguientes ni
//!   reserva un segundo reproductor. Un motor por aplicación es el invariante
//!   que sostiene `app.rs`; aquí simplemente no se le pide un segundo medio.

use egui::{
    Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, Ui, UiBuilder,
    Vec2,
};

use crate::domain::{Card, MediaKind};
use crate::ui::actions::Load;
use crate::ui::app::{Intent, PlayerView};
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{font, metrics, palette};
use crate::ui::widgets::{self, PillKind, ThumbLookup};

// Medidas propias de esta pantalla, tomadas de las capturas 05 y 06. No están
// en `theme` porque no las usa ninguna otra pantalla; cuando otra las necesite
// se subirán allí (la regla del tema es un sitio por número, no un número por
// sitio).
/// Ancho de la columna de acciones (me gusta, comentarios, compartir…).
const RAIL_W: f32 = 64.0;
/// Separación entre el marco del vídeo y la columna de acciones.
const RAIL_GAP: f32 = 8.0;
/// Diámetro de los botones redondos de la columna.
const RAIL_BUTTON: f32 = 48.0;
/// Alto de un elemento de la columna: botón más su cuenta o etiqueta debajo.
const RAIL_ITEM: f32 = RAIL_BUTTON + 18.0;
/// Margen del vídeo con el borde superior del área (captura 06: 14 px).
const TOP_PAD: f32 = 14.0;
/// Franja bajo el vídeo para la barra de progreso y el contador «x / N».
const BOTTOM_BAR: f32 = 40.0;
/// Columna que se aparta a la derecha para las flechas cuando el panel está
/// abierto: en la captura 05 las flechas quedan sobre el fondo de la página,
/// **fuera** del panel, no encima de los comentarios.
const NAV_COL: f32 = 88.0;
/// Diámetro de las flechas de navegación.
const NAV_BUTTON: f32 = 40.0;
/// Margen interior del bloque de metadatos y del panel de comentarios.
const PAD: f32 = 16.0;
/// Título del Short y @handle medidos sobre la captura 05.
const TITLE_SIZE: f32 = 16.0;
const HANDLE_SIZE: f32 = 14.0;
/// Alto de la cabecera del panel («Comentarios», recuento y cerrar).
const PANEL_HEADER: f32 = 56.0;
/// Alto de la caja de escritura anclada al pie del panel.
const PANEL_INPUT: f32 = 64.0;
/// Identificador de la caja de comentario. La navegación lo consulta para no
/// cambiar de Short mientras se está escribiendo.
const INPUT_ID: &str = "shorts_comment_input";

/// Todo lo que la pantalla necesita. Sale de [`crate::ui::app`]: la pantalla no
/// pide comentarios, no resuelve streams y no sabe si el dato viene de caché.
pub struct ShortsInput<'a> {
    pub cards: &'a [Card],
    pub index: usize,
    pub load: &'a Load,
    pub player: &'a PlayerView,
    pub comments: &'a [crate::yt::comments::Comment],
    pub comments_load: &'a Load,
    pub comments_open: bool,
    pub comments_input: &'a mut String,
    pub liked: bool,
    pub subscribed: bool,
}

/// Reparto del espacio. Es el único sitio que decide dónde va cada pieza; las
/// dos capturas salen de aquí cambiando `open`.
struct Layout {
    /// Marco del vídeo, con la proporción que se le pase.
    video: Rect,
    /// Columna de acciones, a la derecha del marco (captura 06).
    rail: Rect,
    /// Barra de progreso, bajo el marco.
    progress: Rect,
    /// Panel de comentarios, si está abierto.
    panel: Option<Rect>,
}

fn layout(area: Rect, aspect: f32, open: bool) -> Layout {
    let panel = if open {
        // El panel se ancla al borde derecho y cede ancho antes que el vídeo:
        // en una ventana estrecha es preferible una lista corta a un vídeo de
        // 120 px de ancho. Los 480 px son la medida de la captura 05
        // (`metrics::COMMENTS_PANEL`).
        let width = (area.width() * 0.42).clamp(320.0, metrics::COMMENTS_PANEL);
        let right = area.right() - NAV_COL;
        Some(Rect::from_min_max(
            egui::pos2(right - width, area.top()),
            egui::pos2(right, area.bottom()),
        ))
    } else {
        None
    };

    // Con el panel cerrado el vídeo se centra en **todo** el ancho (captura 06);
    // las flechas se pintan sobre el fondo de la página, no reservan sitio. Con
    // el panel abierto, el campo se queda con lo que queda a su izquierda.
    let field_right = match panel {
        Some(panel) => panel.left() - PAD,
        None => area.right(),
    };
    let field = Rect::from_min_max(area.min, egui::pos2(field_right, area.bottom()));

    // Primero el alto: las capturas llenan la ventana y dejan que la
    // proporción decida el ancho. Solo si el ancho aprieta manda el ancho.
    let full_h = (field.height() - TOP_PAD - BOTTOM_BAR).max(120.0);
    let max_w = (field.width() - RAIL_W - RAIL_GAP).max(100.0);
    let video_h = if full_h * aspect > max_w {
        max_w / aspect
    } else {
        full_h
    };
    let video_w = video_h * aspect;

    // Se centra el conjunto vídeo + acciones, que es lo que aparece centrado en
    // las dos capturas (no el vídeo solo).
    let group_w = video_w + RAIL_GAP + RAIL_W;
    let left = field.left() + ((field.width() - group_w) * 0.5).max(0.0);
    let video = Rect::from_min_size(
        egui::pos2(left, field.top() + TOP_PAD),
        Vec2::new(video_w, video_h),
    );
    let rail = Rect::from_min_size(
        egui::pos2(video.right() + RAIL_GAP, video.top()),
        Vec2::new(RAIL_W, video.height()),
    );
    let progress = Rect::from_min_size(
        egui::pos2(video.left(), video.bottom() + 16.0),
        Vec2::new(video.width(), 4.0),
    );

    Layout {
        video,
        rail,
        progress,
        panel,
    }
}

pub fn shorts(
    ui: &mut egui::Ui,
    input: &mut ShortsInput<'_>,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());

    if let Load::Failed(message) = input.load {
        if widgets::error_state(ui, message) {
            return Some(Intent::Reload);
        }
        return None;
    }

    if input.cards.is_empty() {
        if input.load.is_loading() {
            ui.vertical_centered(|ui| {
                ui.allocate_exact_size(Vec2::new(1.0, 120.0), Sense::hover());
                ui.spinner();
                ui.label(
                    RichText::new("Buscando Shorts…")
                        .size(font::BODY)
                        .color(p.text2),
                );
            });
        } else {
            widgets::empty_state(ui, "No hay Shorts", "Prueba a recargar en un momento.");
        }
        return None;
    }

    // La pantalla está ocupada entera por una pieza: el resto del feed solo se
    // toca con las flechas o la rueda, nunca pintando otro fotograma.
    let card = input.cards.get(input.index.min(input.cards.len() - 1))?;
    let (area, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());

    // La proporción sale del fotograma real cuando ya hay uno: así un Short que
    // no sea 9:16 se ve completo en vez de deformado. Antes del primer
    // fotograma se usa 9:16, que es lo que anuncia la miniatura.
    let aspect = input
        .player
        .texture
        .as_ref()
        .map(|texture| {
            let size = texture.size_vec2();
            if size.y > 0.0 {
                size.x / size.y
            } else {
                9.0 / 16.0
            }
        })
        .filter(|aspect| aspect.is_finite() && (0.2..=2.5).contains(aspect))
        .unwrap_or(9.0 / 16.0);

    let geo = layout(area, aspect, input.comments_open);
    let mut intent = None;

    // --- vídeo ------------------------------------------------------------
    ui.painter().rect_filled(
        geo.video,
        CornerRadius::same(metrics::RADIUS_CARD),
        Color32::BLACK,
    );

    // Un clic en el marco reproduce o para, como en la web. Se registra **antes**
    // que el bloque de metadatos y las acciones: en egui el clic es para el
    // widget añadido el último, así que el marco tiene que ser el de abajo o se
    // tragaría el de la píldora de suscripción.
    let frame = ui.interact(geo.video, egui::Id::unique("shorts_frame"), Sense::click());
    if frame.clicked() {
        intent = Some(Intent::TogglePlay);
    }
    match (&input.player.texture, &input.player.error) {
        (Some(texture), _) => draw_frame(ui, geo.video, texture),
        (None, Some(error)) => {
            // El error se dice sobre el marco en lugar de dejar un negro mudo.
            let galley = ui.painter().layout(
                error.clone(),
                FontId::proportional(font::BODY),
                Color32::WHITE,
                (geo.video.width() - PAD * 2.0).max(40.0),
            );
            ui.painter().galley(
                geo.video.center() - galley.size() * 0.5,
                galley,
                Color32::WHITE,
            );
        }
        (None, None) => {
            // Mientras se resuelve el stream se ve la miniatura: el cambio de
            // Short se nota en el acto aunque la red tarde.
            widgets::thumb_image(
                ui,
                geo.video,
                &card.thumbs,
                metrics::RADIUS_CARD,
                &mut *lookup,
            );
            egui::Spinner::new().paint_at(
                ui,
                Rect::from_center_size(geo.video.center(), Vec2::splat(40.0)),
            );
        }
    }

    // Velo inferior en tres bandas: el título y el @handle se leen sobre
    // cualquier fotograma. `epaint` no rellena degradados y a 150 px del borde
    // esta aproximación no se distingue de la de la web.
    for (height, alpha) in [(150.0_f32, 45_u8), (96.0, 45), (48.0, 45)] {
        let height = height.min(geo.video.height());
        let band = Rect::from_min_size(
            egui::pos2(geo.video.left(), geo.video.bottom() - height),
            Vec2::new(geo.video.width(), height),
        );
        ui.painter().rect_filled(
            band,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: metrics::RADIUS_CARD,
                se: metrics::RADIUS_CARD,
            },
            Color32::from_black_alpha(alpha),
        );
    }

    // --- piezas sobre el vídeo -------------------------------------------
    if let Some(clicked) = metadata_block(ui, geo.video, card, input.subscribed, &mut *lookup) {
        intent = Some(clicked);
    }

    let comment_count = if input.comments.is_empty() {
        None
    } else {
        // Se muestra el número de comentarios que la aplicación tiene
        // cargados, no un total inventado: `Comment` no trae el recuento del
        // vídeo y `api::watch` no lo guarda.
        Some(input.comments.len())
    };
    if let Some(clicked) = action_rail(ui, geo.rail, card, input.liked, comment_count, &mut *lookup)
    {
        intent = Some(clicked);
    }

    if let Some(clicked) = progress_bar(
        ui,
        geo.progress,
        input.player,
        input.index,
        input.cards.len(),
    ) {
        intent = Some(clicked);
    }

    if let Some(clicked) = nav_arrows(ui, area, input.index) {
        intent = Some(clicked);
    }

    // --- panel de comentarios (captura 05) -------------------------------
    if let Some(panel) = geo.panel {
        if let Some(clicked) = comments_panel(
            ui,
            panel,
            input.comments,
            input.comments_load,
            input.comments_input,
            &mut *lookup,
        ) {
            intent = Some(clicked);
        }
    }

    // --- navegación con rueda y flechas ----------------------------------
    //
    // Solo cuando el puntero no está sobre el panel (ahí la rueda desplaza los
    // comentarios) y la caja de comentario no tiene el foco (las flechas
    // moverían el cursor y, de paso, cambiarían de Short). Se comprueba al
    // final para no pisar un clic que ya se ha decidido arriba.
    let typing = ui.memory(|memory| memory.has_focus(egui::Id::unique(INPUT_ID)));
    let over_panel = geo.panel.is_some_and(|panel| {
        ui.input(|i| i.pointer.hover_pos())
            .is_some_and(|pos| panel.contains(pos))
    });
    if intent.is_none() && !typing && !over_panel {
        let scrolled = ui.input(|i| i.smooth_scroll_delta.y);
        if scrolled.abs() > 20.0 {
            // Rueda hacia abajo (delta negativo) avanza, como en la web.
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
    }

    intent
}

/// Pinta el fotograma del reproductor dentro del marco, con las esquinas del
/// vídeo redondeadas como el resto de la interfaz.
fn draw_frame(ui: &mut Ui, rect: Rect, texture: &egui::TextureHandle) {
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

/// Bloque inferior izquierdo del marco: canal, suscripción, título y sonido.
///
/// Se compone de abajo arriba (`Layout::bottom_up`) porque todo él se apoya en
/// el borde inferior del vídeo: así el chip del sonido queda pegado abajo y el
/// título crece hacia arriba sin números mágicos de altura.
fn metadata_block(
    ui: &mut Ui,
    video: Rect,
    card: &Card,
    subscribed: bool,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;
    let block = Rect::from_min_max(
        egui::pos2(video.left() + PAD, video.top() + PAD),
        egui::pos2(video.right() - PAD, video.bottom() - PAD),
    );

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(block)
            .layout(egui::Layout::bottom_up(egui::Align::Min)),
        |ui| {
            // --- chip del sonido
            sound_chip(ui, "Sonido original");

            ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());

            // --- título
            ui.add(
                egui::Label::new(
                    RichText::new(&card.title)
                        .size(TITLE_SIZE)
                        .strong()
                        .color(Color32::WHITE),
                )
                .wrap(),
            );

            ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());

            // --- fila del canal: avatar, @handle y suscripción
            ui.horizontal(|ui| {
                let handle = if card.author.starts_with('@') {
                    card.author.clone()
                } else {
                    format!("@{}", card.author)
                };
                // El avatar y la píldora solo son acciones si se conoce el
                // canal (`UC…`): sin él no hay a dónde ir ni a quién
                // suscribirse, así que se ven como texto y no como botón.
                let channel_id = card.author_id.clone();
                let (avatar, avatar_response) =
                    ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
                widgets::avatar_image(ui, avatar, &card.thumbs, &mut *lookup);
                if avatar_response.clicked() {
                    if let Some(channel_id) = channel_id.clone() {
                        intent = Some(Intent::Open(Card::new(
                            channel_id,
                            MediaKind::Channel,
                            card.author.clone(),
                        )));
                    }
                }
                ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                ui.add(
                    egui::Label::new(
                        RichText::new(handle)
                            .size(HANDLE_SIZE)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .wrap_mode(egui::TextWrapMode::Truncate),
                );
                if let Some(channel_id) = channel_id {
                    ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                    let label = if subscribed {
                        "Suscrito"
                    } else {
                        "Suscribirse"
                    };
                    if widgets::pill(ui, label, None, PillKind::Solid).clicked() {
                        intent = Some(Intent::ToggleSubscribe {
                            channel_id,
                            on: !subscribed,
                        });
                    }
                }
            });
        },
    );

    intent
}

/// Chip del sonido. Es una etiqueta, no un botón: el `Card` de un Short no
/// trae el id del audio, así que la aplicación no puede abrir su página y el
/// chip no finge que sí. Se quita el triángulo de reproducción de la captura
/// por el mismo motivo.
fn sound_chip(ui: &mut Ui, text: &str) {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(font::META),
        Color32::WHITE,
    );
    let size = Vec2::new(galley.size().x + 16.0 + 16.0 + 16.0, 28.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(metrics::RADIUS_CHIP),
        Color32::from_black_alpha(170),
    );
    icons::draw(
        ui,
        Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            Vec2::splat(16.0),
        ),
        Icon::Music,
        Color32::WHITE,
    );
    let text_height = galley.size().y;
    ui.painter().galley(
        egui::pos2(rect.left() + 32.0, rect.center().y - text_height * 0.5),
        galley,
        Color32::WHITE,
    );
}

/// Columna de acciones de la derecha (capturas 05 y 06): me gusta, comentarios,
/// compartir, remix y el avatar del canal.
///
/// No hay pulgar abajo: `api::set_like` solo sabe dar y quitar el «me gusta»
/// (`like`/`removelike`), así que un botón de «no me gusta» no tendría nada
/// detrás. En lugar de un botón que miente, no está.
fn action_rail(
    ui: &mut Ui,
    rail: Rect,
    card: &Card,
    liked: bool,
    comment_count: Option<usize>,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let mut intent = None;
    let center_x = rail.center().x;

    // El avatar va al final de la columna: es el acceso a la página del canal.
    let mut bottom = rail.bottom() - PAD;
    let avatar = Rect::from_center_size(
        egui::pos2(center_x, bottom - metrics::AVATAR_CARD * 0.5),
        Vec2::splat(metrics::AVATAR_CARD),
    );
    let avatar_response = ui.interact(
        avatar,
        egui::Id::unique("shorts_rail_avatar"),
        Sense::click(),
    );
    widgets::avatar_image(ui, avatar, &card.thumbs, &mut *lookup);
    if avatar_response.clicked() {
        if let Some(channel_id) = card.author_id.clone() {
            intent = Some(Intent::Open(Card::new(
                channel_id,
                MediaKind::Channel,
                card.author.clone(),
            )));
        }
    }
    bottom = avatar.top() - 16.0;

    // De abajo arriba: remix, compartir, comentarios y me gusta. Las etiquetas
    // son las de la captura; lo que no existe como acción (la cuenta de «me
    // gusta») se queda sin número en vez de con un cero.
    let comment_label = comment_count
        .map(|count| count.to_string())
        .unwrap_or_default();
    let items: [(&str, Icon, String, bool); 4] = [
        ("remix", Icon::Remix, "Remix".to_string(), false),
        ("share", Icon::Share, "Compartir".to_string(), false),
        ("comments", Icon::Comment, comment_label, false),
        (
            "like",
            if liked {
                Icon::ThumbsUpFilled
            } else {
                Icon::ThumbsUp
            },
            String::new(),
            liked,
        ),
    ];

    for (key, icon, label, active) in items {
        let top = bottom - RAIL_ITEM;
        let response = rail_button(ui, center_x, top, key, icon, &label, active);
        if response.clicked() {
            intent = Some(match key {
                "like" => Intent::ToggleLike {
                    video_id: card.id.clone(),
                    on: !liked,
                },
                "comments" => Intent::ToggleComments,
                "share" => {
                    Intent::OpenExternal(format!("https://www.youtube.com/shorts/{}", card.id))
                }
                // «Remix» no tiene intent propio: se abre en el navegador, que
                // es donde el remix funciona de verdad (la mezcla del vídeo).
                _ => Intent::OpenExternal(format!(
                    "https://www.youtube.com/watch?v={}&list=RD{}",
                    card.id, card.id
                )),
            });
        }
        bottom = top - 8.0;
    }

    intent
}

/// Un botón redondo de la columna, con su etiqueta debajo. `key` es fijo y no
/// depende del texto: el id de interacción no puede cambiar cuando cambia el
/// recuento o el Short.
fn rail_button(
    ui: &mut Ui,
    center_x: f32,
    top: f32,
    key: &str,
    icon: Icon,
    label: &str,
    active: bool,
) -> Response {
    let p = palette(ui.ctx());
    let center = egui::pos2(center_x, top + RAIL_BUTTON * 0.5);
    let rect = Rect::from_center_size(center, Vec2::splat(RAIL_BUTTON));
    let response = ui.interact(rect, egui::Id::unique(("shorts_rail", key)), Sense::click());
    ui.painter().circle_filled(
        center,
        RAIL_BUTTON * 0.5,
        if response.hovered() {
            Color32::from_white_alpha(60)
        } else {
            Color32::from_white_alpha(28)
        },
    );
    icons::draw(
        ui,
        Rect::from_center_size(center, Vec2::splat(24.0)),
        icon,
        if active { p.like } else { Color32::WHITE },
    );
    if !label.is_empty() {
        ui.painter().text(
            egui::pos2(center_x, rect.bottom() + 2.0),
            Align2::CENTER_TOP,
            label,
            FontId::proportional(font::BADGE),
            Color32::WHITE,
        );
    }
    response
}

/// Barra de progreso del Short y contador «x / N» (captura 06). Se puede
/// arrastrar el tiempo porque es lo que hace la web al pulsarla; sin duración
/// conocida se queda vacía en vez de inventar una posición.
fn progress_bar(
    ui: &mut Ui,
    rect: Rect,
    player: &PlayerView,
    index: usize,
    total: usize,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    let ratio = if player.duration_ms > 0 {
        (player.position_ms as f32 / player.duration_ms as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), Color32::from_white_alpha(70));
    ui.painter().rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(rect.width() * ratio, rect.height())),
        CornerRadius::same(2),
        p.brand,
    );

    // Zona de clic más alta que la barra (4 px no se aciertan con el ratón).
    let hit = rect.expand2(Vec2::new(0.0, 8.0));
    let response = ui.interact(hit, egui::Id::unique("shorts_progress"), Sense::click());
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fraction = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
            intent = Some(Intent::SeekFraction(fraction));
        }
    }

    ui.painter().text(
        egui::pos2(rect.right(), rect.bottom() + 6.0),
        Align2::RIGHT_TOP,
        format!("{} / {}", index + 1, total),
        FontId::proportional(font::BADGE),
        p.text2,
    );
    intent
}

/// Flechas de la derecha, centradas en vertical sobre el borde del área.
///
/// Abajo siempre está: en el último Short sigue habiendo «siguiente» porque
/// `app.rs` pide más piezas al llegar al final. Arriba solo si ya hay una
/// anterior; en la captura 06 no se pinta, y no se pinta porque en el primer
/// Short no haría nada.
fn nav_arrows(ui: &mut Ui, area: Rect, index: usize) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;
    let center_x = area.right() - NAV_COL * 0.5;

    let down = Rect::from_center_size(
        egui::pos2(center_x, area.center().y + 28.0),
        Vec2::splat(NAV_BUTTON),
    );
    let response = ui.interact(down, egui::Id::unique("shorts_nav_down"), Sense::click());
    ui.painter().circle_filled(
        down.center(),
        NAV_BUTTON * 0.5,
        if response.hovered() { p.hover } else { p.chip },
    );
    icons::draw(
        ui,
        Rect::from_center_size(down.center(), Vec2::splat(20.0)),
        Icon::ChevronDown,
        p.text,
    );
    if response.clicked() {
        intent = Some(Intent::ShortsNext);
    }

    if index > 0 {
        let up = Rect::from_center_size(
            egui::pos2(center_x, area.center().y - 28.0),
            Vec2::splat(NAV_BUTTON),
        );
        let response = ui.interact(up, egui::Id::unique("shorts_nav_up"), Sense::click());
        ui.painter().circle_filled(
            up.center(),
            NAV_BUTTON * 0.5,
            if response.hovered() { p.hover } else { p.chip },
        );
        icons::draw(
            ui,
            Rect::from_center_size(up.center(), Vec2::splat(20.0)),
            Icon::ChevronUp,
            p.text,
        );
        if response.clicked() {
            intent = Some(Intent::ShortsPrevious);
        }
    }

    intent
}

/// Panel de comentarios de la captura 05: cabecera, lista con scroll propio y
/// caja de escritura fija al pie.
fn comments_panel(
    ui: &mut Ui,
    rect: Rect,
    comments: &[crate::yt::comments::Comment],
    load: &Load,
    draft: &mut String,
    lookup: ThumbLookup<'_>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    ui.painter()
        .rect_filled(rect, CornerRadius::same(metrics::RADIUS_PANEL), p.raised);

    // --- cabecera
    let header = Rect::from_min_size(
        egui::pos2(rect.left() + PAD, rect.top()),
        Vec2::new(rect.width() - PAD * 2.0, PANEL_HEADER),
    );
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(header)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.add(egui::Label::new(
                RichText::new("Comentarios")
                    .size(font::SECTION_TITLE)
                    .strong()
                    .color(p.text),
            ));
            if !comments.is_empty() {
                ui.allocate_exact_size(Vec2::new(8.0, 1.0), Sense::hover());
                // El recuento es el de la lista cargada, no el total del vídeo:
                // `Comment` no lo trae y no se inventa.
                ui.add(egui::Label::new(
                    RichText::new(comments.len().to_string())
                        .size(font::META)
                        .color(p.text2),
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::icon_button(ui, Icon::Close, 20.0, false, "Cerrar").clicked() {
                    intent = Some(Intent::ToggleComments);
                }
            });
        },
    );
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), header.bottom()),
            egui::pos2(rect.right(), header.bottom()),
        ],
        Stroke::new(1.0, p.divider),
    );

    // --- lista con scroll propio: la rueda dentro del panel no cambia de Short
    let input_top = rect.bottom() - PANEL_INPUT;
    let list = Rect::from_min_max(
        egui::pos2(rect.left(), header.bottom() + 8.0),
        egui::pos2(rect.right(), input_top),
    )
    .shrink2(Vec2::new(PAD, 0.0));
    ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("shorts_comments")
            .auto_shrink([false, false])
            .show(ui, |ui| match load {
                Load::Loading | Load::Idle if comments.is_empty() => {
                    for _ in 0..3 {
                        ui.horizontal_top(|ui| {
                            let (avatar, _) =
                                ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                            ui.painter()
                                .circle_filled(avatar.center(), 16.0, p.skeleton);
                            ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
                            ui.vertical(|ui| {
                                for width in [120.0_f32, 240.0] {
                                    let (line, _) = ui.allocate_exact_size(
                                        Vec2::new(width, 12.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().rect_filled(line, 4, p.skeleton);
                                }
                            });
                        });
                        ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
                    }
                }
                Load::Failed(message) => {
                    ui.add(
                        egui::Label::new(
                            RichText::new(message.as_str())
                                .size(font::BODY)
                                .color(p.text2),
                        )
                        .wrap(),
                    );
                }
                _ if comments.is_empty() => {
                    ui.add(
                        egui::Label::new(
                            RichText::new("Todavía no hay comentarios.")
                                .size(font::BODY)
                                .color(p.text2),
                        )
                        .wrap(),
                    );
                }
                _ => {
                    for comment in comments {
                        comment_row(ui, comment, 0, &mut *lookup, &mut intent);
                        ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
                    }
                }
            });
    });

    // --- caja de escritura, anclada al pie (captura 05)
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + PAD, input_top),
            egui::pos2(rect.right() - PAD, input_top),
        ],
        Stroke::new(1.0, p.divider),
    );
    let input_row = Rect::from_min_max(
        egui::pos2(rect.left() + PAD, input_top),
        egui::pos2(rect.right() - PAD, rect.bottom()),
    )
    .shrink2(Vec2::new(0.0, 12.0));
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(input_row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            // El avatar de quien escribe no está en el estado de la pantalla:
            // se deja el círculo de la referencia, sin inventar una imagen.
            let (avatar, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            ui.painter().circle_filled(avatar.center(), 20.0, p.chip);
            ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
            let width = ui.available_width();
            let edit = ui.add_sized(
                Vec2::new(width, 32.0),
                egui::TextEdit::singleline(draft)
                    .id(egui::Id::unique(INPUT_ID))
                    .frame(egui::Frame::NONE)
                    .hint_text(
                        RichText::new("Añade un comentario…")
                            .size(font::BODY)
                            .color(p.text3),
                    ),
            );
            ui.painter().line_segment(
                [
                    egui::pos2(edit.rect.left(), edit.rect.bottom()),
                    egui::pos2(edit.rect.right(), edit.rect.bottom()),
                ],
                Stroke::new(1.0, p.divider),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !draft.trim().is_empty() {
                    intent = Some(Intent::PostComment(draft.clone()));
                }
            }
        },
    );

    intent
}

/// Un comentario del panel. `depth` sangra las respuestas que ya vienen en la
/// misma página; no hay botón para pedir más porque no hay intent para la
/// continuación de un hilo.
fn comment_row(
    ui: &mut Ui,
    comment: &crate::yt::comments::Comment,
    depth: usize,
    lookup: ThumbLookup<'_>,
    intent: &mut Option<Intent>,
) {
    let p = palette(ui.ctx());
    let indent = 20.0 * depth as f32;
    let side = if depth == 0 { 32.0 } else { 24.0 };
    let width = (ui.available_width() - indent - side - 12.0).max(80.0);

    ui.horizontal_top(|ui| {
        ui.allocate_exact_size(Vec2::new(indent, 1.0), Sense::hover());
        let (avatar, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
        widgets::avatar_image(ui, avatar, &comment.author_avatar, &mut *lookup);
        ui.allocate_exact_size(Vec2::new(12.0, 1.0), Sense::hover());
        ui.allocate_ui_with_layout(
            Vec2::new(width, 0.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                if comment.pinned {
                    ui.add(egui::Label::new(
                        RichText::new("Fijado").size(font::BADGE).color(p.text2),
                    ));
                }
                ui.horizontal(|ui| {
                    let handle = if comment.author.starts_with('@') {
                        comment.author.clone()
                    } else {
                        format!("@{}", comment.author)
                    };
                    ui.add(egui::Label::new(
                        RichText::new(handle)
                            .size(font::META)
                            .strong()
                            .color(p.text),
                    ));
                    if !comment.published.is_empty() {
                        ui.add(egui::Label::new(
                            RichText::new(&comment.published)
                                .size(font::META)
                                .color(p.text2),
                        ));
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
                ui.allocate_exact_size(Vec2::new(1.0, 4.0), Sense::hover());
                ui.horizontal(|ui| {
                    // Me gusta: el número se pinta tal cual lo manda YouTube
                    // («1,2 mil»): reinterpretarlo daría cifras falsas.
                    let (like, like_response) =
                        ui.allocate_exact_size(Vec2::new(52.0, 24.0), Sense::click());
                    icons::draw(
                        ui,
                        Rect::from_center_size(
                            egui::pos2(like.left() + 10.0, like.center().y),
                            Vec2::splat(16.0),
                        ),
                        Icon::ThumbsUp,
                        p.text2,
                    );
                    if !comment.likes.is_empty() {
                        ui.painter().text(
                            egui::pos2(like.left() + 22.0, like.center().y),
                            Align2::LEFT_CENTER,
                            &comment.likes,
                            FontId::proportional(font::META),
                            p.text2,
                        );
                    }
                    if like_response.clicked() {
                        *intent = Some(Intent::RateComment {
                            comment_id: comment.id.clone(),
                            up: true,
                        });
                    }
                    let (down, down_response) =
                        ui.allocate_exact_size(Vec2::new(28.0, 24.0), Sense::click());
                    icons::draw(
                        ui,
                        Rect::from_center_size(
                            egui::pos2(down.left() + 12.0, down.center().y),
                            Vec2::splat(16.0),
                        ),
                        Icon::ThumbsDown,
                        p.text2,
                    );
                    if down_response.clicked() {
                        *intent = Some(Intent::RateComment {
                            comment_id: comment.id.clone(),
                            up: false,
                        });
                    }
                    // Respuestas anunciadas pero no incluidas: se dice cuántas
                    // hay sin ofrecer un botón que no puede pedirlas.
                    if let Some(count) = comment.reply_count {
                        if comment.replies.is_empty() {
                            ui.add(egui::Label::new(
                                RichText::new(format!("{count} respuestas"))
                                    .size(font::META)
                                    .color(p.text2),
                            ));
                        }
                    }
                });
                for reply in &comment.replies {
                    ui.allocate_exact_size(Vec2::new(1.0, 8.0), Sense::hover());
                    comment_row(ui, reply, depth + 1, &mut *lookup, &mut *intent);
                }
            },
        );
    });
}
