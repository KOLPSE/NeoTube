# NeoTube 2.0 — plan de la versión en curso

Fecha: 9 de noviembre de 2026. Carpeta de trabajo: `D:\Proyectos\neotube2.0`.

Estado: **v2.0 empezada**. El esqueleto está escrito y compila; no es una propuesta. Este
documento sustituye al plan anterior: describe lo que hay, lo que no hay y en qué orden se
termina. Cuando una afirmación se puede comprobar en el código, se dice en qué fichero está.

Nota: los apartados van seguidos (1…11) porque el orden de lectura cambió; entre paréntesis
se conserva el número del plan anterior cuando el apartado viene de allí.

## 1. Estado

### 1.1 Lo que ya funciona en el código

| Pieza | Dónde | Qué hace de verdad |
|---|---|---|
| Modelos | `src/domain.rs` | `Card`, `Shelf`, `Feed`, `WatchInfo`, `ResolvedStream`, `QueueItem`, `Route`, `RepeatMode`. No conocen el JSON de YouTube. |
| Sesión | `src/session.rs` | Cookies filtradas a `youtube.com`, firma `SAPISIDHASH` con `SAPISID` (nunca `APISID`), `session.json` en `%APPDATA%\NeoTube2`. |
| Persistencia | `src/store.rs` | SQLite (`neotube.db`, WAL): ajustes, caché de respuestas ya parseadas, historial local (máx. 200), cola. |
| Transporte | `src/yt/client.rs` | `youtubei/v1` con perfiles `WEB` y `WEB_REMIX`, `visitorData` cacheado 30 min, `ApiError` con clase de fallo. |
| Resolución | `src/yt/stream.rs` | `VISIONOS` anónimo (cliente 101, versión `1.02`) con contexto sacado de la página del vídeo; respaldo `ANDROID_VR`. |
| Operaciones | `src/yt/api.rs` | Inicio, búsqueda, `next`, `like`, `subscription`, música (portada, `browse`, búsqueda), biblioteca, historial, cola de YouTube, Shorts, continuación. |
| Reproductor | `src/playback/engine.rs` | libVLC 3 cargado con `libloading`; vídeo por callbacks de memoria en RV32; audio como pista esclava. |
| Cola | `src/playback/queue.rs` | Orden, aleatorio estable, repetir (no/todo/una), persistencia. |
| Interfaz | `src/ui/**` | eframe/egui 0.37: barra superior, barra lateral, reproductor inferior, cola lateral, una pantalla por destino, tema oscuro/claro. |
| Despachador | `src/ui/tasks.rs` | Único sitio con la política de caché; las pantallas no saben si el dato viene de SQLite o de la red. |
| Miniaturas | `src/ui/thumbs.rs` | Caché de texturas con presupuesto en bytes y desalojo por uso. |

Comprobado hoy: `cargo build` termina (con avisos) y `cargo test --lib` pasa 26 pruebas.

### 1.2 Lo que no está

- **Arranque recién cableado.** `src/main.rs` ya crea `ui::app::App`, pero no se ha
  recorrido la aplicación completa: abrir, buscar, reproducir y cola.
- Comentarios: `WatchInfo` no guarda ni un comentario. Chat de directos, tampoco.
- Directos: `resolve_stream` solo mira `formats` y `adaptiveFormats`; un directo llega como
  `hlsManifestUrl` y acaba en «no ofreció ninguna pista reproducible».
- Subtítulos, cambio de pista de audio, cambio de calidad en caliente, velocidad, medición
  de RAM/CPU (no hay ninguna cifra de la aplicación tomada todavía).
- Modo música real: el motor lo soporta (`Engine::open(..., has_video=false)`), pero
  `App::play_stream` elige la pista de vídeo siempre que exista, así que hoy un vídeo
  musical se decodifica como vídeo.
- Paginación en la interfaz: `TaskKind::More` y el `append` del `Payload::Feed` existen;
  ninguna pantalla los dispara todavía.
