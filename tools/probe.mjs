#!/usr/bin/env node
// Sonda de la API interna de YouTube (`youtubei/v1`) para NeoTube 2.0.
//
// Uso: node tools/probe.mjs <search|home|watch-next|music-home|music-search|shorts|player-visionos|player-vr|continuations|all>
//
// Emite una línea JSON (JSONL) por paso con URL, método, cabeceras enviadas,
// cuerpo, status HTTP, tamaño en bytes, milisegundos y las rutas exactas del
// JSON hasta los datos útiles. Al final imprime un resumen compacto legible.
// Regenera las fixtures recortadas de `tests/fixtures/` y mide los rangos
// HTTP de las URLs de `googlevideo` (sección "Rangos y caps" de docs/API_PROBE.md).
//
// Restricciones: solo `fetch` incorporado de Node; sin dependencias, sin login,
// sin cookies de sesión y sin yt-dlp. Todas las peticiones son anónimas.
//
// Transporte: `fetch` (undici) funciona contra youtube.com en Node 24; no hace
// falta `node:https` con maxHeaderSize ampliado. Si una versión de Node falla con
// UND_ERR_HEADERS_OVERFLOW en la página /watch (respuestas de ~1,4 MB de HTML con
// cabeceras grandes) hay que subir el límite del cliente HTTP del proyecto real.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const FIXTURE_DIR = path.join(ROOT, 'tests', 'fixtures');

// UA de Safari de escritorio: es el UA con el que /watch sirve visitorData, STS y
// las cookies de la visita. Se reutiliza como UA del cliente VISIONOS porque
// visionOS también es WebKit/Safari.
const VISIONOS_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15';
const VR_UA =
  'com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A001) gzip';

// Perfil de cliente que resuelve streams completos (audio y vídeo por separado).
export const VISIONOS_CLIENT = {
  clientName: 'VISIONOS',
  clientVersion: '1.02',
  deviceMake: 'Apple',
  deviceModel: 'RealityDevice17,1',
  osName: 'visionOS',
  osVersion: '26.5.23O471',
  hl: 'en',
  timeZone: 'UTC',
  utcOffsetMinutes: 0,
};

// Perfil que solo sirve una ventana de bytes: se mide para documentar el límite.
export const VR_CLIENT = {
  clientName: 'ANDROID_VR',
  clientVersion: '1.65.10',
  deviceMake: 'Oculus',
  deviceModel: 'Quest 3',
  androidSdkVersion: 32,
  osName: 'Android',
  osVersion: '12L',
  hl: 'en',
  timeZone: 'UTC',
  utcOffsetMinutes: 0,
};

const VIDEO_ID = 'dQw4w9WgXcQ';
const YT = 'https://www.youtube.com';
const YTM = 'https://music.youtube.com';
const WATCH_URL = `${YT}/watch?v=${VIDEO_ID}&bpctr=9999999999&has_verified=1`;
const BASE_COOKIE = 'PREF=hl=en&tz=UTC; SOCS=CAI';

// ---------------------------------------------------------------------------
// Transporte HTTP (solo fetch incorporado)
// ---------------------------------------------------------------------------

async function fetchText(url, { method = 'GET', headers = {}, json = null, timeout = 45000 } = {}) {
  const started = Date.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error(`timeout tras ${timeout} ms`)), timeout);
  try {
    const res = await fetch(url, {
      method,
      headers,
      body: json === null ? undefined : JSON.stringify(json),
      signal: controller.signal,
      redirect: 'follow',
    });
    const buffer = Buffer.from(await res.arrayBuffer());
    return {
      url,
      method,
      status: res.status,
      contentType: res.headers.get('content-type'),
      contentRange: res.headers.get('content-range'),
      setCookies: res.headers.getSetCookie(),
      ms: Date.now() - started,
      bytes: buffer.length,
      text: buffer.toString('utf8'),
    };
  } finally {
    clearTimeout(timer);
  }
}

// POST a youtubei/v1 con el perfil de cliente elegido. `origin` selecciona el
// host (youtube.com o music.youtube.com) y las cabeceras X-Youtube-Client-*.
async function postInnerTube(url, { clientName, clientVersion, ua, origin, visitorData, cookie, body, extraHeaders = {} }) {
  return fetchText(`${url}?prettyPrint=false`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'user-agent': ua,
      'accept-language': 'en-US,en;q=0.9',
      origin,
      referer: `${origin}/`,
      'x-youtube-client-name': String(clientName),
      'x-youtube-client-version': clientVersion,
      ...(visitorData ? { 'x-goog-visitor-id': visitorData } : {}),
      ...(cookie ? { cookie } : {}),
      ...extraHeaders,
    },
    json: body,
  });
}

// GET con Range sobre una URL de googlevideo. `rqh=1` en esas URLs obliga a
// enviar Range; aquí se mide qué combinaciones devuelven 200/206/403. Solo se
// leen `maxBytes` del cuerpo (las URLs de vídeo llegan a decenas de MB y no hace
// falta descargarlas para saber si el servidor acepta el rango): `content-range`
// indica el rango realmente concedido.
async function rangeGet(url, { range = null, ua = VISIONOS_UA, maxBytes = 128 * 1024 } = {}) {
  const started = Date.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error('timeout de rango')), 30000);
  try {
    const res = await fetch(url, {
      headers: {
        'user-agent': ua,
        origin: 'https://www.youtube.com',
        referer: 'https://www.youtube.com/',
        ...(range ? { range } : {}),
      },
      signal: controller.signal,
    });
    const chunks = [];
    let received = 0;
    const reader = res.body?.getReader();
    while (reader && received < maxBytes) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(Buffer.from(value));
      received += value.length;
    }
    await reader?.cancel().catch(() => {});
    const buffer = Buffer.concat(chunks);
    return {
      status: res.status,
      ms: Date.now() - started,
      bytesRead: buffer.length,
      contentLengthHeader: res.headers.get('content-length'),
      contentRange: res.headers.get('content-range'),
      contentType: res.headers.get('content-type'),
      bodyPreview: res.status >= 400 ? buffer.toString('utf8').replace(/\s+/g, ' ').slice(0, 120) : null,
    };
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------------
// Utilidades de inspección de JSON
// ---------------------------------------------------------------------------

// Rutas (notación `a.b[0].c`) donde aparece una clave: la evidencia de "qué ruta
// funciona hoy" que se reporta por cada endpoint.
function findKeyPaths(root, keyName, { limit = 3, maxDepth = 14 } = {}) {
  const out = [];
  const walk = (node, nodePath, depth) => {
    if (out.length >= limit || depth > maxDepth || node === null || typeof node !== 'object') return;
    if (Array.isArray(node)) {
      for (let i = 0; i < node.length && out.length < limit; i++) walk(node[i], `${nodePath}[${i}]`, depth + 1);
      return;
    }
    for (const key of Object.keys(node)) {
      if (out.length >= limit) return;
      const childPath = nodePath ? `${nodePath}.${key}` : key;
      if (key === keyName) {
        out.push(childPath);
        continue;
      }
      walk(node[key], childPath, depth + 1);
    }
  };
  walk(root, '', 0);
  return out;
}

function getAt(root, dottedPath) {
  if (!dottedPath) return undefined;
  const parts = dottedPath.replace(/\[(\d+)\]/g, '.$1').split('.');
  let node = root;
  for (const part of parts) {
    if (node == null) return undefined;
    node = node[part];
  }
  return node;
}

function collectByKey(node, keyName, { limit = 50, depth = 0, maxDepth = 16, out = [] } = {}) {
  if (out.length >= limit || node === null || typeof node !== 'object' || depth > maxDepth) return out;
  if (Array.isArray(node)) {
    for (const item of node) collectByKey(item, keyName, { limit, depth: depth + 1, maxDepth, out });
    return out;
  }
  for (const [key, value] of Object.entries(node)) {
    if (key === keyName) out.push(value);
    else collectByKey(value, keyName, { limit, depth: depth + 1, maxDepth, out });
  }
  return out;
}

// Texto de {simpleText}, {runs:[{text}]} o {content}: las tres formas conviven
// en la API (renderers antiguos usan runs, los ViewModel nuevos usan content).
function textOf(value) {
  if (!value || typeof value !== 'object') return null;
  if (typeof value.simpleText === 'string') return value.simpleText;
  if (Array.isArray(value.runs)) return value.runs.map((r) => r.text ?? '').join('');
  if (typeof value.content === 'string') return value.content;
  return null;
}

function countRenderers(node, acc = {}, depth = 0) {
  if (node === null || typeof node !== 'object' || depth > 16) return acc;
  if (Array.isArray(node)) {
    for (const item of node) countRenderers(item, acc, depth + 1);
    return acc;
  }
  for (const [key, value] of Object.entries(node)) {
    if (/(Renderer|ViewModel)$/.test(key) && value && typeof value === 'object') acc[key] = (acc[key] ?? 0) + 1;
    countRenderers(value, acc, depth + 1);
  }
  return acc;
}

function parseJson(step) {
  try {
    return JSON.parse(step.text);
  } catch {
    return null;
  }
}

function kindsOf(items) {
  const kinds = {};
  for (const item of items ?? []) {
    const key = item && typeof item === 'object' ? (Object.keys(item)[0] ?? 'null') : 'null';
    kinds[key] = (kinds[key] ?? 0) + 1;
  }
  return kinds;
}

function continuationTokenAt(items) {
  const list = Array.isArray(items) ? items : [];
  for (let i = list.length - 1; i >= 0; i--) {
    const token = list[i]?.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token;
    if (token) return { index: i, token };
  }
  return null;
}

// ---------------------------------------------------------------------------
// Recorte de fixtures
// ---------------------------------------------------------------------------

