# Contrato de portabilidad: NeoTube (Flutter) → NeoTube 2.0 (Rust)

Extraído **solo** del código y la documentación de `D:\Proyectos\youtube-native`; cada dato lleva
`archivo.dart:línea`. Lo ausente se marca **NO DOCUMENTADO**: no inventar endpoints ni campos. Los literales
(versiones, `params`, cookies, cabeceras) son copias exactas, sin normalizar.
Catálogo (música): `POST https://music.youtube.com/youtubei/v1/<endpoint>` (yt_music_api.dart:135,205) ·
stream: `POST https://www.youtube.com/youtubei/v1/player` (yt_stream.dart:106,259) · todo cuerpo de catálogo
lleva `context` inyectado antes del resto (yt_music_api.dart:216) · sin contrato público de estabilidad: el
parseo original es defensivo a propósito (yt_music_api.dart:117-119,1324-1351).

## 1. Perfiles de cliente

### 1.1 `WEB_REMIX` — catálogo y acciones de cuenta

| Campo | Valor literal | Cita |
|---|---|---|
| host | `music.youtube.com` | yt_music_api.dart:135 |
| `clientName` | `WEB_REMIX` | yt_music_api.dart:170 |
| `clientVersion` | `1.20241201.01.00` | yt_music_api.dart:171 |
| `hl` | 2 primeras letras de `Platform.localeName`; `en` si no hay separador | yt_music_api.dart:156-160 |
| `gl` | país de `Platform.localeName` en mayúsculas; `US` si no se puede | yt_music_api.dart:162-166 |
| `visitorData` | en `context.client`, omitido mientras no se conozca | yt_music_api.dart:174 |
| `context.user` | `{"lockedSafetyMode": false}` | yt_music_api.dart:176-178 |
| `key` (query) | **no se envía** (con cookies+SAPISIDHASH la API no lo pide) | yt_music_api.dart:203-212 |
| `alt` (query) | no se usa | NO DOCUMENTADO |

Cabeceras de cada POST (yt_music_api.dart:208-215): `Content-Type: application/json` ·
`Cookie: <cookieHeader()>` (`"SAPISID=…; …"`, yt_auth.dart:173) · `Authorization: SAPISIDHASH
<epoch>_<sha1>` (yt_auth.dart:178-183) · `X-Goog-AuthUser: 0` · `X-Origin: https://music.youtube.com` ·
`Origin: https://music.youtube.com`. **No** hay `X-Youtube-Client-Name`/`-Version` ni `User-Agent` propio: los
pone el cliente HTTP (yt_music_api.dart:207-216). Timeout 20 s; `statusCode != 200` → error con extracto del
cuerpo; sin sesión no se sale a la red (yt_music_api.dart:198,219-227). Sirve para `search`, `browse`, `next`,
`like/like`, `like/removelike` (§2). `visitorData` se rastrea en **cualquier** respuesta (primeros 2048 bytes,
regex `"visitorData"\s*:\s*"([^"]+)"`) y se reenvía después (yt_music_api.dart:174,182-190,228).

### 1.2 `ANDROID_VR` — solo `youtubei/v1/player`

`context.client` literal (yt_stream.dart:124-135); el `userAgent` va **también** en cabecera y emparejado a
`clientVersion` (yt_stream.dart:137-140):

```json
{"clientName":"ANDROID_VR","clientVersion":"1.65.10","deviceMake":"Oculus","deviceModel":"Quest 3",
 "androidSdkVersion":32,
 "userAgent":"com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
 "osName":"Android","osVersion":"12L","timeZone":"UTC","utcOffsetMinutes":0}
```

Cabeceras: `Content-Type: application/json` · `User-Agent: <el de arriba>` · `X-Youtube-Client-Name: 28` ·
`X-Youtube-Client-Version: 1.65.10` · `Origin: https://www.youtube.com` · `X-Goog-Visitor-Id: <visitorData>`
(solo si se tiene). **Sin `Cookie`, `Authorization` ni `SAPISIDHASH`**, a propósito: con sesión, la URL emitida
luego da 403 a ffmpeg (yt_stream.dart:71-86,263-271; CONTEXTO.md:418-444). Query `?prettyPrint=false`; timeout
10 s (yt_stream.dart:257-259,277). Sirve para una URL de audio con `url` directa, sin `signatureCipher` ni `n`
(yt_stream.dart:52-64). Al **abrir** esa URL hay que mandar ese `User-Agent`, `Referer: https://www.youtube.com/`
y `Origin: https://www.youtube.com` (yt_stream.dart:161-165).

### 1.3 Clientes probados y descartados

- `WEB_REMIX` para `player`: 200 y 4 formatos, **todos en `signatureCipher`** → exigiría ejecutar JS
  (yt_stream.dart:52-58, CONTEXTO.md:250-256). `IOS`: URLs directas y rápidas, pero ffmpeg recibe 403
  (yt_stream.dart:66-72, CONTEXTO.md:268-281).
