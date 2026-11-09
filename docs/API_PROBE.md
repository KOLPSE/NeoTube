# Validación en vivo de la API interna de YouTube (`youtubei/v1`)

Fecha de la prueba: **9 de noviembre de 2026**, ~12:58 UTC.
Máquina: Windows, Node 24.16.0, IP pública española (85.58.201.129).
Cliente de sonda: `tools/probe.mjs` (`node tools/probe.mjs <probe|all>`).
Fixtures: `tests/fixtures/*.json` (respuestas reales recortadas generadas por la misma sonda).

Condiciones de la prueba:

- Todo anónimo: **sin login, sin cookies de sesión, sin `yt-dlp`**.
- El `visitorData` se obtiene de `GET https://www.youtube.com/watch?v=dQw4w9WgXcQ&bpctr=9999999999&has_verified=1`
  con UA de Safari y `Cookie: PREF=hl=en&tz=UTC; SOCS=CAI`; se extrae con
  `/"visitorData":"([^"]+)"/` y se reenvía como cabecera `X-Goog-Visitor-Id`.
- `clientVersion` vigente leída de la misma página: **`2.20261105.05.01`** (`INNERTUBE_CLIENT_VERSION`).
- Todas las peticiones añaden `?prettyPrint=false` y cuerpo JSON.

> **Los tamaños en bytes y los índices `[N]` son los de una ejecución concreta, no constantes.**
> Verificado: el mismo `search` devolvió 962.998 y 898.487 bytes en ejecuciones separadas, y
> `continuations` 997.792 y 1.132.957. Sirven como orden de magnitud (y para dimensionar buffers),
> no como valores fijos. La mezcla de items (anuncios, tarjetas, estantes) cambia entre peticiones
> y por experimento: el mismo `search` devolvió 22 o 23 items en ejecuciones consecutivas. Un parser
> debe **iterar el array y discriminar por clave**, nunca asumir `contents[2]`.

> Nota de transporte importante para el cliente Rust: `fetch` de Node (undici) **falla** contra youtube.com
> con `UND_ERR_HEADERS_OVERFLOW` porque las cabeceras de respuesta superan el límite por defecto.
> La sonda usa `node:https` con `maxHeaderSize` ampliado. Un cliente HTTP con límite de cabeceras
> bajo fallará en `/watch` y posiblemente en `youtubei/v1` (respuestas de hasta ~1,4 MB de HTML).

---

## 1. `search` — POST `/youtubei/v1/search`

**URL**: `https://www.youtube.com/youtubei/v1/search?prettyPrint=false`
**Método**: POST
**Cabeceras**:

```
content-type: application/json
user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) ... Chrome/131.0.0.0 Safari/537.36
accept-language: es-ES,es;q=0.9
origin: https://www.youtube.com
referer: https://www.youtube.com/
x-youtube-client-name: 1
x-youtube-client-version: 2.20261105.05.01
x-goog-visitor-id: <visitorData>
```

**Cuerpo**:

```json
{
  "context": {
    "client": {
      "clientName": "WEB",
      "clientVersion": "2.20261105.05.01",
      "hl": "es",
      "gl": "ES"
    }
  },
  "query": "lofi hip hop"
}
```

**Estado HTTP**: `200`, `application/json; charset=UTF-8`, 962.998 bytes (23 items en la lista principal).

**Ruta a los items** (lista plana, mezcla heterogénea):

```
contents.twoColumnSearchResultsRenderer
  .primaryContents.sectionListRenderer.contents[0]
  .itemSectionRenderer.contents[]
```

**Renderers realmente presentes** (`query: "lofi hip hop"`):
`adSlotRenderer: 1`, `officialCardViewModel: 1`, `videoRenderer: 19`, `gridShelfViewModel: 2`.

| Renderer pedido | ¿Aparece? | Ruta del primer ejemplar |
|---|---|---|
| `videoRenderer` | **Sí** | `contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[0].itemSectionRenderer.contents[2].videoRenderer` |
| `reelShelfRenderer` | **NO** | — (ver §6; sustituido por `gridShelfViewModel`) |
| `channelRenderer` | **No con esta consulta** (sí con consultas de creador, ver abajo) | `...contents[0].itemSectionRenderer.contents[0].channelRenderer` (con `query: "mrbeast"`) |
| `playlistRenderer` | **NO, en ninguna consulta probada** | — (sustituido por `lockupViewModel`, ver abajo) |
| `gridShelfViewModel` (Shorts) | **Sí** | `...contents[19].gridShelfViewModel` |
| `shortsLockupViewModel` | **Sí** | `...contents[19].gridShelfViewModel.contents[0].shortsLockupViewModel` |

**Campos de un `videoRenderer`** (todos verificados):

| Dato | Ruta |
|---|---|
| id | `videoRenderer.videoId` |
| título | `videoRenderer.title.runs[0].text` |
| canal | `videoRenderer.ownerText.runs[0].text` |
| canal id | `videoRenderer.ownerText.runs[0].navigationEndpoint.browseEndpoint.browseId` |
| duración | `videoRenderer.lengthText.simpleText` (**ausente en directos**) |
| vistas | `videoRenderer.viewCountText.simpleText` |
| publicado | `videoRenderer.publishedTimeText.simpleText` |
| miniatura | `videoRenderer.thumbnail.thumbnails[]` (`.url`, `.width`, `.height`) |
| es directo | `videoRenderer.badges[].metadataBadgeRenderer.style == "BADGE_STYLE_TYPE_LIVE_NOW"` |

Ejemplo real devuelto: `rFZHOHl-L8A`, "lofi hip hop radio 📚 beats to relax/study to", canal
"Lofi Girl", `viewCountText = "8685 usuarios"` y **sin `lengthText`** porque es un directo.

**Consultas secundarias** (el mismo endpoint, solo cambia `query`). La mezcla de renderers depende
de la intención de la consulta, así que se probaron dos más:

