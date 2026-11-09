//! Ajustes: calidad, reproducción, sesión, almacenamiento y diagnóstico.
//!
//! Todo lo que se toca aquí se guarda en SQLite en el acto: no hay botón
//! «Guardar». La sección de diagnóstico existe para que un fallo de
//! reproducción se pueda explicar sin abrir una consola.

use egui::{Color32, RichText, Sense, Vec2};

use crate::ui::app::Intent;
use crate::ui::theme::palette;
use crate::ui::widgets::{self, pill_button};

/// Calidades ofrecidas. `None` = la mejor disponible.
pub const QUALITIES: &[(Option<u32>, &str)] = &[
    (None, "Máxima"),
    (Some(2160), "2160p"),
    (Some(1440), "1440p"),
    (Some(1080), "1080p"),
    (Some(720), "720p"),
    (Some(480), "480p"),
    (Some(360), "360p"),
];

#[allow(clippy::too_many_arguments)]
pub fn settings(
    ui: &mut egui::Ui,
    max_height: Option<u32>,
    volume: f32,
    dark: bool,
    thumbs_bytes: &str,
    db_bytes: i64,
    data_dir: &str,
    backend: &str,
    logged_in: bool,
    account_name: &str,
    login_paste: &mut String,
    login_error: Option<&str>,
) -> Option<Intent> {
    let p = palette(ui.ctx());
    let mut intent = None;

    egui::ScrollArea::vertical()
        .id_salt("settings")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(760.0);
            widgets::section_header(ui, "Ajustes", None);

            // --------------------------------------------------- reproducción
            section(ui, "Reproducción");
            ui.label(
                RichText::new("Calidad máxima")
                    .size(14.0)
                    .color(p.text),
            );
            ui.label(
                RichText::new(
                    "Limitar la calidad reduce CPU, GPU y datos. En música no se \
                     decodifica vídeo, así que esto solo afecta a los vídeos.",
                )
                .size(12.0)
                .color(p.text_secondary),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (height, label) in QUALITIES {
                    if widgets::chip(ui, label, *height == max_height).clicked() {
                        intent = Some(Intent::SetMaxHeight(*height));
                    }
                }
            });

            ui.add_space(14.0);
            ui.label(RichText::new(format!("Volumen: {:.0}%", volume)).size(14.0).color(p.text));
            let mut value = volume;
            if ui
                .add(egui::Slider::new(&mut value, 0.0..=100.0).show_value(false))
                .changed()
            {
                intent = Some(Intent::SetVolume(value));
            }

            // ------------------------------------------------------- aspecto
            section(ui, "Aspecto");
            ui.horizontal(|ui| {
                if widgets::chip(ui, "Oscuro", dark).clicked() && !dark {
                    intent = Some(Intent::None);
                    // El cambio de tema se aplica en el siguiente fotograma
                    // desde Ajustes para no reasignar estilos a mitad del
                    // recorrido de widgets.
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::unique("neotube.set_theme"), "dark");
                    });
                }
                if widgets::chip(ui, "Claro", !dark).clicked() && dark {
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::unique("neotube.set_theme"), "light");
                    });
                }
            });

            // ---------------------------------------------------------- cuenta
            section(ui, "Cuenta");
            if logged_in {
                ui.label(
                    RichText::new(if account_name.is_empty() {
                        "Sesión iniciada"
                    } else {
                        account_name
                    })
                    .size(14.0)
                    .color(p.text),
                );
                ui.add_space(6.0);
                if pill_button(ui, "Cerrar sesión", false, false).clicked() {
                    intent = Some(Intent::Logout);
                }
            } else {
                ui.label(
                    RichText::new("Sin sesión: solo se puede ver contenido público.")
                        .size(13.0)
                        .color(p.text_secondary),
                );
                ui.add_space(6.0);
                if pill_button(ui, "Iniciar sesión", true, false).clicked() {
                    intent = Some(Intent::Login);
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Pegar cookies de una sesión de navegador")
                        .size(13.0)
                        .strong()
                        .color(p.text),
                );
                ui.add(
                    egui::TextEdit::multiline(login_paste)
                        .hint_text("SAPISID=…; VISITOR_INFO1_LIVE=…; …")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if let Some(error) = login_error {
                    ui.label(RichText::new(error).size(12.0).color(p.brand));
                }
                ui.add_space(6.0);
                if pill_button(ui, "Usar estas cookies", false, false).clicked() {
                    intent = Some(Intent::LoginWithCookies(login_paste.clone()));
                }
            }

            // --------------------------------------------------- almacenamiento
            section(ui, "Almacenamiento");
            info_row(ui, "Miniaturas en memoria", thumbs_bytes);
            info_row(
                ui,
                "Caché de respuestas",
                &format!("{:.1} KB", db_bytes as f32 / 1024.0),
            );
            info_row(ui, "Datos de la aplicación", data_dir);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if pill_button(ui, "Borrar historial local", false, false).clicked() {
                    intent = Some(Intent::ClearHistory);
                }
            });

            // ------------------------------------------------------ diagnóstico
            section(ui, "Diagnóstico");
            info_row(ui, "Motor de reproducción", backend);
            info_row(
                ui,
                "Ventana de login (WebView)",
                if cfg!(feature = "webview-login") {
                    "disponible"
                } else {
                    "no incluida; usa el pegado de cookies"
                },
            );
            ui.label(
                RichText::new(
                    "Si un vídeo no suena pero la miniatura carga, el problema está en la \
                     resolución del stream o en el motor, no en la red. El registro de la \
                     aplicación (RUST_LOG=debug) lo dice.",
                )
                .size(12.0)
                .color(p.text_secondary),
            );

            // ---------------------------------------------------- atajos
            section(ui, "Atajos");
            for (keys, action) in [
                ("Espacio", "Reproducir / pausar"),
                ("Ctrl + F", "Buscar"),
                ("Alt + ←", "Atrás"),
                ("Alt + →", "Adelante"),
                ("↑ / ↓", "Navegar Shorts"),
            ] {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
                ui.painter().text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    keys,
                    egui::FontId::monospace(12.0),
                    p.text,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 140.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    action,
                    egui::FontId::proportional(13.0),
                    p.text_secondary,
                );
            }

            ui.add_space(20.0);
            ui.label(
                RichText::new("NeoTube 2.0 · cliente independiente, sin relación con YouTube")
                    .size(11.0)
                    .color(p.text_tertiary),
            );
            ui.add_space(30.0);
        });

    intent
}

fn section(ui: &mut egui::Ui, title: &str) {
    let p = palette(ui.ctx());
    ui.add_space(22.0);
    ui.label(RichText::new(title).size(18.0).strong().color(p.text));
    ui.add_space(2.0);
    widgets::divider(ui);
    ui.add_space(10.0);
}

fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let p = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 22.0),
        Sense::hover(),
    );
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        p.text_secondary,
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        value,
        egui::FontId::proportional(13.0),
        p.text,
    );
}

/// Sombra de color para los avisos (no usada aún, reservada para estados).
#[allow(dead_code)]
fn warn_color() -> Color32 {
    Color32::from_rgb(0xff, 0xb3, 0x00)
}
