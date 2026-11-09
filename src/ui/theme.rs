//! Sistema visual: tokens de color, tipografía, espaciado y radios.
//!
//! Los valores salen de la hoja de estilos de youtube.com de escritorio
//! (`--yt-spec-*`, escala tipográfica de Roboto y densidad de 4 px). Están todos
//! aquí para que una diferencia con la referencia se pueda corregir en un sitio
//! y no en cuarenta widgets.
//!
//! Dos reglas que valen para todo el árbol de interfaz:
//!
//! - **Nada de tamaños mágicos.** Si un widget necesita un número, se lee de
//!   este módulo; si el número no existe, se añade aquí y se comprueba contra la
//!   captura de referencia.
//! - **La densidad es la de la web, no la de un móvil.** Eso significa 56 px de
//!   cabecera, 240 de barra lateral, 12 px de radio en miniaturas y 14 px de
//!   cuerpo, ni más ni menos.

use egui::{Color32, CornerRadius, Stroke, Visuals};

// --------------------------------------------------------------------- color

/// Paleta. Los nombres siguen los tokens públicos de YouTube para que se pueda
/// contrastar cada uno con la referencia.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Fondo de la página (`--yt-spec-base-background`).
    pub bg: Color32,
    /// Superficie elevada: menús, paneles flotantes (`--yt-spec-raised-background`).
    pub raised: Color32,
    /// Superficie de menú (`--yt-spec-menu-background`).
    pub menu: Color32,
    /// Relleno de chips, botones y campos (`--yt-spec-badge-chip-background`).
    pub chip: Color32,
    /// Hover sobre superficie (`--yt-spec-button-chip-background-hover`).
    pub hover: Color32,
    pub pressed: Color32,
    /// Texto principal (`--yt-spec-text-primary`).
    pub text: Color32,
    /// Texto secundario: canal, metadatos (`--yt-spec-text-secondary`).
    pub text2: Color32,
    /// Texto deshabilitado / marcas de tiempo de la barra (`--yt-spec-text-disabled`).
    pub text3: Color32,
    /// Separadores (`--yt-spec-10-percent-layer`).
    pub divider: Color32,
    /// Rojo de marca.
    pub brand: Color32,
    /// Azul de enlace.
    pub link: Color32,
    /// Texto sobre rojo/azul.
    pub on_brand: Color32,
    /// Marcador de carga.
    pub skeleton: Color32,
    /// Velo de los overlays.
    pub scrim: Color32,
    /// Color del "me gusta" activo (no cambia entre temas).
    pub like: Color32,
    /// Burbuja del globo de novedades.
    pub badge: Color32,
}

impl Palette {
    /// Tema oscuro (el de las capturas).
    pub const DARK: Palette = Palette {
        bg: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        raised: Color32::from_rgb(0x21, 0x21, 0x21),
        menu: Color32::from_rgb(0x28, 0x28, 0x28),
        chip: Color32::from_rgb(0x27, 0x27, 0x27),
        hover: Color32::from_rgb(0x3f, 0x3f, 0x3f),
        pressed: Color32::from_rgb(0x60, 0x60, 0x60),
        text: Color32::from_rgb(0xf1, 0xf1, 0xf1),
        text2: Color32::from_rgb(0xaa, 0xaa, 0xaa),
        text3: Color32::from_rgb(0x71, 0x71, 0x71),
        divider: Color32::from_rgb(0x30, 0x30, 0x30),
        brand: Color32::from_rgb(0xff, 0x00, 0x00),
        link: Color32::from_rgb(0x3e, 0xa6, 0xff),
        on_brand: Color32::WHITE,
        skeleton: Color32::from_rgb(0x27, 0x27, 0x27),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 170),
        like: Color32::from_rgb(0xff, 0x00, 0x3d),
        badge: Color32::from_rgb(0xcc, 0x00, 0x00),
    };

    /// Tema claro.
    pub const LIGHT: Palette = Palette {
        bg: Color32::from_rgb(0xff, 0xff, 0xff),
        raised: Color32::from_rgb(0xff, 0xff, 0xff),
        menu: Color32::from_rgb(0xff, 0xff, 0xff),
        chip: Color32::from_rgb(0xf2, 0xf2, 0xf2),
        hover: Color32::from_rgb(0xe5, 0xe5, 0xe5),
        pressed: Color32::from_rgb(0xd9, 0xd9, 0xd9),
        text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        text2: Color32::from_rgb(0x60, 0x60, 0x60),
        text3: Color32::from_rgb(0x90, 0x90, 0x90),
        divider: Color32::from_rgb(0xe5, 0xe5, 0xe5),
        brand: Color32::from_rgb(0xff, 0x00, 0x00),
        link: Color32::from_rgb(0x06, 0x5f, 0xd6),
        on_brand: Color32::WHITE,
        skeleton: Color32::from_rgb(0xe9, 0xe9, 0xe9),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 120),
        like: Color32::from_rgb(0xff, 0x00, 0x3d),
        badge: Color32::from_rgb(0xcc, 0x00, 0x00),
    };
}