- `query: "mrbeast"` → `channelRenderer: 4`, `videoRenderer: 15`, `gridShelfViewModel: 3`,
  `lockupViewModel: 1`, `shelfRenderer: 1`.
  Ruta del canal: `...itemSectionRenderer.contents[0].channelRenderer`.
  Campos: `channelRenderer.channelId` (`UCX6OQ3DkcsbYNE6H8uQQuVA`), `.title.runs[0].text`,
  `.descriptionSnippet.runs[].text`, `.videoCountText` (aquí **contiene el número de suscriptores**),
  `.subscriberCountText` (aquí **contiene el handle** `@MrBeast`: los dos campos están intercambiados).
- `query: "daft punk"` → `videoRenderer: 13`, `lockupViewModel: 6`, `gridShelfViewModel: 2`,
  `officialCardViewModel: 1`. Sin `channelRenderer` ni `playlistRenderer`.

**Playlists en 2026: `lockupViewModel`.** Ruta:
`...itemSectionRenderer.contents[18].lockupViewModel` con

| Dato | Ruta |
|---|---|
| id | `lockupViewModel.contentId` (p. ej. `PLsdoVPM5WnndLX6Ngmb8wktMF61dJirKl`, `RDEMni3jl65KF37F9JYsJM8DGg`) |
| tipo | `lockupViewModel.contentType` = `LOCKUP_CONTENT_TYPE_PLAYLIST` \| `_VIDEO` \| `_SHOW` |
| título | `lockupViewModel.metadata.lockupMetadataViewModel.title.content` |
| subtítulo | `...metadata.contentMetadataViewModel.metadataRows[0].metadataParts[0].text.content` |
| miniatura | `lockupViewModel.contentImage.collectionThumbnailViewModel.primaryThumbnail.thumbnailViewModel.image.sources[]` |

Los ids distinguen tipos sin ambigüedad: `PL…`/`RD…`/`VL…` playlist, `MPREb_…` álbum (Music),
`UC…` canal, 11 caracteres sin prefijo = vídeo.

**Anuncios**: `...contents[0].adSlotRenderer` aparece de forma intermitente (1–2 por respuesta según
la consulta). Está **en el mismo array que los vídeos** y hay que filtrarlo.

**Continuación**: `contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[1].continuationItemRenderer.continuationEndpoint.continuationCommand.token` (616 caracteres).

---

## 2. `home` — POST `/youtubei/v1/browse` (`FEwhat_to_watch`)

**URL**: `https://www.youtube.com/youtubei/v1/browse?prettyPrint=false`
**Método**: POST. Mismas cabeceras que `search`.
**Cuerpo**:

```json
{
  "context": { "client": { "clientName": "WEB", "clientVersion": "2.20261105.05.01", "hl": "es", "gl": "ES" } },
  "browseId": "FEwhat_to_watch"
}
```

**Estado HTTP**: `200` (59.433 bytes).

**Resultado real: el feed de inicio anónimo está VACÍO.**

```
contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.richGridRenderer
  .contents[0].richSectionRenderer.content.feedNudgeRenderer
```

- `contents` tiene **1 solo elemento**, un `richSectionRenderer` con un `feedNudgeRenderer`
  ("Tu historial de YouTube está desactivado").
- `videoRenderer`: **0**. `richItemRenderer`: **0**. `adSlotRenderer`: **0**.
  `promotedSparklesWebRenderer`: **0**. `continuationItemRenderer`: **0**.
- Por tanto **no hay ruta a `videoRenderer` que reportar**: no existen items.

**Qué se intentó y también falló** (para descartar que fuera un problema de la sonda):

| Variante | Resultado |
|---|---|
| Cookies anónimas (`SOCS=CAI`, `PREF`, `VISITOR_INFO1_LIVE`, `__Secure-YEC`, `__Secure-YNID`, `GPS`, `YSC`, `__Secure-ROLLOUT_TOKEN`) | Igual: solo nudge |
| `visitorData` dentro de `context.client` además de la cabecera | Igual: solo nudge |
| `hl=en&gl=US` + visitante nuevo + calentar visitante con `search` y `next` | Igual: solo nudge |
| `params: "EgZ0YWJfMQ%3D%3D"` y `params: "4qmFsgI"` | Igual: solo nudge |
| Cliente `ANDROID` (3) y `IOS` (5) con el mismo `browseId` | 200 con 2 MB, pero `eomFlowRenderer` + `webView`; **0 `videoRenderer`** |
| Cliente `TVHTML5` (7) | 200, `tvBrowseRenderer` con `feedNudgeRenderer`; **0 `videoRenderer`** |
| `GET https://www.youtube.com/?hl=es&gl=ES` (HTML real de la portada) | contiene `feedNudgeRenderer` y **ningún** `videoRenderer`/`richItemRenderer` |

Es decir: YouTube ya no sirve un feed de recomendaciones a un visitante anónimo sin historial;
el HTML público de la portada muestra exactamente lo mismo. **`FEwhat_to_watch` no es utilizable
como fuente de datos sin sesión.** `FEexplore`, `FEtrending`, `FEshorts`, `FEshorts_tab` responden
**400 INVALID_ARGUMENT**. `FElibrary` y `FEsubscriptions` responden 200 pero con
`backgroundPromoRenderer` (promo de inicio de sesión), sin items.

_Fixture_: `home.json` contiene la respuesta real (nudge), con la ruta anterior intacta.

---

## 3. `watch-next` — POST `/youtubei/v1/next`

**URL**: `https://www.youtube.com/youtubei/v1/next?prettyPrint=false`
**Método**: POST. Mismas cabeceras que `search`.
**Cuerpo**:

```json
{
  "context": { "client": { "clientName": "WEB", "clientVersion": "2.20261105.05.01", "hl": "es", "gl": "ES" } },
  "videoId": "dQw4w9WgXcQ",
  "contentCheckOk": true,
  "racyCheckOk": true
}
```