- «Ver más tarde», edición de listas, notificaciones, SMTC/MPRIS, bandeja, atajos globales,
  actualizadores, instaladores, subida y Studio.

### 1.3 Deuda conocida (cosas que el código ya tiene mal)

1. `src/yt/client.rs` mantiene `ANDROID_VR` 1.65.10 como respaldo y `playback_headers()`
   devuelve el `User-Agent` de `ANDROID_VR` aunque la URL venga de `VISIONOS`. Hay que
   probar con qué UA abre VLC las URLs de Vision Pro antes de fiarse del respaldo.
2. Las versiones de cliente (`WEB_CLIENT_VERSION`, `REMIX_CLIENT_VERSION`) están fijadas a
   mano; la sonda documentó que caducan cada pocos días y que lo correcto es leer
   `INNERTUBE_CLIENT_VERSION` de la página.
3. El login por WebView está detrás de `#[cfg(feature = "webview-login")]`, feature **no
   declarada en `Cargo.toml`**: el botón siempre contesta «pega la cabecera Cookie:». Lo que
   funciona es pegar la cabecera/`cookies.txt` o leer las cookies de Firefox (`ui/login.rs`).
4. En `src/ui/tasks.rs`, la rama `Browse` no musical llama a `music_browse` dos veces (la
   segunda como «respaldo»): falta la llamada `browse` del host web para canales.
5. `Store::cache_get` recibe un `ttl` y lo ignora (`let _ = ttl;`): la caducidad efectiva es
   la guardada al escribir. `Capability` (`src/domain.rs`) no lo usa ninguna pantalla.

## 2. Alcance de esta primera versión

Lo que se considera «2.0 usable en el escritorio», sin prometer paridad con la web (§3 del
plan anterior).

| Función | ¿Dentro? | Nota |
|---|---|---|
| Inicio con chips y rejilla | Sí | Anónimo llega vacío (ver §9.2); con sesión es el camino real. |
| Búsqueda con filtros y resultados mezclados | Sí | Filtros: todo, vídeos, canales, listas, directos. |
| Página de vídeo | Sí | Título, canal, vistas, fecha, descripción, capítulos, relacionados, like, suscribir. |
| Reproductor integrado | Sí | Ventana única, sin ventana hija; vídeo + audio DASH. |
| Cola, aleatorio, repetir | Sí | Local, persistente, no se escribe en la cuenta. |
| Shorts | Sí, limitado | Feed desde búsqueda; sin `reel_watch_sequence` (necesita `sequenceParams` de una shelf). |
| Música: portada, álbum, artista, lista, búsqueda | Sí | `WEB_REMIX`; funciona anónimo. |
| Biblioteca e historial de la cuenta | Sí, con sesión | Tres `browse` distintos para listas, álbumes y artistas. |
| Historial local | Sí | 200 entradas, en SQLite, borrable. |
| Ajustes | Sí | Calidad máxima, volumen, tema, tamaño de caché, diagnóstico del reproductor. |
| Iniciar sesión | Sí, manual | Pegar cookies o leer Firefox. WebView sin compilar. |
| Comentarios y respuestas | No | El panel viene en `next`, pero no se pinta. |
| Directos y chat | No | Falta HLS y el cliente de chat. |
| Subtítulos | No | — |
| Subida, edición, Studio | No | Fuera de alcance. |
| Instaladores y actualizaciones | No | Solo `cargo run` / binario de `target/`. |
| Linux y macOS | Sin verificar | Hay rutas en `engine.rs`; nada probado fuera de Windows. |

### 2.1 Estados de capacidad

Cuatro estados, y ningún botón que mienta: **funciona** (probado de punta a punta),
**con limitaciones** (bajo una condición conocida), **requiere web** (solo vía navegador) y
**no disponible** (sin mecanismo verificado).

## 3. Arquitectura real

Corresponde a `src/lib.rs` (§5 del plan anterior). Regla de oro que se cumple en todo el
árbol: **la interfaz no ve JSON de YouTube ni nombres de renderers**.