- `WEB_EMBEDDED_PLAYER`: único que sirve el fichero entero, pero su URL exige huella TLS de navegador → solo
  vía yt-dlp para la descarga completa (yt_player.dart:637, CONTEXTO.md:330-340).

### 1.4 `visitorData` (obligatorio para `ANDROID_VR`)

`GET https://www.youtube.com/watch?v=dQw4w9WgXcQ&bpctr=9999999999&has_verified=1` con `User-Agent` de Safari
y `Cookie: PREF=hl=en&tz=UTC; SOCS=CAI`; se extrae del HTML con `"visitorData":\s*"([^"]+)"` y se des-escapa
como literal JSON (yt_stream.dart:296-335). Vida 30 min; se renueva si el reproductor rechaza una URL
(yt_stream.dart:204,212-215). Sin él: `LOGIN_REQUIRED` «confirma que no eres un bot», **haya o no cookies**
(yt_stream.dart:168-186, CONTEXTO.md:399-417).

## 2. Cuerpos de petición

El `context` de §1.1 va en todos; aquí solo el resto.

| Endpoint | Cuerpo | Cita |
|---|---|---|
| `search` general | `{"query":"<q>"}` | yt_music_api.dart:270 |
| `search` filtrada | `{"query":"<q>","params":"<params>"}` | yt_music_api.dart:286-289 |
| `search` continuación | `{"continuation":"<token>"}` | yt_music_api.dart:294 |
| `browse` secciones/artista/colección | `{"browseId":"<id>"}` | yt_music_api.dart:315,368,372,977 |
| `browse` continuación moderna | `{"continuation":"<token>"}` | yt_music_api.dart:327,1057 |
| `browse` continuación de respaldo | cuerpo `{}` + `?ctoken=<t>&continuation=<t>&type=next` | yt_music_api.dart:329-330,1059-1060 |
| `next` | `{"playlistId":"<id>","videoId":"<id>","isAudioOnly":true,"params":"wAEB"}` | yt_music_api.dart:1120-1125 |
| `like/like` canción | `{"target":{"videoId":"<id>"}}` | yt_music_api.dart:1179-1180 |
| `like/removelike` canción | `{"target":{"videoId":"<id>"}}` | yt_music_api.dart:1183-1184 |
| `like/like` álbum/lista | `{"target":{"playlistId":"<OLAK5uy_…>"}}` | yt_music_api.dart:1198-1199 |
| `like/removelike` álbum/lista | `{"target":{"playlistId":"<OLAK5uy_…>"}}` | yt_music_api.dart:1202-1203 |

`params` de los chips de búsqueda — opacos, protobuf urlencodeado, copiados literalmente (yt_music_api.dart:38-49):

```
canciones EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D   albumes EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D
artistas  EgWKAQIgAWoKEAkQChAFEAMQBA%3D%3D   listas  EgWKAQIoAWoKEAkQChAFEAMQBA%3D%3D
```

- `todo` = sin `params` (yt_music_api.dart:44); el chip «listas» da «Listas de la comunidad», no las destacadas
  de YT Music (yt_music_api.dart:39-41).
- Búsqueda sin filtro = **dos peticiones en paralelo** (general + `EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D`), a la
  segunda una **continuación** cuyo resultado se **fusiona dentro** del `musicShelfRenderer` general antes de
  parsear (yt_music_api.dart:234,270-277,285-303,581-615).
- `next`: `playlistId`/`videoId` **se omiten si son null** en el original (`'playlistId': ?playlistId`). Radio de
  una canción = `playlistId "RDAMVM<videoId>"` + `videoId` (yt_music_api.dart:1160-1163); los ids `RD…` **solo**
  responden por `next` (yt_music_api.dart:120-122, CONTEXTO.md:149-153).
- «Me gusta» y «guardar en biblioteca» son la **misma** acción cambiando el `target` (yt_music_api.dart:1186-1197).
  El `playlistId` debe venir del `toggleButtonRenderer` de la cabecera, **nunca** del `browseId`: con `MPREb_…`
  contesta 200 y no guarda (yt_music_api.dart:1290-1310).
- `browseId` fijos (yt_music_api.dart:141-146): `FEmusic_home` (portada), `FEmusic_explore` (explorar),
  `FEmusic_liked_playlists`, `FEmusic_liked_albums`, `FEmusic_liked_videos`,
  `FEmusic_library_corpus_track_artists`. Derivados: `VL<playlistId>` si el id no empieza por `RD`
  (yt_music_api.dart:956-957); álbum tal cual `MPREb_…` (939-941); artista canónico sin `MPLA` (1534-1535);
  `VLLM` para «Me gusta» como lista de pistas, a partir de `LM` (yt_favoritos.dart:26-33,55).
  **`FEmusic_liked_videos` no vale como lista de pistas**: es una rejilla de secciones (yt_favoritos.dart:26-33).
