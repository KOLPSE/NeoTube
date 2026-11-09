# NeoTube 2.0

Cliente nativo de escritorio de YouTube y YouTube Music, escrito en Rust. Sin
Electron, sin WebView navegando, sin HTML para la interfaz.

Es la reescritura de [NeoTube 1.x](https://github.com/KOLPSE/NeoTube) (Flutter +
media_kit): la misma idea —tu cuenta, tus listas y tus canciones, con el aspecto
de youtube.com— con un consumo menor y una interfaz que dibuja ella misma.

## Estado

Versión **2.0.0**, primera entrega funcional. Lo que hay funciona con datos
reales de YouTube; lo que no está, está listado en [`PLAN.md`](PLAN.md) y no se
anuncia como si estuviera.

| Área | Estado |
|---|---|
| Inicio, búsqueda con filtros, canales, página de vídeo con relacionados | funciona |
| Reproducción de vídeo (hasta 4K, según equipo) y de solo audio | funciona |
| Shorts con navegación vertical | funciona |
| YouTube Music: portada, álbumes, artistas, listas | funciona |
| Cola: orden, aleatorio, repetir, persistencia | funciona |
| Biblioteca e historial de la cuenta | requiere iniciar sesión |
| Me gusta y suscripciones | requiere iniciar sesión |
| Comentarios, chat de directos, subidas, Studio | **no implementado** |

## Requisitos

- **VLC 3.x instalado.** NeoTube usa `libVLC` para decodificar y no reescribe un
  motor multimedia. En Windows se busca en `C:\Program Files\VideoLAN\VLC`; si
  está en otro sitio, define `NEOTUBE_VLC_DIR` apuntando a la carpeta que
  contiene `libvlc.dll`.
- Windows 10/11 o Linux con las bibliotecas de ventana habituales (X11/Wayland).
- Para compilar: Rust estable (probado con 1.97).

## Compilar y ejecutar

```powershell
cargo run --release
```

Pruebas (no necesitan red ni VLC):

```powershell
cargo test
```

Comprobaciones contra YouTube real, útiles al depurar:

```powershell
# Resuelve vídeos y comprueba que la URL sirve el fichero ENTERO (no solo que exista)
cargo run --example resolve -- dQw4w9WgXcQ

# Reproduce de verdad (necesita VLC): posición, estado y fotogramas por segundo
cargo run --example play -- dQw4w9WgXcQ 10

# Recorre las pantallas (inicio, búsqueda, música, vídeo, shorts, álbum)
cargo run --example probe_home
```

## Iniciar sesión

NeoTube no usa OAuth: las recomendaciones, la biblioteca y el historial solo
existen dentro de youtube.com, así que se guarda la sesión real del navegador.

- **Pegar cookies** (siempre disponible): en Ajustes → Cuenta, pega la cabecera
  `Cookie:` de una sesión de youtube.com, o el `cookies.txt` de una extensión.
  Hace falta que incluya `SAPISID`.
- **WebView** (opcional): `cargo run --features webview-login` compila el login
  con WebView2/WebKitGTK. La ventana se abre solo para eso y se cierra al
  terminar.

Las cookies se guardan en `%APPDATA%\NeoTube2\session.json` y solo se envían a
dominios de YouTube. Cerrar sesión borra el fichero.

## Cómo está hecho

```
src/
  domain.rs            modelos de la aplicación (no conocen el JSON de YouTube)
  session.rs           cookies + firma SAPISIDHASH + persistencia
  store.rs             SQLite: ajustes, caché de respuestas, historial, cola
  yt/
    client.rs          transporte youtubei/v1 (perfiles WEB, WEB_REMIX, VISIONOS)
    parse.rs           renderers de YouTube -> modelos
    api.rs             una función por pantalla/acción
    stream.rs          resolución de streams reproducibles
  playback/
    engine.rs          libVLC: vídeo en memoria (RV32) + audio como esclavo
    queue.rs           cola, aleatorio, repetir
  ui/                  egui 0.37: pantallas, widgets, tema, miniaturas, tareas
```

Tres decisiones que explican el resto:

1. **El vídeo se pinta desde memoria.** libVLC entrega cada fotograma por
   callbacks (`RV32`, BGRA), se convierte a RGBA y se sube a una textura de egui.
   No hay ventana hija del reproductor: los controles son widgets normales y el
   escalado lo lleva la interfaz.
2. **Vídeo y audio vienen separados.** YouTube sirve pistas DASH distintas; el
   audio se engancha como pista esclava del vídeo. En modo música no se
   decodifica vídeo en absoluto.
3. **La caché guarda modelos, no JSON.** Si mañana cambia el parser, lo guardado
   sigue sirviendo; y el presupuesto de miniaturas (48 MB) es lo que impide que
   navegar una hora haga crecer la memoria.

## Documentación

- [`PLAN.md`](PLAN.md) — alcance, fases, riesgos y matriz de viabilidad por función.
- [`docs/PORTING_CONTRACT.md`](docs/PORTING_CONTRACT.md) — lo que se portó de la
  versión Flutter, con referencia a fichero y línea: cuerpos de petición,
  cabeceras, firmas, rutas de parseo y fallos documentados.
- [`docs/API_PROBE.md`](docs/API_PROBE.md) — lo que responde YouTube hoy, medido:
  endpoints, clientes y los límites encontrados.

## Licencia

Ver [`LICENSE`](LICENSE).