| Módulo | Ficheros | Responsabilidad | Estado |
|---|---|---|---|
| `domain` | `src/domain.rs` | Modelos de la aplicación; `Thumbs::best_for` elige la miniatura por tamaño de pintado. | Completo para lo que hay. |
| `session` | `src/session.rs` | Cookies, `SAPISIDHASH`, sesión persistida. | Completo. |
| `store` | `src/store.rs` | SQLite: ajustes, caché, historial, cola. | Completo; falta límite de tamaño. |
| `yt::client` | `src/yt/client.rs` | Transporte Innertube y perfiles de cliente. | `VISIONOS` vive en `stream`; `ANDROID_VR` sobra como respaldo por defecto. |
| `yt::parse` | `src/yt/parse.rs` | Recorrido por nombre de renderer, sin rutas absolutas. | Probado con fixtures literales en los tests. |
| `yt::api` | `src/yt/api.rs` | Una función por pantalla o acción. | Cobertura de lo que hay en §2. |
| `yt::stream` | `src/yt/stream.rs` | Resolución de pistas y contexto de la página. | Completo para `VISIONOS`. |
| `playback::engine` | `src/playback/engine.rs` | libVLC: carga, medio, callbacks, audio esclavo, volumen. | Completo; sin subtítulos ni HLS. |
| `playback::queue` | `src/playback/queue.rs` | Orden, aleatorio, repetir, persistencia. | Completo. |
| `ui::app` | `src/ui/app.rs` | Estado, navegación, ejecución de `Intent`, bucle de fotograma, un solo `Engine`. | Escrito y cableado desde `main.rs`. |
| `ui::screens` | `src/ui/screens/*.rs` | Una función por destino; reciben estado y devuelven `Intent`. | Ocho módulos escritos. |
| `ui::widgets` | `src/ui/widgets.rs` | Tarjeta, fila, chip, icono, esqueleto, barra de progreso, paneles de error/vacío. | Completo. |
| `ui::thumbs` | `src/ui/thumbs.rs` | Caché acotada y asíncrona de miniaturas. | Completo. |
| `ui::theme` | `src/ui/theme.rs` | Paleta y medidas tomadas de youtube.com. | Completo. |
| `ui::tasks` | `src/ui/tasks.rs`, `src/ui/actions.rs` | Despachador, TTLs de caché, invalidaciones, `TaskKind`/`Payload`/generaciones. | Completo; falta disparador de «más». |
| `ui::login` | `src/ui/login.rs` | Login: WebView (no compilada), pegado de cookies, lectura de Firefox. | Parcial por diseño (§1.3). |

### 3.1 Reglas que se respetan

- Red y reproducción fuera del hilo de interfaz: runtime de Tokio con 2 hilos de trabajo y
  canal `std::sync::mpsc` para devolver resultados; respuestas obsoletas descartadas por
  generación de tarea (`TaskIds` en `ui/actions.rs`).
- Un solo reproductor: `App::ensure_engine` crea uno y `Engine::open` para el medio anterior.
- La cola local se distingue de las listas de la cuenta; nunca se escribe de vuelta.
- Los errores se muestran como frases (`ApiError::user_message`), no como códigos.

### 3.2 Pendiente de arquitectura

Probar el arranque de punta a punta; un disparador de «más» que no duplique el merge de
feeds; sustituir el respaldo `ANDROID_VR` por un segundo cliente verificado.

## 4. Integración con YouTube

Sigue siendo el riesgo principal (§4 del plan anterior). Todo lo de aquí está medido hoy
contra YouTube en vivo, en Windows, y no se cambia por lo que diga la documentación de
terceros.

### 4.1 Tres perfiles de cliente