- La biblioteca se pide en **paralelo, las cuatro tabs**, emitiendo por partes; los títulos de sección los pone
  el cliente: `Tus playlists`/`Tus álbumes`/`Canciones que te gustan`/`Tus artistas` (yt_music_api.dart:842-909,861-866).
- `player` (yt_stream.dart:230-254):

```json
{"context":{"client":{…§1.2,"hl":"<hl>","gl":"<gl>","visitorData":"<v>"},"user":{"lockedSafetyMode":false}},
 "videoId":"<id>","playbackContext":{"contentPlaybackContext":{"html5Preference":"HTML5_PREF_WANTS"}},
 "contentCheckOk":true,"racyCheckOk":true}
```

Los dos últimos evitan que un vídeo con aviso de contenido pida confirmación de edad en vez de formatos
(yt_stream.dart:248-251).

- **No implementado en el original**: `subscribe`/`unsubscribe` **NO DOCUMENTADO** (`subscribeButtonRenderer`
  solo se lee para mostrar el contador, yt_music_api.dart:399-401); edición de playlists (crear, renombrar,
  añadir/quitar, reordenar) **NO DOCUMENTADO**; `alt=`/`key=` no se envían (yt_music_api.dart:203-212).

## 3. Cookies y `SAPISIDHASH`

Solo cookies cuyo `domain` contenga `youtube.com`; se descartan `.google.com`/`accounts.google.com`
(yt_auth.dart:96-127). Se limpian **nombre y valor** a ASCII imprimible `[\x21-\x7E]` (WebView2 devuelve un `\0`
pegado) (yt_auth.dart:109-119) y se **deduplica por nombre**, gana la primera (orden de la WebView, sin ordenar)
(yt_auth.dart:100-101,130).

Lista **cerrada y ordenada** de cookies que firman (yt_auth.dart:71): `SAPISID`, `__Secure-3PAPISID`,
`__Secure-1PAPISID`. `APISID` **no está y no puede estar**: es otra cookie con otro valor y llega antes; firmar
con ella deja la sesión anónima (yt_auth.dart:60-70); nunca se busca por sufijo `APISID` (yt_auth.dart:66-68).
`isLoggedIn` = «hay valor no vacío en alguna de las tres»; el login se da por bueno solo si las cookies
capturadas sirven para firmar (yt_auth.dart:55,79-95).

```
epoch = floor(now_ms / 1000)                                  # segundos
hash  = sha1_hex("{epoch} {sapisid} https://music.youtube.com")
Authorization: SAPISIDHASH {epoch}_{hash}
Cookie: name=value; name=value…                               # unidos por "; "
```

(yt_auth.dart:173,178-183). Origen **siempre** `https://music.youtube.com`, incluso hacia `www`: con `www` da
`400 INVALID_ARGUMENT` (yt_auth.dart:177, CONTEXTO.md:443).

Login con WebView real (WebView2/WebKitGTK), solo para esa pantalla; perfil en `appDataDir()/webview2`
(yt_auth.dart:24-44,76-77,196-201). Entrada literal (yt_auth.dart:256-261):
`https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F`.
Éxito = navegación a `https://music.youtube.com` con cookies que firman: se persisten y la ventana se cierra sola
(yt_auth.dart:225-237,245-249); respaldo: sondeo cada 1 s; timeout 5 min; cerrar antes = error
(yt_auth.dart:240-244,267-274).

| Fallo de autenticación | Síntoma | Cita |
|---|---|---|
| firmar con `APISID` | **200 con sesión anónima**: portada genérica llena, biblioteca con `messageRenderer` «Inicia sesión para escuchar tus canciones favoritas» | yt_auth.dart:60-70, CONTEXTO.md:101-125 |
| cookie homónima de dos dominios | `400: solicitud mal formada` | yt_auth.dart:100-107 |
| `\0` de WebView2 | nombre que no encaja y `FormatException: Invalid HTTP header field value` | yt_auth.dart:109-119 |
| sesión caducada | NO DOCUMENTADO: `expires` se guarda y no se comprueba; se manifiesta como sesión anónima | yt_auth.dart:79-85,146-164 |

## 4. Rutas de parseo

### 4.1 Bloques de secciones (portada, explorar, biblioteca, artista)

Orden de intentos (yt_music_api.dart:1207-1213): (1)
`contents.singleColumnBrowseResultsRenderer.tabs[].tabRenderer.content.sectionListRenderer.contents` (1221-1230);
(2) `contents.twoColumnBrowseResultsRenderer.tabs[].tabRenderer.content.sectionListRenderer.contents` (1232-1241).
Para **pistas de una colección** se prueba antes (yt_music_api.dart:1215-1219):
`contents.twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer.contents` (1243-1250). En
listas/álbumes la pestaña solo trae cabecera: las pistas están en `secondaryContents` (CONTEXTO.md:158-162).
Búsqueda: `contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents`, con
respaldo `contents.singleColumnBrowseResultsRenderer.tabs[0]…` (yt_music_api.dart:633-638,563-568).

### 4.2 Cola (`next`) — yt_music_api.dart:1135-1142

