//! Punto de entrada de NeoTube 2.0.
//!
//! Solo monta el contexto de egui y arranca la aplicación. Todo lo demás está en
//! la biblioteca (`src/lib.rs`) para poder probarlo sin abrir una ventana:
//! `cargo test` cubre parsers, cola, sesión y almacenamiento.

// En Windows, las compilaciones de desarrollo abren una consola; en release se
// oculta para que el programa se comporte como una aplicación de escritorio.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use neotube2::ui::app::App;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("neotube2=info,warn"),
    )
    .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1040.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("NeoTube 2.0"),
        ..Default::default()
    };

    eframe::run_native(
        "NeoTube 2.0",
        options,
        Box::new(|cc| {
            // El constructor devuelve `Result` porque abrir la base de datos o
            // crear el cliente HTTP puede fallar; si falla, la ventana no se
            // abre y el error se ve en la consola en lugar de una ventana vacía.
            match App::new(cc) {
                Ok(app) => Ok(Box::new(app)),
                Err(e) => {
                    // `anyhow::Error` no implementa `std::error::Error` para
                    // poder convertirse en `Box<dyn Error>` de eframe, así que
                    // se pasa como error de cadena ya formateado con su cadena
                    // de causas.
                    let mensaje = format!("no se pudo iniciar NeoTube: {e:#}");
                    log::error!("{mensaje}");
                    Err(mensaje.into())
                }
            }
        }),
    )
}