| Perfil | Host y endpoints | Para qué | Autenticación | Estado |
|---|---|---|---|---|
| `WEB` | `www.youtube.com`: `browse`, `search`, `next`, `like/*`, `subscription/*` | Inicio, búsqueda, página de vídeo, acciones de cuenta | Cookies + `SAPISIDHASH` solo para acciones de cuenta | Funciona; `FEwhat_to_watch` anónimo llega vacío |
| `WEB_REMIX` | `music.youtube.com`: `browse`, `search`, `next` | Catálogo de YouTube Music: portada, álbum, artista, lista, búsqueda, historial, cola | Anónimo para catálogo; sesión para biblioteca e historial | Funciona |
| `VISIONOS` | `www.youtube.com/youtubei/v1/player` | **Solo** resolver streams | Anónimo, con cookies de la visita y `signatureTimestamp` | Es la única vía verificada que sirve el fichero completo |

Por qué tres y no uno: no son intercambiables. `WEB` no devuelve `streamingData`
reproducible (contesta `UNPLAYABLE`; necesitaría `poToken` y ejecutar el JavaScript del
reproductor), `WEB_REMIX` no sirve el vídeo general, y el cliente que sirve el fichero
entero no expone la experiencia de navegación.

### 4.2 Por qué `VISIONOS` y no `ANDROID_VR`

`ANDROID_VR` fue el resolutor heredado. Hoy **ya no sirve para reproducir de verdad**:

- Con `clientVersion` 1.65.10 + `visitorData` + anónimo, googlevideo sirve solo una
  **ventana** de cada fichero: el audio responde 200 hasta ~1.000.000 bytes (a partir de
  1,1 M, 403) y el vídeo hasta ~30 % de `contentLength` (en un clip de 8:59: 20 M bien,
  30 M → 403). `Range: bytes=0-` (abierto) da 403 en cuanto pasaría del tope, y el itag 18
  (muxado 360p) da 403 siempre.
- `ANDROID_VR` 1.62.27 **sí** sirve el fichero completo, pero YouTube contesta
  `LOGIN_REQUIRED` («confirma que no eres un bot») a la mayoría de vídeos por esa vía.

`VISIONOS` (cliente 101, versión `1.02`, `deviceMake Apple`, `deviceModel
RealityDevice17,1`, `osName visionOS`, `osVersion 26.5.23O471`, UA de Safari) sirve el
fichero entero: comprobado con `206` y `contentLength` completo en vídeo 2160p y en audio.
Es el mismo cliente que usa yt-dlp 2026.10.26.

### 4.3 Contexto de la página

`VISIONOS` necesita dos cosas que no van en el cuerpo del `player` y que se sacan del HTML
de `/watch` (`Innertube::video_page_context` y `parse_watch_page`, en `src/yt/stream.rs`):
las cookies de la visita (`VISITOR_INFO1_LIVE`, `__Secure-YNID`, `YSC`…) leídas de
`set-cookie`, y el `signatureTimestamp` (`sts`) del `ytcfg`. Sin ellas, `ERROR` o
`UNPLAYABLE`. Todo va **anónimo**: nunca se manda la sesión de la cuenta. Decir «soy un
visor de Vision Pro» y adjuntar una sesión web hace que googlevideo emita URLs que luego
rechaza al reproducir.

### 4.4 Lección sobre versiones de cliente

Una `clientVersion` inventada devuelve 400 o `UNPLAYABLE`. Tiene que ser real o salir de
`INNERTUBE_CLIENT_VERSION` de la página. Un perfil nuevo se prueba en `tools/probe.mjs`
antes de tocar la aplicación.

## 5. Reproductor

### 5.1 Por qué libVLC y no libmpv

El plan anterior decía libmpv (§5). Se descartó por una razón comprobada: las compilaciones
de mpv para Windows publican `libmpv-2.dll` pero no una `libmpv.lib` con la que enlazar;
habría que generar los stubs a mano o depender de un fork. VLC 3.0.23 instala `libvlc.dll`
en una ruta estable, su API pública es antigua y estable, y decodifica con FFmpeg integrado,
que es lo que hace falta para Opus/VP9 y H.264.

### 5.2 Cómo se pinta el vídeo