```
contents.singleColumnMusicWatchNextResultsRenderer.tabbedRenderer.watchNextTabbedResultsRenderer
  .tabs[0].tabRenderer.content.musicQueueRenderer.content.playlistPanelRenderer
    .title.runs → título ; .contents[].playlistPanelVideoRenderer
```

Sin pistas → excepción propia (yt_music_api.dart:1146-1148).

### 4.3 Sección → items

Renderers aceptados: `musicShelfRenderer`, `musicCarouselShelfRenderer`, `gridRenderer`, `musicCardShelfRenderer`
y `itemSectionRenderer` (se desenvuelve; en búsqueda sus items sueltos se agrupan por tipo)
(yt_music_api.dart:648-696,1324-1332).

- **`gridRenderer` guarda en `items`; todo lo demás en `contents`** (yt_music_api.dart:1336, CONTEXTO.md:138-142).
- Título: `header.<renderer>.title.runs` → `header.<renderer>.strapline.runs` → `title.runs` → `''`
  (yt_music_api.dart:1339-1341,1377-1387).
- Se descarta toda sección sin items o con forma inesperada (yt_music_api.dart:443-450,1324-1351).

### 4.4 Item → tarjeta

`musicTwoRowItemRenderer` (tarjeta/carrusel) → `_parseTwoRowItem` (yt_music_api.dart:1399-1428);
`musicResponsiveListItemRenderer` (fila) → `_parseListItem` (1440-1491); `playlistPanelVideoRenderer` (cola) →
`_parsePanelVideo` (1493-1509). Dispatched en yt_music_api.dart:1353-1375.

- `videoId`, en este orden (yt_music_api.dart:1550-1568): `playlistItemData.videoId` →
  `overlay.musicItemThumbnailOverlayRenderer.content.musicPlayButtonRenderer.playNavigationEndpoint.watchEndpoint.videoId`
  → `navigationEndpoint.watchEndpoint.videoId` →
  `flexColumns[0].musicResponsiveListItemFlexColumnRenderer.text.runs[0].navigationEndpoint.watchEndpoint.videoId`.
- `playlistId` (yt_music_api.dart:1404-1406,1430-1437,1457-1458): `navigationEndpoint.watchPlaylistEndpoint.playlistId`
  ?? `navigationEndpoint.watchEndpoint.playlistId` ??
  `thumbnailOverlay.musicItemThumbnailOverlayRenderer.content.musicPlayButtonRenderer.playNavigationEndpoint.watchPlaylistEndpoint.playlistId`.
  Si falta y el `browseId` empieza por `VL` → `browseId[2..]` (yt_music_api.dart:822-823,1420-1421,1482-1483).
- `browseId` (yt_music_api.dart:1407-1409,1456-1475): `navigationEndpoint.browseEndpoint.browseId` ??
  `title.runs[0].navigationEndpoint.browseEndpoint.browseId` ?? (solo filas) primer `browseEndpoint` de
  `flexColumns[0]` y luego de `flexColumns[1]`.
- Título = `runs[0].text`; subtítulo = **concatenación** de todos los `runs` (yt_music_api.dart:1389-1397,1444-1452).
- Miniatura: primera ruta existente de `musicThumbnailRenderer.thumbnail.thumbnails` ·
  `croppedSquareThumbnailRenderer.thumbnail.thumbnails` · `thumbnails` · `thumbnail.thumbnails`; se toma la
  **última** URL, la mayor (yt_music_api.dart:1594-1613).
- Duración: `fixedColumns[0].musicResponsiveListItemFixedColumnRenderer.text.runs[0].text` o
  `lengthText.runs[0].text`; formatos `m:ss` y `h:mm:ss` (yt_music_api.dart:1571-1592,1504).
- Un item sin `videoId`/`playlistId`/`browseId` se descarta (`tieneDestino`) (yt_music_api.dart:1342-1346,
  yt_models.dart:98); así cae el botón «Nueva lista» de la rejilla de biblioteca (CONTEXTO.md:176-178).

### 4.5 Distinción de tipo (`_tipoDe`, orden estricto) — yt_music_api.dart:1537-1548

```
videoId != null                                          → cancion
browseId.startsWith("MPRE")                              → album
browseId.startsWith("UC"|"MPLAUC"|"MPLA"|"FEmusic_library_privately_owned_artist") → artista
playlistId != null || browseId.startsWith("VL"|"RD")     → lista
resto                                                    → desconocido
```

Prefijos en yt_music_api.dart:1520-1525. Un artista no es reproducible (yt_models.dart:102-108). `MPLAUC…` se
reduce a `UC…` para pedir la página completa (yt_music_api.dart:1527-1535).

### 4.6 Cabecera de colección (lista/álbum)