**Estado HTTP**: `200` (499.736 bytes). 21 items relacionados.

**Ruta a los vídeos relacionados**:

```
contents.twoColumnWatchNextResults.secondaryResults.secondaryResults.results[]
```

Mezcla real: `lockupViewModel: 20`, `continuationItemRenderer: 1`.
**`compactVideoRenderer`: AUSENTE** (0 apariciones en toda la respuesta). El plan o el código
antiguo que busque `compactVideoRenderer` en `next` **no encontrará nada**.

| Dato | Ruta |
|---|---|
| id relacionado | `...results[N].lockupViewModel.contentId` |
| tipo | `...results[N].lockupViewModel.contentType` (`LOCKUP_CONTENT_TYPE_VIDEO`) |
| título | `...lockupViewModel.metadata.lockupMetadataViewModel.title.content` |
| canal | `...lockupViewModel.metadata.lockupMetadataViewModel.metadata.contentMetadataViewModel.metadataRows[0].metadataParts[0].text.content` |
| duración | `...thumbnailViewModel.overlays[].thumbnailBottomOverlayViewModel.badges[].thumbnailBadgeViewModel.text` (formato `m:ss`) |
| miniatura | `...lockupViewModel.contentImage.thumbnailViewModel.image.sources[]` |
| id alternativo | `...lockupViewModel.rendererContext.commandContext.onTap.innertubeCommand.watchEndpoint.videoId` |

**Datos del vídeo actual** (todo verificado en la misma respuesta):

| Dato | Ruta | Valor devuelto |
|---|---|---|
| título | `contents.twoColumnWatchNextResults.results.results.contents[1].videoPrimaryInfoRenderer.title.runs[0].text` | "Rick Astley - Never Gonna Give You Up (Official Video) (4K Remaster)" |
| vistas | `...contents[1].videoPrimaryInfoRenderer.viewCount.videoViewCountRenderer.viewCount.simpleText` | "1.835.365.724 visualizaciones" |
| likes | `...contents[1].videoPrimaryInfoRenderer.videoActions.menuRenderer.topLevelButtons[0].segmentedLikeDislikeButtonViewModel.likeButtonViewModel...defaultButtonViewModel.buttonViewModel.title` | "19 M" |
| descripción | `...contents[2].videoSecondaryInfoRenderer.attributedDescription.content` | texto completo (saltos de línea incluidos) |
| descripción con enlaces | `...attributedDescription.commandRuns[]` (`.startIndex`, `.length`, `.onTap`) | — |
| canal (nombre) | `...contents[2].videoSecondaryInfoRenderer.owner.videoOwnerRenderer.title.runs[0].text` | "Rick Astley" |
| canal (id) | `...videoOwnerRenderer.navigationEndpoint.browseEndpoint.browseId` | `UCuAXFkgsw1L7xaCfnd5JJOw` |
| suscriptores | `...videoOwnerRenderer.subscriberCountText.simpleText` | "4,56 M de suscriptores" |
| fecha | `...contents[1].videoPrimaryInfoRenderer.dateText.simpleText` | — |
| comentarios | `engagementPanels[0].engagementPanelSectionListRenderer.panelIdentifier == "engagement-panel-comments-section"` con `visibility: ENGAGEMENT_PANEL_VISIBILITY_HIDDEN`; el contenido (y su `continuationCommand`) **viene ya incluido** | — |

**Nota sobre el like count**: no hay un campo numérico. Es un texto localizado (`"19 M"`, `"19 mil"`).
En `segmentedLikeDislikeButtonViewModel` aparecen 4 `buttonViewModel` con títulos repetidos
("19 M", "19 M", "No me gusta", "No me gusta"): hay que quedarse con el primero y parsear el sufijo.

---

## 4. `music-home` — POST `music.youtube.com/youtubei/v1/browse` (`FEmusic_home`)

**URL**: `https://music.youtube.com/youtubei/v1/browse?prettyPrint=false`
**Método**: POST
**Cabeceras**: como `search`, pero con `origin`/`referer` de `https://music.youtube.com`,
`x-youtube-client-name: 67` y `x-youtube-client-version: 1.20241201.01.00`.

**Cuerpo**:

```json
{
  "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20241201.01.00", "hl": "es", "gl": "ES" } },
  "browseId": "FEmusic_home"
}
```

**Estado HTTP**: `200` (487.482 bytes). **Funciona con la `clientVersion` de 2024 incluida en el
encargo** (la versión viva del sitio es `1.20261104.00.00`, leída de `https://music.youtube.com/`).

**Ruta a los estantes**:

```
contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[]
```

**Ruta a los items**: `...contents[N].musicCarouselShelfRenderer.contents[]`.
`musicShelfRenderer` **no aparece** en la home; `musicResponsiveHeaderRenderer` tampoco.

**Estantes reales** (el conjunto **varía entre peticiones**: se observaron 2 y 3 estantes; el orden
de los estantes 1 y 2 también cambia, y "Selecciones rápidas" a veces no viene):

| # | Título | Items | Renderer de item | Tipo de id |
|---|---|---|---|---|
| 0 | "Selecciones rápidas" | 20 | `musicResponsiveListItemRenderer` | vídeo (`playlistItemData.videoId`, p. ej. `Kev3r6W4TPo`) |
| 1 | "Listas de reproducción destacadas para ti" | 5 | `musicTwoRowItemRenderer` | playlist (`navigationEndpoint.browseEndpoint.browseId = VLOLAK5uy_…`) |
| 2 | "Últimos lanzamientos" | 10 | `musicTwoRowItemRenderer` | álbum (`MPREb_wuOPdv2mXYf`) |

En una ejecución con solo 2 estantes, faltaba precisamente "Selecciones rápidas" (el único con
`musicResponsiveListItemRenderer`). **Un parser debe tolerar ambas formas en cualquier estante** y
no asumir el número ni el orden de los carruseles.