No hay ventana hija de VLC (`set_hwnd`). Se piden **callbacks de memoria**
(`libvlc_video_set_format_callbacks` + `libvlc_video_set_callbacks`) en `RV32` (BGRA) y cada
fotograma se sube a una textura de egui:

- dos búferes (`front`/`back`): VLC escribe en el de atrás mientras la interfaz lee el de
  delante, para que redimensionar no parta fotogramas;
- conversión BGRA→RGBA una vez por fotograma (dos bytes por píxel) y textura de egui; los
  controles superpuestos son widgets normales porque no hay segunda ventana.

### 5.3 Audio y vídeo separados

YouTube sirve audio y vídeo en pistas separadas. El vídeo se abre como medio principal y el
audio se añade como **esclavo** con `libvlc_media_player_add_slave` (tipo 1), que es como VLC
sincroniza un segundo flujo contra el primero. En modo audio (sin pista de vídeo) se abre
directamente y se añade `:no-video`.

`libvlc_media_player_set_audio_track` **no existe** en libVLC 3 (`GetProcAddress` devuelve
127): no se usa ni se debe mencionar como solución.

### 5.4 Windows: lo que costó y queda documentado

- `libvlc.dll` se carga con `LoadLibraryExW` + `LOAD_WITH_ALTERED_SEARCH_PATH`
  (`libloading::os::windows::Library::load_with_flags`). Sin esa bandera, su dependencia
  `libvlccore.dll`, que está al lado, no se encuentra y `LoadLibrary` falla aunque el
  fichero exista.
- `--ipv4` ya no es una opción de instancia válida en 3.0.23. Va como opción de medio
  (`:ipv4`). El síntoma de no forzarla, en equipos con IPv6 anunciada y sin salida real, es
  «TLS handshake: Resource temporarily unavailable» y el medio no abre.
- Las peticiones a googlevideo necesitan cabecera `Range` (`rqh=1` va en la URL). El motor
  no reescribe la URL: se comprueba en `examples/resolve.rs`, que pide `Range: bytes=0-` y
  exige 206.

### 5.5 Qué está medido y qué no

Medido: resolución con `VISIONOS`, `206` con `contentLength` completo (2160p y audio), y
reproducción real con `examples/play.rs`, que imprime estado, posición, duración, tamaño de
fotograma y número de fotogramas recibidos. **No medido**: RAM, CPU y GPU de la aplicación,
fotogramas perdidos, tiempo de arranque.

### 5.6 Pendiente en el reproductor

Comprobar con qué `User-Agent` abre VLC las URLs de `VISIONOS` (y corregir
`playback_headers()`); renovar el stream al caducar (`ResolvedStream::is_fresh` existe y
nadie lo llama); directos (HLS), subtítulos, cambio de calidad sin cerrar el medio.

## 6. Dirección visual y criterios de fidelidad (§7)

Referencia: youtube.com de escritorio. La reconstrucción se hace desde esa referencia, no
portando el diseño de la versión Flutter.

| Pantalla | Estado |
|---|---|
| Inicio | Chips con scroll horizontal, rejilla adaptable (`CARD_MIN_WIDTH` 300), esqueletos. |
| Vídeo | Reproductor, título, canal y acciones, descripción con capítulos, relacionados a la derecha. |
| Shorts | Marco 9:16 centrado, acciones laterales, navegación con rueda/flechas/botones. |
| Canal | Banner, avatar, pestañas (Inicio, Vídeos, Shorts, Directos, Listas) sobre el mismo feed. |
| Búsqueda | Barra de filtros y resultados mezclados. |
| Biblioteca | Cabecera y carruseles de la cuenta. |
| Música | Cabecera grande de álbum/artista/lista y listas de pistas. |
| Ajustes | Secciones con chips y sliders; sin botón «Guardar». |

Medidas y colores en `src/ui/theme.rs`: barra superior 56, barra lateral 240 (mini 72),
radio de tarjeta 12, chips 8, hueco 16, reproductor inferior 72, cola 400; fondo oscuro
`#0f0f0f` y claro `#ffffff` como la hoja pública de YouTube.

