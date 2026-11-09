//! Comprobación end-to-end del cliente contra YouTube real.
use neotube2::session::SessionStore;
use neotube2::yt::client::Innertube;

#[tokio::main]
async fn main() {
    let api = Innertube::new(SessionStore::new()).unwrap();
    match api.watch("dQw4w9WgXcQ").await {
        Ok(w) => {
            println!(
                "watch: «{}» canal={} rel={} con_miniatura={}",
                w.title,
                w.channel.name,
                w.related.len(),
                w.related.iter().filter(|c| !c.thumbs.is_empty()).count()
            );
            for c in w.related.iter().take(3) {
                println!(
                    "   {} | {} | {} | dur={:?}",
                    c.title, c.author, c.meta, c.duration
                );
            }
        }
        Err(e) => println!("watch: FALLO {e}"),
    }
    match api.shorts_feed("gatos").await {
        Ok(c) => println!(
            "shorts: {} items, primero «{}» id={} miniaturas={}",
            c.len(),
            c[0].title,
            c[0].id,
            c[0].thumbs.0.len()
        ),
        Err(e) => println!("shorts: FALLO {e}"),
    }
    // Álbum: se toma un id real de la búsqueda de música.
    if let Ok(f) = api.music_search("daft punk", None).await {
        let album = f
            .shelves
            .iter()
            .flat_map(|s| s.items.iter())
            .find(|c| c.kind == neotube2::domain::MediaKind::Album);
        if let Some(album) = album {
            println!("álbum de la búsqueda: «{}» id={}", album.title, album.id);
            match api.music_browse(&album.id).await {
                Ok(a) => println!(
                    "  browse álbum: {} estantes, {} pistas",
                    a.shelves.len(),
                    a.shelves.iter().map(|s| s.items.len()).sum::<usize>()
                ),
                Err(e) => println!("  browse álbum: FALLO {e}"),
            }
        } else {
            println!("no se encontró álbum en la búsqueda");
        }
    }
}