**Campos por tipo de item**:

`musicTwoRowItemRenderer` (álbumes, playlists, artistas):

| Dato | Ruta |
|---|---|
| id | `.navigationEndpoint.browseEndpoint.browseId` |
| tipo de página | `.navigationEndpoint.browseEndpoint.browseEndpointContextSupportedConfigs.browseEndpointContextMusicConfig.pageType` (`MUSIC_PAGE_TYPE_PLAYLIST`, `MUSIC_PAGE_TYPE_ALBUM`, `MUSIC_PAGE_TYPE_ARTIST`) |
| título | `.title.runs[0].text` |
| subtítulo | `.subtitle.runs[].text` (concatenar; p. ej. "Ed Sheeran, Imagine Dragons…") |
| miniatura | `.thumbnailRenderer.musicThumbnailRenderer.thumbnail.thumbnails[]` |

`musicResponsiveListItemRenderer` (filas compactas):

| Dato | Ruta |
|---|---|
| título | `.flexColumns[0].musicResponsiveListItemFlexColumnRenderer.text.runs[].text` |
| metadatos | `.flexColumns[1].musicResponsiveListItemFlexColumnRenderer.text.runs[].text` ("Canción • 3:12", "Artista • 233 M usuarios mensuales") |
| id de vídeo | `.playlistItemData.videoId` |
| miniatura | `.thumbnail.musicThumbnailRenderer.thumbnail.thumbnails[]` |

---

## 5. `music-search` — POST `music.youtube.com/youtubei/v1/search`

**URL**: `https://music.youtube.com/youtubei/v1/search?prettyPrint=false`
**Método**: POST. Mismas cabeceras y `context` (`WEB_REMIX`) que `music-home`.
**Cuerpo**:

```json
{ "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20241201.01.00", "hl": "es", "gl": "ES" } }, "query": "daft punk" }
```

**Estado HTTP**: `200` (246.481 bytes). **31 secciones**.

**Rutas**:

```
contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents[]
```

- Sección `[1]` = **`musicCardShelfRenderer`** (resultado principal / tarjeta del artista).
- El resto (30 secciones) = **`itemSectionRenderer`**, cada una con
  `.contents[0].musicResponsiveListItemRenderer`.
- **`musicShelfRenderer`: AUSENTE**. La estructura clásica `musicShelfRenderer.contents[]` ya no
  existe en la búsqueda de Music; hay que iterar `itemSectionRenderer` o buscar por clave
  recursivamente.

Rutas de ejemplo en la ejecución capturada (misma lista `sectionListRenderer.contents[]`):

- álbum: `contents[2].itemSectionRenderer.contents[0].musicResponsiveListItemRenderer` → `MPREb_K8qWMWVqXGi`
- artista: `contents[3].itemSectionRenderer.contents[0].musicResponsiveListItemRenderer` → `UClYV6hHlupm_S_ObS1W-DYw`
- canción: `contents[4].itemSectionRenderer.contents[0].musicResponsiveListItemRenderer` → `52fNscjkST4`
- primera canción de la tarjeta principal: `contents[1].musicCardShelfRenderer.contents[0].musicResponsiveListItemRenderer`

**Tipos de id devueltos** (verificados en la misma respuesta):

| Tipo | Ruta del id | Ejemplo |
|---|---|---|
| Canción | `.playlistItemData.videoId` **y sin `navigationEndpoint`** | `52fNscjkST4` ("Aerodynamic") |
| Álbum | `.navigationEndpoint.browseEndpoint.browseId` + `pageType: MUSIC_PAGE_TYPE_ALBUM` | `MPREb_K8qWMWVqXGi` ("Random Access Memories") |
| Artista | `.navigationEndpoint.browseEndpoint.browseId` + `pageType: MUSIC_PAGE_TYPE_ARTIST` | `UClYV6hHlupm_S_ObS1W-DYw` |
| Playlist | `.navigationEndpoint.browseEndpoint.browseId` + `pageType: MUSIC_PAGE_TYPE_PLAYLIST` | `VLPLGfGJFH5ltY8` |

**Trampa real**: las canciones **no** traen `navigationEndpoint`; su id está solo en
`playlistItemData.videoId`. Las filas de álbum/artista/playlist **no** traen `playlistItemData` y su
id está solo en `navigationEndpoint.browseEndpoint.browseId`. Hay que leer ambos.

El texto de la fila está en `flexColumns`:
`"Aerodynamic"`, `"Canción • Daft Punk"`, `"76 M reproducciones"`.

`musicCardShelfRenderer` (resultado principal): `.title.runs[0].text` = "Daft Punk",
`.subtitle.runs[].text` = "Artista • 67,4 M usuarios mensuales", con
`.contents[]` = 3 `musicResponsiveListItemRenderer` (una canción, un álbum y una playlist).
El id del artista está en `.title.runs[0].navigationEndpoint.browseEndpoint.browseId`
(y en `.onTap.browseEndpoint.browseId`, **sin** `innertubeCommand` de por medio: la tarjeta usa una
forma distinta a la de los items de lista).

**Ruido**: la primera `itemSectionRenderer` contiene un `messageRenderer`
("Acerca de estos resultados", un aviso legal) sin id. Hay que ignorarlo.

---

## 6. `shorts` — qué devuelve Shorts hoy

### 6.1 `reelShelfRenderer` en `search`: **NO EXISTE**

Probado con `query` = `shorts`, `funny shorts`, `viral`, `dance`, `cats`, `asmr`, `#shorts`,
y con clientes `WEB`, `MWEB` y `ANDROID`: **0 apariciones** de `reelShelfRenderer`.
Los Shorts se sirven ahora como **`gridShelfViewModel.contents[].shortsLockupViewModel`**:

```
contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents[1]
  .itemSectionRenderer.contents[0].gridShelfViewModel.contents[0].shortsLockupViewModel
```