// ------------------------------------------------------------------ tipografía

/// Escala tipográfica de la web. Los nombres son el uso, no el tamaño: así un
/// cambio de escala se hace en un sitio.
pub mod font {
    pub const CARD_TITLE: f32 = 14.0;
    pub const CARD_TITLE_LINE: f32 = 20.0;
    pub const META: f32 = 12.0;
    pub const META_LINE: f32 = 18.0;
    pub const BODY: f32 = 14.0;
    pub const BUTTON: f32 = 14.0;
    pub const CHIP: f32 = 14.0;
    pub const SIDEBAR: f32 = 14.0;
    pub const SIDEBAR_GROUP: f32 = 16.0;
    pub const SECTION_TITLE: f32 = 20.0;
    pub const PAGE_TITLE: f32 = 20.0;
    pub const CHANNEL_NAME: f32 = 36.0;
    pub const VIDEO_TITLE: f32 = 20.0;
    pub const MENU: f32 = 14.0;
    pub const MENU_HEADER: f32 = 16.0;
    pub const BADGE: f32 = 12.0;
    pub const DURATION: f32 = 12.0;
    pub const NOTIFICATION: f32 = 14.0;
    pub const COMMENT: f32 = 14.0;
    pub const FOOTER: f32 = 13.0;
}

// -------------------------------------------------------------------- métricas

/// Medidas de la referencia, en puntos lógicos.
pub mod metrics {
    /// Alto de la cabecera.
    pub const TOP_BAR: f32 = 56.0;
    /// Ancho de la barra lateral desplegada.
    pub const SIDEBAR: f32 = 240.0;
    /// Ancho de la barra lateral compacta (solo iconos).
    pub const SIDEBAR_MINI: f32 = 72.0;
    /// Por debajo de este ancho la barra lateral se oculta sola.
    pub const SIDEBAR_HIDE_UNDER: f32 = 792.0;
    /// Alto de la píldora del buscador.
    pub const SEARCH_HEIGHT: f32 = 40.0;
    /// Ancho máximo del buscador.
    pub const SEARCH_MAX: f32 = 536.0;
    /// Alto de los chips de filtro.
    pub const CHIP_HEIGHT: f32 = 32.0;
    pub const CHIP_PAD_X: f32 = 12.0;
    /// Radio de miniaturas y tarjetas.
    pub const RADIUS_CARD: u8 = 12;
    /// Radio de chips y botones pequeños.
    pub const RADIUS_CHIP: u8 = 8;
    /// Radio de la píldora del buscador.
    pub const RADIUS_PILL: u8 = 20;
    /// Separación entre tarjetas de la rejilla.
    pub const GRID_GAP: f32 = 16.0;
    /// Separación vertical dentro de una tarjeta.
    pub const GRID_GAP_Y: f32 = 24.0;
    /// Ancho mínimo de tarjeta antes de quitar una columna.
    pub const CARD_MIN: f32 = 300.0;
    /// Alto de la fila de la barra lateral.
    pub const SIDEBAR_ROW: f32 = 40.0;
    /// Radio de la fila de la barra lateral.
    pub const SIDEBAR_RADIUS: u8 = 10;
    /// Avatar pequeño de la barra lateral (suscripciones).
    pub const AVATAR_SMALL: f32 = 24.0;
    /// Avatar de una tarjeta.
    pub const AVATAR_CARD: f32 = 36.0;
    /// Avatar de cabecera de canal.
    pub const AVATAR_CHANNEL: f32 = 160.0;
    /// Alto del banner de canal.
    pub const BANNER: f32 = 192.0;
    /// Margen del contenido principal.
    pub const CONTENT_PAD: f32 = 24.0;
    /// Ancho del panel de comentarios de Shorts.
    pub const COMMENTS_PANEL: f32 = 480.0;
    /// Ancho del panel de playlist.
    pub const PLAYLIST_PANEL: f32 = 400.0;
    /// Ancho del panel de notificaciones.
    pub const NOTIFICATIONS_PANEL: f32 = 480.0;
    /// Alto máximo del panel de notificaciones.
    pub const NOTIFICATIONS_MAX_H: f32 = 640.0;
    /// Radio de los paneles flotantes.
    pub const RADIUS_PANEL: u8 = 12;
    /// Ancho de la barra de progreso del reproductor.
    pub const PLAYER_BAR: f32 = 4.0;
    /// Alto de los controles superpuestos del reproductor.
    pub const PLAYER_CONTROLS: f32 = 48.0;