// Ramas de la web que no intervienen en ninguna ruta de parseo y dominan el
// tamaño (chrome de la página, mutaciones de entidades, config del reproductor).
const DROPPED_BRANCHES = new Set([
  'responseContext',
  'frameworkUpdates',
  'topbar',
  'playerConfig',
  'storyboards',
  'annotations',
]);
const DROPPED_KEYS = new Set(['trackingParams', 'clickTrackingParams', 'loggingDirectives']);
// Decoración de UI (menús de tres puntos, previews, snippets, avatares,
// overlays): multiplica el tamaño por item y no aporta datos de dominio.
const DROPPED_DECORATION = new Set([
  'menu',
  'menus',
  'menuOnTap',
  'menuOnTapA11yLabel',
  'richThumbnail',
  'movingThumbnailRenderer',
  'detailedMetadataSnippets',
  'thumbnailOverlays',
  'inlinePlaybackEndpoint',
  'inlinePlayerData',
  'expandableMetadata',
  'channelThumbnailSupportedRenderers',
  'avatar',
  'themedPalette',
  'lottieData',
  'superTitleLink',
  'pageVisualEffects',
  'playerOverlays',
  'impressionEndpoints',
]);

const AD_HOST = /(^|\.)(doubleclick\.net|googleadservices\.com|googlesyndication\.com|google\.com\/pagead)$/i;
const MEDIA_HOST = /(^|\.)googlevideo\.com$/i;
// Parámetros de seguimiento que se eliminan de las URLs (miniaturas): solo
// `sqp` y `rs`; el resto de la URL se conserva tal cual.
const TRACKING_PARAMS = ['sqp', 'rs'];

function cleanString(value, maxString) {
  let out = value;
  if (/^https?:\/\//i.test(value)) {
    let parsed = null;
    try {
      parsed = new URL(value);
    } catch {
      parsed = null;
    }
    if (parsed && AD_HOST.test(parsed.hostname)) return '[url-de-tracking-eliminada]';
    if (parsed && MEDIA_HOST.test(parsed.hostname)) return '[url-googlevideo-eliminada]';
    if (parsed) {
      for (const param of TRACKING_PARAMS) parsed.searchParams.delete(param);
      out = parsed.toString();
    }
  }
  return out.length > maxString ? `${out.slice(0, maxString)}…` : out;
}

function trimArray(items, state, depth) {
  const out = [];
  let keptContinuation = false;
  for (const item of items) {
    if (out.length < state.maxItems) {
      out.push(trimResponse(item, state, depth + 1));
      continue;
    }
    // El centinela de continuación suele vivir al final del array: se conserva
    // aunque se recorte el resto para no perder el token de la página siguiente.
    const isContinuation = Boolean(item && typeof item === 'object' && item.continuationItemRenderer);
    if (isContinuation && !keptContinuation) {
      keptContinuation = true;
      out.push(trimResponse(item, state, depth + 1));
    }
  }
  state.droppedItems += items.length - out.length;
  return out;
}

function trimResponse(node, state, depth = 0) {
  if (node === null || node === undefined) return node;
  if (typeof node === 'string') return cleanString(node, state.maxString);
  if (typeof node !== 'object') return node;
  if (depth > 30) return null;
  if (Array.isArray(node)) return trimArray(node, state, depth);
  const out = {};
  for (const [key, value] of Object.entries(node)) {
    if (DROPPED_KEYS.has(key) || DROPPED_BRANCHES.has(key) || DROPPED_DECORATION.has(key)) continue;
    out[key] = trimResponse(value, state, depth + 1);
  }
  return out;
}

function writeFixture(fileName, payload) {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  let lastBytes = 0;
  for (const [maxItems, maxString] of [
    [40, 360],
    [24, 320],
    [16, 280],
    [10, 240],
    [6, 200],
  ]) {
    const state = { maxItems, maxString, droppedItems: 0 };
    const trimmed = trimResponse(payload, state);
    const text = `${JSON.stringify(trimmed, null, 1)}\n`;
    const bytes = Buffer.byteLength(text);
    lastBytes = bytes;
    if (bytes < 290 * 1024) {
      const target = path.join(FIXTURE_DIR, fileName);
      fs.writeFileSync(target, text);
      return {
        file: path.relative(ROOT, target).replaceAll('\\', '/'),
        bytes,
        maxItems,
        maxString,
        itemsDropped: state.droppedItems,
      };
    }
  }
  throw new Error(`la fixture ${fileName} no baja de 300 KB (último intento: ${lastBytes} bytes)`);
}

// Fixture del reproductor: conserva la estructura de `streamingData` y sustituye
// cada `url` firmada por una URL inerte con `itag` y `clen`. Las URLs reales
// caducan y están ligadas a IP: no se versionan.
function playerFixturePayload(payload) {
  const copy = JSON.parse(JSON.stringify(payload));
  const streaming = copy.streamingData;
  if (streaming) {
    for (const group of ['formats', 'adaptiveFormats']) {
      for (const format of streaming[group] ?? []) {
        if (typeof format.url === 'string' || typeof format.signatureCipher === 'string') {
          delete format.signatureCipher;
          format.url = `https://example.invalid/videoplayback?itag=${format.itag}&clen=${format.contentLength ?? ''}`;
        }
      }
    }
    for (const key of ['hlsManifestUrl', 'dashManifestUrl', 'serverAbrStreamingUrl']) {
      if (typeof streaming[key] === 'string') streaming[key] = `https://example.invalid/${key}`;
    }
  }
  return copy;
}

// ---------------------------------------------------------------------------
// Sesión anónima: visitorData, cookies y STS desde la página /watch
// ---------------------------------------------------------------------------

async function bootstrap() {
  const page = await fetchText(WATCH_URL, {
    headers: {
      'user-agent': VISIONOS_UA,
      'accept-language': 'en-US,en;q=0.9',
      cookie: BASE_COOKIE,
    },
  });
  const html = page.text;
  const visitorData = html.match(/"visitorData":"([^"]+)"/)?.[1] ?? null;
  const clientVersion = html.match(/"INNERTUBE_CLIENT_VERSION":"([^"]+)"/)?.[1] ?? null;
  const signatureTimestamp = html.match(/"STS":(\d+)/)?.[1] ?? null;
  const cookies = page.setCookies.map((c) => c.split(';')[0]).filter((c) => c && !/^GPS=/.test(c));
  const cookieHeader = [BASE_COOKIE, ...cookies].join('; ');
  log({
    probe: 'bootstrap',
    step: 'watch-page',
    method: 'GET',
    url: WATCH_URL,
    headersSent: { 'user-agent': VISIONOS_UA, cookie: BASE_COOKIE, 'accept-language': 'en-US,en;q=0.9' },
    status: page.status,
    ms: page.ms,
    bytes: page.bytes,
    found: {
      visitorDataPreview: visitorData ? `${visitorData.slice(0, 12)}…` : null,
      INNERTUBE_CLIENT_VERSION: clientVersion,
      signatureTimestamp: signatureTimestamp ? Number(signatureTimestamp) : null,
      cookiesSet: cookies.map((c) => c.split('=')[0]),
    },
  });
  if (!visitorData || !clientVersion || !signatureTimestamp) {
    throw new Error('la página /watch no expone visitorData, INNERTUBE_CLIENT_VERSION o STS');
  }
  return { visitorData, webClientVersion: clientVersion, signatureTimestamp: Number(signatureTimestamp), cookieHeader };
}

const LOG = [];
const SUMMARY = [];

function log(entry) {
  LOG.push(entry);
  process.stdout.write(`${JSON.stringify(entry)}\n`);
}

function summarize(entry) {
  SUMMARY.push(entry);
}

// ---------------------------------------------------------------------------
// Perfiles de contexto
// ---------------------------------------------------------------------------

function webContext(ctx) {
  return { client: { clientName: 'WEB', clientVersion: ctx.webClientVersion, hl: 'en', gl: 'US' } };
}

function remixContext(ctx) {
  return { client: { clientName: 'WEB_REMIX', clientVersion: ctx.remixClientVersion, hl: 'en', gl: 'US' } };
}

function playbackContext(ctx, { html5 = true, sts = true } = {}) {
  const contentPlaybackContext = {};
  if (html5) contentPlaybackContext.html5Preference = 'HTML5_PREF_WANTS';
  if (sts) contentPlaybackContext.signatureTimestamp = ctx.signatureTimestamp;
  return { contentPlaybackContext };
}

async function ytPost(ctx, apiPath, body) {
  return postInnerTube(`${YT}/youtubei/v1/${apiPath}`, {
    clientName: 1,
    clientVersion: ctx.webClientVersion,
    ua: VISIONOS_UA,
    origin: YT,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body,
  });
}

// ---------------------------------------------------------------------------
// 1. search (WEB)
// ---------------------------------------------------------------------------

const SEARCH_SECTION_LIST_PATH = 'contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents';
const SEARCH_ITEMS_PATH = `${SEARCH_SECTION_LIST_PATH}[0].itemSectionRenderer.contents`;

function videoRendererPreview(video) {
  if (!video) return null;
  return {
    videoId: video.videoId ?? null,
    title: textOf(video.title),
    channel: textOf(video.ownerText) ?? textOf(video.longBylineText),
    channelId: video.ownerText?.runs?.[0]?.navigationEndpoint?.browseEndpoint?.browseId ?? null,
    lengthText: video.lengthText?.simpleText ?? null,
    viewCount: textOf(video.viewCountText),
    publishedTime: textOf(video.publishedTimeText),
    thumbnail: video.thumbnail?.thumbnails?.[0]?.url ?? null,
  };
}

function shortsLockupPreview(lockup) {
  const reel = lockup?.onTap?.innertubeCommand?.reelWatchEndpoint ?? null;
  return {
    videoId: reel?.videoId ?? null,
    entityId: lockup?.entityId ?? null,
    title: textOf(lockup?.overlayMetadata?.primaryText),
    views: textOf(lockup?.overlayMetadata?.secondaryText),
    hasParams: Boolean(reel?.params),
    hasPlayerParams: Boolean(reel?.playerParams),
    hasSequenceParams: Boolean(reel?.sequenceParams),
  };
}