| Dato | Ruta |
|---|---|
| id | `.shortsLockupViewModel.onTap.innertubeCommand.reelWatchEndpoint.videoId` |
| entity id | `.shortsLockupViewModel.entityId` (`shorts-shelf-item-<videoId>`) |
| título | `.shortsLockupViewModel.overlayMetadata.primaryText.content` (**`.content`, no `.runs`**) |
| vistas | `.shortsLockupViewModel.overlayMetadata.secondaryText.content` |
| parámetros de secuencia | `.shortsLockupViewModel.onTap.innertubeCommand.reelWatchEndpoint.sequenceParams` (**imprescindible**, ver 6.2) |
| miniatura | `.shortsLockupViewModel.thumbnailViewModel.image.sources[]` (proporción 9:16, p. ej. 720×720 o 1080×1920) |

### 6.2 `POST /youtubei/v1/reel/reel_watch_sequence`

**URL**: `https://www.youtube.com/youtubei/v1/reel/reel_watch_sequence?prettyPrint=false`
**Método**: POST, mismas cabeceras `WEB`.

| Cuerpo enviado | Estado | Resultado |
|---|---|---|
| `{context, videoId: "dQw4w9WgXcQ"}` | **400** | `INVALID_ARGUMENT` / "Request contains an invalid argument." |
| `{context, videoId: <id de un short real>, params: <reelWatchEndpoint.params>}` | **400** | `INVALID_ARGUMENT` |
| `{context, sequenceParams: <reelWatchEndpoint.sequenceParams>}` | **200** | 308 KB, 12 entradas |

El endpoint **solo funciona con `sequenceParams`**, y esos parámetros solo se obtienen de un
`reelWatchEndpoint` previo (de la shelf de búsqueda o del feed). No hay forma de arrancar una
secuencia de Shorts "en frío" desde un `videoId`.

Respuesta correcta (`200`): claves `responseContext`, `entries`, `trackingParams`,
`continuationEndpoint`, `onResponseReceivedActions`.

```
entries[]                      -> una entrada por Short
entries[N].command.reelWatchEndpoint.videoId
entries[N].command.reelWatchEndpoint.playerParams
entries[N].command.reelWatchEndpoint.thumbnail.thumbnails[]
entries[N].command.reelWatchEndpoint.overlay.reelPlayerOverlayRenderer
continuationEndpoint.continuationCommand.token   -> para la siguiente página
```

### 6.3 `POST /youtubei/v1/reel/reel_item_watch`

`200` pero **inútil**: 1.965 bytes con claves `responseContext`, `status`, `tooltip`, `background`.
**0 `reelWatchEndpoint`**. No devuelve datos del Short.

### 6.4 Conclusión de Shorts

Los ids de Shorts hoy salen de: **(a)** `gridShelfViewModel → shortsLockupViewModel` en `search`;
**(b)** `entries[].command.reelWatchEndpoint.videoId` del feed de `reel_watch_sequence`;
**(c)** canales (`/channel/<id>/shorts`) y `browseId` de pestañas de canal.
El feed infinito vertical requiere `sequenceParams` obtenidos de (a) o (c).
El fixture `shorts.json` guarda la shelf real y la respuesta de `reel_watch_sequence` con sus rutas.

---

## 7. `player-video` — POST `/youtubei/v1/player` con `ANDROID_VR`

**URL**: `https://www.youtube.com/youtubei/v1/player?prettyPrint=false`
**Método**: POST
**Cabeceras**:

```
content-type: application/json
user-agent: com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A001) gzip
x-youtube-client-name: 28
x-youtube-client-version: 1.65.10
x-goog-visitor-id: <visitorData>
origin: https://www.youtube.com
referer: https://www.youtube.com/
accept-language: es-ES,es;q=0.9
```

**Cuerpo**:

```json
{
  "context": {
    "client": {
      "clientName": "ANDROID_VR",
      "clientVersion": "1.65.10",
      "deviceMake": "Oculus",
      "deviceModel": "Quest 3",
      "androidSdkVersion": 32,
      "osName": "Android",
      "osVersion": "12L",
      "timeZone": "UTC",
      "utcOffsetMinutes": 0,
      "hl": "es",
      "gl": "ES"
    }
  },
  "videoId": "dQw4w9WgXcQ",
  "contentCheckOk": true,
  "racyCheckOk": true
}
```

**Estado HTTP**: `200` (75.527 bytes).

**`playabilityStatus`**: `status: "OK"`, sin `reason`, con `videoDetails` completo
(título, `lengthSeconds: "213"`, `author`, `viewCount`, `isLiveContent: false`).

**Forma de `streamingData`** (claves: `formats`, `adaptiveFormats`; sin `hlsManifestUrl` en VOD):

- `streamingData.formats` (**muxed**): **1** formato, itag **18**.
- `streamingData.adaptiveFormats`: **26** formatos.
- **`url` directa: 26/26 adaptive + 1/1 muxed. `signatureCipher`: 0.** No hace falta descifrar
  firmas con este cliente.
- Sin `pot` (poToken) ni `n` (nsig) en las URLs.

**Itags (muxed y adaptive) de `dQw4w9WgXcQ`**:

