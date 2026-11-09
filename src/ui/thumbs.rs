//! Carga asíncrona de miniaturas con caché de texturas acotada.
//!
//! Tres reglas que son la diferencia entre 90 MB y 600 MB de RAM (PLAN.md §8):
//!
//! 1. Se pide **la miniatura del tamaño que se va a pintar** (`Thumbs::best_for`),
//!    no la máxima disponible.
//! 2. Nada se descarga en el hilo de la interfaz: cada petición va a un runtime
//!    de Tokio y vuelve como mensaje.
//! 3. La caché tiene un presupuesto en bytes y se vacía por lo viejo, así que
//!    navegar durante una hora no hace crecer la memoria.

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use egui::{ColorImage, TextureHandle, TextureOptions};

use crate::domain::Thumbs;

/// Presupuesto de la caché de miniaturas. 48 MB dan para ~200 tarjetas a 480 px
/// de ancho, que es más de lo que se ve en dos pantallas.
const BUDGET_BYTES: usize = 48 * 1024 * 1024;

struct Entry {
    texture: TextureHandle,
    bytes: usize,
    used_at: u64,
    failed: bool,
}

/// Caché de texturas de miniaturas.
#[derive(Default)]
pub struct ThumbCache {
    entries: HashMap<String, Entry>,
    in_flight: std::collections::HashSet<String>,
    clock: u64,
    total_bytes: usize,
}

/// Lo que devuelve el hilo de descarga.
pub struct LoadedThumb {
    pub key: String,
    pub image: Result<ColorImage, String>,
}

impl ThumbCache {
    /// Pide una miniatura y devuelve la textura si ya está lista.
    ///
    /// `want_px` es el ancho en puntos al que se va a pintar: se usa para elegir
    /// la miniatura más pequeña que sirva.
    pub fn get_or_request(
        &mut self,
        ctx: &egui::Context,
        thumbs: &Thumbs,
        want_px: u32,
        sender: &Sender<LoadedThumb>,
        http: &reqwest::Client,
        runtime: &tokio::runtime::Handle,
    ) -> Option<&TextureHandle> {
        let thumb = thumbs.best_for(want_px)?.clone();
        self.clock += 1;
        let key = thumb.url.clone();
        let now = self.clock;

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.used_at = now;
            if entry.failed {
                return None;
            }
            return Some(&entry.texture);
        }

        if self.in_flight.insert(key.clone()) {
            let sender = sender.clone();
            let http = http.clone();
            let url = key.clone();
            // Se pide a 2x el tamaño de presentación para que se vea nítido en
            // pantallas HiDPI sin descargar la imagen de 1280.
            runtime.spawn(async move {
                let image = fetch_and_decode(&http, &url).await;
                let _ = sender.send(LoadedThumb { key, image });
            });
        }
        let _ = ctx;
        None
    }

    /// Incorpora una miniatura descargada. Devuelve si hay que repintar.
    pub fn insert(&mut self, ctx: &egui::Context, loaded: LoadedThumb) -> bool {
        self.in_flight.remove(&loaded.key);
        match loaded.image {
            Ok(image) => {
                let bytes = image.width() * image.height() * 4;
                let texture = ctx.load_texture(
                    format!("thumb:{}", loaded.key),
                    image,
                    TextureOptions::LINEAR,
                );
                self.clock += 1;
                self.total_bytes += bytes;
                self.entries.insert(
                    loaded.key,
                    Entry {
                        texture,
                        bytes,
                        used_at: self.clock,
                        failed: false,
                    },
                );
                self.evict();
                true
            }
            Err(e) => {
                log::debug!("miniatura falló ({}): {e}", loaded.key);
                self.clock += 1;
                self.entries.insert(
                    loaded.key,
                    Entry {
                        texture: ctx.load_texture(
                            "thumb-failed",
                            ColorImage::filled([1, 1], egui::Color32::TRANSPARENT),
                            TextureOptions::NEAREST,
                        ),
                        bytes: 0,
                        used_at: self.clock,
                        failed: true,
                    },
                );
                false
            }
        }
    }

    /// Vacía lo más viejo hasta volver al presupuesto.
    fn evict(&mut self) {
        while self.total_bytes > BUDGET_BYTES {
            let Some(oldest) = self
                .entries
                .iter()
                .filter(|(_, e)| e.bytes > 0)
                .min_by_key(|(_, e)| e.used_at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
            }
        }
    }

    /// Memoria usada por las texturas, para Ajustes.
    pub fn bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
    }
}

/// Descarga y decodifica una imagen en miniatura.
async fn fetch_and_decode(http: &reqwest::Client, url: &str) -> Result<ColorImage, String> {
    let bytes = http
        .get(url)
        .header("Accept", "image/avif,image/webp,image/*")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    // Se decodifica en el hilo de Tokio (bloqueante pero fuera de la interfaz).
    let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

/// `file://` para una ruta local, útil para la portada de un álbum ya cacheado.
pub fn file_uri(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}