Criterios de fidelidad (los mismos del plan anterior, resumidos):

1. Capturas de referencia fechadas: idioma, tema, estado de sesión y tamaño de ventana.
2. Tipografía, espaciados, colores, iconos, densidad y navegación documentados y comparados
   con la web en los mismos tamaños.
3. Teclado, foco visible, nombres accesibles, escalado de Windows y ventanas estrechas.
4. Identidad propia de NeoTube: no se copia marca ni logotipo.

## 7. Rendimiento y consumo (§8)

Objetivo: consumo bajo, acotado y verificable. No se promete una cifra sin medirla.

### 7.1 Presupuestos y cómo se aplican

| Presupuesto | Valor | Dónde se aplica |
|---|---|---|
| Miniaturas en memoria | 48 MB | `BUDGET_BYTES` en `src/ui/thumbs.rs`; desalojo por `used_at` más antiguo al insertar. |
| Tamaño de miniatura | el del pintado, a 2x por HiDPI | `Thumbs::best_for` (tolerancia 1.3x) y `ThumbCache::get_or_request`. |
| Descargas duplicadas | ninguna | `in_flight: HashSet` en `thumbs.rs`; las que fallan se marcan para no reintentar en bucle. |
| Reproductor y fotogramas | uno y uno | `App::ensure_engine`; `Engine::open` para el medio anterior; el búfer y la textura se sustituyen, no se acumulan. |
| Repintado | 16 ms reproduciendo, 250 ms en reposo | `impl eframe::App::ui` en `app.rs`, según ruta y estado del reproductor. |
| Caché de respuestas | TTL por pantalla | `ttl` en `src/ui/tasks.rs`: inicio 10 min, búsqueda 15, música 30, vídeo 5, browse 60, biblioteca 10. |
| Invalidación | por acción | `like` limpia biblioteca, «me gusta» (`VLLM`, `FEmusic_liked*`) y portada musical; `subscribe` limpia inicio; login/logout limpian todo. |
| Historial local | 200 filas | `Store::record_history` recorta al insertar. |

### 7.2 Lo que aún no tiene presupuesto

- Virtualización de rejillas: `card_grid` pinta un `egui::Grid` completo dentro de un
  `ScrollArea`; con feeds largos crece el trabajo por fotograma. Caché en disco: sin límite
  ni purga por tamaño (`cache_size` solo se muestra en Ajustes).
- Presupuesto de la aplicación (RAM/CPU): sin medir, por tanto sin número.
- Modo música: el ahorro está en el motor, pero la interfaz no lo activa (§1.2).

## 8. Fases (§9)

| Fase | Trabajo | Estado |
|---|---|---|
| 0. Inventario | Funciones, datos, integración y dependencias de la versión Flutter | **Hecha**: `docs/PORTING_CONTRACT.md` y `docs/API_PROBE.md`. |
| 1. Prototipo técnico | Ventana Rust, rejilla con miniaturas, vídeo real, controles superpuestos | **Hecha en forma de esqueleto**: pantallas y motor escritos; falta la prueba de conjunto. |
| 2. Núcleo Rust | Sesión, clientes, modelos, caché, persistencia | **Hecha en esqueleto**: `session`, `store`, `yt`, `domain` con pruebas. |
| 3. Paridad musical | Canciones, álbumes, artistas, biblioteca, likes, cola | **Parcial**: catálogo y biblioteca sí; guardar/quitar de la biblioteca y radio no. |
| 4. Arranque y YouTube principal | Arranque, inicio, búsqueda, canal, página de vídeo, paginación | En curso: el arranque ya está cableado; falta probarlo. |
| 5. Shorts y comunidad | `reel_watch_sequence`, comentarios, respuestas, suscripciones | Pendiente. |
| 6. Directos y funciones avanzadas | HLS, chat, subtítulos, notificaciones, multimedia del sistema | Pendiente. |
| 7. Distribución | Instaladores, actualizaciones, pruebas prolongadas, Linux | Pendiente. |

