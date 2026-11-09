//! Tema visual: paleta y medidas tomadas de youtube.com de escritorio.
//!
//! Los valores están tomados de la hoja de estilos pública de YouTube (tema
//! oscuro y claro) y se dejan en un solo sitio para que la fidelidad sea
//! revisable y comparable contra la referencia (PLAN.md §7). Lo que **no** se
//! copia es la identidad: NeoTube mantiene su nombre y su icono.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Paleta. Los nombres son los de YouTube (`--yt-spec-*`) traducidos.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub bg_elevated: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub divider: Color32,
    pub brand: Color32,
    pub link: Color32,
    pub chip: Color32,
    pub chip_active_text: Color32,
    pub skeleton: Color32,
    pub scrim: Color32,
}

impl Palette {
    /// Tema oscuro (`--yt-spec-base-background` #0f0f0f).
    pub const DARK: Palette = Palette {
        bg: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        bg_elevated: Color32::from_rgb(0x21, 0x21, 0x21),
        surface: Color32::from_rgb(0x27, 0x27, 0x27),
        surface_hover: Color32::from_rgb(0x3f, 0x3f, 0x3f),
        surface_active: Color32::from_rgb(0x60, 0x60, 0x60),
        text: Color32::from_rgb(0xf1, 0xf1, 0xf1),
        text_secondary: Color32::from_rgb(0xaa, 0xaa, 0xaa),
        text_tertiary: Color32::from_rgb(0x8c, 0x8c, 0x8c),
        divider: Color32::from_rgb(0x3f, 0x3f, 0x3f),
        brand: Color32::from_rgb(0xff, 0x00, 0x00),
        link: Color32::from_rgb(0x3e, 0xa6, 0xff),
        chip: Color32::from_rgb(0x27, 0x27, 0x27),
        chip_active_text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        skeleton: Color32::from_rgb(0x27, 0x27, 0x27),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 170),
    };

    /// Tema claro (`--yt-spec-base-background` #ffffff).
    pub const LIGHT: Palette = Palette {
        bg: Color32::from_rgb(0xff, 0xff, 0xff),
        bg_elevated: Color32::from_rgb(0xff, 0xff, 0xff),
        surface: Color32::from_rgb(0xf2, 0xf2, 0xf2),
        surface_hover: Color32::from_rgb(0xe5, 0xe5, 0xe5),
        surface_active: Color32::from_rgb(0xd9, 0xd9, 0xd9),
        text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        text_secondary: Color32::from_rgb(0x60, 0x60, 0x60),
        text_tertiary: Color32::from_rgb(0x90, 0x90, 0x90),
        divider: Color32::from_rgb(0xe5, 0xe5, 0xe5),
        brand: Color32::from_rgb(0xff, 0x00, 0x00),
        link: Color32::from_rgb(0x06, 0x5f, 0xd6),
        chip: Color32::from_rgb(0xf2, 0xf2, 0xf2),
        chip_active_text: Color32::from_rgb(0x0f, 0x0f, 0x0f),
        skeleton: Color32::from_rgb(0xe9, 0xe9, 0xe9),
        scrim: Color32::from_rgba_premultiplied(0, 0, 0, 120),
    };
}

/// Medidas de la referencia. Todo en puntos lógicos (egui trabaja en puntos,
/// no en píxeles físicos), así que esto vale igual con escalado de Windows.
pub mod metrics {
    /// Alto de la barra superior.
    pub const TOP_BAR: f32 = 56.0;
    /// Ancho de la barra lateral desplegada.
    pub const SIDEBAR: f32 = 240.0;
    /// Ancho de la barra lateral de iconos.
    pub const SIDEBAR_MINI: f32 = 72.0;
    /// Radio de las tarjetas (`--yt-spec-12` en la web).
    pub const CARD_RADIUS: u8 = 12;
    /// Radio de los chips (píldora).
    pub const CHIP_RADIUS: u8 = 8;
    /// Ancho mínimo de tarjeta de rejilla.
    pub const CARD_MIN_WIDTH: f32 = 300.0;
    /// Hueco entre tarjetas.
    pub const GAP: f32 = 16.0;
    /// Alto del minirreproductor.
    pub const MINI_PLAYER: f32 = 72.0;
    /// Ancho del panel de cola.
    pub const QUEUE_PANEL: f32 = 400.0;
}

/// Aplica la paleta al contexto de egui.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = if dark { Palette::DARK } else { Palette::LIGHT };
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.bg_elevated;
    visuals.extreme_bg_color = p.bg;
    visuals.faint_bg_color = p.surface;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.link;
    visuals.selection.bg_fill = p.surface_active;
    visuals.selection.stroke = Stroke::new(1.0, p.text);
    visuals.widgets.noninteractive.bg_fill = p.bg;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.bg_fill = p.surface;
    visuals.widgets.inactive.weak_bg_fill = p.surface;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.bg_fill = p.surface_hover;
    visuals.widgets.hovered.weak_bg_fill = p.surface_hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.bg_fill = p.surface_active;
    visuals.widgets.active.weak_bg_fill = p.surface_active;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.window_stroke = Stroke::new(1.0, p.divider);
    ctx.set_visuals(visuals);

    // egui 0.37 separa los estilos por tema: se mutan todos los que haya para
    // que el cambio se aplique tanto en claro como en oscuro.
    ctx.all_styles_mut(|style| {
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    // Tipografía: la web usa Roboto; aquí se usan las fuentes del sistema para
    // no empaquetar 400 KB de fuente, pero con la misma escala (título 20,
    // cuerpo 14, secundario 12).
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();
    });
}

/// Paleta activa. Se pregunta al tema en curso del contexto.
pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.theme() == egui::Theme::Dark {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

/// Color del "me gusta" activo, el mismo rosado que no cambia entre temas.
pub fn like_active() -> Color32 {
    Color32::from_rgb(0xff, 0x00, 0x3d)
}