Se prueba el **primer bloque de secciones** y luego `j['header']`; el renderer se desenvuelve y, si contiene otro
`header` anidado, otra vez (yt_music_api.dart:1253-1288). Campos: `title.runs`, `subtitle.runs` +
`secondSubtitle.runs` unidos por `" • "`, `thumbnail`/`background`, y el botón de guardar (yt_music_api.dart:1264-1277).
Tres formas del mismo dato según propiedad: `musicEditablePlaylistDetailHeaderRenderer`,
`musicResponsiveHeaderRenderer`, `musicDetailHeaderRenderer` (CONTEXTO.md:163-166).

```
buttons[].toggleButtonRenderer.defaultServiceEndpoint.likeEndpoint.target.playlistId
   (o)             .toggledServiceEndpoint.likeEndpoint.target.playlistId
                   .isToggled → guardada
```

(yt_music_api.dart:1300-1314). Cabecera de artista: `j['header']` con `musicImmersiveHeaderRenderer` (público) o
`musicHeaderRenderer` (biblioteca) (yt_music_api.dart:389-396); rutas `title.runs`,
`subscriptionButton.subscribeButtonRenderer.subscriberCountText.runs`, `monthlyListenerCount.runs`, `thumbnail`,
`description.runs`, y `playButton`/`startRadioButton` → `buttonRenderer.navigationEndpoint.watchEndpoint.playlistId`
(yt_music_api.dart:398-422); sus secciones son las de §4.1 (yt_models.dart:175-210).

Miniaturas: el sufijo `=w120-h120-l90-rj` de `googleusercontent` **es una petición de reescalado**: se reescribe
al tamaño pintado, cuantizado (la URL es la clave de caché); las de `i.ytimg.com` se dejan intactas
(art_image.dart:62-76, CONTEXTO.md:216-232).

## 5. Paginación

Dónde vive el token, en orden (yt_music_api.dart:1099-1115): `shelf.continuations[0].nextContinuationData.continuation`
y `shelf.contents.last.continuationItemRenderer.continuationEndpoint.continuationCommand.token`. En la primera
respuesta de `browse`: `tabs[].tabRenderer.content.sectionListRenderer.contents[].continuations`,
`…gridRenderer.items[].continuations` y `twoColumnBrowseResultsRenderer.secondaryContents.sectionListRenderer`
(yt_music_api.dart:527-558); en `search`, solo en el primer `musicShelfRenderer` de la primera pestaña (560-579).

- Se manda en el cuerpo `{"continuation": token}`; la forma antigua (respaldo si falla) es cuerpo `{}` +
  `?ctoken=<t>&continuation=<t>&type=next` (yt_music_api.dart:327,329-330,1057,1059-1060). Las dos conviven; una
  lista larga llega en **trozos de 100** (CONTEXTO.md:167-171).
- Respuesta: `onResponseReceivedActions[].appendContinuationItemsAction.continuationItems` (459-476) ·
  `continuationContents.sectionListContinuation.contents` (481-494) · `continuationContents.gridContinuation.items`
  (496-511) · `continuationContents.<renderer>.contents|items` (1080-1087) · búsqueda:
  `continuationContents.musicShelfContinuation.contents` u `onResponseReceivedActions[0]…` (584-594). Una respuesta
  **completa** de browse se acepta como continuación y se reextrae el token (515-522).
- Tope **20 iteraciones** por `browse` y por colección; agotado, se para en silencio (yt_music_api.dart:321-322,994-999).
  Se acumulan pistas en un `List` y se emite la colección crecida en cada página (982-999).
- **Sin dedupe entre páginas** (320-334,994-999). Solo dedupe: en el `search` combinado por
  `videoId`/`browseId`/`playlistId` (622,710-762) y en `anexar` a la cola por `videoId` (yt_player.dart:1008-1016).
- Un fallo de red en una tab de biblioteca no tumba las otras tres (891-905).

## 6. Reproductor y cola

Cola: `cola: List<YtTrack>`, `indice: int` (-1 = nada), `contexto: String?` = `playlistId`/`browseId` de origen
(yt_player.dart:218-224). `YtTrack` = `{videoId, titulo, artista, miniatura?, duracion?}` con igualdad **por
`videoId`** (yt_models.dart:20-57).

- `YtRepeticion` = ciclo `ninguna → cola → pista → ninguna`, excluyentes por diseño (yt_player.dart:24-44,274-281).
- Aleatorio **persistente** (no «barajar una vez»): guarda el orden real en `_colaSinBarajar`, deja la pista actual
  en el índice 0 y baraja el resto; al apagar recupera el orden y reindexa por `videoId` (yt_player.dart:230-268).
  Con aleatorio activo, `reproducirLista` rebaraja igual (971-996).
- `puedeSaltar` = hay siguiente **o** (`cola` y >1 pista); `puedeVolver` = cola no vacía (yt_player.dart:296-299).
- **Siguiente** (yt_player.dart:1225-1268): fin de cola con `cola` → `indice = 0`; fin sin bucle y automático →
  cede a `alAcabarLaCola`, si no `pause()` + `seek(0)` dejando la pista; manual → nada. Fallo al abrir: automático →
  sigue bajando (sin dar la vuelta); manual → revierte índice, limpia `error` y **reintenta a los 4 s**.
