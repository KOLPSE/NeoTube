# NeoTube — cómo funciona todo esto

Este código sale de **NeoFy** (el cliente de Spotify, `KOLPSE/NeoFy`), donde NeoTube era el
*segundo modo completo* de la misma app: su propia sesión, su propia biblioteca y su propio
reproductor, no una pestaña dentro del otro. Aquí se ha separado para que sea una aplicación
por su cuenta.

Este documento es el contexto que **no se deduce leyendo el código**: por qué cada decisión
rara es la correcta, y qué pasa si se «arregla» por lo obvio. Casi todo lo de abajo está
verificado contra la API real con `tool/probe_yt.dart`, no deducido de documentación —
porque documentación no hay.

> **Estado de este repositorio:** arranca de verdad, en Windows y en Linux. Tiene
> instalador de Windows (Inno Setup) y paquete + repositorio pacman para Arch Linux,
> publicados en [Releases](https://github.com/KOLPSE/NeoTube/releases). La sección
> [Cómo se compila y se publica](#9-cómo-se-compila-y-se-publica) explica el proceso
> completo, incluido `sudo pacman -S neotube-bin`.

---

## 1. La idea en una frase

YouTube Music no tiene API pública para terceros. NeoTube habla con **la misma API interna
que usa la web** (`youtubei/v1`), autenticándose con **las cookies de una sesión de verdad**,
y reproduce el audio pidiéndole la URL del stream **a esa misma API** y sonándola con
**media_kit** (libmpv). No hay servidor propio, no hay clave de API, no hay OAuth, y —desde
que resolver dejó de ser un subproceso— tampoco hace falta ejecutar JavaScript de nadie.

```
   WebView de login          youtubei/v1            youtubei/v1/player      media_kit
  (una sola vez) ─────► cookies + SAPISIDHASH ─────► URL del stream ─────► altavoces
     yt_auth.dart            yt_music_api.dart         yt_stream.dart      yt_player.dart
```

Cuatro piezas, cuatro ficheros. Todo lo demás es interfaz.

---

## 2. Mapa del código

### El núcleo (`lib/core/`)

| Fichero | Qué es |
|---|---|
| `yt_auth.dart` | La sesión. Abre la WebView, captura las cookies y firma cada petición. |
| `yt_music_api.dart` | El cliente de `youtubei/v1`: buscar, portada, biblioteca, listas, radios. 762 líneas de las cuales la mitad son parseo defensivo, y hay motivo para cada rama. |
| `yt_models.dart` | `YtTrack`, `YtItem`, `YtSection`, `YtColeccion`, `YtTipo`. |
| `yt_player.dart` | La cola y el sonido: libmpv, y el plan B de yt-dlp. |
| `yt_stream.dart` | Resuelve la URL del stream por `youtubei/v1/player`. Dart puro, sin Flutter dentro. |
| `yt_local_proxy.dart` | Retransmite esa URL en `127.0.0.1`, troceada, porque libmpv no puede pedirla de un tirón. Ver §5. |
| `yt_home_store.dart` | Estado de una pantalla de secciones (portada, explorar, biblioteca). |
| `discord_rpc.dart` | Rich Presence por IPC nativo de Discord. Apagado por defecto. |
| `yt_sesion.dart` | La cola de la vez pasada, en `sesion.json`. **No reproduce nada al abrir** — ver §5. |

### La interfaz (`lib/ui/`)

| Fichero | Qué es |
|---|---|
| `neotube_shell.dart` | El armazón: barra lateral, vistas, barra de reproducción abajo. Es el punto de entrada de la app. |
| `yt_browse_screen.dart` | Portada, Explorar y Biblioteca — las tres son «secciones con tarjetas», así que las tres son esta pantalla. |
| `yt_search_screen.dart` | Buscador. |
| `yt_playlist_screen.dart` | Una lista o un álbum abiertos, con el corazón que lo guarda en la biblioteca. |
| `yt_artista_screen.dart` | La página de un artista: cabecera, más escuchadas (como filas, no como tarjetas) y sus carruseles. |
| `yt_login_screen.dart` | La pantalla que lanza la WebView. |
| `yt_acciones.dart` | Qué pasa al pulsar una tarjeta. Está aparte porque lo comparten las tres pantallas. |
| `yt_ajustes.dart` | El bloque de Ajustes propio: versión y ruta de yt-dlp. |

### Infraestructura que viene de NeoFy y es genérica

`app_config.dart` (configuración en disco), `art_cache.dart` + `art_image.dart` (caché de
carátulas), `settings.dart`, `updater.dart`, `resource_monitor.dart`, `procesos.dart`,
`settings_dialog.dart`, `tira_horizontal.dart`, `atajos.dart`.

Están tal cual salieron de NeoFy y **todavía hablan de NeoFy**: `updater.dart` apunta a las
releases de `KOLPSE/NeoFy`, `app_config.dart` guarda `clientId` de Spotify y `volumenNeoTube`,
y `settings_dialog.dart` trae un widget `ReiniciarAudioDeNeoFy` que aquí no pinta nada
(reinicia librespot, que no existe en esta app). Hay que pasarles la mano.

### Lo que sobra ya

`app_mode.dart`, `mode_host.dart` y `mode_toggle_text.dart` son **el conmutador entre los dos
modos**. Aquí no hay dos modos. Vienen porque son literalmente la pieza que se estaba
retirando de NeoFy y porque `settings.dart` todavía importa `app_mode.dart`, pero en cuanto
se limpie esa importación se pueden borrar los tres.

---

## 3. La sesión: cookies, no OAuth

### Por qué una WebView y no un registro de app en Google

Se intentó primero el camino «limpio» (proyecto en Google Cloud, OAuth). Funciona, pero
obliga a **cada usuario** a crear su propio proyecto antes de poder entrar. El requisito era
«te logueas y ya», como en Metrolist o InnerTune. La única forma de conseguirlo es dejar que
Google pinte su propia pantalla de login en una ventana de verdad y quedarse con las cookies.

La WebView (`desktop_webview_window`: WebView2 en Windows, WebKitGTK en Linux) se usa **solo
para esa pantalla**. Una vez capturada la sesión no se vuelve a abrir un navegador en toda la
vida de la app; el resto es Flutter nativo.

### ⚠️ `APISID` y `SAPISID` son cookies distintas, y firmar con la primera te deja anónimo

Ésta es **la trampa más cara de todo el proyecto**. La API se autentica con la cabecera
`Cookie` más una firma `SAPISIDHASH` calculada sobre la cookie `SAPISID`.

Buscar esa cookie por sufijo (`name.endsWith('APISID')`) parece razonable para tolerar las
variantes modernas `__Secure-1PAPISID` / `__Secure-3PAPISID`. Pero **`APISID` a secas también
acaba en `APISID`**, es una cookie *distinta*, con otro valor, y llega **antes** en la lista
que devuelve la WebView. Resultado: se firmaba con el valor equivocado.

Y así es como falla, que es lo peor de todo: **Google no contesta 401 ni 403. Contesta 200 con
la sesión anónima.** La portada llegaba llena —de recomendaciones genéricas para nadie— y todo
parecía ir bien. Lo único que delataba el problema era la biblioteca, que traía un
`messageRenderer` diciendo «Inicia sesión para escuchar tus canciones favoritas» donde
deberían estar tus playlists.

Por eso `YtAuth._nombresDeFirma` es una **lista cerrada y ordenada**, nunca una búsqueda por
sufijo. Y por eso la comprobación de «¿el login ha terminado?» mira exactamente la misma
cookie que se va a usar para firmar: darlo por bueno con una cookie que luego no sirve es
justo cómo se llegaba a una sesión que parecía iniciada y no lo estaba.

**Cómo se verifica:** `tool/probe_yt.dart` contra la cuenta real. Firmando con `APISID`,
`FEmusic_liked_playlists` devuelve el mensaje de «inicia sesión»; firmando con `SAPISID`,
devuelve las playlists.

### La sonda se puede ejecutar con la app abierta

`tool/probe_yt.dart` **solo lee**: no renueva ni rota nada. (Ojo si vienes de NeoFy, donde las
sondas de Spotify sí rotan el refresh token y hay que cerrar la app antes de lanzarlas.)

---

## 4. La API interna: el catálogo de trampas

Todo lo de esta sección está comprobado contra la cuenta real. Cada punto es un fallo que ya
ocurrió.

- **`gridRenderer` guarda sus elementos en `items`; todo lo demás, en `contents`.** La
  biblioteca entera (`FEmusic_liked_playlists`, `FEmusic_liked_albums`…) llega dentro de un
  `gridRenderer`. Un parseo que solo mire `contents` ve cero elementos y pinta la pantalla
  vacía **sin un solo error de por medio**.

- **Una lista no se reproduce por `videoId`, porque no tiene.** Las tarjetas de listas y
  álbumes traen un `browseId` (`VL…` listas, `MPREb_…` álbumes, `UC…` artistas) con el que hay
  que pedir sus pistas *después*. Aplanarlo todo a «pista sin `videoId`» es lo que dejaba
  únicamente canciones sueltas reproducibles. El `playlistId` que quiere `browse` es el
  `browseId` **sin el prefijo `VL`**: con el prefijo puesto dos veces, 400.

- **Las mezclas y radios de la portada solo responden por `next`, no por `browse`.** Sus ids
  empiezan por `RD` y `browse` sobre `VLRD…` falla. `next` es el endpoint que usa el propio
  reproductor web para llenar su cola y traga casi cualquier id: es el camino principal para
  las mezclas y el plan B para todo lo demás.

- **El `playlistId` de una mezcla no está en la tarjeta, sino en su botón de play.** Hay que
  bajar hasta
  `thumbnailOverlay.musicItemThumbnailOverlayRenderer.content.musicPlayButtonRenderer.playNavigationEndpoint.watchPlaylistEndpoint`.

- **Las listas y los álbumes llegan en `twoColumnBrowseResultsRenderer`, y la pestaña solo
  trae la cabecera.** Las pistas están en `secondaryContents`. Quedarse con la pestaña y dar la
  lista por vacía es el error fácil aquí.

- **La cabecera de una lista *propia* va envuelta.** Si la puedes editar llega dentro de
  `musicEditablePlaylistDetailHeaderRenderer`; las ajenas y «Música que me gusta» llegan como
  `musicResponsiveHeaderRenderer`; los álbumes, todavía como `musicDetailHeaderRenderer`
  colgando de la raíz. Son tres formas del mismo dato.

- **Una lista larga llega en trozos de 100, con dos formatos de continuación vivos a la vez.**
  El moderno manda el token en el cuerpo y contesta con `onResponseReceivedActions`; el
  antiguo lo manda por la URL (`ctoken`/`continuation`/`type=next`) y contesta con
  `continuationContents`. Se prueban los dos porque los dos aparecen.

- **La biblioteca no es un `browseId`, son cuatro.** Playlists, álbumes, canciones y artistas
  viven en endpoints distintos y se piden en paralelo. Por eso `YtHomeStore` recibe *cómo*
  cargar (una función) y no un `browseId`: meter esa excepción dentro del store era peor.

- **La primera tarjeta de la rejilla de biblioteca es el botón «Nueva lista».** Tiene título y
  carátula, pero no lleva a ningún sitio. Se filtra por `YtItem.tieneDestino`.

- **El buscador con el filtro de «solo canciones» no puede devolver listas.** Visto así es una
  obviedad, pero pasarle los `params` de canciones era justo lo que impedía llegar a una
  playlist desde la búsqueda.

- **Los artistas de tu biblioteca llegan como `MPLAUC…`, no como `UC…`.** La pestaña
  `FEmusic_library_corpus_track_artists` antepone `MPLA` al id del canal. Reconocer solo `UC`
  dejaba esas tarjetas en `YtTipo.desconocido` y —como sí traen `browseId`— acababan
  intentando abrirse **como si fueran una lista**, con el mensaje «No hay lista que abrir en
  este elemento» encima de una tarjeta que decía tener canciones. Los dos ids responden, pero
  **no lo mismo**: `MPLAUC…` devuelve solo lo que tú tienes en biblioteca de ese artista;
  `UC…` devuelve la página completa (más escuchadas, álbumes, singles, vídeos). Por eso
  `YtMusicApi.browseIdDeArtista` le quita el `MPLA` antes de pedirla.

- **Un álbum se abre por `MPREb_…` y se guarda por `OLAK5uy_…`, y no son el mismo id.** Para
  meter un álbum o una lista ajenos en la biblioteca no hay endpoint propio: es el **mismo**
  `like/like` de las canciones cambiando `videoId` por `playlistId` en el `target`. El id que
  hay que mandar sale del `toggleButtonRenderer` de la cabecera
  (`buttons[].toggleButtonRenderer.defaultServiceEndpoint.likeEndpoint.target.playlistId`), y
  ⚠️ **mandar el `browseId` en su lugar no da error: contesta 200 y no guarda nada.**

  Ese mismo botón trae `isToggled`, que es **la única forma barata de saber si ya está
  guardado**: no hay una lista de "álbumes guardados" que consultar de golpe como sí la hay
  para los "me gusta" de canciones (`VLLM`). De ahí que `YtColeccionesGuardadas` no cargue
  nada al arrancar y se vaya enterando según se abren colecciones.

- **Los `params` de los filtros de búsqueda son opacos y hay que copiarlos.** Son protobuf
  serializado y urlencodeado; no se construyen. Están en `YtFiltroBusqueda`, comprobados uno a
  uno contra la cuenta real. El de listas devuelve «Listas de la comunidad» — las destacadas
  van en otro `params` distinto.

- **Las pistas de un álbum no traen miniatura, pero las de una lista sí.** Ya estaba anotado en
  `YtTrack.conMiniatura`, y es lo que hace que pintar la carátula en cada fila de
  `YtPlaylistScreen` funcione en los dos casos: en una lista cada pista trae la suya, y en un
  álbum heredan la portada de la cabecera.

### ⚠️ El tamaño de una carátula no lo decide la API, lo decide la URL

La biblioteca y las canciones de la búsqueda traen **como mucho 120 px** de miniatura (dos
escalones: 60 y 120), y la app las pinta en tarjetas de 132 px lógicos — 264 reales en una
pantalla HiDPI. El resultado era una carátula borrosa donde debería haber una portada.

Lo que no es evidente: el `=w120-h120-l90-rj` del final de esas URLs **no describe la imagen,
es una petición de reescalado** que `googleusercontent` atiende al vuelo. La misma URL con
`=w544-h544-l90-rj` devuelve 200 y 62 KB frente a los 4,4 KB de la de 120 — comprobado contra
el servidor real. `urlDeCaratulaEscalada` (`ui/art_image.dart`) reescribe ese token al tamaño
que de verdad se va a pintar, cuantizado en escalones fijos porque **la URL es la clave de la
caché de disco** y pedir 132 aquí y 133 allí serían dos descargas del mismo JPEG.

Dos cosas que no se pueden tocar ahí: se conserva la proporción (el banner de un artista llega
como `=w540-h225` y forzarlo a cuadrado lo deforma), y las miniaturas de vídeo de `i.ytimg.com`
se devuelven **intactas** — no entienden esos parámetros, y añadírselos cambia una imagen
borrosa por un 404.

Y dos trampas más de `ArtImage`, las dos aprendidas rompiéndolo:

- **`cacheWidth` sin `cacheHeight` pixela las carátulas cuadradas.** Muchas miniaturas de
  YouTube son 16:9. Descodificando solo por el ancho salen 132 × 74 para una tarjeta de 132, y
  entonces `BoxFit.cover` tiene que **ampliar 1,8×** para llenar el cuadrado. Por eso se pasan
  las dos medidas en las cajas cuadradas y solo el ancho en la apaisada del banner, donde la
  imagen ya viene apaisada y `cover` se limita a recortar.

- **`cacheWidth` no puede ser el tamaño al que se pinta si eso pasa del original.** El banner
  maximizado ocupa unos 3840 px reales: pedir esa descodificación son ~24 MB de bitmap por
  imagen, **ampliando** un JPEG de 1280 —ni un píxel de nitidez de más—, y al maximizar se
  encadenan varias. `anchoDeDecodificacion` lo capa al escalón que de verdad se ha descargado.

---

## 5. El sonido: `youtubei/v1/player` + libmpv (y yt-dlp de plan B)

### Los clientes móviles devuelven URLs sin cifrar, y por eso no hace falta ejecutar JavaScript

Resolver la URL del stream es **una llamada más a la API interna** (`yt_stream.dart`), no un
subproceso. El detalle del que depende todo:

| cliente | formatos de audio | qué hace falta para abrirlos |
|---|---|---|
| `WEB_REMIX` (el del resto de la app) | 4, **todos en `signatureCipher`** | ejecutar el JS del reproductor de YouTube |
| `ANDROID_VR` | 5, **todos con `url` directa** | nada |

Pedirle `player` a `WEB_REMIX` devuelve 200 y unos formatos de aspecto perfectamente normal;
lo que no traen es una URL que se pueda abrir. Descifrarlos es lo que obliga a arrastrar un
runtime de JavaScript (Deno, en el caso de yt-dlp) o un DOM falso más un PoToken de BotGuard
(es lo que hace `pear-desktop` con `happy-dom` + `bgutils-js`). Con un cliente móvil no hay
nada que descifrar: ni `signatureCipher`, ni parámetro `n`.

Medido en Windows sobre la misma pista: **~120 ms de mediana contra los ~2700 ms de yt-dlp**.
Esa diferencia era el "se queda pensando" entre canciones.

### ⚠️ Que la URL se pueda descargar no significa que libmpv la acepte

**La trampa más cara de esta parte.** Se eligió primero el cliente `IOS`: respondía igual de
rápido, con URLs igual de directas, y se comprobó además bajando los primeros megas por HTTP
—llegaban a 14 MB/s—. Y aun así **no sonaba absolutamente nada**.

googlevideo le contesta `403 Forbidden` a ffmpeg con las URLs de `IOS`, mientras le da `206` a
una petición normal **desde el mismo proceso, con la misma URL y en el mismo segundo**. Con
`ANDROID_VR` (el que acaba usando yt-dlp — mira el `c=` de sus URLs) suena.

La consecuencia práctica, que vale para cualquier cambio futuro aquí: **una URL de audio no se
valida descargándola.** Hay que abrirla con el reproductor de verdad. La única forma de verlo
es `flutter run` con `MPVLogLevel.debug` en el constructor del `Player` y mirar el log de mpv;
por HTTP todo parecía correcto, y el `Failed to open` de `media_kit` no dice el motivo.

### ⚠️ El `403` no era de `IOS`: era de pedir el fichero de un tirón — y esto es SABR

Meses después volvió el mismo síntoma con `ANDROID_VR`, la única vez que parecía imposible:
`flutter run` con `MPVLogLevel.debug` mostraba `ffmpeg: https: HTTP error 403 Forbidden` contra
la misma IP de CDN, con firmas frescas, en el 100% de los intentos — no ocasional, sistemático.

Se aisló variable a variable con `dart:io HttpClient` puro (mismo cliente HTTP que ffmpeg usa
por debajo, sin `package:http` de por medio): cabeceras, identidad de visitante y cliente
(`ANDROID_VR`/`IOS`) daban igual. Lo único que cambiaba el resultado era **el tamaño del
`Range` pedido**. Con la misma URL, la misma sesión, la misma prueba:

| `Range` pedido | resultado |
|---|---|
| `bytes=0-900000` | `206`, sirve |
| `bytes=0-1000000` | `403` |
| `bytes=900000-1799999`, con una **URL recién resuelta y nunca usada** | `403` igual |
| `bytes=0-` (lo que pide libmpv al abrir, sin más) | `403` |

**googlevideo corta en 1.000.000 de bytes exactos, y es un tope de posición, no un cupo que se
gasta**: una URL fresca pidiendo directamente la "segunda mitad" falla igual que la que ya venía
de servir la primera. libmpv, al abrir un stream nuevo, pide `Range: bytes=0-` —el fichero
entero de un golpe—, así que siempre cae del lado malo del corte.

**Y yt-dlp, con su cliente por defecto, tropieza exactamente igual** — probado a pelo, sin nada
de este código de por medio: mismo vídeo, mismo `403`, ni siquiera baja el primer byte. Encaja
con lo que yt-dlp llama SABR (*Server Ad-Behavior Reporting*, github.com/yt-dlp/yt-dlp#12482):
YouTube está sustituyendo la descarga progresiva por rangos por un protocolo servido a medida, y
`ANDROID_VR` sin ese protocolo solo sirve el primer tramo — probablemente pensado como margen
para que el reproductor arranque, no como un fallo a explotar.

De los clientes de yt-dlp probados uno por uno (`ios`, `tv`, `tv_simply`, `web`, `web_safari`,
`mweb`, `android`, `web_creator`, `web_music`…), el único que **descargó el fichero entero** fue
`WEB_EMBEDDED_PLAYER`. Pero pedir esa URL desde `dart:io HttpClient` —con las cabeceras exactas
que manda yt-dlp, copiadas con `--print-traffic`— **también da `403`**: esa URL solo la sirve un
cliente con la huella TLS de un navegador de verdad (`curl_cffi`, la librería que usa yt-dlp por
debajo para imitar el *fingerprint* TLS de Chrome). Dart no la puede replicar. Así que la única
vía que de verdad consigue el fichero completo es dejar que **yt-dlp haga la descarga entera**
(`_descargarCompletaConYtDlp` en `yt_player.dart`, con `-o <fichero>` y
`--extractor-args youtube:player_client=web_embedded`), no solo resolver su URL con `-g`.

### ⚠️ Por qué el arreglo no empalma bytes, aunque parezca lo obvio

El primer intento fue trocear también la descarga de repuesto y, al chocar con el corte,
**servir el resto en la misma respuesta HTTP** (`yt_local_proxy.dart` pedía el trozo que
faltaba a la URL de `WEB_EMBEDDED_PLAYER` y lo pegaba a continuación de lo que ya venía de
`ANDROID_VR`). El tamaño cuadraba exacto —hasta el último byte— y aun así la canción se cortaba
igual, unos segundos más tarde, sin ningún error de libmpv.

La razón: son **dos descargas independientes** del mismo audio, y un contenedor como WebM no es
una tira plana de bytes — está organizado en *clusters* que el códec coloca donde le conviene a
él, no en el byte 900 000 (un número elegido aquí, sin relación con esa estructura). Empalmar
ahí cae casi siempre a mitad de un bloque. mpv no lanza un error al toparse con eso: el
demuxer simplemente deja de encontrar datos válidos y da la pista por terminada en silencio —
exactamente el mismo síntoma que el bug original, solo que corregido en apariencia y roto igual
por debajo.

### La solución de verdad: reabrir en un fichero aparte y saltar, no empalmar

`yt_local_proxy.dart` solo sirve la vía rápida troceada en 900 000 bytes (con `dart:io
HttpClient`, que transmite según llega en vez de cargar la respuesta entera como
`package:http`) y, si un trozo posterior al primero falla, **avisa con un `onCorte` y corta
limpio** — sin `Content-Length` declarado de antemano (así una respuesta corta es válida, no
rota) y sin intentar rellenar el resto él mismo.

Quien reacciona es `YtPlayer`: en paralelo, desde que arranca la pista, ya está descargando el
fichero entero por yt-dlp (`_descargarCompletaConYtDlp`, deja el resultado en
`_continuaciones`). Cuando `mpv.stream.completed` dispara y coincide con el aviso de corte
(`_pistaConCorte`), en vez de saltar a la siguiente pista, `_continuarTrasCorte` **reabre** el
reproductor apuntando al fichero ya descargado y salta a la posición donde iba con
`Media(archivo, start: posicion)` — dos ficheros de verdad sí se pueden encadenar así, sin
corrupción. Se nota un parón de bien menos de un segundo en ese punto (parar, cargar el fichero
nuevo, saltar), pero la canción sigue en vez de saltar a la siguiente. Los ficheros descargados
se guardan en `Directory.systemTemp` y se limitan a los dos últimos (la pista actual y la
anterior); de tres en adelante se van borrando solos.

Si mañana vuelve a fallar algo con este aspecto: **medir el tamaño del trozo y comprobar en qué
posición corta antes que sospechar del cliente o de las cabeceras** — ya se investigó por ahí
una vez y no era eso. Y si se te ocurre "empalmar en la misma conexión", ya se probó: no
funciona, y el porqué está arriba.

### ⚠️ `ANDROID_VR` sin `visitorData` es "confirma que no eres un bot"

Y aquí está el segundo escalón, que el primero tapaba. Con el cliente correcto pero sin
`visitorData`, la API contesta `LOGIN_REQUIRED` con el motivo *"Inicia sesión para confirmar
que no eres un bot"* — **y lo hace tenga o no tenga las cookies de la cuenta delante**, así que
el mensaje engaña: no le falta tu sesión, le falta saber que hay un visitante detrás.

Medido: **0 de 4 pistas sin `visitorData`, 4 de 4 con él.** Añadir las cabeceras de yt-dlp
(`X-Youtube-Client-Name: 28`, `X-Youtube-Client-Version`, `Origin`) y copiar su descripción de
cliente exacta **no basta por sí solo**: sigue siendo 0 de 4. El `visitorData` es la pieza.

Se saca como lo saca yt-dlp: de la página de un vídeo cualquiera, que lo trae incrustado en su
JavaScript. Se cachea para toda la vida del proceso y solo se vuelve a pedir si Google lo
rechaza, en cuyo caso se reintenta una vez antes de caer al plan B.

**Cómo se averiguó, y cómo averiguar el siguiente:** `yt-dlp --print-traffic` vuelca la
petición entera que manda —cabeceras, cuerpo y todo—. yt-dlp ya tiene resuelto este problema;
cuando algo aquí deje de funcionar, ese comando es el primer sitio donde mirar, no el último.

### ⚠️ Esta petición va anónima, y mandar la sesión rompe la reproducción

**Lo más contraintuitivo de todo el proyecto.** `youtubei/v1/player` se pide **sin `Cookie`,
sin `Authorization` y sin `SAPISIDHASH`** — es lo único de la app que no se autentica, y no es
un descuido.

Mandar las cookies de la cuenta **resuelve perfectamente**: 200, formatos con `url` directa,
mismos ~120 ms. Pero la URL que devuelve viene envenenada, y googlevideo le contesta
`403 Forbidden` a ffmpeg al abrirla. Al parecer, decir «soy una app de Oculus Quest» y
adjuntar a la vez una sesión web de escritorio es una contradicción que Google acepta al
emitir la URL y castiga al servirla. yt-dlp tampoco manda cookies con los clientes móviles.

Medido, con el resto del código idéntico:

| | resoluciones | fallos de libmpv | reintentos por yt-dlp |
|---|---|---|---|
| **con** cookies de cuenta | todas OK | **todas** | 6 de 6, a 2,6 s cada una |
| **sin** cookies de cuenta | todas OK | **0** | **0** |

Perder la sesión aquí no cuesta nada: reproducir no depende de la cuenta. `yt_music_api.dart`
sigue firmando todo lo demás, que es donde importa saber quién eres.

Por eso esta petición **no reutiliza `_postBytes()` de `yt_music_api.dart`**: aquella firma
siempre y manda `Origin` y `X-Origin`, y aquí las tres cosas sobran o hacen daño. (Si alguna
vez hiciera falta firmar algo contra `www.youtube.com`, el `SAPISIDHASH` va con origen
`https://music.youtube.com` igualmente; firmarlo con `www` es `400 INVALID_ARGUMENT`.)

### `yt_stream.dart` es Dart puro, y tiene que seguir siéndolo

No importa `yt_auth.dart` (que arrastraría `desktop_webview_window`) ni
`package:flutter/...`: recibe las cabeceras de sesión como **función**
(`YtAuth.cabecerasDeSesion`). Con un plugin de Flutter en el árbol de imports `dart run` no
compila, y sin `dart run` este fichero se queda sin sonda — que es la única forma de comprobar
que YouTube no ha cambiado nada, porque `flutter test` no puede hacer red.

Es función y no valor porque el `SAPISIDHASH` lleva dentro el epoch en que se calculó:
guardarlo sería firmar todas las pistas de la sesión con la hora del arranque.

### yt-dlp sigue estando, ahora como plan B

Se cae a él cuando la vía rápida falla, salvo si el fallo es del vídeo y no del método
(`YtStreamException.reintentarConYtDlp`): con un vídeo retirado o privado, lanzar el proceso
solo sirve para esperar tres segundos y volver a fallar. `YtPlayer.findYtDlpBinary()` lo busca
en tres sitios, por orden:

1. junto al ejecutable (lo que se empaqueta),
2. `tool/ytdlp-build/bin/` (el árbol de desarrollo),
3. **el PATH**.

Lo tercero es una diferencia deliberada respecto a los sidecars de NeoFy: yt-dlp es el mismo
binario que empaqueta cualquier distribución, así que el del usuario sirve igual y **suele
estar más fresco**.

- **No se compila, se baja** (`tool/fetch_ytdlp.ps1` / `.sh`), y se rebaja **en cada
  empaquetado, sin cachear**: YouTube le rompe los extractores cada pocas semanas y el de la
  release anterior puede estar ya muerto.
- **En Linux se empaqueta `yt-dlp_linux`, el binario autónomo (~40 MB), no el *zipapp*
  (3 MB).** El zipapp necesita `python3 >= 3.9` en el sistema, y el problema no era el tamaño:
  era que la app dejaba de reproducir en cuanto el intérprete no estaba o era viejo — una
  dependencia que el usuario tiene que entender y resolver antes de que la app le sirva. El
  autónomo trae su propio Python dentro. **Si algún día se vuelve al zipapp, hay que devolver
  la dependencia `python3` a los paquetes.**
- El empaquetado no se limita a comprobar que el fichero está: ejecuta **`yt-dlp --version`**.
  Un binario presente pero roto existe, es ejecutable y no arranca, y eso sin la comprobación
  solo se descubre pulsando una canción en una copia ya instalada. El contenedor de Arch donde
  se comprueba **no lleva `python` instalado a propósito**: es lo que demuestra que el binario
  es de verdad autónomo.
- Ajustes enseña la versión y la ruta del yt-dlp que se está usando (`yt_ajustes.dart`), por lo
  mismo.

Las URLs resueltas se cachean **45 minutos** (`YtPlayer._vidaDeUrl`): caducan solas (traen su
propio `expire`, unas 6 h), así que guardarlas más tiempo es guardarse un fallo para luego.

La siguiente pista se sigue resolviendo mientras suena la actual, aunque ahora cueste ~120 ms:
es barato, y quita el silencio entre pistas también cuando toca caer al plan B.

### La sesión que se recupera pero no se reproduce sola

`yt_sesion.dart` guarda en `sesion.json` la cola, el índice, la posición y los modos (aleatorio
y repetición): al cambiar de pista, cada 20 s —lo que cambia sin avisar es la **posición**, y
es justo lo que hace falta si la app se cierra de malas maneras— y en `_quit`, ⚠️ **antes de
`player.stop()`**, que vacía la cola: guardar después es guardar que no había nada sonando.

Al abrir, `YtPlayer.restaurar` deja la cola puesta y la pista seleccionada **sin abrir el
audio** (`_pendienteDeAbrir`). El primer `alternar`/`resume` —el botón de play de la app, el
del panel del sistema o el de MPRIS, los tres— es el que abre de verdad y salta a la posición
guardada, pasándola como `Media(start:)` y no como un `seek` posterior: un seek justo después
de `open` se pierde si el medio aún no ha terminado de abrirse.

Que no suene sola es deliberado, y por dos motivos: una app que se pone a sonar al arrancar es
una app que hay que correr a silenciar, y además resolver el stream cuesta una petición a
YouTube por cada arranque, la escuche alguien o no. Mientras nada se ha abierto, `posicion` y
`duracion` devuelven lo guardado en vez de los ceros de libmpv, para que la barra de abajo
enseñe dónde te quedaste desde el primer frame.

No vive en `config.json` porque aquello son **preferencias** (se escriben cuando el usuario
decide algo) y esto es un *snapshot* que se reescribe solo y puede ocupar cientos de veces
más: mezclarlos haría que un cierre a mitad de escritura se llevara por delante los ajustes.

### ⚠️ En Windows, maximizar la ventana cerraba la app — y la culpa es de la accesibilidad

El síntoma: **la app desaparece al maximizar**, en cualquier pantalla. Sin excepción de Dart,
sin stack, sin una línea en el log. Pasaba también en la 1.0.0 publicada, así que no es de la
interfaz nueva.

Lo que lo delata no está en el log de Flutter sino en el **visor de eventos de Windows**
(`Application Error`): `neotube.exe`, código `0xc0000005` —violación de acceso—, módulo
`flutter_windows.dll`. Es decir, revienta el *embedder*, no el código Dart; por eso no queda
rastro por el lado de Dart.

La causa está a la vista en el log, pero parecía ruido inofensivo: desde el primer frame se
repite

```
[ERROR:...accessibility_bridge.cc(114)] Failed to update ui::AXTree, error:
Nodes left pending by the update: 5 6 7 8 9 10 11 12 13 14 15 16 17 20
```

El puente de accesibilidad no consigue construir el árbol de esta app. Con el árbol en ese
estado, en cuanto Windows pide eventos de accesibilidad —y maximizar pide varios— el motor los
despacha sobre nodos que no existen.

El rodeo es un `ExcludeSemantics` sobre la raíz, en `main.dart`. **No se pierde accesibilidad
real**: el árbol nunca llegaba a construirse, así que un lector de pantalla ya no recibía nada;
lo que se quita es un árbol roto que además mataba la app.

**Cómo se comprueba, sin tener que maximizar a mano:** se lanza el ejecutable y se le manda
`ShowWindow(hwnd, SW_SHOWMAXIMIZED)` desde PowerShell, comprobando después si el proceso sigue
vivo. Sin el `ExcludeSemantics` muere en el primer intento; con él aguanta el ciclo entero de
maximizar / restaurar / minimizar. Es la única forma rápida de saber si una versión lo tiene:
mirar el log no vale, porque el log no dice nada.

Si algún día se quiere accesibilidad de verdad, el arreglo **no** es tocar esa línea: hay que
encontrar qué widget deja nodos colgando y corregir ese árbol.

### ⚠️ libmpv se abre con `dlopen`, así que `ldd` no la ve

`media_kit` **no empaqueta libmpv en Linux** (en Windows sí: `libmpv-2.dll` viaja dentro). La
busca en el sistema y la abre en caliente al llamar a `MediaKit.ensureInitialized()`. Nadie
enlaza contra ella, así que **no aparece en `ldd`** y el binario parece completo sin estarlo.

Toda la serie 0.2 de NeoFy se publicó sin declararla, y el síntoma fue el peor posible: como
esa inicialización va en `main()` antes de `runApp()`, en un Arch sin `mpv` instalado —o sea,
en casi cualquiera— la app **abría y se cerraba sin ventana y sin un mensaje que mirar**.

Hacen falta las dos cosas:

- **Declarar la dependencia, con un nombre distinto por familia**: `mpv` en Arch (no hay un
  `mpv-libs` suelto en los repositorios oficiales), `libmpv2 | libmpv1` en Debian y Ubuntu
  (cambió de soname en Debian 13 y Ubuntu 24.04).
- **Que faltar no mate la app.** La llamada va en un `try`/`catch` que pone
  `YtPlayer.libmpvDisponible = false`, y el shell enseña **qué paquete instalar**
  (`_SinLibmpv` en `neotube_shell.dart`) en vez de aceptar pulsaciones que no harían nada.
  Hace falta por el tarball, que no tiene forma de exigirle nada a nadie.

Y una tercera para que no se repita: el CI **arranca la app de verdad**
(`dbus-run-session xvfb-run`), comprueba que sigue viva a los 25 s y que no ha renunciado al
reproductor por el camino. Es el único paso que la ejecuta; todo lo demás mira ficheros y
enlazado estático, que es exactamente lo que no pilla un `dlopen`. Por eso `mpv` **no** se
instala a mano en el paso de dependencias: lo tiene que traer el gestor de paquetes
resolviendo las del paquete, o la comprobación no comprueba nada.

---

## 6. Linux: la WebView del login

### ⚠️ Sale en blanco si WebKitGTK no puede componer por GPU

WebKitGTK intenta **composición acelerada** al crear la WebView y, donde no hay un contexto
OpenGL utilizable, falla con `Failed to setup compositor shaders, unable to make OpenGL
context current` y **la ventana sale completamente en blanco**.

Lo que hace este fallo difícil de diagnosticar es que **no parece un fallo de renderizado**:
WebKit arranca perfectamente, `WebKitWebProcess` y `WebKitNetworkProcess` siguen vivos, y a la
app no le llega ningún error. Durante un tiempo se dio por hecho que en Arch «WebKitGTK no
llegaba a cargar», y era falso: cargaba y no pintaba.

El arreglo va en el runner de C++ (`linux/runner/main.cc` en NeoFy):

```cpp
setenv("WEBKIT_DISABLE_COMPOSITING_MODE", "1", 0);
```

Tres cosas de esa línea que no son evidentes:

- **Va en C++ y no en Dart** porque WebKit lee la variable al inicializarse, antes de que
  exista ningún código nuestro de Flutter.
- **El tercer argumento es `0`** (`overwrite` a falso): si el usuario ya la trae puesta en su
  entorno, se respeta su valor.
- **Renunciar a la composición acelerada no cuesta nada aquí**, porque la WebView existe *solo*
  para el login y no se vuelve a abrir. Por eso es incondicional en vez de intentar detectar la
  distribución, que sería frágil y no ganaría nada.

Verificado en Ubuntu 24.04 con GNOME: sin la variable, ventana en blanco; con ella, el login
entra y se capturan las 18 cookies, comprobadas contra la API real.

### ⚠️ El otro fallo de la WebView sigue abierto

Al cerrarse la ventana, `desktop_webview_window` 0.3.0 le pide al motor que elimine **la vista
implícita** —la ventana principal— y **se lleva la app por delante**
(`FlutterEngineRemoveView` → `kInvalidArguments`, y después punteros liberados). Es una
incompatibilidad con el embedder multi-vista de Flutter, y **0.3.0 es la última versión
publicada**: no hay actualización que esperar.

Las cookies se guardan justo antes de morir, así que reiniciando la app quedas logueado. Es
feo pero no pierde la sesión. **Es el argumento principal para sustituir la WebView por el
flujo de código de dispositivo**, que sería el siguiente paso natural de este repositorio.

En NeoFy hay además un aborto distinto en gráficas híbridas Intel+NVIDIA (GBM/DMA-BUF) que
apareció en la 0.2.9; si sale aquí, es el mismo.

### Compilar en Linux necesita `libwebkit2gtk-4.1-dev`

Y ojo con el orden: el paquete busca `webkit2gtk-4.1` y, si no lo encuentra, **exige
`webkit2gtk-4.0`**, que ya no está en las distribuciones modernas. En tiempo de ejecución el
paquete es `webkit2gtk-4.1`.

---

## 7. Cosas que en NeoFy costaron y aquí salen gratis

En NeoFy los dos modos compartían ventana, teclado y altavoces, y eso obligó a separar tres
cosas a mano. **Como app suelta, nada de esto hace falta** — pero conviene saber que existió,
porque el código movido todavía tiene las costuras:

- **El teclado.** `IgnorePointer` solo tapa el ratón; un evento de teclado va por el árbol de
  foco. Los atajos tuvieron que subirse por encima de los dos shells (`ui/atajos.dart`) y
  repartirse según el modo activo, más un `ExcludeFocus` sobre el modo invisible. Aquí
  `atajos.dart` puede simplificarse: ya no hay a quién repartir.
- **Los mandos de fuera.** Teclas multimedia, panel de Windows, MPRIS y bandeja iban *siempre*
  a Spotify hasta que se hizo que miraran el modo activo.
- **Los altavoces.** Arrancar una pista aquí pausaba Spotify (`YtPlayer.alEmpezarAReproducir`,
  que aquí se queda sin uso). Se pausaba y **no** se apagaba, porque librespot es lo que
  mantiene el dispositivo registrado en Spotify Connect.

---

## 8. La capa de escritorio

`lib/core/sistema.dart`, `mpris.dart`, `smtc.dart` y `media_keys.dart` reexpresan
`EstadoDelSistema` directamente sobre `YtTrack` — nada del modelo `Track` de Spotify se
arrastró desde NeoFy, que era el motivo por el que esta capa se dejó fuera al mover el
código (356 líneas de DTOs de la Web API que aquí no pintaban nada).

La trampa que había que respetar, ya resuelta:

> ⚠️ **El `mpris:trackid` es una ruta de objeto D-Bus** y solo admite `[A-Za-z0-9_]`. Los
> `videoId` de YouTube **traen `-` con frecuencia**: sin sanearlo, D-Bus rechaza el
> diccionario entero y el widget del escritorio se queda sin título y sin carátula.
> `mpris.dart` sanea el id con `replaceAll(RegExp(r'[^A-Za-z0-9_]'), '_')` antes de
> construir la ruta del objeto.

En Windows, SMTC (`system_media.cpp`/`.h`) habla con WinRT **sin C++/WinRT**:
`RoGetActivationFactory` + `HSTRING` a mano, porque la carátula que se le pasa al panel del
sistema se envuelve con `CreateRandomAccessStreamOverStream` sobre `SHCreateStreamOnFileEx`
—la única vía síncrona; la de `StorageFile` es asíncrona y esperarla bloquearía el hilo de
la ventana.

### Discord Rich Presence (`discord_rpc.dart`)

Portado de NeoFy (`spotify-native/lib/core/discord_rpc.dart`) y adaptado a `YtTrack`. Habla
el **IPC nativo de Discord sin paquete de terceros**: named pipe (`\\.\pipe\discord-ipc-N`)
en Windows y socket Unix en Linux, con los mensajes empaquetados a mano —opcode y longitud
en dos `uint32` little-endian, y detrás el JSON—. Fuera de esas dos plataformas el
transporte es un *no-op*, así que la app no se entera de que no hay Discord.

Está apagado por defecto (`AppConfig.discordRpcEnabled`) y se enciende en Ajustes. El
`clientId` es configurable pero viene con el de NeoTube puesto (`kDiscordClientId`).

Lo que cambió respecto al de NeoFy, y por qué:

- **Fuera `sync_id` y `flags: 48`.** Son lo que hace que Discord pinte el botón de
  «Reproducir en Spotify», y necesitan un id de track de Spotify. Aquí no significan nada.
- **El `end` del timestamp va condicionado a que haya duración.** `Track.durationMs` de
  Spotify siempre venía; `YtTrack.duracion` es **nullable**, y mandar
  `end == start` deja la barra de progreso de Discord en un estado absurdo.
- El asset grande es la carátula de la pista y el pequeño el logo.

> ⚠️ **`kDiscordAssetLogo` tiene que coincidir *exactamente* con el asset del Developer
> Portal, mayúsculas incluidas.** Si no coincide, Discord **no falla ni avisa**: la presencia
> sale sin imagen y ya. Estuvo puesto `'Logo'` mientras el asset se llamaba `logo`, y así se
> vio: todo lo demás correcto y el minilogo ausente.
>
> El nombre real se consulta sin entrar al portal, porque la lista es pública:
> `curl https://discord.com/api/v9/oauth2/applications/<clientId>/assets`

#### ⚠️ Discord descarta actualizaciones y no lo dice

`SET_ACTIVITY` está limitado a unas **5 llamadas cada 20 segundos**. Pasado el cupo, Discord no
contesta un error ni cierra el pipe: simplemente ignora lo que le mandes. Escribir en cada
cambio de estado —que es lo que se hacía— significaba que **saltar de canción deprisa gastaba
el cupo en pistas ya descartadas**: la presencia se quedaba clavada en una de en medio, y la
actualización que traía la duración (la única que hace que Discord dibuje la barra de progreso,
porque sin `end` no hay barra) llegaba tarde o no llegaba.

`DiscordRpc._programarEnvio` lo arregla fundiendo la ráfaga: el primer cambio sale al instante
—un cambio de canción normal no se retrasa nada— y los que lleguen dentro de la ventana se
juntan en un solo envío diferido. Dos detalles que parecen de adorno y no lo son:

- **El envío diferido lee los campos, no unos parámetros capturados.** Tiene que describir lo
  que suena *cuando le toca salir*, no lo que sonaba cuando se programó; capturar los valores
  era exactamente lo que dejaba la presencia hablando de una canción ya saltada.
- **El progreso se extrapola** desde el instante en que se anotó (`_instanteDelProgreso`): el
  reproductor solo avisa cuando cambia algo, no cada segundo, así que mandar la posición tal
  cual corría la barra de Discord hacia atrás.

---

## 9. Cómo se compila y se publica

### El orden de arranque, en `lib/main.dart`

No es negociable, cada paso rompe algo distinto si se mueve:

1. `WidgetsFlutterBinding.ensureInitialized()` — antes que nada: el constructor de
   `Settings` toca `PaintingBinding.instance.imageCache`.
2. `if (runWebViewTitleBarWidget(args)) return;` — en Windows, `desktop_webview_window`
   relanza este mismo ejecutable con argumentos especiales para pintar solo la barra de
   título de su ventana de login. Si se entra por aquí, no es una segunda instancia de
   NeoTube, es esa ventanita.
3. `try { MediaKit.ensureInitialized(); } catch (e) { YtPlayer.libmpvDisponible = false; ... }`
   — **antes** de construir ningún `YtPlayer`: su lista de inicialización lee
   `libmpvDisponible` directamente. El `debugPrint` de ese catch contiene la cadena literal
   `'sin libmpv'`, que el job `arch` del pipeline de release busca con `grep` para demostrar
   que el paquete de verdad declara `mpv` como dependencia.
4. Instancia única por socket TCP local (puerto `53331`, distinto del `53330` de NeoFy para
   que las dos convivan): enlazar, y si el puerto ya está ocupado, **intentar conectar**
   antes de rendirse — si tampoco hay nadie al otro lado, seguir adelante en vez de
   desaparecer en silencio.
5. `windowManager` (tamaño, `setPreventClose(true)` — siempre junto con `onWindowClose`, o
   la ventana queda imposible de cerrar).
6. `AppConfig.load()`, `YtAuth().loadStored()` **antes** de `runApp()` (`YtAuth` no notifica
   a nadie; si las cookies llegan después del primer frame, la interfaz no se entera).

### Empaquetado

- **Windows**: `tool\build_installer.ps1` compila en release y llama a Inno Setup
  (`installer\neotube.iss`) para producir `NeoTube-<version>-windows-x64.exe`.
- **Linux**: `tool/build_linux_bundle.sh` compila en release y arma un tarball con el
  `.desktop`, los iconos y `yt-dlp`. `linux/packaging/PKGBUILD` empaqueta ese tarball para
  Arch (`neotube-bin`); `mpv` va en su `depends=` — **no** se instala a mano en ningún paso
  de comprobación, tiene que llegar resolviéndolo el propio gestor de paquetes, o el
  problema del `dlopen` (sección 5) no se detecta.
- **`.github/workflows/release.yml`** dispara con cada tag `v*` y hace las dos cosas a la
  vez: compila el instalador de Windows, y en un contenedor `archlinux:latest` monta el
  paquete, lo instala de verdad con `pacman -U` y **arranca la app** (`dbus-run-session
  xvfb-run`) para comprobar que sobrevive 25 s y que el log no contiene `sin libmpv`. Es el
  único paso de todo el pipeline que ejecuta el binario; todo lo demás mira ficheros y
  enlazado estático, que es justo lo que no detecta un `dlopen`.
- ⚠️ **El cuerpo de la release sale de `RELEASE_NOTES.md`, y hay que reescribirlo en cada
  versión.** Si se olvida, la release nueva se publica con las notas de la anterior y nadie
  lo nota hasta que alguien las lee. Va por fichero y no por el mensaje del tag porque
  `softprops/action-gh-release` no usa el mensaje del tag, y porque así se revisa en el mismo
  *diff* que el código. Los dos pasos que suben ficheros (Windows y Linux) lo declaran **los
  dos a propósito**: corren en paralelo y cualquiera puede ser el que cree la release.
- El mismo job publica el paquete además en una release de etiqueta fija `repo`, con la
  base de datos que genera `repo-add`, para que sea un repositorio pacman de verdad:

  ```
  # /etc/pacman.conf
  [neotube]
  SigLevel = Optional TrustAll
  Server = https://github.com/KOLPSE/NeoTube/releases/download/repo
  ```

  y luego `sudo pacman -Sy neotube-bin`; a partir de ahí se actualiza solo con
  `pacman -Syu`, igual que cualquier otro paquete del sistema. ⚠️ Esa release lleva
  `--latest=false` a propósito: `updater.dart` lee `releases/latest` de la API de GitHub, y
  si la release del repositorio quedara marcada como la más reciente, el comprobador de
  actualizaciones de la propia app dejaría de ver las versiones de verdad.

Los tres tests (`test/yt_parseo_test.dart`, `yt_tarjeta_test.dart`, `yt_sin_libmpv_test.dart`,
más `yt_cache_test.dart` y `yt_error_reproduccion_test.dart`) corren con `flutter test` y la
red mockeada. **Lo que de verdad comprueba la API es `tool/probe_yt.dart`**, que habla con la
cuenta real y solo lee: se puede lanzar con la app abierta.

---

## 10. Qué se quedó NeoFy

NeoFy **no** perdió YouTube del todo. Su reproducción para cuentas **sin Premium** («la vía
libre», `core/reproduccion_libre.dart`) coge los metadatos de Spotify, busca el equivalente en
YouTube Music con `core/puente_yt.dart` y lo suena con este mismo `YtPlayer`. Así que allí
siguen viviendo copias de `yt_auth.dart`, `yt_music_api.dart`, `yt_models.dart`,
`yt_player.dart` y la pantalla de login.

**Son dos copias del mismo código a partir de ahora.** Si aquí se arregla una trampa de
parseo de la API interna —de las de la sección 4—, en NeoFy sigue rota. Merece la pena
comprobarlo en los dos sitios mientras no se extraiga a un paquete compartido.