function lockupPreview(lockup) {
  const metadata = lockup?.metadata?.lockupMetadataViewModel;
  const watch = lockup?.rendererContext?.commandContext?.onTap?.innertubeCommand?.watchEndpoint;
  return {
    contentType: lockup?.contentType ?? null,
    contentId: lockup?.contentId ?? null,
    title: metadata?.title?.content ?? null,
    channel:
      metadata?.metadata?.contentMetadataViewModel?.metadataRows?.[0]?.metadataParts?.[0]?.text?.content ?? null,
    watchVideoId: watch?.videoId ?? null,
  };
}

async function probeSearch(ctx) {
  const body = { context: webContext(ctx), query: 'lofi hip hop' };
  const step = await ytPost(ctx, 'search', body);
  const payload = parseJson(step) ?? {};
  const items = getAt(payload, SEARCH_ITEMS_PATH) ?? [];
  const paths = {
    items: SEARCH_ITEMS_PATH,
    videoRenderer: findKeyPaths(payload, 'videoRenderer', { limit: 1 }),
    lockupViewModel: findKeyPaths(payload, 'lockupViewModel', { limit: 1 }),
    gridShelfViewModel: findKeyPaths(payload, 'gridShelfViewModel', { limit: 1 }),
    shortsLockupViewModel: findKeyPaths(payload, 'shortsLockupViewModel', { limit: 1 }),
    channelRenderer: findKeyPaths(payload, 'channelRenderer', { limit: 1 }),
    playlistRenderer: findKeyPaths(payload, 'playlistRenderer', { limit: 1 }),
    adSlotRenderer: findKeyPaths(payload, 'adSlotRenderer', { limit: 1 }),
    continuationItemRenderer: findKeyPaths(payload, 'continuationItemRenderer', { limit: 1 }),
  };
  const firstVideo = videoRendererPreview(items.find((i) => i?.videoRenderer)?.videoRenderer);
  const lockup = items.find((i) => i?.lockupViewModel)?.lockupViewModel;
  const shortsShelf = items.find((i) => i?.gridShelfViewModel)?.gridShelfViewModel;
  const shorts = (shortsShelf?.contents ?? []).filter((c) => c?.shortsLockupViewModel).map((c) => shortsLockupPreview(c.shortsLockupViewModel));
  const continuation = continuationTokenAt(getAt(payload, SEARCH_SECTION_LIST_PATH) ?? []);
  log({
    probe: 'search',
    step: 'primary',
    method: 'POST',
    url: `${YT}/youtubei/v1/search?prettyPrint=false`,
    clientProfile: 'WEB',
    headersSent: { 'x-youtube-client-name': '1', 'x-youtube-client-version': ctx.webClientVersion, 'x-goog-visitor-id': 'presente', cookie: 'presente' },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    itemsPath: SEARCH_ITEMS_PATH,
    itemCount: items.length,
    itemKinds: kindsOf(items),
    paths,
    firstVideo,
    firstLockup: lockupPreview(lockup),
    shortsShelf: shortsShelf ? { itemCount: shorts.length, items: shorts.slice(0, 4) } : null,
    continuation: continuation
      ? {
          path: `${SEARCH_SECTION_LIST_PATH}[${continuation.index}].continuationItemRenderer.continuationEndpoint.continuationCommand.token`,
          tokenLength: continuation.token.length,
        }
      : null,
    rendererCounts: countRenderers(payload),
    errors: payload.error ?? null,
  });
  ctx.state.searchContinuation = continuation?.token ?? null;
  ctx.state.shortsShelfPath = shortsShelf ? `${SEARCH_ITEMS_PATH}[${items.indexOf(items.find((i) => i?.gridShelfViewModel))}].gridShelfViewModel` : null;

  // La mezcla de renderers depende de la intención de la consulta: con nombres
  // de creador aparecen channelRenderer y playlists, con "lofi hip hop" solo
  // vídeos y estantes de Shorts.
  const altBody = { context: webContext(ctx), query: 'daft punk' };
  const altStep = await ytPost(ctx, 'search', altBody);
  const alt = parseJson(altStep) ?? {};
  const altItems = getAt(alt, SEARCH_ITEMS_PATH) ?? [];
  log({
    probe: 'search',
    step: 'secondary-query:daft punk',
    method: 'POST',
    url: `${YT}/youtubei/v1/search?prettyPrint=false`,
    clientProfile: 'WEB',
    requestBody: altBody,
    status: altStep.status,
    ms: altStep.ms,
    bytes: altStep.bytes,
    itemsPath: SEARCH_ITEMS_PATH,
    itemCount: altItems.length,
    itemKinds: kindsOf(altItems),
    paths: {
      channelRenderer: findKeyPaths(alt, 'channelRenderer', { limit: 1 }),
      playlistRenderer: findKeyPaths(alt, 'playlistRenderer', { limit: 1 }),
      lockupViewModel: findKeyPaths(alt, 'lockupViewModel', { limit: 1 }),
      gridShelfViewModel: findKeyPaths(alt, 'gridShelfViewModel', { limit: 1 }),
    },
    firstChannel: (() => {
      const channel = collectByKey(alt, 'channelRenderer', { limit: 1 })[0];
      return channel
        ? {
            channelId: channel.channelId ?? null,
            title: textOf(channel.title),
            subscribers: textOf(channel.videoCountText),
            handle: textOf(channel.subscriberCountText),
            path: '…itemSectionRenderer.contents[N].channelRenderer',
          }
        : null;
    })(),
    firstPlaylist: (() => {
      const playlist = collectByKey(alt, 'playlistRenderer', { limit: 1 })[0];
      return playlist ? { playlistId: playlist.playlistId ?? null, title: textOf(playlist.title) } : null;
    })(),
    errors: alt.error ?? null,
  });

  const fixture = writeFixture('search.json', payload);
  log({ probe: 'search', step: 'fixture', ...fixture });
  summarize({ probe: 'search', status: step.status, bytes: step.bytes, note: `${items.length} items · ${Object.entries(kindsOf(items)).map(([k, v]) => `${k}:${v}`).join(' ')}` });
}

// ---------------------------------------------------------------------------
// 2. browse FEwhat_to_watch (WEB)
// ---------------------------------------------------------------------------

const HOME_GRID_PATH = 'contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.richGridRenderer.contents';