### 8.1 Siguiente hito

Que `cargo run` abra la aplicación completa y se pueda ver un vídeo de principio a fin con
la cola funcionando. Condición de salida: `examples/resolve.rs` da 206 en un vídeo de más de
una hora, `examples/play.rs` avanza de posición sin error y no se retiene el fotograma
anterior al cambiar de vídeo.

## 9. Riesgos y matriz de viabilidad por función (§12)

### 9.1 Riesgos técnicos

| Riesgo | Estado | Respuesta |
|---|---|---|
| Cambios en la API interna | Constante | Perfiles aislados en `yt::client`; fixtures y pruebas de parseo. |
| `VISIONOS` deja de servir el fichero completo o cambian las versiones de cliente | Vigilar; ocurre cada pocos días | Sonda en `tools/probe.mjs` antes de tocar la app; leer la versión de la página (§1.3). |
| URLs que caducan a mitad | Conocido (~6 h, ligadas a IP) | `ResolvedStream::is_fresh` y re-resolución (sin cablear). |
| Caché que crece sin freno | Abierto | Límite por tamaño y purga por antigüedad. |
| Diferencias Windows/Linux y restricciones de distribución | Sin verificar / sin resolver | Probar el motor en Linux antes de anunciar soporte; separar descarga de streams y decidir el modelo antes de publicar. |

### 9.2 Matriz por función

Evidencia = código (`src/…`), documento local (`docs/…`) o prueba en vivo de hoy.

| Función | Mecanismo | ¿Sesión? | Estado | Evidencia |
|---|---|---|---|---|
| Inicio | `browse(FEwhat_to_watch)` WEB | Sí para feed útil | Con limitaciones | Anónimo llega solo con aviso (`API_PROBE` §2, fixture `home.json` de 5 KB); `api::home` cae a `FEtrending`, que también falla anónimo. |
| Búsqueda | `search` WEB, `params` por filtro | No | Funciona | `API_PROBE` §1, fixture `search_video.json`; `api::search`. |
| Vídeo / relacionados | `next` WEB | No | Funciona | `API_PROBE` §3, fixture `watch_next.json`; `api::watch` y `watch_from_json` (título, canal, vistas, descripción, capítulos, `lockupViewModel`). |
| Música: portada y búsqueda | `browse(FEmusic_home)`, `search` REMIX | No | Funciona | `API_PROBE` §4–5, fixtures `music_home.json`, `music_search.json`; `api::music_*`. |
| Música: álbum, artista, lista | `browse(browseId)` REMIX | No | Funciona | `api::music_browse`; ids `MPREb_`, `UC…`, `OLAK5uy_`. |
| Shorts | `search("#shorts …")` → `gridShelfViewModel` | No | Con limitaciones | `API_PROBE` §6: `reelShelfRenderer` no existe y `reel_watch_sequence` exige `sequenceParams`; `api::shorts_feed` los saca de búsqueda y deduplica. |
| Biblioteca | Tres `browse` REMIX (listas, álbumes, artistas) | Sí | Con limitaciones | `api::library`; sin sesión devuelve `NeedsLogin`. Falta «me gusta» de pistas y guardado. |
| Historial de la cuenta | `browse(FEmusic_history)` | Sí | Con limitaciones | `api::music_history`; si YouTube no lo devuelve, error explicable (no lista vacía). |
| Me gusta | `like/like`, `like/removelike` | Sí | Con limitaciones | `api::set_like`; probado en la versión anterior (`PORTING_CONTRACT` §2). Solo vídeos; guardar álbum necesita `OLAK5uy_`, no `MPREb_`. |
| Suscribirse | `subscription/subscribe` + `unsubscribe` | Sí | Con limitaciones | `api::set_subscribed`, con reintento en la forma larga; no se comprueba el estado devuelto. |
| Iniciar sesión | Cookies pegadas o de Firefox; WebView no compilada | — | Con limitaciones | `ui/login.rs`; `session.json` filtrado a `youtube.com`. |
| Comentarios | `next` ya trae el panel y su `continuation` | Sí (para responder) | No disponible en el cliente | `API_PROBE` §3; `WatchInfo` no guarda comentarios. Factible sin endpoint nuevo. |
| Chat de directos | `live_chat` | Sí | No disponible | No hay sonda hecha; no se ofrece el botón. |
| Directos | `hlsManifestUrl` | No | No disponible | `API_PROBE` §7; `resolve_stream` no maneja HLS. |
| Subidas / Studio | — | — | No disponible | Fuera de alcance. |
| Guardar en «Ver más tarde» | `playlist` REMIX | Sí | No disponible | Sin implementar. |