- **Anterior**: `posicion > 3 s` o `indice <= 0` → `seek(0)`; si no, `indice--` (1272-1292). `saltarA(i)` en 1294-1308.
- **Sesión restaurada**: `restaurar(...)` deja cola, índice, `_posicionPendiente` y `_pendienteDeAbrir = true` **sin
  abrir audio**; el primer `alternar()`/`resume()` abre con `Media(start: posicion)`, no con un `seek` posterior
  (yt_player.dart:308-355,1028-1035). Mientras está pendiente, `posicion`/`duracion` devuelven lo guardado
  (205-215); `anterior()` sobre pendiente reabre la misma pista desde 0 (1276-1281).
- **Autoplay**: canción suelta = `reproducirPista(t)` y **en paralelo** `radioDe(t.videoId)`; si sigue siendo la
  actual, `anexar(radio)` (yt_acciones.dart:66-107, yt_music_api.dart:1160-1168). No se espera la radio para sonar
  (yt_player.dart:1000-1006); `anexar` deduplica por `videoId` y mantiene `_colaSinBarajar` (1008-1026).
- **URL del stream** (`core/yt_stream.dart`, contrato): `YtStreamResolver.resolver(videoId, {hl, gl}) → String url`
  y `renovarVisitante()` (yt_stream.dart:212-229). Resuelve con `ANDROID_VR` **anónimo** + `visitorData`, elige el
  `adaptiveFormats[]` con `mimeType` que empieza por `audio/`, `url` no vacía y mayor `bitrate`, e ignora
  `signatureCipher` (374-396). `YtStreamException{message, avisoDeBot, reintentarConYtDlp}`: `avisoDeBot` ⇔
  `playabilityStatus.status == "LOGIN_REQUIRED"` (un reintento renovando visitante); `reintentarConYtDlp == false`
  ⇔ `UNPLAYABLE`/`ERROR` (10-27,168-172,223-228,339-372).
- Caché de URLs **45 min**, máx. 24 entradas, dedupe de resoluciones en vuelo (yt_player.dart:394,556-566); vía
  rápida primero, yt-dlp solo si procede (568-580); **prefetch** de 2 pistas con máx. 2 resoluciones concurrentes
  (954-969).
- **Corte de posición**: proxy local en `127.0.0.1` que sirve la vía rápida en trozos de **900 000 bytes** y avisa
  por `onCorte` la primera vez que falla un trozo posterior al primero; **no empalma bytes**
  (yt_local_proxy.dart:49-58,73-80, CONTEXTO.md:301-360). Al `completed` de mpv coincidiendo con el corte se reabre
  el **fichero completo** y se salta, `Media(fichero, start: posicion)` (yt_player.dart:1154-1200).
- **Descarga de repuesto**: `yt-dlp -f bestaudio --extractor-args youtube:player_client=<cliente> -o
  <dir>/<videoId>.audio` (yt_player.dart:760-780); clientes `['web_embedded','web_embedded','web_embedded,tv']`,
  **4 s** entre intentos (637-638); se lanza **4 s después** de abrir, para no disparar el aviso de bot al saltar
  rápido (609,640-656,670-682); timeout de proceso 40 s; progreso estimado por tiempo contra 9 s, tope 0.95 (800-837).
  Si al llegar el corte no hay fichero: un intento más con `web_embedded,tv`; si falla, mensaje `No se pudo descargar
  el resto de esta pista.` y se salta **avisando** (1168-1192).
- **Reapertura fallida**: máx. **2** reintentos (`_maxReintentos`), por yt-dlp, renovando `visitorData` y **vaciando
  la caché de URLs** (387,1105-1152). Ventana de «abriendo»: **7 s** (1061,1079). yt-dlp se busca junto al ejecutable
  → `tool/ytdlp-build/bin/` → `PATH`; `--js-runtimes deno:<ruta>` si hay Deno; subprocesos con `PATH` mínimo
  (422-451,475-514,582-607); `stderr` traducido al español (429 / bot / privado / no disponible / país) (925-950).
- **Escritorio**: teclado `mediaPlayPause`/`mediaTrackNext`/`mediaTrackPrevious`/`space` → play/pausa **salvo** foco
  en `EditableText` (atajos.dart:40-50,60-79); teclas globales por canal `neotube/media_keys`
  (`playPause`/`next`/`previous`/`pause`, media_keys.dart:15-37, registradas en el runner C++). SMTC (Windows): canal
  `neotube/system_media` con `playPause`/`play`/`pause`/`stop`/`next`/`previous`/`seek(ms)` (smtc.dart:20-44).
  MPRIS (Linux): bus `org.mpris.MediaPlayer2.neotube`, objeto `/org/mpris/MediaPlayer2`, `Identity` `NeoTube`, señal
  `Seeked` en **microsegundos** (mpris.dart:56,92,138-155). Estado publicado:
  `{track, sonando, posicionMs, puedeSaltar, puedeVolver, volumen}` (sistema.dart:9-28), emitido al cambiar
  pista/estado y en cada salto (main.dart:284-294,324-329); el volumen se persiste al **soltar** el mando
  (main.dart:330-333, neotube_shell.dart:964-965). Discord RPC recibe la cola **futura** (main.dart:296-300).
