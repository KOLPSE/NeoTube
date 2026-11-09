# API interna de YouTube (`youtubei/v1`): evidencia medida

Fecha de la ejecución que respalda este documento: **9 de noviembre de 2026, 12:53 UTC**.
Entorno: Windows, **Node 26.13.0** (probado también con el `fetch` de Node 24), IP pública española
(85.58.201.129), sin VPN. Sonda: `tools/probe.mjs` (`node tools/probe.mjs all`).
Fixtures: `tests/fixtures/*.json`, generadas por la misma ejecución.
Todo es **anónimo**: sin login, sin cookies de sesión y sin `yt-dlp`.

Este documento es evidencia, no diseño. Los números (`bytes`, `ms`, índices `[N]`) son los de **una
ejecución**; sirven para dimensionar buffers y decidir rutas de parseo, no como constantes. Dos
peticiones seguidas del mismo endpoint devolvieron 899.856 y 837.482 bytes para `search`, y la
mezcla de renderers cambió entre ejecuciones (por ejemplo `lockupViewModel`: 0 o 1 por página).

## Reproducción

```bash
node tools/probe.mjs all                 # todas las sondas + todas las fixtures + resumen
node tools/probe.mjs search              # una sola sonda (autónoma)
node tools/probe.mjs player-visionos     # perfil de reproducción
```

La sonda escribe una línea JSON (JSONL) por paso con URL, método, cabeceras, cuerpo, status, ms,
bytes y rutas del JSON; al final imprime un resumen compacto. Solo usa el `fetch` incorporado de
Node: sin dependencias.

> Transporte: `fetch` (undici) funcionó contra `youtube.com` en esta máquina. La página `/watch`
> pesa ~1,4 MB con cabeceras de respuesta grandes; si un cliente usa un límite de cabeceras bajo
> (undici por defecto en algunas versiones) fallará con `UND_ERR_HEADERS_OVERFLOW`, y con él toda
> la secuencia de arranque.

## Perfiles de cliente usados

| Perfil | `clientName` / cabecera | `clientVersion` | `X-Youtube-Client-Name` | Origen | Uso |
|---|---|---|---|---|---|
| WEB | `WEB` | `2.20261105.05.01` (leída de `/watch`) | `1` | `https://www.youtube.com` | search, browse, next, continuaciones |
| WEB_REMIX | `WEB_REMIX` | `1.20261104.00.00` (leída de music.youtube.com) | `67` | `https://music.youtube.com` | portada y búsqueda de YouTube Music |
| VISIONOS | `VISIONOS` | `1.02` | `101` | `https://www.youtube.com` | reproducción (audio y vídeo) |
| ANDROID_VR | `ANDROID_VR` | `1.65.10` | `28` | `https://www.youtube.com` | solo medición del tope de bytes; **no usar** |

Las `clientVersion` de WEB y WEB_REMIX **no se fijan a mano**: se leen del HTML de la visita
(`"INNERTUBE_CLIENT_VERSION":"…"`). Una versión inventada es la primera causa de respuestas vacías.

## Estado por endpoint

| Endpoint | Estado | Nota |
|---|---|---|
| `POST /youtubei/v1/search` (WEB) | Funciona | 22 items, continuación en `sectionListRenderer.contents[1]` |
| `POST /youtubei/v1/browse` `FEwhat_to_watch` (WEB) | **No disponible anónimo** | 200 con `feedNudgeRenderer` y 0 vídeos |
| `POST /youtubei/v1/next` (WEB) | Funciona | metadatos, relacionados y token de comentarios |
| `POST music.youtube.com/youtubei/v1/browse` `FEmusic_home` | Funciona con limitaciones | 3 secciones, contenido variable por visita |
| `POST music.youtube.com/youtubei/v1/search` | Funciona | 31 secciones, `musicCardShelfRenderer` como resultado principal |
| `POST /youtubei/v1/reel/reel_watch_sequence` | Funciona con limitaciones | exige `sequenceParams`; sin ellos 400 |
| `POST /youtubei/v1/reel/reel_item_watch` | No sirve | 200 con 1.971 B sin datos |
| `POST /youtubei/v1/player` (VISIONOS) | Funciona | 27 formatos adaptativos, audio 139/140/249/250/251 |
| `POST /youtubei/v1/player` (WEB) | No funciona | `UNPLAYABLE: Video unavailable` |
| `POST /youtubei/v1/player` (ANDROID_VR) | Funciona con limitaciones | ventana ≈1 MiB y luego 403 |
| `POST /youtubei/v1/search` / `next` con `continuation` | Funciona | ver §8 |

---