    // --- nombres anteriores que aún usan pantallas sin migrar ---
    /// Radio de tarjeta (nombre antiguo de [`RADIUS_CARD`]).
    pub const CARD_RADIUS: u8 = RADIUS_CARD;
    /// Ancho del panel de cola (nombre antiguo de [`PLAYLIST_PANEL`]).
    pub const QUEUE_PANEL: f32 = PLAYLIST_PANEL;
    /// Alto de la barra de reproducción inferior.
    pub const MINI_PLAYER: f32 = 72.0;
    /// Ancho mínimo de tarjeta (nombre antiguo de [`CARD_MIN`]).
    pub const CARD_MIN_WIDTH: f32 = CARD_MIN;
    /// Hueco entre tarjetas (nombre antiguo de [`GRID_GAP`]).
    pub const GAP: f32 = GRID_GAP;
}

/// Aplica la paleta y la escala tipográfica al contexto de egui.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = if dark { Palette::DARK } else { Palette::LIGHT };
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.menu;
    visuals.extreme_bg_color = p.bg;
    visuals.faint_bg_color = p.chip;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.link;
    visuals.selection.bg_fill = p.pressed;
    visuals.selection.stroke = Stroke::new(1.0, p.text);
    visuals.widgets.noninteractive.bg_fill = p.bg;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.divider);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.bg_fill = p.chip;
    visuals.widgets.inactive.weak_bg_fill = p.chip;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.bg_fill = p.hover;
    visuals.widgets.hovered.weak_bg_fill = p.hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.bg_fill = p.pressed;
    visuals.widgets.active.weak_bg_fill = p.pressed;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.open.bg_fill = p.raised;
    visuals.widgets.open.weak_bg_fill = p.raised;
    visuals.window_corner_radius = CornerRadius::same(metrics::RADIUS_PANEL);
    visuals.menu_corner_radius = CornerRadius::same(metrics::RADIUS_PANEL);
    visuals.window_stroke = Stroke::new(1.0, p.divider);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    visuals.popup_shadow = visuals.window_shadow;
    ctx.set_visuals_of(if dark { egui::Theme::Dark } else { egui::Theme::Light }, visuals);
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });

    // La escala tipográfica se aplica a los dos temas para que cambiar de tema
    // no cambie el tamaño del texto.
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.spacing.scroll.bar_inner_margin = 2.0;
        style.spacing.interact_size.y = 32.0;
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(font::SECTION_TITLE, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(font::BODY, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(font::BUTTON, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(font::META, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Paleta activa.
pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.theme() == egui::Theme::Dark {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

/// Color del "me gusta" activo.
pub fn like_active() -> Color32 {
    Palette::DARK.like
}