- Sin libmpv la app arranca igual, con `disponible == false` y `motivoSinLibmpv` (yt_player.dart:144-161,193-195);
  `stop()` vacía cola, índice, contexto y errores (1378-1393).

## 7. Persistencia

Todo es **JSON en disco**; no hay base de datos.

| Ruta | Contenido | Cita |
|---|---|---|
| `appDataDir()` | Windows `%APPDATA%\neotube`; Linux `$XDG_CONFIG_HOME/neotube` o `~/.config/neotube` | app_config.dart:24-49 |
| `cacheDir()` | Windows = `appDataDir()`; Linux `$XDG_CACHE_HOME/neotube` o `~/.cache/neotube` | app_config.dart:58-73 |
| `config.json` | preferencias | app_config.dart:100-165 |
| `yt_cookies.json` | array de cookies de sesión | yt_auth.dart:146-164 |
| `sesion.json` | cola y estado del reproductor | yt_sesion.dart:56-162 |
| `webview2/` | perfil WebView2 del login | yt_auth.dart:76-77,196-201 |
| `art/<sha1(url)>.img` | carátulas, tope 50 MB | art_cache.dart:18-31,88-146 |
| `<caché>/continuaciones/<videoId>.audio` | copias completas de repuesto | yt_player.dart:686-700,867-889 |

`config.json` (app_config.dart:100-140; lectura tolerante en 127-141): `{"performanceMode":false,
"volumenNeoTube":60,"discordRpcEnabled":false,"discordClientId":"1538661228040298556",
"limiteCacheContinuacionesMB":300,"rutaCacheContinuaciones":null,"nombreReporte":""}`.
`limiteCacheContinuacionesMB` se acota a `0..100000` y `0` **apaga** la caché de repuesto (settings.dart:82-88,
yt_player.dart:654). `kVersion = '1.2.1'` (app_config.dart:11). La ruta de la caché de continuaciones es la elegida
por el usuario o `cacheDir()/continuaciones` (app_config.dart:75-86, yt_player.dart:867-889).

`yt_cookies.json`: serialización de la cookie del paquete (`name`, `value`, `domain`, `path`, `expires`, `secure`,
`httpOnly`, `sessionOnly`) (yt_auth.dart:133-141,150-160); `logout()` vacía memoria y borra el fichero (166-171).
Migración heredada: al crear `appDataDir()` se copia `<parent>/neofy/yt_cookies.json` si existe (app_config.dart:41-48).

`sesion.json` (yt_sesion.dart:134-152): `{"indice":0,"posicionMs":0,"contexto":"<playlistId|browseId|null>",
"aleatorio":false,"repeticion":"ninguna|cola|pista","cola":[{"videoId":"","titulo":"","artista":"",
"miniatura":null,"duracionMs":null}]}`. Máximo **400** pistas, recortadas **alrededor de la actual** (50 por detrás)
(54-55,125-132); escritura **atómica** `.tmp` + `rename` (155-162); fichero corrupto/ausente → `null`, sin excepción
(64-90); sin pista o cola se **borra** el fichero (106-112). Se guarda al cambiar de pista, cada **20 s** y en `_quit`
**antes** de `player.stop()` (main.dart:351-360,370-375,385-392; CONTEXTO.md:494-500). `aleatorio` se restaura **sin
rebarajar**; se pierde `_colaSinBarajar` (yt_player.dart:320-346). Sin campo de versión de esquema ni migraciones:
**NO DOCUMENTADO**.

Cachés: carátulas con tope **50 MB**, poda por mtime al arrancar, descargas en vuelo deduplicadas, escritura atómica
y fichero de 0 bytes descartado (art_cache.dart:18-45,88-146,130-165; main.dart:99); continuaciones podadas por mtime
al límite en MB sin borrar la recién bajada (yt_player.dart:891-923); memoria `YtCache` = LRU de **24** entradas con
modelos ya parseados (nunca JSON crudo), TTL `browse:FEmusic_home` 30 min · `browse:FEmusic_explore` 6 h ·
`browse:FEmusic_liked*`/`browse:FEmusic_library*`/`biblioteca` 10 min · `coleccion:` 24 h · `artista:` 1 h ·
`buscar:<filtro>:<q>` 10 min · resto 10 min (yt_cache.dart:12-45), claves reales `browse:<id>`, `buscar:<filtro>:<q>`,
`artista:<id>`, `coleccion:<id sin VL>`, `biblioteca` (yt_music_api.dart:251,312,363,855,913-921). Invalidación tras
like/guardado: `coleccion:LM`, `biblioteca`, `browse:FEmusic_liked_albums`, `browse:FEmusic_liked_playlists`
(yt_favoritos.dart:107,195-199).