## 0. Arranque de la visita: `GET /watch`

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/watch?v=dQw4w9WgXcQ&bpctr=9999999999&has_verified=1` |
| Método | GET |
| Perfil | ninguno (petición de página, no de API) |
| Cabeceras | `user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15`<br>`accept-language: en-US,en;q=0.9`<br>`cookie: PREF=hl=en&tz=UTC; SOCS=CAI` |
| Cuerpo | — |
| Status | **200** |
| Tamaño / tiempo | 1.421.450 B / 1.137 ms |
| Se extrae | `"visitorData":"([^"]+)"` → `X-Goog-Visitor-Id`<br>`"INNERTUBE_CLIENT_VERSION":"([^"]+)"` → `2.20261105.05.01`<br>`"STS":(\d+)` → **20760** (firma de `playbackContext`) |
| Cookies que devuelve | `__Secure-YNID`, `YSC`, `__Secure-ROLLOUT_TOKEN`, `__Secure-YEC`, `VISITOR_INFO1_LIVE`, `VISITOR_PRIVACY_METADATA` (y `GPS`, descartada) |

La cookie de la visita (`VISITOR_INFO1_LIVE` + `__Secure-YNID` + `YSC`) se reenvía en todas las
peticiones `youtubei/v1`. `visitorData` va **también** en `context.client.visitorData` para el
reproductor y como cabecera `X-Goog-Visitor-Id` en el resto.

Qué **no** funcionó: buscar `signatureTimestamp` en el HTML (0 apariciones). El valor de firma está
en `"STS"`, dentro del `ytcfg`. Buscar `"sts"` en `ytInitialPlayerResponse` tampoco lo devuelve.

---

## 1. `search` (WEB)

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/search?prettyPrint=false` |
| Método | POST |
| Perfil | WEB (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | `content-type: application/json`<br>`user-agent: … Version/26.0 Safari/605.1.15`<br>`accept-language: en-US,en;q=0.9`<br>`origin: https://www.youtube.com`<br>`referer: https://www.youtube.com/`<br>`x-youtube-client-name: 1`<br>`x-youtube-client-version: 2.20261105.05.01`<br>`x-goog-visitor-id: <visitorData>`<br>`cookie: PREF=…; SOCS=CAI; VISITOR_INFO1_LIVE=…; __Secure-YNID=…; YSC=…` |
| Cuerpo | `{"context":{"client":{"clientName":"WEB","clientVersion":"2.20261105.05.01","hl":"en","gl":"US"}},"query":"lofi hip hop"}` |
| Status | **200** |
| Tamaño / tiempo | 899.856 B / 796 ms |
| Items | 22 en `contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[0].itemSectionRenderer.contents[]` |
| Mezcla | `videoRenderer: 19`, `lockupViewModel: 1`, `gridShelfViewModel: 2` (varía en cada ejecución) |

Cadena de rutas hasta los datos útiles:

```
contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[0]
  .itemSectionRenderer.contents[N]
    .videoRenderer
      .videoId
      .title.runs[].text
      .ownerText.runs[0].text                      → canal
      .ownerText.runs[0].navigationEndpoint.browseEndpoint.browseId → UC…
      .lengthText.simpleText                        → "3:21" (ausente en directos)
      .viewCountText.simpleText                     → "8,685 watching" en directos
      .publishedTimeText.simpleText
      .thumbnail.thumbnails[0].url
    .lockupViewModel                                → variante nueva del mismo vídeo
      .contentId / .contentType ("LOCKUP_CONTENT_TYPE_VIDEO")
      .metadata.lockupMetadataViewModel.title.content
      .rendererContext.commandContext.onTap.innertubeCommand.watchEndpoint.videoId
    .gridShelfViewModel                             → estante horizontal de Shorts
      .contents[N].shortsLockupViewModel
        .entityId ("shorts-shelf-item-<videoId>")
        .overlayMetadata.primaryText.content
        .overlayMetadata.secondaryText.content
        .onTap.innertubeCommand.reelWatchEndpoint
          .videoId / .playerParams / .params / .sequenceParams
contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[1]
  .continuationItemRenderer.continuationEndpoint.continuationCommand.token   → 600 caracteres
```

Segunda consulta de contraste (`"daft punk"`, 674.300 B, 200): mezcla
`officialCardViewModel: 1`, `videoRenderer: 17`, `lockupViewModel: 2`, `gridShelfViewModel: 2`.
En otra ejecución la misma consulta devolvió `channelRenderer: 1`.

Qué **no** funcionó / avisos:

- **No hay token de continuación dentro de `itemSectionRenderer.contents`**: el centinela es un
  **hermano** de sección (`sectionListRenderer.contents[1].continuationItemRenderer`). Buscar
  `continuationItemRenderer` en la lista de items devuelve vacío y deja la paginación muerta.
- `adSlotRenderer`: 0 apariciones en todas las ejecuciones (sin sesión no se sirven anuncios).
- `channelRenderer` y `playlistRenderer`: dependen de la consulta; con `"lofi hip hop"` no
  aparecieron y con `"daft punk"` aparecieron 0–1 según la ejecución. Un parser no puede
  presuponerlos.
- No asumir `contents[0]` como vídeo: en esta ejecución `contents[0]` era un `lockupViewModel` y
  `contents[18]`/`[21]` eran estantes de Shorts.

---

## 2. `browse` `FEwhat_to_watch` (WEB) — portada

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/browse?prettyPrint=false` |
| Método | POST |
| Perfil | WEB (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | las mismas de §1 |
| Cuerpo | `{"context":{"client":{"clientName":"WEB","clientVersion":"2.20261105.05.01","hl":"en","gl":"US"}},"browseId":"FEwhat_to_watch"}` |
| Status | **200** (pero sin feed) |
| Tamaño / tiempo | 116.653 B / 130 ms |
| Contenido real | `contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.richGridRenderer.contents[0].richSectionRenderer.content.feedNudgeRenderer` |
| Texto del aviso | título `Try searching to get started`; subtítulo `Start watching videos to help us build a feed of videos you'll love.` |
| Vídeos | **0** (`richItemRenderer`, `videoRenderer`, `lockupViewModel`, `adSlotRenderer`: 0 apariciones) |
| Continuación | no hay token (no hay lista que paginar) |

Qué **no** funcionó (medido, no supuesto):

- El feed de portada **no se sirve a visitantes anónimos sin historial**: es un `feedNudgeRenderer`
  y nada más. Se probó además: (a) visitar antes la página `/watch`, (b) emitir `player` y `next`
  de un vídeo y esperar 3 s, (c) `visitorData` nuevo leído de la propia portada, (d) con y sin
  cookies. En los cuatro casos: 200 con el mismo aviso y 0 vídeos.
- `FEtrending` y `FEexplore` → **400** (285 B) `{"code":400,"message":"Request contains an invalid
  argument."}`. Con `params: "6gQJRkVleHBsb3Jl"` también 400. No son una alternativa anónima.
- `FEsubscriptions` y `FEhistory` responden 200 pero con `sectionListRenderer` vacío (requieren
  sesión).

Consecuencia para NeoTube 2: la pantalla de inicio **necesita** el `visitorData`/cookies del usuario
(o una cuenta), y aun así el feed puede llegar vacío. El cliente debe renderizar el aviso, no una
rejilla vacía, y ofrecer búsqueda como salida. La fixture `home.json` guarda exactamente esta
respuesta real (1.395 B): sirve para probar el camino "portada vacía".

---

## 3. `next` (WEB) — página de vídeo

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/next?prettyPrint=false` |
| Método | POST |
| Perfil | WEB (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | las mismas de §1 |
| Cuerpo | `{"context":{"client":{"clientName":"WEB","clientVersion":"2.20261105.05.01","hl":"en","gl":"US"}},"videoId":"dQw4w9WgXcQ","contentCheckOk":true,"racyCheckOk":true}` |
| Status | **200** |
| Tamaño / tiempo | 459.546 B / 991 ms |
| Bloques superiores | 5 en `contents.twoColumnWatchNextResults.results.results.contents[]`: `ticketShelfRenderer`, `videoPrimaryInfoRenderer`, `videoSecondaryInfoRenderer`, `compositeVideoPrimaryInfoRenderer`, `itemSectionRenderer` |
| Relacionados | 21–22 en `contents.twoColumnWatchNextResults.secondaryResults.secondaryResults.results[]` (`lockupViewModel` ×20, `reelShelfRenderer` ×1 en una ejecución, `continuationItemRenderer` ×1) |

Cadena de rutas:

```
contents.twoColumnWatchNextResults.results.results.contents[1].videoPrimaryInfoRenderer
  .title.runs[].text                        → título
  .viewCount.videoViewCountRenderer.viewCount.runs[].text → "1,835,373,950 views"
  .videoActions.menuRenderer.topLevelButtons[0]
    .segmentedLikeDislikeButtonViewModel…buttonViewModel.title → "19M" / "Dislike"
contents.twoColumnWatchNextResults.results.results.contents[2].videoSecondaryInfoRenderer
  .owner.videoOwnerRenderer.title.runs[].text
  .owner.videoOwnerRenderer.subscriberCountText.simpleText
  .owner.videoOwnerRenderer.navigationEndpoint.browseEndpoint.browseId → UC…
  .attributedDescription.content            → descripción completa (texto plano)
contents.twoColumnWatchNextResults.secondaryResults.secondaryResults.results[N].lockupViewModel
  .contentType ("LOCKUP_CONTENT_TYPE_VIDEO") / .contentId
  .metadata.lockupMetadataViewModel.title.content
  .rendererContext.commandContext.onTap.innertubeCommand.watchEndpoint.videoId
contents.twoColumnWatchNextResults.secondaryResults.secondaryResults.results[21]
  .continuationItemRenderer.continuationEndpoint.continuationCommand.token    → 2.674 caracteres
contents.twoColumnWatchNextResults.results.results.contents[4].itemSectionRenderer
  .sectionIdentifier "comment-item-section"
  .contents[0].continuationItemRenderer.continuationEndpoint.continuationCommand.token → 78 caracteres
engagementPanels[N].engagementPanelSectionListRenderer
  .panelIdentifier ∈ {engagement-panel-comments-section, engagement-panel-structured-description,
                      engagement-panel-searchable-transcript, engagement-panel-ads-business-agent-chat}
```

Qué **no** funcionó / avisos:

- `commentSectionRenderer`: 0 apariciones. Los comentarios **no vienen** en esta respuesta: solo el
  token de 78 caracteres. Hay que pedirlos con `continuation` (§8c).
- El panel de descripción (`engagement-panel-structured-description`, ~54 KB) duplica la
  descripción de `attributedDescription`. Parsear la segunda y no el panel.
- El panel de comentarios llega con `visibility: ENGAGEMENT_PANEL_VISIBILITY_HIDDEN` y `hasContent:
  true`; no implica que el contenido esté cargado.
- Los me gusta no son un número en el renderer de acciones: hay que recorrer
  `segmentedLikeDislikeButtonViewModel` → `buttonViewModel.title` (devolvió `["19M","19M","Dislike","Dislike"]`).

---

## 4. `browse` `FEmusic_home` (WEB_REMIX) — portada de música

| Campo | Valor observado |
|---|---|
| URL | `https://music.youtube.com/youtubei/v1/browse?prettyPrint=false` |
| Método | POST |
| Perfil | WEB_REMIX (`clientName: "WEB_REMIX"`, `clientVersion: "1.20261104.00.00"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | `content-type: application/json`<br>`user-agent: … Version/26.0 Safari/605.1.15`<br>`origin: https://music.youtube.com`<br>`referer: https://music.youtube.com/`<br>`x-youtube-client-name: 67`<br>`x-youtube-client-version: 1.20261104.00.00`<br>`x-goog-visitor-id: <visitorData>`<br>`cookie: <las de §0>` |
| Cuerpo | `{"context":{"client":{"clientName":"WEB_REMIX","clientVersion":"1.20261104.00.00","hl":"en","gl":"US"}},"browseId":"FEmusic_home"}` |
| Status | **200** |
| Tamaño / tiempo | 352.840 B / 398 ms (en otra ejecución: 173.699 B) |
| Secciones | 3 en `contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[]` |
| Tipos | `musicCarouselShelfRenderer` "Quick picks" (20 items), `musicTastebuilderShelfRenderer` (aviso de personalización), `musicCarouselShelfRenderer` "Featured playlists for you" (5 items) |

Cadena de rutas:

```
contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[i]
  .musicCarouselShelfRenderer
    .header.musicCarouselShelfBasicHeaderRenderer.title.runs[].text
    .contents[N]
      .musicResponsiveListItemRenderer            → canción o álbum
        .flexColumns[0].musicResponsiveListItemFlexColumnRenderer.text.runs[].text → título
        .flexColumns[1]…[2]                        → "Artista • 2.2B plays", álbum
        .playlistItemData.videoId                  → id de la canción (cuando está)
        .overlay.musicItemThumbnailOverlayRenderer.content.musicPlayButtonRenderer
          .playNavigationEndpoint.watchEndpoint.videoId
          .playNavigationEndpoint.watchEndpoint…watchEndpointMusicConfig.musicVideoType
                                                   ("MUSIC_VIDEO_TYPE_ATV" = canción)
      .musicTwoRowItemRenderer                    → playlist, álbum, artista
        .title.runs[].text
        .navigationEndpoint.browseEndpoint.browseId
        .navigationEndpoint.browseEndpoint…browseEndpointContextMusicConfig.pageType
                                                   ("MUSIC_PAGE_TYPE_PLAYLIST", …)
```

Qué **no** funcionó / avisos:

- `musicShelfRenderer`: 0 apariciones en la portada de esta ejecución (solo carruseles). El parser
  debe aceptar ambos (`musicCarouselShelfRenderer` y `musicShelfRenderer`) porque la portada cambia
  por experimento.
- El número de items por carrusel varió entre ejecuciones (20 → 10; 10 → 5) para la misma visita.
- `musicTastebuilderShelfRenderer` no es contenido: es el aviso de "elige artistas". Hay que
  descartarlo explícitamente; si se trata como estante, la UI muestra una sección vacía.
- `musicTwoRowItemRenderer.title`/`.subtitle` no siempre existen como `runs`: usar `textOf` con las
  tres formas (`simpleText`, `runs[]`, `content`).

---

## 5. `search` (WEB_REMIX) — búsqueda de música

| Campo | Valor observado |
|---|---|
| URL | `https://music.youtube.com/youtubei/v1/search?prettyPrint=false` |
| Método | POST |
| Perfil | WEB_REMIX (`clientName: "WEB_REMIX"`, `clientVersion: "1.20261104.00.00"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | las mismas de §4 |
| Cuerpo | `{"context":{"client":{"clientName":"WEB_REMIX","clientVersion":"1.20261104.00.00","hl":"en","gl":"US"}},"query":"daft punk"}` |
| Status | **200** |
| Tamaño / tiempo | 240.471 B / 540 ms |
| Secciones | 31 en `contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[]` (`itemSectionRenderer` ×30, `musicCardShelfRenderer` ×1) |
| Resultado principal | `musicCardShelfRenderer`: `Daft Punk`, `Artist • 67.4M monthly audience`, `onTap.browseEndpoint.browseId = UCRr1xG_2WIDs18a6cIiCxeA` |

Cadena de rutas:

```
contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[0]
  .itemSectionRenderer.contents[0]  → messageRenderer (aviso, NO es un resultado: saltar)
contents.tabbedSearchResultsRenderer…contents[1].musicCardShelfRenderer
  .title.runs[].text / .subtitle     → resultado principal
  .onTap.browseEndpoint.browseId     → artista/álbum/playlist
  .contents[]                        → accesos rápidos del propio card
contents.tabbedSearchResultsRenderer…contents[N].itemSectionRenderer.contents[M]
  .musicResponsiveListItemRenderer
    .flexColumns[0] → título ("Random Access Memories", "Face to Face")
    .flexColumns[1] → tipo y autor ("Album • Daft Punk • 2013", "Song • Daft Punk")
    .navigationEndpoint.browseEndpoint.browseId            → álbum (MPREb_…) o artista (UC…)
    .navigationEndpoint.browseEndpoint…pageType            ("MUSIC_PAGE_TYPE_ALBUM", …)
    .overlay.musicItemThumbnailOverlayRenderer.content.musicPlayButtonRenderer
      .playNavigationEndpoint.watchEndpoint.videoId        → id de la canción ("qXI87eMP-bs")
      .playNavigationEndpoint.watchEndpoint…musicVideoType ("MUSIC_VIDEO_TYPE_ATV")
      .playNavigationEndpoint.watchPlaylistEndpoint.playlistId
```

Qué **no** funcionó / avisos:

- `musicTwoRowItemRenderer`: 0 apariciones en la búsqueda (es exclusivo de portadas/carruseles).
- La primera sección es un `messageRenderer`: iterar sin discriminarla produce un item "vacío".
- El id de una **canción** no está en `navigationEndpoint` (que apunta al álbum): está en
  `overlay…musicPlayButtonRenderer.playNavigationEndpoint.watchEndpoint.videoId`. Sin esa ruta, las
  canciones quedan sin `videoId` y no se pueden reproducir.

---

## 6. Shorts: descubrimiento y secuencia

### 6a. Descubrimiento por `search`

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/search?prettyPrint=false` |
| Método | POST |
| Perfil | WEB (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | las mismas de §1 |
| Cuerpo | `{"context":{"client":{…}},"query":"shorts"}` |
| Status | **200** |
| Tamaño / tiempo | 764.180 B / 1.820 ms |
| Estantes | `contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[0].itemSectionRenderer.contents[0 | 18].gridShelfViewModel` |
| Items | 5 en `…gridShelfViewModel.contents[].shortsLockupViewModel` |

Cadena de rutas por Short:

```
…itemSectionRenderer.contents[N].gridShelfViewModel.contents[M].shortsLockupViewModel
  .entityId                                     → "shorts-shelf-item-<videoId>"
  .overlayMetadata.primaryText.content           → título
  .overlayMetadata.secondaryText.content         → "208K views"
  .onTap.innertubeCommand.reelWatchEndpoint
    .videoId
    .playerParams
    .params                                      → necesarios para la secuencia
    .sequenceParams                              → necesarios para la secuencia
```

### 6b. `reel/reel_watch_sequence`

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/reel/reel_watch_sequence?prettyPrint=false` |
| Método | POST |
| Perfil | WEB (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`, `hl: "en"`, `gl: "US"`) |
| Cabeceras | las mismas de §1 |
| Cuerpos probados | (1) `{context, videoId}` → **400** (285 B)<br>(2) `{context, videoId, params}` → **400** (285 B)<br>(3) `{context, videoId, params, sequenceParams}` → **200** |
| Tamaño / tiempo (caso 3) | 712.830 B / 340 ms |
| Entradas | 2 en `entries[]` (`y87gRvFWcJ4`, `50MR8HO7cy0`) |

```
entries[N].command.reelWatchEndpoint.videoId
entries[N].command.reelWatchEndpoint.playerParams
entries[N].command.reelWatchEndpoint.overlay.reelPlayerOverlayRenderer…
entries[0].command.reelWatchEndpoint.unserializedPrefetchData.reelItemWatchResponse
  .engagementPanels[2].engagementPanelSectionListRenderer.content.richGridRenderer.contents[0]
    .continuationItemRenderer.continuationEndpoint.continuationCommand.token   → comentarios del Short
continuationEndpoint                                        → token para seguir la secuencia
```

### 6c. `reel/reel_item_watch`

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/reel/reel_item_watch?prettyPrint=false` |
| Método | POST |
| Perfil | WEB |
| Cuerpo | `{"context":{…},"videoId":"ppYUGr3UxU8"}` |
| Status | **200** |
| Tamaño | 1.971 B |
| Claves | `responseContext`, `status`, `trackingParams`, `tooltip`, `background` |
| Datos útiles | **ninguno** (0 `reelWatchEndpoint`) |

### 6d. Reproducción de un Short con VISIONOS

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/player?prettyPrint=false` |
| Método | POST |
| Perfil | VISIONOS (el mismo de §7) |
| Cuerpo | `{"context":{"client":{…VISIONOS…},"playbackContext":{"contentPlaybackContext":{"html5Preference":"HTML5_PREF_WANTS","signatureTimestamp":20760}}},"videoId":"ppYUGr3UxU8","contentCheckOk":true,"racyCheckOk":true}` |
| Status | **200** |
| Tamaño / tiempo | 265.828 B / 104 ms |
| Formatos | **115** adaptativos: 22 de vídeo + 95 de audio |
| Audio | 19 pistas × 5 itags (139/140/249/250/251), p. ej. `audioTrack.id` = `en-US.4` (`displayName: "English (US) original"`, `audioIsDefault: true`), `bn.10`, `de-DE.10`, `es-US.10`, `fr-FR.10`, `hi.10`, `id.10`, `it.10`, `iw.10`, `ja.10`, `ko.10`, `ml.10`, `nl-NL.10`, `pa.10`, `pl.10`, `pt-BR.10`, `ta.10`, `te.10`, `uk.10` (todas `isAutoDubbed: true` salvo la original) |

Qué **no** funcionó / avisos:

- `reel_watch_sequence` sin `sequenceParams` → **400**; sin `params` → **400**. No hay forma de pedir
  la secuencia con solo `videoId`.
- `reel_item_watch` → 200 sin datos: no sirve para descubrir ni para metadatos.
- Los Shorts con doblaje automático traen **formatos duplicados por pista de audio**: hay que
  agrupar por `itag` + `audioTrack.id` y elegir `audioTrack.audioIsDefault` (o el idioma preferido).
  Un `HashMap<itag, format>` se queda con una pista arbitraria.

---

## 7. `player` con VISIONOS (reproducción)

| Campo | Valor observado |
|---|---|
| URL | `https://www.youtube.com/youtubei/v1/player?prettyPrint=false` |
| Método | POST |
| Perfil | VISIONOS (`clientName: "VISIONOS"`, `clientVersion: "1.02"`, `deviceMake: "Apple"`, `deviceModel: "RealityDevice17,1"`, `osName: "visionOS"`, `osVersion: "26.5.23O471"`, `hl: "en"`, `timeZone: "UTC"`, `utcOffsetMinutes: 0`, `visitorData`) |
| Cabeceras | `content-type: application/json`<br>`user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15`<br>`accept-language: en-US,en;q=0.9`<br>`origin: https://www.youtube.com`<br>`referer: https://www.youtube.com/`<br>`x-youtube-client-name: 101`<br>`x-youtube-client-version: 1.02`<br>`x-goog-visitor-id: <visitorData>`<br>`cookie: PREF=…; SOCS=CAI; VISITOR_INFO1_LIVE=…; __Secure-YNID=…; YSC=…` |
| Cuerpo | `{"context":{"client":{"clientName":"VISIONOS","clientVersion":"1.02","deviceMake":"Apple","deviceModel":"RealityDevice17,1","osName":"visionOS","osVersion":"26.5.23O471","hl":"en","timeZone":"UTC","utcOffsetMinutes":0,"visitorData":"<vd>"},"playbackContext":{"contentPlaybackContext":{"html5Preference":"HTML5_PREF_WANTS","signatureTimestamp":20760}}},"videoId":"dQw4w9WgXcQ","contentCheckOk":true,"racyCheckOk":true}` |
| Status | **200**, `playabilityStatus.status = "OK"` |
| Tamaño / tiempo | 90.425 B / 99 ms |
| Formatos | 27 adaptativos, **0 muxed** (`streamingData.formats` no existe) |

Pistas de audio (todas presentes, `streamingData.adaptiveFormats[]` con `mimeType ^= "audio/"`):

| itag | mimeType | bitrate | contentLength | audioQuality | sampleRate | canales |
|---|---|---|---|---|---|---|
| 139 | `audio/mp4; codecs="mp4a.40.5"` | 50.152 | 1.300.631 | AUDIO_QUALITY_LOW | 22.050 | 2 |
| 140 | `audio/mp4; codecs="mp4a.40.2"` | 130.677 | 3.449.447 | AUDIO_QUALITY_MEDIUM | 44.100 | 2 |
| 249 | `audio/webm; codecs="opus"` | 49.496 | 1.231.355 | AUDIO_QUALITY_LOW | 48.000 | 2 |
| 250 | `audio/webm; codecs="opus"` | 65.508 | 1.628.559 | AUDIO_QUALITY_LOW | 48.000 | 2 |
| 251 | `audio/webm; codecs="opus"` | 136.544 | 3.433.755 | AUDIO_QUALITY_MEDIUM | 48.000 | 2 |

Pistas de vídeo (22; catálogo completo de esta ejecución):

| itag | códec / contenedor | qualityLabel | resolución | fps | bitrate | contentLength |
|---|---|---|---|---|---|---|
| 313 | video/webm vp9 | 2160p | 3840×2160 | 25 | 18.076.636 | 358.608.461 |
| 401 | video/mp4 av01 | 2160p | 3840×2160 | 25 | 17.400.774 | 240.334.643 |
| 271 | video/webm vp9 | 1440p | 2560×1440 | 25 | 8.978.328 | 151.103.346 |
| 400 | video/mp4 av01 | 1440p | 2560×1440 | 25 | 7.755.221 | 122.168.277 |
| 137 | video/mp4 avc1 | 1080p | 1920×1080 | 25 | 4.334.157 | 80.911.999 |
| 248 | video/webm vp9 | 1080p | 1920×1080 | 25 | 1.549.172 | 30.846.580 |
| 399 | video/mp4 av01 | 1080p | 1920×1080 | 25 | 1.594.543 | 30.415.996 |
| 136 | video/mp4 avc1 | 720p | 1280×720 | 25 | 1.058.310 | 26.455.880 |
| 247 | video/webm vp9 | 720p | 1280×720 | 25 | 867.871 | 17.686.717 |
| 398 | video/mp4 av01 | 720p | 1280×720 | 25 | 1.012.348 | 17.593.076 |
| 135 | video/mp4 avc1 | 480p | 854×480 | 25 | 690.764 | 14.103.519 |
| 244 | video/webm vp9 | 480p | 854×480 | 25 | 466.632 | 9.381.481 |
| 397 | video/mp4 av01 | 480p | 854×480 | 25 | 525.728 | 9.874.826 |
| 134 | video/mp4 avc1 | 360p | 640×360 | 25 | 450.863 | 8.390.921 |
| 243 | video/webm vp9 | 360p | 640×360 | 25 | 288.453 | 6.014.765 |
| 396 | video/mp4 av01 | 360p | 640×360 | 25 | 304.466 | 5.685.561 |
| 133 | video/mp4 avc1 | 240p | 426×240 | 25 | 236.224 | 4.310.122 |
| 242 | video/webm vp9 | 240p | 426×240 | 25 | 131.647 | 2.706.929 |
| 395 | video/mp4 av01 | 240p | 426×240 | 25 | 159.709 | 3.095.447 |
| 160 | video/mp4 avc1 | 144p | 256×144 | 25 | 109.973 | 2.058.142 |
| 278 | video/webm vp9 | 144p | 256×144 | 25 | 68.500 | 1.540.844 |
| 394 | video/mp4 av01 | 144p | 256×144 | 25 | 83.289 | 1.505.504 |

Resto de rutas útiles:

```
playabilityStatus.status / .reason
videoDetails.videoId / .title / .lengthSeconds ("213") / .author / .isLiveContent / .viewCount
streamingData.adaptiveFormats[]                       → audio y vídeo separados
streamingData.hlsManifestUrl                          → presente (no usado)
captions.playerCaptionsTracklistRenderer.captionTracks[]
  .languageCode / .kind ("asr") / .name / .baseUrl / .isTranslatable
streamingData.expiresInSeconds                        → 21.540 (≈6 h)
```

Parámetros de las URLs de `googlevideo` (esta ejecución, itag 140):

```
expire, ei, ip, id, itag, source, requiressl, xpc, cps, met, mh, mm, mn, ms, mv, mvi,
pcm2cms, pl, rms, pcm2, initcwndbps, bui, spc, vprv, svpuc, mime, rqh, gir, clen, dur,
lmt, mt, fvip, epbp, keepalive, fexp, c, txp, sparams, sig, lsparams, lsig
rqh=1 · gir=yes · keepalive=yes · expire−ahora = 21.600 s · ip=<IP del cliente>
sig/lsig en claro · sin parámetro n (nsig) · sin pot (poToken)
```

Subtítulos vistos (6): `en`, `en` (`kind: asr`), `de-DE`, `ja`, `pt-BR`, `es-419`; todos con
`baseUrl` de `api/timedtext`.

Qué **no** funcionó (control negativo y variantes):

| Prueba | Status | Resultado |
|---|---|---|
| Mismo `/player` con perfil **WEB** (`clientName: "WEB"`, `clientVersion: "2.20261105.05.01"`) | 200 | `playabilityStatus.status = "UNPLAYABLE"`, `reason: "Video unavailable"`, **0 formatos**, 14.723 B |
| VISIONOS **sin** cabecera `cookie` | 200 | `OK`, 27 formatos — no se apoya en eso |
| VISIONOS **sin** `signatureTimestamp` | 200 | `OK`, 27 formatos — no se apoya en eso |

Las dos variantes "sin cookie" y "sin `sts`" respondieron bien en esta ejecución, pero el perfil
canónico las envía: son las primeras en degradarse cuando cambian los experimentos, y sin cookies
la URL puede quedar ligada a una visita distinta. No se documentan como perfil válido.

---

## 8. Continuaciones

| Caso | Petición | Status | Bytes / ms | Ruta de los items |
|---|---|---|---|---|
| (a) `search` página 2 | `POST /search` con `{"context":{…},"continuation":"<600 car.>"}` | **200** | 756.973 B / 915 ms | `onResponseReceivedCommands[0].appendContinuationItemsAction.continuationItems[]` (2: `itemSectionRenderer` + `continuationItemRenderer`) |
| (b) `next` relacionados página 2 | `POST /next` con el token de `secondaryResults` (2.674 car.) | **200** | 231.023 B / 810 ms | `onResponseReceivedEndpoints[0].appendContinuationItemsAction.continuationItems[]` (21: 20 `lockupViewModel` + `continuationItemRenderer`) |
| (c) `next` comentarios página 1 | `POST /next` con el token de `comment-item-section` (78 car.) | **200** | 292.077 B / 306 ms | `onResponseReceivedEndpoints[]` — 2 endpoints |
| (d) `browse` portada página 2 | — | no aplica | — | no hay token (portada vacía) |

Detalle de (c): `onResponseReceivedEndpoints[0].reloadContinuationItemsCommand.continuationItems`
trae solo `commentsHeaderRenderer`; las hebras están en
`onResponseReceivedEndpoints[1].reloadContinuationItemsCommand.continuationItems[]` → 20
`commentThreadRenderer` + 1 `continuationItemRenderer`.

```
commentThreadRenderer.commentViewModel.commentViewModel.commentId     → id del comentario
commentThreadRenderer.replies.commentRepliesRenderer.viewReplies.buttonRenderer.text.runs[0].text
                                                                     → "628 replies"
frameworkUpdates.entityBatchUpdate.mutations[].payload.commentEntityPayload
  .properties.content.content     → texto ("Let's put the meme aside, …")
  .properties.publishedTime       → "6 years ago"
  .author.displayName             → "@LK-ht7qz"
  .toolbar.likeCountNotliked      → "221K"
```

Token siguiente de (a): `onResponseReceivedCommands[0].appendContinuationItemsAction.continuationItems[1].continuationItemRenderer.continuationEndpoint.continuationCommand.token`.

Qué **no** funcionó / avisos:

- `findKeyPaths(json, "continuationItems")` devuelve la **primera** aparición, que en comentarios
  es la del endpoint `[0]` (solo cabecera). Hay que recorrer `onResponseReceivedEndpoints` y elegir
  el endpoint que traiga `commentThreadRenderer`.
- El texto del comentario **no está** en `commentThreadRenderer`: está en
  `frameworkUpdates.entityBatchUpdate.mutations[].payload.commentEntityPayload`. Un cliente que
  ignore `frameworkUpdates` no puede mostrar comentarios (y las fixtures lo descartan: §10).
- El token de search (600 car.) caduca: pedirlo de nuevo si la página 2 responde 400.

---

## 9. Rangos y caps

Todas las URLs de media de estos clientes llevan **`rqh=1`**: el servidor espera `Range`. Lo medido
(`rangeGet` lee como máximo 128 KiB por petición; `bytesRead` es lo descargado, `contentRange` lo
concedido).

### 9a. VISIONOS — sin cap de ventana

| itag | `contentLength` | `Range` enviado | Status | `content-range` devuelto | bytes leídos |
|---|---|---|---|---|---|
| 140 (audio) | 3.449.447 | *(ninguno)* | **200** | — (`content-length: 3449447`) | 131.072 |
| 140 | 3.449.447 | `bytes=0-1023` | **206** | `bytes 0-1023/3449447` | 1.024 |
| 140 | 3.449.447 | `bytes=0-` | **206** | `bytes 0-3449446/3449447` | 131.072 |
| 140 | 3.449.447 | `bytes=1724723-` | **206** | `bytes 1724723-3449446/3449447` | 131.072 |
| 140 | 3.449.447 | `bytes=3400000-3449446` | **206** | `bytes 3400000-3449446/3449447` | 49.447 |
| 313 (vídeo) | 358.608.461 | *(ninguno)* | **200** | — (`content-length: 358608461`) | 131.072 |
| 313 | 358.608.461 | `bytes=0-` | **206** | `bytes 0-358608460/358608461` | 131.072 |
| 313 | 358.608.461 | `bytes=0-1023` | **206** | `bytes 0-1023/358608461` | 1.024 |

Lectura: con VISIONOS se puede pedir el fichero completo (`Range: bytes=0-` responde 206 y
`content-range` abarca todo), rangos pequeños y colas. `rqh=1` no exige `Range` en la práctica
(sin `Range` responde 200), pero enviarlo es lo correcto y evita descargar 358 MB por error.

### 9b. ANDROID_VR — ventana ≈1 MiB y 403

Recorrido secuencial por bloques de 65.536 B sobre una URL **recién emitida** por cada itag:

| itag | `contentLength` | Último bloque OK (offset) | Primer offset que devuelve 403 | Bytes servidos antes del 403 |
|---|---|---|---|---|
| 140 (audio) | 3.449.447 | **983.040** (206, 65.536 B) | **1.048.576** | **1.048.576 B (1 MiB)** |
| 251 (audio) | 3.433.755 | 851.968 (206, 65.536 B) | **917.504** | 917.504 B |
| 139 (audio) | 1.300.631 | 393.216 (206, 65.536 B) | **458.752** | 458.752 B |

El primer itag de la ejecución concede 1 MiB exacto (1.048.576 B); los siguientes, sobre la misma
visita, reciben menos (917.504 y 458.752 B). El tope no es una constante del fichero: es un
presupuesto que se agota y se degrada al repetir sondeos. Con bloques de 262.144 B el corte cae en
el mismo byte (1.048.576 para el itag 140).

Rangos sobre la misma URL (itag 140):

| `Range` | Status | `content-range` | Nota |
|---|---|---|---|
| *(ninguno)* | **403** | — | `rqh=1` sin `Range`: rechazado |
| `bytes=0-` | **403** | — | abierto: supera el tope |
| `bytes=0-1048575` | **206** | `bytes 0-1048575/3449447` | 1 MiB exacto: aceptado |
| `bytes=0-2097151` | **403** | — | 2 MiB: rechazado |
| `bytes=1048576-2097151` | **403** | — | empieza justo pasado el tope |

itag 18 (muxed, el único de `streamingData.formats`): la URL responde **403** tanto sin `Range`
como con `bytes=0-1023`; además `contentLength` llega vacío. (En una ejecución anterior, sobre una
visita intacta, esa misma URL sirvió `bytes=0-1023` con 206 y 11.829.048 B de tamaño: es
inestable. No se debe usar.)

Consecuencia para NeoTube 2: **no** usar ANDROID_VR para reproducir. Solo VISIONOS entrega audio y
vídeo completos. Si algún día se usa VR como respaldo, hay que asumirte un tope de ~1 MiB por URL y
renovar la URL (`/player`) al agotarlo, lo que hace inviable un vídeo de 213 s.

---

## 10. Fixtures

| Archivo | Bytes | Contenido | Origen |
|---|---|---|---|
| `search.json` | 136.407 | 22 items de `itemSectionRenderer` + `continuationItemRenderer`; `search` "lofi hip hop" | §1 |
| `home.json` | 1.395 | `feedNudgeRenderer` real (portada vacía); sirve para el estado "sin feed" | §2 |
| `watch_next.json` | 176.944 | 5 bloques de `results` + 21 relacionados + paneles + tokens | §3 |
| `music_home.json` | 89.048 | 3 secciones de `sectionListRenderer` (carruseles reales) | §4 |
| `music_search.json` | 84.439 | 31 secciones; card principal + ítems de búsqueda | §5 |
| `player_visionos.json` | 30.221 | `playabilityStatus`, `streamingData` (27 formatos), `captions`, `videoDetails`, `attestation` | §7 |

Reglas de recorte aplicadas por `tools/probe.mjs`:

- Máximo **40 items por array** (`KEEP_FULL_ARRAYS` excluye `translationLanguages` y `keywords`, que
  son metadatos y se conservan completos). Se conserva siempre el **centinela de continuación**
  aunque quede fuera del cupo, para no perder el token de la página siguiente.
- Se eliminan ramas que no participan en ninguna ruta de parseo: `responseContext`,
  `frameworkUpdates`, `topbar`, `playerConfig`, `storyboards`, `annotations`, `playbackTracking`.
- Se elimina decoración por item: menús (`menu`, `menuButton`, `menuOnTap`), previews
  (`richThumbnail`, `movingThumbnailRenderer`), `thumbnailOverlays`, avatares, `themedPalette`,
  `inlinePlayerData`, `playerOverlays`, `impressionEndpoints`, etc. Por eso los comentarios del
  fixture `watch_next.json` no incluyen texto: ese texto vive en `frameworkUpdates`.
- Strings de hasta 360 caracteres (se recortan con `…`).
- URLs: se eliminan los parámetros de seguimiento `sqp` y `rs` de las miniaturas; las URLs de
  publicidad (`doubleclick.net`, `googleadservices.com`, `googlesyndication.com`) se sustituyen por
  `[url-de-tracking-eliminada]` y las de `googlevideo.com` por `[url-googlevideo-eliminada]`.
- `player_visionos.json`: **toda** `url` de `streamingData.formats`/`adaptiveFormats` se sustituye
  por `https://example.invalid/videoplayback?itag=<itag>&clen=<contentLength>`; se conservan
  `itag`, `mimeType`, `bitrate`, `width`, `height`, `fps`, `audioChannels`, `contentLength`,
  `initRange`, `indexRange`, `audioQuality`, `audioSampleRate`, `approxDurationMs`…; se elimina
  `signatureCipher` si apareciera. `hlsManifestUrl`, `dashManifestUrl` y `serverAbrStreamingUrl` se
  sustituyen por `https://example.invalid/<campo>` y cada `captions…captionTracks[].baseUrl` por
  `https://example.invalid/api/timedtext?v=<videoId>&lang=<languageCode>`. Ninguna URL firmada se
  versiona: caducan en ~6 h y están ligadas a la IP.
- JSON compacto: la indentación de 1 espacio inflaba ~50% y obligaba a recortar items reales para
  respetar el tope de 300 KB.

Nombres sustituidos: `search_video.json` → `search.json`, `player_video.json` +
`player_audio.json` → `player_visionos.json`. `shorts.json` ya no se genera (la evidencia de Shorts
se emite por consola en la sonda `shorts`).

---

## 11. Riesgos

- **La API interna no tiene contrato.** En una misma semana `search` devolvió vídeos como
  `videoRenderer` (19), `lockupViewModel` (1) y `officialCardViewModel` (1) en páginas distintas de
  la misma ejecución. Cualquier parser debe **iterar y discriminar por clave**, nunca asumir
  `contents[0]` ni un índice fijo. Las fixtures fijan solo las variantes vistas en esta fecha.
- **Portada vacía sin historial.** `FEwhat_to_watch` devuelve 200 con `feedNudgeRenderer` y 0
  vídeos a cualquier visitante anónimo (probado con y sin cookies, con y sin visita previa). La
  pantalla de inicio no puede prometer contenido si el usuario no tiene sesión/historial.
- **poToken.** En esta ejecución las URLs de VISIONOS **no** llevan `pot`; la firma va en
  `sig`/`lsig` y no hay parámetro `n`. Si YouTube empieza a exigir `poToken` para este cliente, las
  peticiones de media devolverán 403 con el player aún en `OK`. Hay que detectar "player OK +
  403 en media" y degradar con un mensaje claro, no en bucle de reintentos.
- **Comprobaciones anti-bot.** El cliente WEB del `/player` devuelve `UNPLAYABLE: "Video
  unavailable"` (aunque `/next` y `/search` funcionen con el mismo WEB), y la portada llega vacía.
  Ambas cosas apuntan a evaluación de visita/IP: el cliente debe asumir que un 200 no implica
  reproducibilidad y verificar el primer `Range` antes de pintar "reproducir".
- **Caducidad y ligadura de URL.** `expire−ahora = 21.600 s` e `ip=<IP del cliente>`. Las URLs no
  se pueden cachear ni compartir entre equipos/redes; hay que pedir `/player` de nuevo antes de 6 h
  o cuando un `Range` devuelva 403. Los `baseUrl` de subtítulos caducan igual.
- **`rqh=1`.** Las URLs de ANDROID_VR rechazan peticiones sin `Range` (403) y las de VISIONOS
  aceptan 200. Enviar siempre `Range` es lo único que funciona con ambos y evita descargar cientos
  de MB por accidente.
- **Tope de bytes del VR (≈1 MiB).** Verificado: 1.048.576 B en el primer itag, menos en los
  siguientes de la misma visita. ANDROID_VR no sirve para reproducir; documentado para no
  reintentarlo.
- **Doblaje automático.** Un mismo vídeo/Short puede traer 19 pistas de audio × 5 itags. Elegir
  `audioTrack.audioIsDefault` o el idioma del usuario; deduplicar por `(itag, audioTrack.id)`.
- **`hl`/`gl` y personalización.** `hl=en, gl=US` se usó en todas las pruebas; cambiar `hl` altera
  textos y puede alterar la mezcla de renderers. El contenido de `FEmusic_home` y de `search`
  depende de la visita: dos ejecuciones dieron 20 y 10 items por carrusel.
- **Tokens largos y caducidad.** Los tokens de continuación miden 78–2.674 caracteres; los de
  relacionados (2.674) y search (600) caducan. Almacenarlos con su endpoint de origen y descartarlos
  ante 400.
- **Coste.** `search` ~0,8–0,9 MB y `next` ~0,46 MB por petición; `reel_watch_sequence` 0,7 MB para
  2 entries. Cachear por clave y no repedir al volver a una pantalla.

---

## 12. Resumen de la ejecución

```
bootstrap        watch-page    200    1421450B   1137ms
search           primary       200     899856B    796ms   (22 items, continuación 600 car.)
search           daft punk     200     674300B    907ms
home             primary       200     116653B    130ms   (feedNudgeRenderer, 0 vídeos)
home             FEtrending    400        285B     35ms   (invalid argument)
home             FEexplore     400        285B     33ms   (invalid argument)
watch-next       primary       200     459546B    991ms   (21 relacionados, comentarios 78 car.)
music-home       primary       200     352840B    398ms   (3 secciones)
music-search     primary       200     240471B    540ms   (31 secciones)
shorts           discovery     200     764180B   1820ms   (5 shorts)
shorts           sequence-400  400        285B     53ms   (solo videoId / +params)
shorts           sequence-400  400        285B     39ms
shorts           sequence-200  200     712830B    340ms   (2 entries, con sequenceParams)
shorts           reel_item     200       1971B     48ms   (sin datos)
shorts           player        200     265828B    104ms   (115 formatos, 19 pistas de audio)
player-visionos  primary       200      90425B     99ms   (27 formatos, audio 139/140/249/250/251)
player-visionos  WEB control   200      14723B     87ms   (UNPLAYABLE: Video unavailable)
player-vr        primary       200      75631B    211ms   (ventana 1.048.576 B)
continuations    search-2      200     756973B    915ms
continuations    next-2        200     231023B    810ms
continuations    comments-1    200     292077B    306ms   (20 hebras)
```

`FALLOS: ninguno` — los 400 y los 403 son respuestas esperadas de los casos negativos, no fallos de
la sonda.