| itag | tipo | mimeType | bitrate | calidad | WxH | contentLength |
|---|---|---|---|---|---|---|
| **18** | **muxed** | `video/mp4; codecs="avc1.42001E, mp4a.40.2"` | 444.226 | 360p | 640×360 | *(ausente)* |
| 22 | — | **no existe** | — | — | — | — |
| 17 | — | **no existe** | — | — | — | — |
| 313 | adaptive | `video/webm; codecs="vp9"` | 18.076.636 | 2160p | 3840×2160 | 358.608.461 |
| 401 | adaptive | `video/mp4; codecs="av01.0.12M.08"` | 17.400.774 | 2160p | 3840×2160 | 240.334.643 |
| 271 | adaptive | `video/webm; codecs="vp9"` | 8.978.328 | 1440p | 2560×1440 | 151.103.346 |
| 400 | adaptive | `video/mp4; codecs="av01.0.12M.08"` | 7.755.221 | 1440p | 2560×1440 | 122.168.277 |
| 137 | adaptive | `video/mp4; codecs="avc1.640028"` | 4.334.157 | 1080p | 1920×1080 | 80.911.999 |
| 248 | adaptive | `video/webm; codecs="vp9"` | 1.549.172 | 1080p | 1920×1080 | 30.846.580 |
| 399 | adaptive | `video/mp4; codecs="av01.0.08M.08"` | 1.594.543 | 1080p | 1920×1080 | 30.415.996 |
| 136 | adaptive | `video/mp4; codecs="avc1.4D401F"` | 1.058.310 | 720p | 1280×720 | 26.455.880 |
| 247 | adaptive | `video/webm; codecs="vp9"` | 867.871 | 720p | 1280×720 | 17.686.717 |
| 398 | adaptive | `video/mp4; codecs="av01.0.05M.08"` | 1.012.348 | 720p | 1280×720 | 17.593.076 |
| 135 | adaptive | `video/mp4; codecs="avc1.4D401E"` | 690.764 | 480p | 854×480 | 14.103.519 |
| 244 | adaptive | `video/webm; codecs="vp9"` | 466.632 | 480p | 854×480 | 9.381.481 |
| 397 | adaptive | `video/mp4; codecs="av01.0.04M.08"` | 525.728 | 480p | 854×480 | 9.874.826 |
| 134 | adaptive | `video/mp4; codecs="avc1.4D401E"` | 450.863 | 360p | 640×360 | 8.390.921 |
| 243 | adaptive | `video/webm; codecs="vp9"` | 288.453 | 360p | 640×360 | 6.014.765 |
| 396 | adaptive | `video/mp4; codecs="av01.0.01M.08"` | 304.466 | 360p | 640×360 | 5.685.561 |
| 133 | adaptive | `video/mp4; codecs="avc1.4D4015"` | 236.224 | 240p | 426×240 | 4.310.122 |
| 242 | adaptive | `video/webm; codecs="vp9"` | 131.647 | 240p | 426×240 | 2.706.929 |
| 395 | adaptive | `video/mp4; codecs="av01.0.00M.08"` | 159.709 | 240p | 426×240 | 3.095.447 |
| 160 | adaptive | `video/mp4; codecs="avc1.4D400C"` | 109.973 | 144p | 256×144 | 2.058.142 |
| 278 | adaptive | `video/webm; codecs="vp9"` | 68.500 | 144p | 256×144 | 1.540.844 |
| 394 | adaptive | `video/mp4; codecs="av01.0.00M.08"` | 83.289 | 144p | 256×144 | 1.505.504 |
| 139 | adaptive audio | `audio/mp4; codecs="mp4a.40.5"` | 50.152 | LOW / 22050 Hz | — | 1.300.631 |
| 140 | adaptive audio | `audio/mp4; codecs="mp4a.40.2"` | 130.677 | MEDIUM / 44100 Hz | — | 3.449.447 |
| 249 | adaptive audio | `audio/webm; codecs="opus"` | 49.496 | LOW / 48000 Hz | — | 1.231.355 |
| 251 | adaptive audio | `audio/webm; codecs="opus"` | 136.544 | MEDIUM / 48000 Hz | — | 3.433.755 |

**Formatos muxed (audio+vídeo en uno)**: solo **itag 18** existe. **itag 22 e itag 17 no se ofrecen**
con `ANDROID_VR` en 2026. Además, `formats[].contentLength` viene **ausente** en el muxed.

### 7.1 ¿La URL directa descarga de verdad? — **solo a medias**

`GET` con `Range: bytes=0-1023` sobre la URL de googlevideo:

| Formato | Cabeceras del GET | Estado | Resultado |
|---|---|---|---|
| **itag 18 (muxed)** | `UA: ANDROID_VR` + `Referer` + `Origin: https://www.youtube.com` | **403** | cuerpo vacío, `content-type: text/plain`, 0 bytes |
| **itag 18 (muxed)** | `UA: curl/8.5.0`, sin `Referer`/`Origin` | **403** | idéntico |
| itag 18 (muxed) | sin `User-Agent` | **403** | idéntico |
| **itag 313 (adaptive vídeo)** | `UA: ANDROID_VR` + `Referer` + `Origin` | **206** | `bytes 0-1023/358608461`, `content-type: video/webm`, cabecera WebM válida |
| **itag 251 (adaptive audio)** | `UA: ANDROID_VR` + `Referer` + `Origin` | **206** | `bytes 0-1023/3433755`, `content-type: audio/webm` |

Detalle del 403: lo emite `Server: gvs 1.0` con `content-length: 0` y
`last-modified: Wed, 02 May 2007 10:26:10 GMT` (respuesta canónica de rechazo de googlevideo).
**Las cabeceras no influyen**: 403 con y sin `UA`/`Referer`/`Origin`.

Comprobaciones cruzadas para entender el 403:

| Prueba | Resultado |
|---|---|
| Mismo itag 18 pedido al cliente `ANDROID` (3), GET con `UA: Mozilla/5.0` | **206** ✅ |
| itag 18 de `ANDROID_VR` en 3 vídeos distintos (`dQw4w9WgXcQ`, `jNQXAC9IVRw`, `9bZkp7q19f0`), 3 intentos cada uno | **403** siempre ❌ |
| itag 137/313/251 de `ANDROID_VR`, cualquier combinación de cabeceras | **206** siempre ✅ |

Conclusión: con `ANDROID_VR`, las URLs **muxed (itag 18) están bloqueadas** por googlevideo hoy,
mientras que **las adaptativas (vídeo-only y audio-only) funcionan sin ninguna cabecera especial**.
Por tanto el reproductor debe usar `adaptiveFormats` (y muxear vídeo+audio en el cliente), no
`formats[]`.