// Recorre la rejilla de la portada: los items pueden venir sueltos
// (`richItemRenderer`) o dentro de un estante (`richSectionRenderer` →
// `richShelfRenderer.contents[]`).
async function probeHome(ctx) {
  const body = { context: webContext(ctx), browseId: 'FEwhat_to_watch' };
  const step = await ytPost(ctx, 'browse', body);
  const payload = parseJson(step) ?? {};
  const gridPath = findKeyPaths(payload, 'richGridRenderer', { limit: 1 })[0] ?? null;
  const grid = (gridPath ? getAt(payload, `${gridPath}.contents`) : getAt(payload, HOME_GRID_PATH)) ?? [];
  const flatItems = [];
  for (const item of grid) {
    if (item?.richItemRenderer) flatItems.push(item.richItemRenderer);
    else if (item?.richSectionRenderer?.content?.richShelfRenderer) flatItems.push(...(item.richSectionRenderer.content.richShelfRenderer.contents ?? []));
  }
  const videos = flatItems.map((item) => item?.richItemRenderer?.content?.videoRenderer ?? item?.content?.videoRenderer).filter(Boolean);
  const lockups = flatItems.map((item) => item?.richItemRenderer?.content?.lockupViewModel ?? item?.content?.lockupViewModel).filter(Boolean);
  const nudge = collectByKey(payload, 'feedNudgeRenderer', { limit: 1 })[0] ?? null;
  const paths = {
    richGridRenderer: gridPath ? [gridPath] : [],
    richSectionRenderer: findKeyPaths(payload, 'richSectionRenderer', { limit: 1 }),
    richShelfRenderer: findKeyPaths(payload, 'richShelfRenderer', { limit: 1 }),
    richItemRenderer: findKeyPaths(payload, 'richItemRenderer', { limit: 2 }),
    videoRenderer: findKeyPaths(payload, 'videoRenderer', { limit: 1 }),
    lockupViewModel: findKeyPaths(payload, 'lockupViewModel', { limit: 1 }),
    adSlotRenderer: findKeyPaths(payload, 'adSlotRenderer', { limit: 1 }),
    feedNudgeRenderer: findKeyPaths(payload, 'feedNudgeRenderer', { limit: 1 }),
    continuationItemRenderer: findKeyPaths(payload, 'continuationItemRenderer', { limit: 1 }),
  };
  const continuation = continuationTokenAt(grid);
  log({
    probe: 'home',
    step: 'primary',
    method: 'POST',
    url: `${YT}/youtubei/v1/browse?prettyPrint=false`,
    clientProfile: 'WEB',
    headersSent: { 'x-youtube-client-name': '1', 'x-youtube-client-version': ctx.webClientVersion, 'x-goog-visitor-id': 'presente', cookie: 'presente' },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    gridPath: gridPath ?? HOME_GRID_PATH,
    gridItemCount: grid.length,
    gridItemKinds: kindsOf(grid),
    paths,
    videoCount: videos.length,
    lockupCount: lockups.length,
    firstVideo: videoRendererPreview(videos[0]),
    firstLockup: lockupPreview(lockups[0]),
    emptyFeed: Boolean(nudge) && videos.length === 0,
    feedNudge: nudge ? { title: textOf(nudge.title), subtitle: textOf(nudge.subtitle) } : null,
    continuation: continuation
      ? { path: `${gridPath ?? HOME_GRID_PATH}[${continuation.index}].continuationItemRenderer.continuationEndpoint.continuationCommand.token`, tokenLength: continuation.token.length }
      : null,
    rendererCounts: countRenderers(payload),
    errors: payload.error ?? null,
  });

  // Explorar/trending exigen `params` que la web firma: sin ellos el endpoint
  // responde 400. Se mide para documentar que no es una alternativa anónima.
  for (const browseId of ['FEtrending', 'FEexplore']) {
    const alt = await ytPost(ctx, 'browse', { context: webContext(ctx), browseId });
    const altJson = parseJson(alt) ?? {};
    log({
      probe: 'home',
      step: `negative-control:${browseId}`,
      method: 'POST',
      url: `${YT}/youtubei/v1/browse?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { context: 'WEB', browseId },
      status: alt.status,
      ms: alt.ms,
      bytes: alt.bytes,
      error: altJson.error ? { code: altJson.error.code, message: altJson.error.message } : null,
      rawPreview: altJson.error ? null : alt.text.slice(0, 120),
    });
  }

  ctx.state.homeContinuation = continuation?.token ?? null;
  const fixture = writeFixture('home.json', payload);
  log({ probe: 'home', step: 'fixture', ...fixture });
  summarize({ probe: 'home', status: step.status, bytes: step.bytes, note: `${grid.length} secciones · vídeos:${videos.length} lockups:${lockups.length}${nudge ? ' · feed vacío (feedNudgeRenderer)' : ''}` });
}

// ---------------------------------------------------------------------------
// 3. next (WEB): metadatos del vídeo, relacionados y comentarios
// ---------------------------------------------------------------------------

async function probeWatchNext(ctx) {
  const body = { context: webContext(ctx), videoId: VIDEO_ID, contentCheckOk: true, racyCheckOk: true };
  const step = await ytPost(ctx, 'next', body);
  const payload = parseJson(step) ?? {};
  const relatedPath = 'contents.twoColumnWatchNextResults.secondaryResults.secondaryResults.results';
  const related = getAt(payload, relatedPath) ?? [];
  const resultsPath = 'contents.twoColumnWatchNextResults.results.results.contents';
  const results = getAt(payload, resultsPath) ?? [];
  const paths = {
    results: resultsPath,
    related: relatedPath,
    videoPrimaryInfoRenderer: findKeyPaths(payload, 'videoPrimaryInfoRenderer', { limit: 1 }),
    videoSecondaryInfoRenderer: findKeyPaths(payload, 'videoSecondaryInfoRenderer', { limit: 1 }),
    videoOwnerRenderer: findKeyPaths(payload, 'videoOwnerRenderer', { limit: 1 }),
    segmentedLikeDislikeButtonViewModel: findKeyPaths(payload, 'segmentedLikeDislikeButtonViewModel', { limit: 1 }),
    attributedDescription: findKeyPaths(payload, 'attributedDescription', { limit: 1 }),
    lockupViewModel: findKeyPaths(payload, 'lockupViewModel', { limit: 2 }),
    commentSectionRenderer: findKeyPaths(payload, 'commentSectionRenderer', { limit: 1 }),
    itemSectionRenderer: findKeyPaths(payload, 'itemSectionRenderer', { limit: 2 }),
    continuationItemRenderer: findKeyPaths(payload, 'continuationItemRenderer', { limit: 2 }),
    engagementPanelSectionListRenderer: findKeyPaths(payload, 'engagementPanelSectionListRenderer', { limit: 2 }),
  };
  const primary = collectByKey(payload, 'videoPrimaryInfoRenderer', { limit: 1 })[0];
  const owner = collectByKey(payload, 'videoOwnerRenderer', { limit: 1 })[0];
  const likeText = collectByKey(
    collectByKey(payload, 'segmentedLikeDislikeButtonViewModel', { limit: 1 })[0] ?? {},
    'buttonViewModel',
    { limit: 4 },
  )
    .map((b) => b.title)
    .filter((t) => typeof t === 'string');
  const description = collectByKey(payload, 'attributedDescription', { limit: 1 })[0];
  const relatedLockups = related.map((item) => item?.lockupViewModel).filter(Boolean);
  const relatedContinuation = continuationTokenAt(related);
  const panels = collectByKey(payload, 'engagementPanelSectionListRenderer', { limit: 8 }).map((p) => ({
    panelIdentifier: p.panelIdentifier ?? null,
    visibility: p.visibility ?? null,
    hasContent: Boolean(p.content),
  }));
  // El bloque de comentarios (`sectionIdentifier: comment-item-section`) trae su
  // propio token: es la continuación que necesita la pantalla de vídeo.
  const commentSectionIndex = results.findIndex((item) => item?.itemSectionRenderer?.sectionIdentifier === 'comment-item-section');
  const commentsToken =
    results[commentSectionIndex]?.itemSectionRenderer?.contents?.[0]?.continuationItemRenderer?.continuationEndpoint
      ?.continuationCommand?.token ?? null;
  log({
    probe: 'watch-next',
    step: 'primary',
    method: 'POST',
    url: `${YT}/youtubei/v1/next?prettyPrint=false`,
    clientProfile: 'WEB',
    headersSent: { 'x-youtube-client-name': '1', 'x-youtube-client-version': ctx.webClientVersion, 'x-goog-visitor-id': 'presente', cookie: 'presente' },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    resultsPath,
    resultsItemCount: results.length,
    resultsItemKinds: kindsOf(results),
    relatedPath,
    relatedItemCount: related.length,
    relatedItemKinds: kindsOf(related),
    paths,
    title: textOf(primary?.title),
    viewCount: textOf(primary?.viewCount?.videoViewCountRenderer?.viewCount),
    likeCounts: likeText,
    owner: owner ? { title: textOf(owner.title), subscribers: textOf(owner.subscriberCountText), channelId: owner.navigationEndpoint?.browseEndpoint?.browseId ?? null } : null,
    descriptionPreview: description?.content ? description.content.slice(0, 120) : null,
    firstRelated: relatedLockups[0] ? lockupPreview(relatedLockups[0]) : null,
    relatedContinuation: relatedContinuation
      ? { path: `${relatedPath}[${relatedContinuation.index}].continuationItemRenderer.continuationEndpoint.continuationCommand.token`, tokenLength: relatedContinuation.token.length }
      : null,
    commentsContinuation: commentsToken
      ? {
          path: `${resultsPath}[${commentSectionIndex}].itemSectionRenderer.contents[0].continuationItemRenderer.continuationEndpoint.continuationCommand.token`,
          tokenLength: commentsToken.length,
        }
      : null,
    engagementPanels: panels,
    rendererCounts: countRenderers(payload),
    errors: payload.error ?? null,
  });
  ctx.state.nextContinuation = relatedContinuation?.token ?? null;
  ctx.state.commentsContinuation = commentsToken;
  const fixture = writeFixture('watch_next.json', payload);
  log({ probe: 'watch-next', step: 'fixture', ...fixture });
  summarize({ probe: 'watch-next', status: step.status, bytes: step.bytes, note: `${results.length} bloques · ${related.length} relacionados · título: ${textOf(primary?.title) ?? 'n/d'}` });
}

// ---------------------------------------------------------------------------
// 4/5. YouTube Music: FEmusic_home y search (WEB_REMIX)
// ---------------------------------------------------------------------------

function musicItemPreview(item) {
  const kind = item && typeof item === 'object' ? Object.keys(item)[0] : null;
  const renderer = kind ? item[kind] : null;
  if (!renderer) return null;
  const flexColumns = (renderer.flexColumns ?? [])
    .map((column) => textOf(column?.musicResponsiveListItemFlexColumnRenderer?.text))
    .filter(Boolean);
  const browse = renderer.navigationEndpoint?.browseEndpoint;
  const watch = renderer.navigationEndpoint?.watchEndpoint;
  return {
    renderer: kind,
    title: textOf(renderer.title) ?? flexColumns[0] ?? null,
    subtitle: textOf(renderer.subtitle) ?? flexColumns[1] ?? null,
    videoId: renderer.videoId ?? watch?.videoId ?? null,
    browseId: browse?.browseId ?? null,
    pageType: browse?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType ?? null,
    itemCount: renderer.itemCount ?? null,
  };
}

// Lee la clientVersion vigente de WEB_REMIX desde la home de music.youtube.com:
// se reutiliza la misma visita para no inventar una versión que el servidor ya
// no sirve.
async function detectRemixClientVersion(ctx) {
  const page = await fetchText(`${YTM}/`, {
    headers: { 'user-agent': VISIONOS_UA, 'accept-language': 'en-US,en;q=0.9', cookie: ctx.cookieHeader },
  });
  const remixClientVersion = page.text.match(/"INNERTUBE_CLIENT_VERSION":"([^"]+)"/)?.[1] ?? null;
  log({
    probe: 'music-home',
    step: 'bootstrap-music',
    method: 'GET',
    url: `${YTM}/`,
    status: page.status,
    ms: page.ms,
    bytes: page.bytes,
    found: { INNERTUBE_CLIENT_VERSION: remixClientVersion },
  });
  if (!remixClientVersion) throw new Error('no se pudo leer INNERTUBE_CLIENT_VERSION de music.youtube.com');
  return remixClientVersion;
}

async function probeMusicHome(ctx) {
  ctx.remixClientVersion ??= await detectRemixClientVersion(ctx);
  const url = `${YTM}/youtubei/v1/browse`;
  const body = { context: remixContext(ctx), browseId: 'FEmusic_home' };
  const step = await postInnerTube(url, {
    clientName: 67,
    clientVersion: ctx.remixClientVersion,
    ua: VISIONOS_UA,
    origin: YTM,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body,
  });
  const payload = parseJson(step) ?? {};
  const sectionsPath = 'contents.singleColumnBrowseResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents';
  const sections = getAt(payload, sectionsPath) ?? [];
  const shelves = sections.map((section, index) => {
    const carousel = section?.musicCarouselShelfRenderer;
    const shelf = section?.musicShelfRenderer;
    const container = carousel ?? shelf;
    if (!container) return { index, sectionKeys: Object.keys(section ?? {}) };
    const items = container.contents ?? [];
    return {
      index,
      kind: carousel ? 'musicCarouselShelfRenderer' : 'musicShelfRenderer',
      title:
        textOf(container.header?.musicCarouselShelfBasicHeaderRenderer?.title) ??
        textOf(container.header?.musicShelfBasicHeaderRenderer?.title) ??
        textOf(container.title),
      itemCount: items.length,
      firstItem: musicItemPreview(items[0]),
      itemKinds: kindsOf(items),
    };
  });
  log({
    probe: 'music-home',
    step: 'primary',
    method: 'POST',
    url: `${url}?prettyPrint=false`,
    clientProfile: 'WEB_REMIX',
    headersSent: { 'x-youtube-client-name': '67', 'x-youtube-client-version': ctx.remixClientVersion, 'x-goog-visitor-id': 'presente', cookie: 'presente' },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    sectionsPath,
    sectionsCount: sections.length,
    sectionKinds: kindsOf(sections),
    shelves,
    paths: {
      singleColumnBrowseResultsRenderer: findKeyPaths(payload, 'singleColumnBrowseResultsRenderer', { limit: 1 }),
      musicCarouselShelfRenderer: findKeyPaths(payload, 'musicCarouselShelfRenderer', { limit: 1 }),
      musicShelfRenderer: findKeyPaths(payload, 'musicShelfRenderer', { limit: 1 }),
      musicTwoRowItemRenderer: findKeyPaths(payload, 'musicTwoRowItemRenderer', { limit: 1 }),
      musicResponsiveListItemRenderer: findKeyPaths(payload, 'musicResponsiveListItemRenderer', { limit: 1 }),
      musicTastebuilderShelfRenderer: findKeyPaths(payload, 'musicTastebuilderShelfRenderer', { limit: 1 }),
    },
    rendererCounts: countRenderers(payload),
    errors: payload.error ?? null,
  });
  const fixture = writeFixture('music_home.json', payload);
  log({ probe: 'music-home', step: 'fixture', ...fixture });
  summarize({ probe: 'music-home', status: step.status, bytes: step.bytes, note: `${sections.length} secciones · ${shelves.map((s) => `${s.kind ?? '?'}(${s.itemCount ?? 0})`).join(' ')}` });
}

const MUSIC_SEARCH_ITEMS_PATH =
  'contents.tabbedSearchResultsRenderer.tabs[0].tabRenderer.content.sectionListRenderer.contents';
const MUSIC_ITEM_RENDERER = /^(musicTwoRowItemRenderer|musicResponsiveListItemRenderer|musicCardShelfRenderer)$/;

async function probeMusicSearch(ctx) {
  ctx.remixClientVersion ??= await detectRemixClientVersion(ctx);
  const url = `${YTM}/youtubei/v1/search`;
  const body = { context: remixContext(ctx), query: 'daft punk' };
  const step = await postInnerTube(url, {
    clientName: 67,
    clientVersion: ctx.remixClientVersion,
    ua: VISIONOS_UA,
    origin: YTM,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body,
  });
  const payload = parseJson(step) ?? {};
  const sections = getAt(payload, MUSIC_SEARCH_ITEMS_PATH) ?? [];
  const sampled = [];
  for (const section of sections) {
    const items = section?.itemSectionRenderer?.contents ?? (section?.musicCardShelfRenderer ? [section] : []);
    for (const item of items) {
      const kind = item && typeof item === 'object' ? Object.keys(item)[0] : null;
      if (!MUSIC_ITEM_RENDERER.test(kind ?? '')) continue;
      const preview = musicItemPreview(item);
      if (preview && sampled.length < 10) sampled.push(preview);
    }
  }
  const card = collectByKey(payload, 'musicCardShelfRenderer', { limit: 1 })[0];
  log({
    probe: 'music-search',
    step: 'primary',
    method: 'POST',
    url: `${url}?prettyPrint=false`,
    clientProfile: 'WEB_REMIX',
    headersSent: { 'x-youtube-client-name': '67', 'x-youtube-client-version': ctx.remixClientVersion, 'x-goog-visitor-id': 'presente', cookie: 'presente' },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    sectionsPath: MUSIC_SEARCH_ITEMS_PATH,
    sectionsCount: sections.length,
    sectionKinds: kindsOf(sections),
    paths: {
      tabbedSearchResultsRenderer: findKeyPaths(payload, 'tabbedSearchResultsRenderer', { limit: 1 }),
      musicCardShelfRenderer: findKeyPaths(payload, 'musicCardShelfRenderer', { limit: 1 }),
      itemSectionRenderer: findKeyPaths(payload, 'itemSectionRenderer', { limit: 1 }),
      musicResponsiveListItemRenderer: findKeyPaths(payload, 'musicResponsiveListItemRenderer', { limit: 2 }),
      musicShelfRenderer: findKeyPaths(payload, 'musicShelfRenderer', { limit: 1 }),
      musicTwoRowItemRenderer: findKeyPaths(payload, 'musicTwoRowItemRenderer', { limit: 1 }),
      continuationItemRenderer: findKeyPaths(payload, 'continuationItemRenderer', { limit: 1 }),
    },
    topResult: card
      ? {
          title: textOf(card.title),
          subtitle: textOf(card.subtitle),
          browseId: card.onTap?.browseEndpoint?.browseId ?? null,
          contentsCount: card.contents?.length ?? 0,
        }
      : null,
    sampledItems: sampled,
    rendererCounts: countRenderers(payload),
    errors: payload.error ?? null,
  });
  const fixture = writeFixture('music_search.json', payload);
  log({ probe: 'music-search', step: 'fixture', ...fixture });
  summarize({ probe: 'music-search', status: step.status, bytes: step.bytes, note: `${sections.length} secciones · top: ${textOf(card?.title) ?? 'n/d'}` });
}

// ---------------------------------------------------------------------------
// 6. Shorts: descubrimiento por search y secuencia por reel/reel_watch_sequence
// ---------------------------------------------------------------------------

async function probeShorts(ctx) {
  const searchBody = { context: webContext(ctx), query: 'shorts' };
  const searchStep = await ytPost(ctx, 'search', searchBody);
  const searchJson = parseJson(searchStep) ?? {};
  const items = getAt(searchJson, SEARCH_ITEMS_PATH) ?? [];
  const shelfItem = items.find((item) => item?.gridShelfViewModel);
  const shelf = shelfItem?.gridShelfViewModel ?? null;
  const shelfPath = shelfItem ? `${SEARCH_ITEMS_PATH}[${items.indexOf(shelfItem)}].gridShelfViewModel` : null;
  const lockups = (shelf?.contents ?? []).filter((item) => item?.shortsLockupViewModel).map((item) => shortsLockupPreview(item.shortsLockupViewModel));
  const seedLockup = (shelf?.contents ?? []).find((item) => item?.shortsLockupViewModel)?.shortsLockupViewModel ?? null;
  const reel = seedLockup?.onTap?.innertubeCommand?.reelWatchEndpoint ?? null;
  log({
    probe: 'shorts',
    step: 'discovery:search',
    method: 'POST',
    url: `${YT}/youtubei/v1/search?prettyPrint=false`,
    clientProfile: 'WEB',
    requestBody: searchBody,
    status: searchStep.status,
    ms: searchStep.ms,
    bytes: searchStep.bytes,
    itemsPath: SEARCH_ITEMS_PATH,
    shelfPath,
    shelfItemCount: lockups.length,
    items: lockups.slice(0, 5),
    paths: {
      gridShelfViewModel: findKeyPaths(searchJson, 'gridShelfViewModel', { limit: 1 }),
      shortsLockupViewModel: findKeyPaths(searchJson, 'shortsLockupViewModel', { limit: 1 }),
      reelWatchEndpoint: findKeyPaths(searchJson, 'reelWatchEndpoint', { limit: 1 }),
    },
    errors: searchJson.error ?? null,
  });
  if (!reel?.videoId) throw new Error('el estante de Shorts no trajo reelWatchEndpoint con videoId');

  // Tres variantes de cuerpo para localizar la forma mínima que acepta el
  // endpoint de secuencia: solo-videoId (400), +params, +sequenceParams.
  const attempts = [
    { label: 'videoId', body: { context: webContext(ctx), videoId: reel.videoId } },
    { label: 'videoId+params', body: { context: webContext(ctx), videoId: reel.videoId, params: reel.params } },
    { label: 'videoId+params+sequenceParams', body: { context: webContext(ctx), videoId: reel.videoId, params: reel.params, sequenceParams: reel.sequenceParams } },
  ];
  let sequenceJson = null;
  for (const attempt of attempts) {
    const step = await ytPost(ctx, 'reel/reel_watch_sequence', attempt.body);
    const json = parseJson(step);
    const entries = json?.entries ?? [];
    const entryPaths = findKeyPaths(json ?? {}, 'reelWatchEndpoint', { limit: 1 });
    if (json && entries.length) sequenceJson = json;
    log({
      probe: 'shorts',
      step: `reel_watch_sequence:${attempt.label}`,
      method: 'POST',
      url: `${YT}/youtubei/v1/reel/reel_watch_sequence?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { ...attempt.body, params: attempt.body.params ? '…' : undefined, sequenceParams: attempt.body.sequenceParams ? '…' : undefined },
      status: step.status,
      ms: step.ms,
      bytes: step.bytes,
      ok: Boolean(json),
      responseKeys: json ? Object.keys(json) : null,
      entryCount: entries.length,
      entryIds: collectByKey(json ?? {}, 'reelWatchEndpoint', { limit: 10 }).map((e) => e.videoId),
      paths: {
        entries: findKeyPaths(json ?? {}, 'entries', { limit: 1 }),
        command: entryPaths.length ? [`${entryPaths[0]}.command`] : [],
        reelWatchEndpoint: entryPaths,
        continuationEndpoint: findKeyPaths(json ?? {}, 'continuationEndpoint', { limit: 1 }),
      },
      continuationToken: findKeyPaths(json ?? {}, 'continuationCommand', { limit: 1 }).length
        ? collectByKey(json, 'continuationCommand', { limit: 1 })[0]?.token?.slice(0, 24) + '…'
        : null,
      error: json?.error ?? null,
      rawPreview: json ? null : step.text.replace(/\s+/g, ' ').slice(0, 160),
    });
  }

  // reel_item_watch devuelve 200 pero vacío: se deja medido para no reintentarlo.
  const itemBody = { context: webContext(ctx), videoId: reel.videoId };
  const itemStep = await ytPost(ctx, 'reel/reel_item_watch', itemBody);
  const itemJson = parseJson(itemStep);
  log({
    probe: 'shorts',
    step: 'reel_item_watch',
    method: 'POST',
    url: `${YT}/youtubei/v1/reel/reel_item_watch?prettyPrint=false`,
    clientProfile: 'WEB',
    requestBody: itemBody,
    status: itemStep.status,
    ms: itemStep.ms,
    bytes: itemStep.bytes,
    responseKeys: itemJson ? Object.keys(itemJson) : null,
    hasReelData: Boolean(collectByKey(itemJson ?? {}, 'reelWatchEndpoint', { limit: 1 }).length),
    bodyPreview: itemStep.text.replace(/\s+/g, ' ').slice(0, 160),
  });

  // La reproducción de un Short es el mismo /player con el videoId: se comprueba
  // con VISIONOS para confirmar que los ids del estante son vídeos normales.
  const shortId = reel.videoId;
  const playerStep = await postInnerTube(`${YT}/youtubei/v1/player`, {
    clientName: 101,
    clientVersion: VISIONOS_CLIENT.clientVersion,
    ua: VISIONOS_UA,
    origin: YT,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body: {
      context: { client: { ...VISIONOS_CLIENT }, playbackContext: playbackContext(ctx) },
      videoId: shortId,
      contentCheckOk: true,
      racyCheckOk: true,
    },
  });
  const playerJson = parseJson(playerStep) ?? {};
  log({
    probe: 'shorts',
    step: 'player:visionos-short',
    method: 'POST',
    url: `${YT}/youtubei/v1/player?prettyPrint=false`,
    clientProfile: 'VISIONOS',
    requestBody: { videoId: shortId, context: 'VISIONOS' },
    status: playerStep.status,
    ms: playerStep.ms,
    bytes: playerStep.bytes,
    playabilityStatus: playerJson.playabilityStatus?.status ?? null,
    adaptiveFormats: (playerJson.streamingData?.adaptiveFormats ?? []).length,
    audioItags: (playerJson.streamingData?.adaptiveFormats ?? []).filter((f) => /^audio\//.test(f.mimeType ?? '')).map((f) => f.itag),
    videoDetails: playerJson.videoDetails ? { title: playerJson.videoDetails.title, lengthSeconds: playerJson.videoDetails.lengthSeconds } : null,
  });
  summarize({
    probe: 'shorts',
    status: searchStep.status,
    bytes: searchStep.bytes,
    note: `${lockups.length} shorts en el estante · secuencia: ${sequenceJson ? 'OK' : 'sin entries'} · short ${shortId} reproduce con VISIONOS: ${playerJson.playabilityStatus?.status ?? 'n/d'}`,
  });
}

// ---------------------------------------------------------------------------
// 7. player con VISIONOS + medición de rangos
// ---------------------------------------------------------------------------

const AUDIO_ITAGS = [139, 140, 249, 250, 251];

function formatSummary(format, kind) {
  return {
    kind,
    itag: format.itag,
    mimeType: format.mimeType,
    bitrate: format.bitrate ?? null,
    averageBitrate: format.averageBitrate ?? null,
    qualityLabel: format.qualityLabel ?? null,
    width: format.width ?? null,
    height: format.height ?? null,
    fps: format.fps ?? null,
    audioQuality: format.audioQuality ?? null,
    audioSampleRate: format.audioSampleRate ?? null,
    audioChannels: format.audioChannels ?? null,
    contentLength: format.contentLength ? Number(format.contentLength) : null,
    hasDirectUrl: typeof format.url === 'string',
    hasSignatureCipher: typeof format.signatureCipher === 'string',
    urlParams: typeof format.url === 'string' ? [...new URL(format.url).searchParams.keys()].join(',') : null,
    hasRqh: typeof format.url === 'string' ? new URL(format.url).searchParams.get('rqh') : null,
    hasPot: typeof format.url === 'string' ? new URL(format.url).searchParams.has('pot') : null,
  };
}

async function playerVisionos(ctx, { client = VISIONOS_CLIENT, ua = VISIONOS_UA, cookie = ctx.cookieHeader, body = null } = {}) {
  const requestBody = body ?? {
    context: { client: { ...client }, playbackContext: playbackContext(ctx) },
    videoId: VIDEO_ID,
    contentCheckOk: true,
    racyCheckOk: true,
  };
  const step = await postInnerTube(`${YT}/youtubei/v1/player`, {
    clientName: 101,
    clientVersion: VISIONOS_CLIENT.clientVersion,
    ua,
    origin: YT,
    visitorData: ctx.visitorData,
    cookie,
    body: requestBody,
  });
  return { requestBody, step, json: parseJson(step) ?? {} };
}

async function probePlayerVisionos(ctx) {
  const { requestBody, step, json } = await playerVisionos(ctx);
  const streaming = json.streamingData ?? {};
  const formats = (streaming.formats ?? []).map((f) => formatSummary(f, 'muxed'));
  const adaptive = (streaming.adaptiveFormats ?? []).map((f) => formatSummary(f, 'adaptive'));
  const audio = adaptive.filter((f) => /^audio\//.test(f.mimeType ?? ''));
  const video = adaptive.filter((f) => /^video\//.test(f.mimeType ?? ''));
  log({
    probe: 'player-visionos',
    step: 'primary',
    method: 'POST',
    url: `${YT}/youtubei/v1/player?prettyPrint=false`,
    clientProfile: 'VISIONOS',
    headersSent: {
      'x-youtube-client-name': '101',
      'x-youtube-client-version': VISIONOS_CLIENT.clientVersion,
      'x-goog-visitor-id': 'presente',
      cookie: ctx.cookieHeader,
      'user-agent': VISIONOS_UA,
      origin: YT,
      referer: `${YT}/`,
    },
    requestBody,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    responseKeys: Object.keys(json),
    playabilityStatus: json.playabilityStatus
      ? { status: json.playabilityStatus.status, reason: json.playabilityStatus.reason ?? null }
      : null,
    videoDetails: json.videoDetails
      ? {
          videoId: json.videoDetails.videoId,
          title: json.videoDetails.title,
          lengthSeconds: json.videoDetails.lengthSeconds,
          author: json.videoDetails.author,
          isLiveContent: json.videoDetails.isLiveContent,
          viewCount: json.videoDetails.viewCount,
        }
      : null,
    paths: {
      playabilityStatus: 'playabilityStatus',
      videoDetails: 'videoDetails',
      streamingData: 'streamingData',
      muxedFormats: 'streamingData.formats[]',
      adaptiveFormats: 'streamingData.adaptiveFormats[]',
      audioFormats: 'streamingData.adaptiveFormats[] (mimeType ^= audio/)',
      videoFormats: 'streamingData.adaptiveFormats[] (mimeType ^= video/)',
      captions: 'captions.playerCaptionsTracklistRenderer.captionTracks[]',
      hlsManifestUrl: 'streamingData.hlsManifestUrl',
    },
    counts: {
      muxed: formats.length,
      adaptive: adaptive.length,
      audio: audio.length,
      video: video.length,
    },
    muxedItags: formats.map((f) => f.itag),
    audioItags: audio.map((f) => ({ itag: f.itag, mimeType: f.mimeType, bitrate: f.bitrate, contentLength: f.contentLength, audioQuality: f.audioQuality, audioSampleRate: f.audioSampleRate, audioChannels: f.audioChannels })),
    audioItagsExpected: Object.fromEntries(AUDIO_ITAGS.map((itag) => [itag, audio.some((f) => f.itag === itag)])),
    videoItags: video.map((f) => ({ itag: f.itag, mimeType: f.mimeType, qualityLabel: f.qualityLabel, width: f.width, height: f.height, fps: f.fps, bitrate: f.bitrate, contentLength: f.contentLength })),
    captions: (json.captions?.playerCaptionsTracklistRenderer?.captionTracks ?? []).map((t) => ({
      languageCode: t.languageCode ?? null,
      kind: t.kind ?? null,
      name: textOf(t.name),
      hasBaseUrl: Boolean(t.baseUrl),
    })),
    hlsManifestUrl: typeof streaming.hlsManifestUrl === 'string' ? 'presente' : null,
    errors: json.error ?? null,
  });

  // Control negativo: el mismo /player con el cliente WEB no entrega streams.
  const webControl = await postInnerTube(`${YT}/youtubei/v1/player`, {
    clientName: 1,
    clientVersion: ctx.webClientVersion,
    ua: VISIONOS_UA,
    origin: YT,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body: {
      context: { client: { clientName: 'WEB', clientVersion: ctx.webClientVersion, hl: 'en', gl: 'US' }, playbackContext: playbackContext(ctx) },
      videoId: VIDEO_ID,
      contentCheckOk: true,
      racyCheckOk: true,
    },
  });
  const webJson = parseJson(webControl) ?? {};
  log({
    probe: 'player-visionos',
    step: 'negative-control:web-client',
    method: 'POST',
    url: `${YT}/youtubei/v1/player?prettyPrint=false`,
    clientProfile: 'WEB',
    requestBody: { videoId: VIDEO_ID, context: 'WEB' },
    status: webControl.status,
    ms: webControl.ms,
    bytes: webControl.bytes,
    playabilityStatus: webJson.playabilityStatus?.status ?? null,
    reason: webJson.playabilityStatus?.reason ?? null,
    adaptiveFormats: (webJson.streamingData?.adaptiveFormats ?? []).length,
  });

  // Variantes del perfil VISIONOS: sin cookie y sin signatureTimestamp. Se miden
  // porque el perfil canónico envía ambas y no queremos suponer qué es opcional.
  const noCookie = await playerVisionos(ctx, { cookie: null });
  const noSts = await playerVisionos(ctx, {
    body: {
      context: { client: { ...VISIONOS_CLIENT }, playbackContext: playbackContext(ctx, { sts: false }) },
      videoId: VIDEO_ID,
      contentCheckOk: true,
      racyCheckOk: true,
    },
  });
  for (const [label, variant] of [
    ['sin-cookie', noCookie],
    ['sin-signatureTimestamp', noSts],
  ]) {
    log({
      probe: 'player-visionos',
      step: `variant:${label}`,
      method: 'POST',
      url: `${YT}/youtubei/v1/player?prettyPrint=false`,
      clientProfile: 'VISIONOS',
      status: variant.step.status,
      playabilityStatus: variant.json.playabilityStatus?.status ?? null,
      adaptiveFormats: (variant.json.streamingData?.adaptiveFormats ?? []).length,
      audioItags: (variant.json.streamingData?.adaptiveFormats ?? []).filter((f) => /^audio\//.test(f.mimeType ?? '')).map((f) => f.itag),
    });
  }

  // Rangos sobre las URLs de VISIONOS: no llevan cap de ventana y aceptan
  // `bytes=0-` abierto. Se mide sobre audio 140 y el mejor vídeo disponible.
  const audio140 = (streaming.adaptiveFormats ?? []).find((f) => f.itag === 140) ?? (streaming.adaptiveFormats ?? []).find((f) => /^audio\//.test(f.mimeType ?? ''));
  const videoBest = [...(streaming.adaptiveFormats ?? [])].filter((f) => /^video\//.test(f.mimeType ?? '')).sort((a, b) => (b.bitrate ?? 0) - (a.bitrate ?? 0))[0] ?? null;
  const ranges = [];
  if (audio140?.url) {
    const clen = Number(audio140.contentLength);
    const cases = [
      { range: null, note: 'sin Range' },
      { range: 'bytes=0-1023', note: 'primer KiB' },
      { range: 'bytes=0-', note: 'abierto hasta el final' },
      { range: `bytes=${Math.floor(clen / 2)}-`, note: 'desde la mitad' },
      { range: `bytes=${clen - 49447}-${clen - 1}`, note: 'últimos 49.447 B' },
    ];
    for (const item of cases) {
      const result = await rangeGet(audio140.url, { range: item.range });
      ranges.push({ itag: audio140.itag, contentLength: clen, ...item, ...result });
    }
  }
  if (videoBest?.url) {
    for (const item of [{ range: null, note: 'sin Range' }, { range: 'bytes=0-', note: 'abierto hasta el final' }, { range: 'bytes=0-1023', note: 'primer KiB' }]) {
      const result = await rangeGet(videoBest.url, { range: item.range });
      ranges.push({ itag: videoBest.itag, contentLength: Number(videoBest.contentLength), ...item, ...result });
    }
  }
  log({ probe: 'player-visionos', step: 'range-checks', clientProfile: 'VISIONOS', ranges });

  const fixture = writeFixture('player_visionos.json', playerFixturePayload(json));
  log({ probe: 'player-visionos', step: 'fixture', sanitizedUrls: true, ...fixture });
  summarize({
    probe: 'player-visionos',
    status: step.status,
    bytes: step.bytes,
    note: `playability ${json.playabilityStatus?.status} · audio ${audio.length} (${audio.map((f) => f.itag).join('/')}) · vídeo ${video.length} · sin Range en itag 140: ${ranges.find((r) => r.itag === audio140?.itag && r.range === null)?.status ?? 'n/d'}`,
  });
}

// ---------------------------------------------------------------------------
// 8. ANDROID_VR: ventana de bytes servida (cap)
// ---------------------------------------------------------------------------

async function probePlayerVr(ctx) {
  const body = {
    context: { client: { ...VR_CLIENT }, playbackContext: playbackContext(ctx) },
    videoId: VIDEO_ID,
    contentCheckOk: true,
    racyCheckOk: true,
  };
  const step = await postInnerTube(`${YT}/youtubei/v1/player`, {
    clientName: 28,
    clientVersion: VR_CLIENT.clientVersion,
    ua: VR_UA,
    origin: YT,
    visitorData: ctx.visitorData,
    cookie: ctx.cookieHeader,
    body,
  });
  const json = parseJson(step) ?? {};
  const streaming = json.streamingData ?? {};
  const muxed = streaming.formats ?? [];
  const adaptive = streaming.adaptiveFormats ?? [];
  const audio = adaptive.filter((f) => /^audio\//.test(f.mimeType ?? ''));
  log({
    probe: 'player-vr',
    step: 'primary',
    method: 'POST',
    url: `${YT}/youtubei/v1/player?prettyPrint=false`,
    clientProfile: 'ANDROID_VR',
    headersSent: {
      'x-youtube-client-name': '28',
      'x-youtube-client-version': VR_CLIENT.clientVersion,
      'x-goog-visitor-id': 'presente',
      cookie: 'presente',
      'user-agent': VR_UA,
    },
    requestBody: body,
    status: step.status,
    ms: step.ms,
    bytes: step.bytes,
    playabilityStatus: json.playabilityStatus?.status ?? null,
    counts: { muxed: muxed.length, adaptive: adaptive.length, audio: audio.length },
    muxedItags: muxed.map((f) => ({ itag: f.itag, mimeType: f.mimeType, contentLength: f.contentLength ? Number(f.contentLength) : null })),
    audioItags: audio.map((f) => ({ itag: f.itag, mimeType: f.mimeType, bitrate: f.bitrate, contentLength: Number(f.contentLength) })),
    errors: json.error ?? null,
  });

  // itag 18 (muxed): el cliente lo anuncia pero la URL siempre responde 403.
  const muxed18 = muxed.find((f) => f.itag === 18) ?? muxed[0];
  const itag18 = muxed18
    ? [
        { range: null, ...(await rangeGet(muxed18.url, { range: null, ua: VR_UA })) },
        { range: 'bytes=0-1023', ...(await rangeGet(muxed18.url, { range: 'bytes=0-1023', ua: VR_UA })) },
      ]
    : [];
  log({ probe: 'player-vr', step: 'itag18-muxed', itag: muxed18?.itag ?? null, contentLength: muxed18?.contentLength ?? null, attempts: itag18 });

  // Recorrido secuencial por bloques de 64 KiB: mide cuántos bytes sirve una URL
  // antes del primer 403 (la "ventana" del cliente VR).
  const walkWindow = async (format, chunk = 65536) => {
    const clen = Number(format.contentLength);
    let offset = 0;
    const steps = [];
    for (let i = 0; i < 200; i++) {
      const result = await rangeGet(format.url, { range: `bytes=${offset}-${offset + chunk - 1}`, ua: VR_UA, maxBytes: chunk });
      steps.push({ offset, status: result.status, bytesRead: result.bytesRead });
      if (result.status !== 206) {
        return { itag: format.itag, chunk, contentLength: clen, bytesServed: offset, failedAtOffset: offset, status: result.status, steps: steps.slice(-4) };
      }
      offset += result.bytesRead;
      if (offset >= clen) return { itag: format.itag, chunk, contentLength: clen, bytesServed: offset, completed: true };
    }
    return { itag: format.itag, chunk, contentLength: clen, bytesServed: offset, completed: 'sin 403 en 200 bloques' };
  };
  const walks = [];
  for (const itag of [140, 251, 139]) {
    // Cada medición usa una URL recién emitida: el presupuesto concedido se
    // reduce al repetir sondeos sobre la misma visita.
    const fresh = await postInnerTube(`${YT}/youtubei/v1/player`, {
      clientName: 28,
      clientVersion: VR_CLIENT.clientVersion,
      ua: VR_UA,
      origin: YT,
      visitorData: ctx.visitorData,
      cookie: ctx.cookieHeader,
      body,
    });
    const freshJson = parseJson(fresh) ?? {};
    const freshFormat = (freshJson.streamingData?.adaptiveFormats ?? []).find((f) => f.itag === itag);
    walks.push(freshFormat?.url ? await walkWindow(freshFormat) : { itag, error: 'sin url en la respuesta fresca' });
  }

  // Rangos abiertos y rangos grandes sobre la misma URL: siempre 403.
  const audio140 = adaptive.find((f) => f.itag === 140) ?? audio[0];
  const openRanges = [];
  if (audio140?.url) {
    for (const range of [null, 'bytes=0-', 'bytes=0-2097151', 'bytes=0-1048575', 'bytes=1048576-2097151']) {
      openRanges.push({ range, ...(await rangeGet(audio140.url, { range, ua: VR_UA })) });
    }
  }
  log({ probe: 'player-vr', step: 'window-walk', clientProfile: 'ANDROID_VR', chunk: 65536, walks, openRanges });
  summarize({
    probe: 'player-vr',
    status: step.status,
    bytes: step.bytes,
    note: `muxed:${muxed.length} audio:${audio.length} · ventana 140=${walks.find((w) => w.itag === 140)?.bytesServed ?? 'n/d'} B 251=${walks.find((w) => w.itag === 251)?.bytesServed ?? 'n/d'} B 139=${walks.find((w) => w.itag === 139)?.bytesServed ?? 'n/d'} B · bytes=0- → ${openRanges.find((r) => r.range === 'bytes=0-')?.status ?? 'n/d'}`,
  });
}

// ---------------------------------------------------------------------------
// 9. Continuaciones: página 2 de search, relacionados y comentarios
// ---------------------------------------------------------------------------

async function probeContinuations(ctx) {
  const cases = [];

  // (a) Página 2 de search: se reutiliza el token del paso `search` o se pide.
  let searchToken = ctx.state.searchContinuation ?? null;
  if (!searchToken) {
    const seedBody = { context: webContext(ctx), query: 'lofi hip hop' };
    const seed = await ytPost(ctx, 'search', seedBody);
    const seedSectionList = getAt(parseJson(seed) ?? {}, SEARCH_SECTION_LIST_PATH) ?? [];
    searchToken = continuationTokenAt(seedSectionList)?.token ?? null;
  }
  if (searchToken) {
    const body = { context: webContext(ctx), continuation: searchToken };
    const step = await ytPost(ctx, 'search', body);
    const json = parseJson(step) ?? {};
    const commandsPath = findKeyPaths(json, 'onResponseReceivedCommands', { limit: 1 })[0] ?? null;
    const itemsPath = findKeyPaths(json, 'continuationItems', { limit: 1 })[0] ?? null;
    const items = itemsPath ? getAt(json, itemsPath) ?? [] : [];
    const nextToken = continuationTokenAt(items);
    cases.push({
      name: 'search-pagina-2',
      method: 'POST',
      url: `${YT}/youtubei/v1/search?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { context: 'WEB', continuation: `${searchToken.slice(0, 24)}…` },
      status: step.status,
      ms: step.ms,
      bytes: step.bytes,
      paths: { onResponseReceivedCommands: commandsPath, continuationItems: itemsPath, appendContinuationItemsAction: findKeyPaths(json, 'appendContinuationItemsAction', { limit: 1 })[0] ?? null },
      itemCount: items.length,
      itemKinds: kindsOf(items),
      nextTokenPath: nextToken ? `${itemsPath}[${nextToken.index}].continuationItemRenderer.continuationEndpoint.continuationCommand.token` : null,
      nextTokenLength: nextToken?.token.length ?? 0,
      errors: json.error ?? null,
    });
    ctx.state.searchContinuation2 = nextToken?.token ?? null;
  } else {
    cases.push({ name: 'search-pagina-2', status: 'sin token' });
  }

  // (b) Relacionados del vídeo (next → secondaryResults): token propio.
  if (ctx.state.nextContinuation) {
    const body = { context: webContext(ctx), continuation: ctx.state.nextContinuation };
    const step = await ytPost(ctx, 'next', body);
    const json = parseJson(step) ?? {};
    const itemsPath = findKeyPaths(json, 'continuationItems', { limit: 1 })[0] ?? null;
    const items = itemsPath ? getAt(json, itemsPath) ?? [] : [];
    const lockups = collectByKey(json, 'lockupViewModel', { limit: 10 });
    cases.push({
      name: 'next-relacionados-pagina-2',
      method: 'POST',
      url: `${YT}/youtubei/v1/next?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { context: 'WEB', continuation: 'token de secondaryResults' },
      status: step.status,
      ms: step.ms,
      bytes: step.bytes,
      paths: { continuationItems: itemsPath, onResponseReceivedEndpoints: findKeyPaths(json, 'onResponseReceivedEndpoints', { limit: 1 })[0] ?? null },
      itemCount: items.length,
      itemKinds: kindsOf(items),
      lockups: lockups.slice(0, 3).map((l) => lockupPreview(l)),
      errors: json.error ?? null,
    });
  } else {
    cases.push({ name: 'next-relacionados-pagina-2', status: 'sin token' });
  }

  // (c) Comentarios: el token sale del panel de comentarios de `next`.
  if (ctx.state.commentsContinuation) {
    const body = { context: webContext(ctx), continuation: ctx.state.commentsContinuation };
    const step = await ytPost(ctx, 'next', body);
    const json = parseJson(step) ?? {};
    const itemsPath = findKeyPaths(json, 'continuationItems', { limit: 1 })[0] ?? null;
    const items = itemsPath ? getAt(json, itemsPath) ?? [] : [];
    cases.push({
      name: 'next-comentarios-pagina-1',
      method: 'POST',
      url: `${YT}/youtubei/v1/next?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { context: 'WEB', continuation: 'token de comentarios' },
      status: step.status,
      ms: step.ms,
      bytes: step.bytes,
      paths: { continuationItems: itemsPath, commentThreadRenderer: findKeyPaths(json, 'commentThreadRenderer', { limit: 1 })[0] ?? null, commentViewModel: findKeyPaths(json, 'commentViewModel', { limit: 1 })[0] ?? null },
      itemCount: items.length,
      itemKinds: kindsOf(items),
      firstComment: (() => {
        const thread = collectByKey(json, 'commentThreadRenderer', { limit: 1 })[0];
        const comment = thread?.comment?.commentViewModel;
        return comment ? { commentId: comment.commentId ?? null, text: (comment.contentText?.content ?? comment.contentText?.runs?.map((r) => r.text).join('') ?? '').slice(0, 80) } : null;
      })(),
      errors: json.error ?? null,
    });
  } else {
    cases.push({ name: 'next-comentarios-pagina-1', status: 'sin token' });
  }

  // (d) Continuación de la rejilla de inicio (browse).
  if (ctx.state.homeContinuation) {
    const body = { context: webContext(ctx), continuation: ctx.state.homeContinuation };
    const step = await ytPost(ctx, 'browse', body);
    const json = parseJson(step) ?? {};
    const itemsPath = findKeyPaths(json, 'continuationItems', { limit: 1 })[0] ?? null;
    const items = itemsPath ? getAt(json, itemsPath) ?? [] : [];
    cases.push({
      name: 'home-pagina-2',
      method: 'POST',
      url: `${YT}/youtubei/v1/browse?prettyPrint=false`,
      clientProfile: 'WEB',
      requestBody: { context: 'WEB', continuation: 'token de richGridRenderer' },
      status: step.status,
      ms: step.ms,
      bytes: step.bytes,
      paths: { continuationItems: itemsPath, appendContinuationItemsAction: findKeyPaths(json, 'appendContinuationItemsAction', { limit: 1 })[0] ?? null },
      itemCount: items.length,
      itemKinds: kindsOf(items),
      errors: json.error ?? null,
    });
  } else {
    cases.push({ name: 'home-pagina-2', status: 'sin token' });
  }

  for (const item of cases) {
    log({ probe: 'continuations', step: item.name, ...item });
  }
  summarize({
    probe: 'continuations',
    status: cases.find((c) => c.name === 'search-pagina-2')?.status ?? 'n/d',
    bytes: cases.find((c) => c.name === 'search-pagina-2')?.bytes ?? 0,
    note: cases.map((c) => `${c.name}:${typeof c.status === 'number' ? c.status : c.status}`).join(' '),
  });
}

// ---------------------------------------------------------------------------
// Orquestación
// ---------------------------------------------------------------------------

const PROBES = {
  search: probeSearch,
  home: probeHome,
  'watch-next': probeWatchNext,
  'music-home': probeMusicHome,
  'music-search': probeMusicSearch,
  shorts: probeShorts,
  'player-visionos': probePlayerVisionos,
  'player-vr': probePlayerVr,
  continuations: probeContinuations,
};

function printSummary(failures, started) {
  const lines = SUMMARY.map((item) => {
    const status = typeof item.status === 'number' ? item.status : item.status ?? 'n/d';
    const size = item.bytes ? `${(item.bytes / 1024).toFixed(0).padStart(4)} KB` : '     -';
    return `  ${item.probe.padEnd(17)} ${String(status).padStart(3)}  ${size}  ${item.note ?? ''}`;
  });
  process.stdout.write(
    [
      '',
      `== Resumen (${started} → ${new Date().toISOString()}) ==`,
      ...lines,
      failures.length ? `  FALLOS: ${failures.map((f) => `${f.probe}: ${f.error}`).join(' | ')}` : '  FALLOS: ninguno',
      '',
    ].join('\n'),
  );
}

async function main() {
  const name = process.argv[2];
  if (!name || (name !== 'all' && !PROBES[name])) {
    console.error(`uso: node tools/probe.mjs <${Object.keys(PROBES).join('|')}|all>`);
    process.exit(2);
  }
  const started = new Date().toISOString();
  const ctx = { ...(await bootstrap()), state: {} };
  if (name === 'all' || Object.keys(PROBES).includes(name)) {
    // WEB_REMIX necesita su propia clientVersion; se detecta una sola vez.
    ctx.remixClientVersion = await detectRemixClientVersion(ctx);
  }
  log({
    probe: 'bootstrap',
    step: 'context',
    started,
    webClientVersion: ctx.webClientVersion,
    remixClientVersion: ctx.remixClientVersion,
    signatureTimestamp: ctx.signatureTimestamp,
  });

  const queue = name === 'all' ? Object.keys(PROBES) : [name];
  const failures = [];
  for (const probeName of queue) {
    const t0 = Date.now();
    try {
      await PROBES[probeName](ctx);
      log({ probe: probeName, step: 'ok', ms: Date.now() - t0 });
    } catch (err) {
      failures.push({ probe: probeName, error: err.message });
      log({ probe: probeName, step: 'failed', ms: Date.now() - t0, error: err.message });
    }
  }
  log({ probe: 'summary', finished: new Date().toISOString(), probesRun: queue.length, failures, logLines: LOG.length });
  printSummary(failures, started);
  if (failures.length) process.exitCode = 1;
}

const isEntryPoint = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isEntryPoint) {
  main().catch((err) => {
    log({ probe: 'fatal', error: err.message, stack: err.stack?.split('\n').slice(0, 6) });
    process.exit(1);
  });
}

export {
  bootstrap,
  fetchText,
  postInnerTube,
  rangeGet,
  findKeyPaths,
  getAt,
  collectByKey,
  countRenderers,
  textOf,
  trimResponse,
  writeFixture,
  playerFixturePayload,
  webContext,
  remixContext,
  playbackContext,
  VISIONOS_UA,
  VR_UA,
  VIDEO_ID,
  YT,
  YTM,
  WATCH_URL,
};