## 8. Fallos documentados y remedio

| Fallo | Causa real | Remedio | Cita |
|---|---|---|---|
| Portada llena y biblioteca vacía / «inicia sesión» | se firmó con `APISID`: **200 anónimo** | lista cerrada de cookies de firma | yt_auth.dart:60-71 |
| `400: solicitud mal formada` | cookie homónima de dos dominios en `Cookie:` | filtrar `youtube.com` + dedupe por nombre | yt_auth.dart:96-130 |
| `FormatException: Invalid HTTP header field value` | `\0` final de WebView2 | limpiar a ASCII `\x21-\x7E` | yt_auth.dart:109-119 |
| Biblioteca vacía sin error | `gridRenderer` usa `items` | leer `contents ?? items` | yt_music_api.dart:1336 |
| «No hay lista que abrir en este elemento» sobre un artista | `MPLAUC…` no reconocido | prefijo `MPLA` + `browseIdDeArtista` | yt_music_api.dart:1520-1535 |
| Lista abierta con solo cabecera | pistas en `secondaryContents` | mirar esa ruta primero | yt_music_api.dart:1215-1250 |
| Mezclas/radios no abren | `browse VL<RD…>` falla | `next` con `playlistId` `RD…` | yt_music_api.dart:120-122,956-973 |
| `400` al abrir una lista | `VL` duplicado | `VL` solo si el id no lo lleva | yt_music_api.dart:957, CONTEXTO.md:143-148 |
| Guardar álbum «funciona» y no guarda | `MPREb_…` enviado a `like/like` (200 sin efecto) | `idParaGuardar` (`OLAK5uy_…`) del `toggleButtonRenderer` | yt_music_api.dart:1193-1199,1300-1314 |
| Favoritos vacíos, corazones que no encienden | `FEmusic_liked_videos` no es lista de pistas | `coleccion(playlistId: 'LM')` → `VLLM` | yt_favoritos.dart:26-33 |
| Corazón que se apaga solo al reentrar | cabecera cacheada con `isToggled` viejo | `sembrar` no pisa lo conocido | yt_favoritos.dart:166-173 |
| Biblioteca cacheada obsoleta tras un like | no se invalidaba la caché | borrar `coleccion:LM` / `biblioteca` / `browse:FEmusic_liked_*` | yt_favoritos.dart:107,195-199 |
| Filtro de búsqueda sin listas | se usaban los `params` de canciones | `params` propio por chip | yt_music_api.dart:38-49 |
| Nada suena aunque la URL descarga rápido | `IOS`: ffmpeg recibe 403 | `ANDROID_VR` | yt_stream.dart:66-72, CONTEXTO.md:268-281 |
| 403 sistemático pasado ~1 min | googlevideo corta en **1 000 000** de bytes (SABR); `Range: bytes=0-` siempre cae mal | trozos de 900 000 + reabrir fichero y saltar | yt_local_proxy.dart:11-58, CONTEXTO.md:283-360 |
| Canciones que «se saltan solas» | repuesto `web_embedded` falla a rachas | 3 intentos con `tv` al final + 1 al llegar al corte + aviso visible | yt_player.dart:637-638,1168-1192, CONTEXTO.md:364-398 |
| `LOGIN_REQUIRED` «confirma que no eres un bot» | `ANDROID_VR` sin `visitorData`, aunque haya cookies | obtener/renovar `visitorData` | yt_stream.dart:168-186,204-215 |
| Todas las pistas fallan al abrir | se mandaron cookies a `player` | petición anónima | yt_stream.dart:71-86, CONTEXTO.md:418-444 |
| `400 INVALID_ARGUMENT` al firmar | `SAPISIDHASH` con origen `www.youtube.com` | firmar con `https://music.youtube.com` | yt_auth.dart:177, CONTEXTO.md:443 |
| Biblioteca recortada sin explicación | modo restringido del navegador | `context.user.lockedSafetyMode: false` | yt_music_api.dart:176-178 |
| App se cierra sin ventana al arrancar en Linux | falta libmpv (`dlopen`) | `libmpvDisponible = false` + mensaje | yt_player.dart:144-161, CONTEXTO.md:553-581 |
| Ninguna canción arranca sin pista del motivo | yt-dlp ausente o roto | buscar en 3 sitios + `--version` en empaquetado | yt_player.dart:422-451,520-529 |
| Mensajes crudos de yt-dlp al usuario | `stderr` en inglés | traducción por patrón | yt_player.dart:925-950 |

## 9. Huecos explícitos (no rellenar de memoria)

`subscribe`/`unsubscribe` · edición de playlists · `alt=`/`key=` · caducidad de cookies · versión y migraciones de los
JSON · dedupe entre páginas de continuación · cualquier endpoint de YouTube de vídeo/Shorts/comentarios (el original
solo usa `youtubei/v1` de **música** + `player`): **NO DOCUMENTADO**.
