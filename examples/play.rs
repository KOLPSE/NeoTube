//! Prueba de reproducción sin interfaz: `cargo run --example play -- <videoId> [segundos]`
//!
//! Resuelve el vídeo, abre el motor, reproduce y va imprimiendo posición,
//! estado y tamaño de fotograma. Sirve para distinguir "la URL se descarga"
//! de "VLC la reproduce", que no es lo mismo.

use neotube2::playback::engine::{Engine, State};
use neotube2::session::SessionStore;
use neotube2::yt::client::Innertube;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "dQw4w9WgXcQ".into());
    let secs: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6.0);

    let api = Innertube::new(SessionStore::new())?;
    let stream = api.resolve_stream(&id, None).await?;
    println!(
        "resuelto {} ({:?} s) calidad {}",
        stream.video_id, stream.duration_seconds, stream.best_quality_label()
    );
    let url = stream
        .video
        .as_ref()
        .map(|v| v.url.clone())
        .or_else(|| stream.muxed.as_ref().map(|m| m.url.clone()))
        .or_else(|| stream.audio.as_ref().map(|a| a.url.clone()))
        .ok_or_else(|| anyhow::anyhow!("sin pista"))?;
    let slave = stream.audio.as_ref().map(|a| a.url.clone());

    let engine = Engine::new(false)?;
    println!("backend: {}", neotube2::playback::engine::backend_info());
    engine.open(&url, slave.as_deref(), &stream.headers, stream.video.is_some())?;
    engine.set_volume(50);
    engine.play()?;

    let started = std::time::Instant::now();
    let mut frames = 0;
    let mut last_pos = -1;
    let buffer = engine.buffer();
    while started.elapsed().as_secs_f32() < secs {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let state = engine.refresh_state();
        if let Some(_frame) = buffer.next_frame() {
            frames += 1;
        }
        let pos = engine.position_ms();
        if pos != last_pos {
            let (w, h) = buffer.dimensions();
            println!(
                "  {:.1}s estado={state:?} pos={}ms dur={}ms vídeo={}x{} fotogramas={frames}",
                started.elapsed().as_secs_f32(),
                pos,
                engine.duration_ms(),
                w,
                h
            );
            last_pos = pos;
        }
        if state == State::Error {
            anyhow::bail!("VLC informó de un error de reproducción");
        }
    }
    engine.stop();
    if last_pos <= 0 {
        anyhow::bail!("no avanzó la posición: no está reproduciendo");
    }
    println!("OK: reprodujo hasta {last_pos} ms con {frames} fotogramas de vídeo");
    Ok(())
}