**Vídeos en directo**: `ANDROID_VR` no devuelve `formats`/`adaptiveFormats` para directos, sino
`streamingData.hlsManifestUrl` (`https://manifest.googlevideo.com/.../index.m3u8`). Un directo ya
terminado devuelve `UNPLAYABLE` con `reason: "La grabación de esta emisión en directo no está
disponible."`.

**Otros clientes para comparar** (mismo `videoId`):

| Cliente | `playabilityStatus` | `streamingData` |
|---|---|---|
| `WEB` (1) | **`UNPLAYABLE`** — "El vídeo no está disponible" / "Es necesario volver a cargar la página." | **ausente** |
| `MWEB` (2) | **`UNPLAYABLE`** — "Es necesario volver a cargar la página." | **ausente** |
| `ANDROID` (3) | `OK` | 1 muxed + 26 adaptive, URLs directas |
| `IOS` (5) | `OK` | 0 muxed + 16 adaptive, URLs directas |
| `TVHTML5_SIMPLY_EMBEDDED_PLAYER` (85) | `ERROR` | ausente |

**Robustez de `ANDROID_VR`**: devuelve `OK` **quitando** `x-goog-visitor-id`, quitando
`x-youtube-client-*`, usando un `User-Agent` genérico o sin `origin`. Solo el `context.client` del
cuerpo parece determinar el resultado.

---

## 8. `player-audio` — mejor pista de audio

**URL/método/cabeceras/cuerpo**: idénticos a §7 (mismo `videoId`).
**Estado HTTP**: `200` (75.337 bytes). 4 pistas solo-audio con `url` directa (0 `signatureCipher`).

**Ruta**: `streamingData.adaptiveFormats[]` filtrando `mimeType ^= "audio/"`.

| itag | mimeType | bitrate | `audioQuality` | muestra | contentLength |
|---|---|---|---|---|---|
| 139 | `audio/mp4; codecs="mp4a.40.5"` | 50.152 | `AUDIO_QUALITY_LOW` | 22050 Hz | 1.300.631 |
| 140 | `audio/mp4; codecs="mp4a.40.2"` | 130.677 | `AUDIO_QUALITY_MEDIUM` | 44100 Hz | 3.449.447 |
| 249 | `audio/webm; codecs="opus"` | 49.496 | `AUDIO_QUALITY_LOW` | 48000 Hz | 1.231.355 |
| **251** | `audio/webm; codecs="opus"` | **136.544** | `AUDIO_QUALITY_MEDIUM` | 48000 Hz | 3.433.755 |

**Mejor pista por bitrate**: itag **251** (opus, 136 kbps). **No hay pistas `AUDIO_QUALITY_HIGH`
(256 kbps) sin sesión**: `ANDROID_VR` anónimo solo ofrece LOW/MEDIUM.

**Verificación de descarga** (itag 251, `Range: bytes=0-1023` con UA `ANDROID_VR` + `Referer` + `Origin`):
**`206`**, `content-range: bytes 0-1023/3433755`, `content-type: audio/webm`, primeros bytes con
cabecera EBML de WebM. Sin necesidad de cookies ni de `visitorData`.

**Reproducción de audio sin vídeo**: viable — se pide solo el itag 251 (3,4 MB) en vez de descargar
vídeo. Es el camino del "modo música" del plan.

---

## 9. `continuations` — paginación

**URL**: `https://www.youtube.com/youtubei/v1/search?prettyPrint=false`
**Método**: POST. Mismas cabeceras `WEB`.
**Cuerpo**:

```json
{
  "context": { "client": { "clientName": "WEB", "clientVersion": "2.20261105.05.01", "hl": "es", "gl": "ES" } },
  "continuation": "<token de 616 caracteres obtenido de search>"
}
```

**Estado HTTP**: `200` (997.792 bytes).

**Estructura de la respuesta de continuación** (distinta a la primera página):

```
onResponseReceivedCommands[0].appendContinuationItemsAction.continuationItems[]
  [0].itemSectionRenderer.contents[0].videoRenderer      <- items nuevos
  [1].continuationItemRenderer                            <- token de la página siguiente
```

Rutas exactas:

- Items nuevos: `onResponseReceivedCommands[0].appendContinuationItemsAction.continuationItems[0].itemSectionRenderer.contents[]`
  (aquí dentro vuelve a estar la misma mezcla: `itemSectionRenderer` → `videoRenderer`, `gridShelfViewModel`…).
- Token siguiente: `onResponseReceivedCommands[0].appendContinuationItemsAction.continuationItems[1].continuationItemRenderer.continuationEndpoint.continuationCommand.token`
  (596 caracteres).

`itemCount` de la página = 2 contenedores (`itemSectionRenderer` + `continuationItemRenderer`), no el
número de vídeos. El número de vídeos hay que contarlo dentro de `itemSectionRenderer.contents[]`:
en la prueba devolvió **20 items**.

`header.searchHeaderRenderer.chipBar.chipCloudRenderer.chips[]` también contiene
`continuationCommand` (son los filtros: "Todo", "Vídeos", "Shorts"…). **No confundir**: el token de
página es el del último `continuationItemRenderer`, no el de los chips.

---

## 10. Riesgos verificados

### Lo que exige tokens extra o está inestable

