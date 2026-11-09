//! Prueba manual del resolutor: `cargo run --example resolve -- <videoId>...`
//!
//! Comprueba, para cada vídeo: que se resuelva, qué pistas salen y si la URL
//! **sirve el fichero entero** (`Range: bytes=0-` debe devolver 206 con el
//! tamaño completo). Es la prueba que distingue «hay URL» de «se puede
//! reproducir», que es donde se pierden las tardes.

use neotube2::session::SessionStore;
use neotube2::yt::client::Innertube;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let ids: Vec<String> = std::env::args().skip(1).collect();
    let ids = if ids.is_empty() {
        vec!["dQw4w9WgXcQ".to_string()]
    } else {
        ids
    };
    let session = SessionStore::new();
    let api = Innertube::new(session)?;
    let http = reqwest::Client::new();
    let mut fallos = 0;
    for id in ids {
        match api.resolve_stream(&id, None).await {
            Ok(stream) => {
                println!("== {id}");
                println!("   duración: {:?} s, calidad: {}", stream.duration_seconds, stream.best_quality_label());
                for (etiqueta, track) in [
                    ("audio", stream.audio.as_ref()),
                    ("vídeo", stream.video.as_ref()),
                    ("muxado", stream.muxed.as_ref()),
                ] {
                    let Some(t) = track else { continue };
                    let mut req = http.get(&t.url).header("Range", "bytes=0-");
                    for (k, v) in &stream.headers {
                        req = req.header(k, v);
                    }
                    let estado = match req.send().await {
                        Ok(r) => {
                            let status = r.status().as_u16();
                            let len = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                            format!("{status} ({len} B)")
                        }
                        Err(e) => format!("ERROR {e}"),
                    };
                    println!("   {etiqueta:7} itag {:>4} {} {estado}", t.itag, t.mime);
                }
            }
            Err(e) => {
                fallos += 1;
                println!("== {id}: FALLO: {}", e.user_message());
            }
        }
    }
    println!("\n{fallos} fallo(s)");
    Ok(())
}