### 9.3 Cómo se mantiene esta matriz

Cada fila se actualiza con una prueba nueva (sonda o prueba manual), no cuando cambia una
web. Si una función pasa a «con limitaciones», se anota fecha y síntoma; si deja de
funcionar, se retira el botón antes de dejar el estado escrito.

## 10. Validación y pruebas (§11)

### 10.1 Automáticas

`cargo test --lib` (26 pruebas, hoy en verde). Por módulo:

- `domain` (`Thumbs`): elección de miniatura por tamaño.
- `session`: firma con `SAPISID` y no `APISID`; descarte de cookies de `.google.com`; cabecera pegada. Igual en `ui::login` con Netscape.
- `store`: ajustes de ida y vuelta; caché que caduca; historial que recorta y ordena; cola persistente.
- `yt`: desescapado de `visitorData` y locale del sistema (`client`); renderers por nombre, sin descender dos veces, duración/directo y álbum frente a canción (`parse`); mejor audio por bitrate, techo de calidad con preferencia por H.264 y extracción de `visitorData`/`sts` del HTML (`stream`); capítulos, estantes sin duplicados y metadatos de un vídeo (`api`).
- `playback::queue`: siguiente/anterior, repetir cola, repetir una, aleatorio que cubre todo.

### 10.2 Manuales

| Comando | Qué demuestra |
|---|---|
| `cargo run --example resolve -- <videoId>…` | Que hay pistas y que la URL **sirve el fichero entero** (`Range: bytes=0-` → 206 con tamaño completo): separa «hay URL» de «se puede reproducir». |
| `cargo run --example play -- <videoId> [segundos]` | Que VLC abre, reproduce, avanza de posición y entrega fotogramas del tamaño declarado. |
| `node tools/probe.mjs all` | Estado actual de la API interna; regenera fixtures y JSONL de evidencia. |

### 10.3 Huecos de validación

- Los fixtures de `tests/fixtures/` los genera la sonda, pero **ninguna prueba los lee**:
  solo se citan en comentarios. Falta una prueba que parsee cada fixture.
- No hay comparación automática de capturas contra youtube.com, ni medición de RAM/CPU, ni
  prueba de sesión real en esta máquina, ni prueba de reproducción de más de un minuto (así
  se detectó la ventana de `ANDROID_VR`).

## 11. Fuentes técnicas (§14)

- API interna de YouTube (`youtubei/v1`): sin contrato público. Lo que se sabe está en
  `docs/API_PROBE.md` (sonda propia, 9 de noviembre de 2026) y `docs/PORTING_CONTRACT.md`
  (comportamiento de la versión Flutter).
- yt-dlp 2026.10.26, como referencia del cliente `VISIONOS` y de sus parámetros.
- libVLC 3.0.23, API pública de carga dinámica:
  <https://www.videolan.org/developers/vlc/doc/doxygen/html/group__libvlc.html>
- eframe/egui 0.37: <https://docs.rs/egui/0.37.0/egui/> · SQLite (rusqlite `bundled`):
  <https://www.sqlite.org/docs.html>
- Políticas de los servicios de API de YouTube:
  <https://developers.google.com/youtube/terms/developer-policies>
- Sonda y evidencia reproducible: `tools/probe.mjs`, `tests/fixtures/*.json`.

Se revisan al cerrar cada fase y siempre que una función cambie de estado en §9.2.
