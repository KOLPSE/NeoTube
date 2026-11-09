//! Comprueba que la ventana de login se abre y que captura cookies.
//!
//! Uso: `cargo run --example login_probe --features webview-login`
//!
//! Abre la ventana real de Google. Si el usuario entra, al llegar a
//! music.youtube.com se imprimen los NOMBRES de las cookies capturadas (nunca
//! sus valores, que son credenciales).

fn main() {
    let (tx, rx) = std::sync::mpsc::channel();
    neotube2::ui::login::spawn_login(tx);
    println!("ventana abierta; esperando (máx. 5 min)…");
    match rx.recv_timeout(std::time::Duration::from_secs(310)) {
        Ok(outcome) => match outcome {
            neotube2::ui::login::LoginOutcome::Cookies(list) => {
                let cookies = neotube2::session::Cookies::from_raw(list);
                println!("cookies de youtube: {}", cookies.len());
                println!("¿tiene con qué firmar?: {}", cookies.signing_value().is_some());
            }
            neotube2::ui::login::LoginOutcome::Cancelled => println!("cancelado por el usuario"),
            neotube2::ui::login::LoginOutcome::Failed(e) => println!("FALLO: {e}"),
        },
        Err(_) => println!("sin respuesta (tiempo agotado)"),
    }
}