| Endpoint / función | Estado hoy | Riesgo |
|---|---|---|
| `/player` con cliente **`WEB`** o **`MWEB`** | **`UNPLAYABLE`**, sin `streamingData` | Requiere **poToken** (y ejecución del JS de YouTube). No sirve para un cliente nativo. |
| `/player` con **`ANDROID_VR`** | `OK`, URLs directas, sin poToken | **Único camino anónimo fiable** hoy. Si Google lo cierra, hay que rehacer el resolvedor. |
| **Muxed `formats[]` (itag 18) de `ANDROID_VR`** | **403** en el GET de media | Bloqueo verificado; usar `adaptiveFormats` y muxear. |
| `/player` para **directos** | Solo `hlsManifestUrl` (HLS). Directos pasados: `UNPLAYABLE` | Requiere cliente HLS (libmpv sí lo soporta). |
| `/reel/reel_watch_sequence` | **400** sin `sequenceParams` | No se puede iniciar el feed de Shorts desde un `videoId`; hay que arrancar desde una shelf. |
| `/reel/reel_item_watch` | 200 pero **vacío** | No usar. |
| **`browse(FEwhat_to_watch)` anónimo** | **vacío** (solo `feedNudgeRenderer`) | El inicio necesita sesión/historial. **El feed de portada no es viable anónimo.** |
| `browse(FEexplore/FEtrending/FEshorts)` | **400** | Ids obsoletos; no hay sustituto anónimo verificado para "tendencias". |
| `next` → `compactVideoRenderer` | **Ausente** | Cualquier parser basado en el formato antiguo devolverá 0 relacionados. Migrar a `lockupViewModel`. |
| `search` → `reelShelfRenderer`, `playlistRenderer` | **Ausentes** | Sustituidos por `gridShelfViewModel` + `shortslockupViewModel` y `lockupViewModel`. |
| `music search` → `musicShelfRenderer` | **Ausente** | Ahora son `itemSectionRenderer` + `musicCardShelfRenderer`. |
| **Anuncios** (`adSlotRenderer`) en `search` | 0–2 por respuesta, **intermitentes** | Contaminan la lista de vídeos; filtrar por clave. |
| `clientVersion` / `WEB_REMIX` version | Caducan cada pocos días | Leerlas del HTML de `/watch` y de `music.youtube.com` en tiempo de ejecución; no hardcodear. |
| **`fetch`/undici con límite de cabeceras por defecto** | `UND_ERR_HEADERS_OVERFLOW` en `/watch` | El cliente debe permitir cabeceras grandes; con otro stack, verificar. |
| Campos intercambiados en `channelRenderer` | `.videoCountText` = suscriptores, `.subscriberCountText` = handle | No fiarse de los nombres. |
| Textos localizados | Likes y vistas son cadenas ("19 M", "1.835.365.724 visualizaciones") | Parsear por idioma; no hay contador numérico. |
| Cadenas de media | `expire` ≈ 6 h (`expire=1794246619`), ligadas a `ip` | Hay que re-resolver el player al caducar. |

### Lo que funciona de forma estable

- `search` (WEB) → vídeos, canales, playlists (`lockupViewModel`), Shorts (`gridShelfViewModel`).
- `next` (WEB) → título, vistas, likes, descripción, canal, relacionados, panel de comentarios.
- `music-home` / `music-search` (WEB_REMIX) → estantes, canciones, álbumes, artistas, playlists.
- `player` (ANDROID_VR) → `adaptiveFormats` con `url` directa y descarga `206` sin cabeceras ni cookies.
- `continuation` en `search` → paginación con estructura documentada.
- `reel_watch_sequence` con `sequenceParams` → 12 Shorts + token de continuación.

---

## 11. Resumen de la evidencia (JSONL)

Cada línea de `node tools/probe.mjs all` es un registro con `probe`, `step`, `status`, `bytes`,
`paths` y conteos. Totales de la ejecución final: **39 líneas, 0 fallos de sonda**
(los `400` de `reel_watch_sequence` son resultados esperados, no fallos de la sonda).

Fixtures generados (todos < 300 KB):

| Fixture | Bytes (aprox.) | Items conservados | Contenido |
|---|---|---|---|
| `search_video.json` | ~191 KB | 23 contenedores (13 con datos) | `search` "lofi hip hop" completo |
| `home.json` | ~5 KB | 1 | `browse(FEwhat_to_watch)` = solo nudge |
| `watch_next.json` | ~252 KB | 21 contenedores (13 con datos) | `next(dQw4w9WgXcQ)` |
| `music_home.json` | ~240 KB | 3 estantes | `browse(FEmusic_home)` |
| `music_search.json` | ~170 KB | 33 secciones | `search("daft punk")` Music |
| `shorts.json` | ~224 KB | 5 Shorts + 10 entradas | shelf real + `reel_watch_sequence` |
| `player_video.json` | ~39 KB | 27 formatos | `player` ANDROID_VR |
| `player_audio.json` | ~7 KB | 4 pistas de audio | `player` ANDROID_VR, solo audio |

Los conteos exactos de `itemCount`/índices cambian entre ejecuciones; para reproducir la
evidencia basta `node tools/probe.mjs all`, que sobrescribe los fixtures y reimprime el JSONL.

Recorte aplicado por la sonda (documentado para no confundir "no existe" con "recortado"):

- Items de array: se conservan los primeros y **una muestra por cada forma distinta** de renderer;
  el resto se sustituye por `null` **sin desplazar índices**, así las rutas `contents[19]…` siguen
  resolviendo en el fixture. Los `null` de un array en el fixture significan "item recortado".
- Se eliminan: `trackingParams`, `clickTrackingParams`, `loggingDirectives`, `impressionEndpoints`,
  `topbar`, `frameworkUpdates`, `responseContext`, y decoración de UI (`menu`, `richThumbnail`,
  `thumbnailOverlays`, `detailedMetadataSnippets`, `avatar`, `lottieData`, `themedPalette`,
  `superTitleLink`, `pageVisualEffects`, `playerOverlays`, `movingThumbnailRenderer`,
  `inlinePlaybackEndpoint`, `expandableMetadata`, `channelThumbnailSupportedRenderers`).
- URLs: se quita la query de tracking; las de `googlevideo.com`/`gstatic.com` se reducen a
  `origin + path + "?…(signed-params-truncated)"` (firmadas y caducadas); las de publicidad se
  sustituyen por `[tracking-url-stripped]`.
- Los **caminos** de todas las rutas reportadas en este documento siguen presentes en el fixture
  correspondiente (verificado programáticamente).
