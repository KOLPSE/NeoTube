import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

import 'yt_auth.dart';
import 'yt_cache.dart';
import 'yt_models.dart';

class YtApiException implements Exception {
  final String message;
  YtApiException(this.message);
  @override
  String toString() => message;
}

const int _umbralIsolate = 192 * 1024;

List<YtSection> _decodeAndParseBuscar(Uint8List bytesGeneral, [Uint8List? bytesSongs]) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytesGeneral);
  dynamic jSongs;
  if (bytesSongs != null && bytesSongs.isNotEmpty) {
    try {
      jSongs = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytesSongs);
    } catch (_) {}
  }
  return YtMusicApi._buscarDeRespuestaStatic(j, jSongs: jSongs);
}

(List<YtSection>, String?) _decodeAndParseSeccionesConToken(Uint8List bytes) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
  final secciones = YtMusicApi._seccionesDeRespuestaStatic(j);
  final token = YtMusicApi._extraerTokenContinuacionDeBrowse(j);
  return (secciones, token);
}

(List<YtSection>, String?) _decodeAndParseSeccionesContinuation(Uint8List bytes) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
  return YtMusicApi._seccionesContinuationDeRespuestaStatic(j);
}

(YtColeccion, String?) _decodeAndParseColeccion(Uint8List bytes) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
  return YtMusicApi._coleccionDeRespuestaStatic(j);
}

(List<YtTrack>, String?) _decodeAndParseContinuacion(Uint8List bytes) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
  return YtMusicApi._continuacionDeRespuestaStatic(j);
}

YtColeccion _decodeAndParseCola(Uint8List bytes) {
  final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
  return YtMusicApi._colaDeRespuestaStatic(j);
}

/// Cliente de la API interna de YouTube Music (`youtubei/v1`).
///
/// No hay documentación oficial de esta API para terceros: la forma de las
/// peticiones y de las respuestas está sacada de proyectos abiertos que la
/// usan igual (`ytmusicapi`, InnerTune, el extractor de `yt-dlp`).
///
/// ## Las tres trampas que costaron las playlists
///
/// 1. **`gridRenderer` guarda sus elementos en `items`, no en `contents`.**
///    Toda la biblioteca (`FEmusic_liked_playlists`) llega dentro de un
///    `gridRenderer`, así que un parseo que solo mirase `contents` veía cero
///    elementos y la pantalla salía vacía aunque la respuesta viniera llena.
///    Es *el* motivo de que "no aparecen mis playlists".
/// 2. **Una lista no se reproduce por `videoId`.** Las tarjetas de listas y
///    álbumes no traen vídeo: traen un `browseId` (`VL…` para listas,
///    `MPREb_…` para álbumes) con el que hay que pedir sus pistas *después*.
///    Aplanarlo todo a "pista sin videoId" es lo que dejaba solo canciones
///    sueltas reproducibles.
/// 3. **Las mezclas y radios de la portada no se pueden pedir por `browse`.**
///    Sus ids empiezan por `RD` y solo responden por el endpoint `next`, que
///    es el que usa el propio reproductor web para llenar su cola.
///
/// El parseo es deliberadamente defensivo: una sección con forma inesperada
/// se descarta, no tira la pantalla entera. Google cambia este JSON sin
/// avisar a nadie.
class YtMusicApi {
  YtMusicApi(this.auth, {YtCache? cache}) : cache = cache ?? YtCache();

  final YtAuth auth;
  final YtCache cache;
  final http.Client _http = http.Client();
  String? _visitorData;

  static const _base = 'music.youtube.com';

  /// `browseId` de cada pestaña de la biblioteca. Los mismos que usa
  /// `ytmusicapi`. La biblioteca **no es un solo `browseId`**: las playlists,
  /// los álbumes y las canciones guardadas viven en tres sitios distintos, y
  /// pedir solo el primero era otra razón por la que faltaba media pantalla.
  static const browseIdInicio = 'FEmusic_home';
  static const browseIdExplorar = 'FEmusic_explore';
  static const browseIdListas = 'FEmusic_liked_playlists';
  static const browseIdAlbumes = 'FEmusic_liked_albums';
  static const browseIdCanciones = 'FEmusic_liked_videos';
  static const browseIdArtistas = 'FEmusic_library_corpus_track_artists';

  /// Idioma y país de la sesión. Se sacan del sistema para que los títulos de
  /// las secciones ("Escuchado recientemente", "Mezclas para ti") lleguen ya
  /// traducidos desde Google en vez de en inglés: son texto que pintamos tal
  /// cual, no hay nada que traducir por nuestra cuenta.
  /// Públicos porque `yt_stream.dart` pide el `player` con el mismo idioma:
  /// así el `reason` de una pista que no se puede reproducir llega traducido y
  /// se le puede enseñar al usuario tal cual, igual que los títulos de las
  /// secciones.
  static final String hl = () {
    final l = Platform.localeName; // "es_ES.UTF-8", "en-US"…
    final corte = l.indexOf(RegExp(r'[_\-.]'));
    return corte > 0 ? l.substring(0, corte) : 'en';
  }();

  static final String gl = () {
    final m = RegExp(r'^[a-zA-Z]{2}[_\-]([A-Za-z]{2})').firstMatch(Platform.localeName);
    return m == null ? 'US' : m.group(1)!.toUpperCase();
  }();

  Map<String, dynamic> _contexto() => {
        'context': {
          'client': {
            'clientName': 'WEB_REMIX',
            'clientVersion': '1.20241201.01.00',
            'hl': hl,
            'gl': gl,
            if (_visitorData != null) 'visitorData': _visitorData,
          },
          // Sin esto, una cuenta con el modo restringido activado en el
          // navegador recibe una biblioteca recortada sin decir por qué.
          'user': {'lockedSafetyMode': false},
        },
      };

  void _guardarVisitorData(Uint8List bytes) {
    try {
      final extracto = utf8.decode(bytes.take(2048).toList(), allowMalformed: true);
      final m = RegExp(r'"visitorData"\s*:\s*"([^"]+)"').firstMatch(extracto);
      if (m != null) {
        _visitorData = m.group(1);
      }
    } catch (_) {}
  }

  Future<Uint8List> _postBytes(
    String endpoint,
    Map<String, dynamic> body, {
    Map<String, String>? query,
  }) async {
    if (!auth.isLoggedIn) throw YtApiException('No hay sesión de NeoTube iniciada.');
    // Sin parámetro `key`. Autenticando por cookies + SAPISIDHASH, la API no
    // lo pide: comprobado contra la cuenta real con tool/probe_yt.dart, que
    // devuelve exactamente lo mismo con y sin el. Se quita porque era la clave
    // pública del cliente web de YouTube Music -no un secreto nuestro, la
    // misma que reparte music.youtube.com en su propio JavaScript- pero el
    // escaneo de secretos de GitHub la marca igual, y aquí no hay nada que
    // rotar: no es nuestra. La forma de no volver a verla es no llevarla.
    final uri = Uri.https(_base, '/youtubei/v1/$endpoint', query);
    final res = await _http
        .post(
          uri,
          headers: {
            'Content-Type': 'application/json',
            'Cookie': auth.cookieHeader(),
            'Authorization': auth.authorizationHeader(),
            'X-Goog-AuthUser': '0',
            'X-Origin': 'https://$_base',
            'Origin': 'https://$_base',
          },
          body: jsonEncode({..._contexto(), ...body}),
        )
        .timeout(const Duration(seconds: 20));
    if (res.statusCode != 200) {
      final extracto = utf8
          .decode(res.bodyBytes.take(512).toList(), allowMalformed: true)
          .replaceAll(RegExp(r'\s+'), ' ')
          .trim();
      throw YtApiException(
          'YouTube Music devolvió ${res.statusCode}: ${extracto.substring(0, extracto.length.clamp(0, 200))}');
    }
    _guardarVisitorData(res.bodyBytes);
    return res.bodyBytes;
  }

  // ------------------------------------------------------------------ búsqueda

  static const _paramsCanciones = 'EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D';

  /// Busca y devuelve los resultados **por categorías** (canciones, listas,
  /// álbumes, artistas), combinando la búsqueda filtrada por canciones con la
  /// vista general por categorías para ofrecer un listado amplio de canciones.
  Future<List<YtSection>> buscar(String query, {bool forzar = false}) async {
    final queryLimpia = query.trim();
    if (queryLimpia.isEmpty) return const [];
    final clave = 'buscar:${queryLimpia.toLowerCase()}';
    if (!forzar) {
      final cached = cache.get<List<YtSection>>(clave);
      if (cached != null) return cached;
    }

    final respuestas = await Future.wait([
      _postBytes('search', {'query': queryLimpia}),
      _buscarCancionesBytes(queryLimpia).catchError((_) => Uint8List(0)),
    ]);

    final bytesGeneral = respuestas[0];
    final bytesSongs = respuestas[1];

    final secciones = (bytesGeneral.length + bytesSongs.length) >= _umbralIsolate
        ? await Isolate.run(() => _decodeAndParseBuscar(bytesGeneral, bytesSongs))
        : _decodeAndParseBuscar(bytesGeneral, bytesSongs);

    if (secciones.isNotEmpty) cache.set(clave, secciones);
    return secciones;
  }

  Future<Uint8List> _buscarCancionesBytes(String query) async {
    final bytes = await _postBytes('search', {
      'query': query,
      'params': _paramsCanciones,
    });
    try {
      final dynamic j = const Utf8Decoder().fuse(const JsonDecoder()).convert(bytes);
      final token = _extraerTokenContinuacionDeSearch(j);
      if (token != null) {
        final contBytes = await _postBytes('search', {'continuation': token});
        final dynamic jCont = const Utf8Decoder().fuse(const JsonDecoder()).convert(contBytes);
        final fusionado = _fusionarContinuacionDeSearch(j, jCont);
        return Uint8List.fromList(utf8.encode(jsonEncode(fusionado)));
      }
    } catch (_) {}
    return bytes;
  }

  // ------------------------------------------------------- portada / explorar

  /// Portada (`FEmusic_home`), explorar (`FEmusic_explore`) o cualquier
  /// pestaña de biblioteca: el mismo endpoint sirve las tres cambiando solo el
  /// `browseId`. Sigue la cadena de continuaciones progresivamente para no
  /// cortar las secciones personalizadas.
  Future<List<YtSection>> browseSections(String browseId, {bool forzar = false}) async {
    final clave = 'browse:$browseId';
    if (!forzar) {
      final cached = cache.get<List<YtSection>>(clave);
      if (cached != null) return cached;
    }
    final bytes = await _postBytes('browse', {'browseId': browseId});
    final (seccionesIniciales, primerToken) = bytes.length >= _umbralIsolate
        ? await Isolate.run(() => _decodeAndParseSeccionesConToken(bytes))
        : _decodeAndParseSeccionesConToken(bytes);

    final secciones = [...seccionesIniciales];
    var token = primerToken;
    var restantes = 20;

    while (token != null && restantes-- > 0) {
      Uint8List bytesCont;
      try {
        bytesCont = await _postBytes('browse', {'continuation': token});
      } catch (_) {
        bytesCont = await _postBytes('browse', const {},
            query: {'ctoken': token, 'continuation': token, 'type': 'next'});
      }
      final (nuevasSecciones, nuevoToken) = bytesCont.length >= _umbralIsolate
          ? await Isolate.run(() => _decodeAndParseSeccionesContinuation(bytesCont))
          : _decodeAndParseSeccionesContinuation(bytesCont);

      secciones.addAll(nuevasSecciones);
      token = nuevoToken;
    }

    if (secciones.isEmpty) {
      debugPrint('[NeoTube browse $browseId] respuesta sin secciones parseables');
    } else {
      cache.set(clave, secciones);
    }
    return secciones;
  }

  @visibleForTesting
  List<YtSection> seccionesDeRespuesta(dynamic j) => _seccionesDeRespuestaStatic(j);

  @visibleForTesting
  static String? extraerTokenContinuacionDeBrowse(dynamic j) =>
      _extraerTokenContinuacionDeBrowse(j);

  @visibleForTesting
  static (List<YtSection>, String?) seccionesContinuationDeRespuesta(dynamic j) =>
      _seccionesContinuationDeRespuestaStatic(j);

  @visibleForTesting
  List<YtSection> buscarDeRespuesta(dynamic j, {dynamic jSongs}) =>
      _buscarDeRespuestaStatic(j, jSongs: jSongs);

  @visibleForTesting
  static String? extraerTokenContinuacionDeSearch(dynamic j) =>
      _extraerTokenContinuacionDeSearch(j);

  static List<YtSection> _seccionesDeRespuestaStatic(dynamic j) {
    final secciones = <YtSection>[];
    for (final bloque in _bloquesDeSecciones(j)) {
      final s = _parseSeccion(bloque);
      if (s != null && s.items.isNotEmpty) secciones.add(s);
    }
    return secciones;
  }

  static (List<YtSection>, String?) _seccionesContinuationDeRespuestaStatic(dynamic j) {
    if (j is! Map) return (const <YtSection>[], null);
    final secciones = <YtSection>[];
    String? token;

    try {
      // 1. onResponseReceivedActions (formato moderno de continuaciones de browse)
      final acciones = j['onResponseReceivedActions'] as List?;
      if (acciones != null && acciones.isNotEmpty) {
        for (final accion in acciones) {
          if (accion is! Map) continue;
          final items =
              accion['appendContinuationItemsAction']?['continuationItems'] as List?;
          if (items != null) {
            for (final it in items) {
              final s = _parseSeccion(it);
              if (s != null && s.items.isNotEmpty) secciones.add(s);
            }
            token ??= _tokenDeContinuacion(const {}, items);
          }
        }
        if (secciones.isNotEmpty || token != null) {
          return (secciones, token);
        }
      }
    } catch (_) {}

    try {
      // 2. continuationContents (sectionListContinuation o gridContinuation)
      final contContents = j['continuationContents'] as Map?;
      if (contContents != null && contContents.isNotEmpty) {
        final slc = contContents['sectionListContinuation'] as Map?;
        if (slc != null) {
          final contents = slc['contents'] as List?;
          if (contents != null) {
            for (final bloque in contents) {
              final s = _parseSeccion(bloque);
              if (s != null && s.items.isNotEmpty) secciones.add(s);
            }
            token ??= _tokenDeContinuacion(slc, contents);
          }
          return (secciones, token);
        }

        final gc = contContents['gridContinuation'] as Map?;
        if (gc != null) {
          final items = gc['items'] as List?;
          if (items != null) {
            final parsedItems = <YtItem>[];
            for (final it in items) {
              final item = _parseItem(it);
              if (item != null && item.tieneDestino) parsedItems.add(item);
            }
            if (parsedItems.isNotEmpty) {
              secciones.add(YtSection(titulo: '', items: parsedItems));
            }
            token ??= _tokenDeContinuacion(gc, items);
          }
          return (secciones, token);
        }
      }
    } catch (_) {}

    try {
      // 3. Fallback: respuesta completa de browse si llega con contents
      if (j['contents'] != null) {
        secciones.addAll(_seccionesDeRespuestaStatic(j));
        token = _extraerTokenContinuacionDeBrowse(j);
        return (secciones, token);
      }
    } catch (_) {}

    return (secciones, null);
  }

  static String? _extraerTokenContinuacionDeBrowse(dynamic j) {
    if (j is! Map) return null;
    try {
      final tabs = (j['contents']?['singleColumnBrowseResultsRenderer']?['tabs'] ??
          j['contents']?['twoColumnBrowseResultsRenderer']?['tabs']) as List?;
      if (tabs != null) {
        for (final tab in tabs) {
          final content = tab['tabRenderer']?['content'] as Map?;
          if (content == null) continue;
          final slr = content['sectionListRenderer'] as Map?;
          if (slr != null) {
            final t = _tokenDeContinuacion(slr, (slr['contents'] as List?) ?? const []);
            if (t != null) return t;
          }
          final grid = content['gridRenderer'] as Map?;
          if (grid != null) {
            final t = _tokenDeContinuacion(grid, (grid['items'] as List?) ?? const []);
            if (t != null) return t;
          }
        }
      }
    } catch (_) {}
    try {
      final sec = j['contents']?['twoColumnBrowseResultsRenderer']?['secondaryContents']
          ?['sectionListRenderer'] as Map?;
      if (sec != null) {
        final t = _tokenDeContinuacion(sec, (sec['contents'] as List?) ?? const []);
        if (t != null) return t;
      }
    } catch (_) {}
    return null;
  }

  static String? _extraerTokenContinuacionDeSearch(dynamic j) {
    if (j is! Map) return null;
    try {
      final tabs = (j['contents']?['tabbedSearchResultsRenderer']?['tabs'] ??
          j['contents']?['singleColumnBrowseResultsRenderer']?['tabs']) as List?;
      if (tabs == null || tabs.isEmpty) return null;
      final slr = tabs.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
      if (slr == null) return null;
      final contents = slr['contents'] as List? ?? const [];
      for (final b in contents) {
        if (b is! Map) continue;
        final shelf = b['musicShelfRenderer'] as Map?;
        if (shelf != null) {
          final t = _tokenDeContinuacion(shelf, (shelf['contents'] as List?) ?? const []);
          if (t != null) return t;
        }
      }
    } catch (_) {}
    return null;
  }

  static dynamic _fusionarContinuacionDeSearch(dynamic j, dynamic jCont) {
    try {
      if (j is! Map || jCont is! Map) return j;
      List? masItems;
      final msc = jCont['continuationContents']?['musicShelfContinuation'] as Map?;
      if (msc != null && msc['contents'] is List) {
        masItems = msc['contents'] as List;
      }
      if (masItems == null) {
        final acciones = jCont['onResponseReceivedActions'] as List?;
        if (acciones != null && acciones.isNotEmpty) {
          masItems = acciones.first['appendContinuationItemsAction']?['continuationItems'] as List?;
        }
      }
      if (masItems != null && masItems.isNotEmpty) {
        final tabs = (j['contents']?['tabbedSearchResultsRenderer']?['tabs'] ??
            j['contents']?['singleColumnBrowseResultsRenderer']?['tabs']) as List?;
        final slr = tabs?.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
        final contents = slr?['contents'] as List?;
        if (contents != null) {
          for (final b in contents) {
            if (b is Map && b.containsKey('musicShelfRenderer')) {
              final shelf = b['musicShelfRenderer'] as Map;
              final list = shelf['contents'] as List?;
              if (list != null) {
                list.addAll(masItems);
                break;
              }
            }
          }
        }
      }
    } catch (_) {}
    return j;
  }

  static List<YtSection> _buscarDeRespuestaStatic(dynamic j, {dynamic jSongs}) {
    final secciones = <YtSection>[];
    if (j is! Map && jSongs is! Map) return secciones;

    try {
      final idsVistos = <String>{};

      // 1. Parsear canciones de la búsqueda filtrada por canciones (si está disponible)
      List<YtItem> cancionesFiltradas = const [];
      if (jSongs is Map) {
        cancionesFiltradas = _extraerCancionesDeSearch(jSongs);
      }

      // 2. Parsear la respuesta general (Todo)
      final seccionesGeneral = <YtSection>[];
      if (j is Map) {
        final tabs = (j['contents']?['tabbedSearchResultsRenderer']?['tabs'] ??
            j['contents']?['singleColumnBrowseResultsRenderer']?['tabs']) as List? ??
            const [];
        final contenido = tabs.isNotEmpty
            ? (tabs.first['tabRenderer']?['content']?['sectionListRenderer']?['contents'] as List? ?? const [])
            : const [];

        final itemsSueltos = <YtItem>[];

        for (final bloque in contenido) {
          if (bloque is! Map) continue;
          final clave = bloque.keys.first;
          final valor = bloque[clave] as Map?;
          if (valor == null) continue;

          if (clave == 'musicCardShelfRenderer') {
            final card = _parseCardShelf(valor);
            if (card != null && card.items.isNotEmpty) {
              seccionesGeneral.add(card);
            }
          } else if (clave == 'musicShelfRenderer' ||
              clave == 'musicCarouselShelfRenderer' ||
              clave == 'gridRenderer') {
            final s = _parseSeccion(bloque);
            if (s != null && s.items.isNotEmpty) {
              seccionesGeneral.add(s);
            }
          } else if (clave == 'itemSectionRenderer') {
            final dentro = valor['contents'] as List? ?? const [];
            for (final it in dentro) {
              if (it is! Map) continue;
              final k = it.keys.first;
              if (k == 'musicShelfRenderer' ||
                  k == 'musicCarouselShelfRenderer' ||
                  k == 'gridRenderer' ||
                  k == 'musicCardShelfRenderer') {
                final s = _parseSeccion(it);
                if (s != null && s.items.isNotEmpty) seccionesGeneral.add(s);
              } else {
                final item = _parseItem(it);
                if (item != null && item.tieneDestino) itemsSueltos.add(item);
              }
            }
          }
        }

        // Si la respuesta venía con items sueltos en itemSectionRenderer (caso "lofi chill music"),
        // los agrupamos en secciones coherentes por tipo.
        if (itemsSueltos.isNotEmpty) {
          final porTipo = <YtTipo, List<YtItem>>{};
          for (final it in itemsSueltos) {
            porTipo.putIfAbsent(it.tipo, () => []).add(it);
          }
          for (final entry in porTipo.entries) {
            final titulo = switch (entry.key) {
              YtTipo.cancion => 'Otras canciones y vídeos',
              YtTipo.lista => 'Listas de reproducción',
              YtTipo.album => 'Álbumes',
              YtTipo.artista => 'Artistas',
              _ => 'Otros resultados',
            };
            seccionesGeneral.add(YtSection(titulo: titulo, items: entry.value));
          }
        }
      }

      // 3. Organizar y combinar secciones
      // Separamos "Resultado principal" para ponerlo primero
      final resultadoPrincipal = seccionesGeneral
          .where((s) =>
              s.titulo.toLowerCase().contains('principal') ||
              s.titulo.toLowerCase().contains('top'))
          .toList();
      final otrasSecciones = seccionesGeneral
          .where((s) => !resultadoPrincipal.contains(s))
          .toList();

      for (final rp in resultadoPrincipal) {
        if (rp.items.isNotEmpty) {
          secciones.add(rp);
          for (final it in rp.items) {
            if (it.browseId != null) idsVistos.add(it.browseId!);
          }
        }
      }

      // Añadimos sección de "Canciones"
      if (cancionesFiltradas.isNotEmpty) {
        final cancionesUnicas = <YtItem>[];
        for (final s in cancionesFiltradas) {
          final id = s.videoId;
          if (id != null) {
            if (!idsVistos.contains(id)) {
              idsVistos.add(id);
              cancionesUnicas.add(s);
            }
          } else {
            cancionesUnicas.add(s);
          }
        }
        if (cancionesUnicas.isNotEmpty) {
          secciones.add(YtSection(titulo: 'Canciones', items: cancionesUnicas));
        }
      }

      // Añadimos las demás secciones (Álbumes, Artistas, Listas, etc.), deduplicando
      for (final s in otrasSecciones) {
        final esSeccionCanciones =
            s.titulo.toLowerCase() == 'canciones' || s.titulo.toLowerCase() == 'songs';
        if (cancionesFiltradas.isNotEmpty && esSeccionCanciones) {
          continue;
        }

        final itemsFiltrados = <YtItem>[];
        for (final it in s.items) {
          final id = it.videoId ?? it.browseId ?? it.playlistId;
          if (id != null) {
            if (!idsVistos.contains(id)) {
              idsVistos.add(id);
              itemsFiltrados.add(it);
            }
          } else {
            itemsFiltrados.add(it);
          }
        }
        if (itemsFiltrados.isNotEmpty) {
          final titulo = esSeccionCanciones ? 'Canciones' : s.titulo;
          secciones.add(YtSection(titulo: titulo, items: itemsFiltrados));
        }
      }
    } catch (e) {
      debugPrint('[NeoTube buscar] no se pudo parsear: $e');
    }
    return secciones;
  }

  static List<YtItem> _extraerCancionesDeSearch(dynamic j) {
    final items = <YtItem>[];
    try {
      if (j is! Map) return items;
      final tabs = (j['contents']?['tabbedSearchResultsRenderer']?['tabs'] ??
          j['contents']?['singleColumnBrowseResultsRenderer']?['tabs']) as List? ??
          const [];
      if (tabs.isEmpty) return items;
      final slr = tabs.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
      final contents = slr?['contents'] as List? ?? const [];
      for (final b in contents) {
        if (b is! Map) continue;
        final shelf = b['musicShelfRenderer'] as Map?;
        if (shelf != null) {
          final list = shelf['contents'] as List? ?? const [];
          for (final it in list) {
            final parsed = _parseItem(it);
            if (parsed != null && parsed.tieneDestino) items.add(parsed);
          }
        }
      }
    } catch (_) {}
    return items;
  }

  static YtSection? _parseCardShelf(Map valor) {
    try {
      final headerTitle = _runsToText(
              valor['header']?['musicCardShelfHeaderBasicRenderer']?['title']?['runs']) ??
          'Resultado principal';
      final items = <YtItem>[];

      // 1. Tarjeta principal
      final cardTitle = _runsToText(valor['title']?['runs']) ?? '';
      if (cardTitle.isNotEmpty) {
        final cardSubtitle = _runsCompletos(valor['subtitle']?['runs']);
        final nav = valor['onTap'] ?? valor['title']?['runs']?[0]?['navigationEndpoint'];
        final videoId = nav?['watchEndpoint']?['videoId'] as String? ??
            valor['thumbnailOverlay']?['musicItemThumbnailOverlayRenderer']?['content']
                ?['musicPlayButtonRenderer']?['playNavigationEndpoint']?['watchEndpoint']
                ?['videoId'] as String?;
        final playlistId = (nav?['watchPlaylistEndpoint']?['playlistId'] ??
                _playlistDelOverlay(valor)) as String?;
        final browseId = nav?['browseEndpoint']?['browseId'] as String?;
        final miniatura = _urlDeMiniatura(valor['thumbnailRenderer']) ??
            _urlDeMiniatura(valor['thumbnail']);

        final itemPrincipal = YtItem(
          tipo: _tipoDe(videoId: videoId, playlistId: playlistId, browseId: browseId),
          titulo: cardTitle,
          subtitulo: cardSubtitle,
          videoId: videoId,
          browseId: browseId,
          playlistId: playlistId ??
              (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
          miniatura: miniatura,
        );
        if (itemPrincipal.tieneDestino) items.add(itemPrincipal);
      }

      // 2. Canciones / subitems incrustados en la tarjeta
      final subContents = valor['contents'] as List? ?? const [];
      for (final sub in subContents) {
        final p = _parseItem(sub);
        if (p != null && p.tieneDestino) items.add(p);
      }

      return items.isEmpty ? null : YtSection(titulo: headerTitle, items: items);
    } catch (_) {
      return null;
    }
  }

  /// Las pestañas de biblioteca de golpe, cada una como una sección con su
  /// título. Se piden en paralelo porque son cuatro viajes independientes y en
  /// serie se notaba la espera; una que falle no se lleva por delante a las
  /// otras tres.
  /// Las pestañas de biblioteca de golpe, cada una como una sección con su
  /// título, entregadas conforme llega cada una de las cuatro peticiones en
  /// vez de esperar a que acaben las cuatro: con una biblioteca de canciones
  /// favoritas grande, esa era la única que se notaba —su paginación es
  /// secuencial, página a página— y tenía bloqueada de gratis a playlists,
  /// álbumes y artistas, que suelen acabar mucho antes. Se piden en paralelo
  /// por lo mismo que antes: una que falle no se lleva por delante a las
  /// otras tres.
  Stream<List<YtSection>> bibliotecaProgresiva({bool forzar = false}) {
    const clave = 'biblioteca';
    if (!forzar) {
      final cached = cache.get<List<YtSection>>(clave);
      if (cached != null) return Stream.value(cached);
    }

    final peticiones = <String, String>{
      browseIdListas: 'Tus playlists',
      browseIdAlbumes: 'Tus álbumes',
      browseIdCanciones: 'Canciones que te gustan',
      browseIdArtistas: 'Tus artistas',
    };
    final claves = peticiones.keys.toList();
    final resultados = List<List<YtSection>?>.filled(claves.length, null);

    List<YtSection> combinar() {
      final secciones = <YtSection>[];
      for (var i = 0; i < claves.length; i++) {
        final parcial = resultados[i];
        if (parcial == null) continue;
        final titulo = peticiones[claves[i]]!;
        for (final s in parcial) {
          // La respuesta de biblioteca casi nunca trae título de sección
          // propio (es una rejilla pelada), así que se pone el de la
          // pestaña; si trae uno de verdad, manda el suyo.
          secciones.add(s.titulo.isEmpty ? YtSection(titulo: titulo, items: s.items) : s);
        }
      }
      return secciones;
    }

    final controlador = StreamController<List<YtSection>>();
    var restantes = claves.length;

    for (var i = 0; i < claves.length; i++) {
      final indice = i;
      browseSections(claves[indice], forzar: forzar).then((r) {
        resultados[indice] = r;
      }, onError: (Object e) {
        debugPrint('[NeoTube biblioteca ${claves[indice]}] fallo: $e');
        resultados[indice] = const <YtSection>[];
      }).whenComplete(() {
        restantes--;
        if (controlador.isClosed) return;
        final secciones = combinar();
        controlador.add(secciones);
        if (restantes == 0) {
          if (secciones.isNotEmpty) cache.set(clave, secciones);
          unawaited(controlador.close());
        }
      });
    }

    return controlador.stream;
  }

  // ------------------------------------------------------ listas y álbumes

  static String _claveColeccion({String? playlistId, String? browseId}) {
    if (browseId != null && browseId.startsWith('MPRE')) {
      return 'coleccion:$browseId';
    }
    final id = playlistId ?? browseId;
    if (id == null) return 'coleccion:';
    final limpio = id.startsWith('VL') ? id.substring(2) : id;
    return 'coleccion:$limpio';
  }

  /// Emite la cabecera y el primer bloque de pistas de inmediato y continúa
  /// emitiendo colecciones actualizadas conforme llegan más trozos de la lista.
  Stream<YtColeccion> coleccionProgresiva({
    String? playlistId,
    String? browseId,
    bool forzar = false,
  }) async* {
    final clave = _claveColeccion(playlistId: playlistId, browseId: browseId);
    if (!forzar) {
      final cached = cache.get<YtColeccion>(clave);
      if (cached != null) {
        yield cached;
        return;
      }
    }

    if (browseId != null && browseId.startsWith('MPRE')) {
      YtColeccion? ultima;
      await for (final c in _browseColeccionProgresiva(browseId)) {
        ultima = c;
        yield c;
      }
      if (ultima != null && ultima.pistas.isNotEmpty) {
        cache.set(clave, ultima);
        return;
      }
    }

    final id = playlistId;
    if (id == null) {
      throw YtApiException('No hay lista que abrir en este elemento.');
    }

    if (!id.startsWith('RD')) {
      final targetBrowseId = id.startsWith('VL') ? id : 'VL$id';
      YtColeccion? ultima;
      await for (final c in _browseColeccionProgresiva(targetBrowseId)) {
        ultima = c;
        yield c;
      }
      if (ultima != null && ultima.pistas.isNotEmpty) {
        cache.set(clave, ultima);
        return;
      }
    }

    final cola = await _colaDe(playlistId: id);
    if (cola.pistas.isNotEmpty) {
      cache.set(clave, cola);
    }
    yield cola;
  }

  Stream<YtColeccion> _browseColeccionProgresiva(String browseId) async* {
    final bytes = await _postBytes('browse', {'browseId': browseId});
    final (coleccion, continuacion) = bytes.length >= _umbralIsolate
        ? await Isolate.run(() => _decodeAndParseColeccion(bytes))
        : _decodeAndParseColeccion(bytes);

    final pistas = [...coleccion.pistas];
    var cActual = YtColeccion(
      titulo: coleccion.titulo,
      subtitulo: coleccion.subtitulo,
      miniatura: coleccion.miniatura,
      pistas: pistas,
    );
    yield cActual;

    var token = continuacion;
    var restantes = 20;
    while (token != null && restantes-- > 0) {
      token = await _continuacion(token, pistas);
      cActual = YtColeccion(
        titulo: coleccion.titulo,
        subtitulo: coleccion.subtitulo,
        miniatura: coleccion.miniatura,
        pistas: [...pistas],
      );
      yield cActual;
    }
  }

  /// Las pistas de una lista, un álbum o una mezcla, elija el llamador lo que
  /// elija de la portada.
  Future<YtColeccion> coleccion({
    String? playlistId,
    String? browseId,
    bool forzar = false,
  }) async {
    final clave = _claveColeccion(playlistId: playlistId, browseId: browseId);
    if (!forzar) {
      final cached = cache.get<YtColeccion>(clave);
      if (cached != null) return cached;
    }
    return await coleccionProgresiva(
      playlistId: playlistId,
      browseId: browseId,
      forzar: forzar,
    ).last;
  }

  /// El parseo de una lista/álbum ya descodificado, separado del viaje de red
  /// para poder probarlo. Devuelve además el token del siguiente trozo, si la
  /// lista no cabía entera en la respuesta.
  @visibleForTesting
  (YtColeccion, String?) coleccionDeRespuesta(dynamic j) => _coleccionDeRespuestaStatic(j);

  static (YtColeccion, String?) _coleccionDeRespuestaStatic(dynamic j) {
    final cabecera = _parseCabecera(j);
    final pistas = <YtTrack>[];
    String? continuacion;
    for (final bloque in _bloquesDeContenido(j)) {
      final estante = _valorDeRenderer(bloque);
      if (estante == null) continue;
      final lista = estante['contents'] ?? estante['items'];
      if (lista is! List) continue;
      _acumularPistas(lista, pistas);
      continuacion ??= _tokenDeContinuacion(estante, lista);
    }
    return (
      YtColeccion(
        titulo: cabecera.$1,
        subtitulo: cabecera.$2,
        miniatura: cabecera.$3,
        pistas: pistas,
      ),
      continuacion,
    );
  }

  /// Pide el siguiente trozo y devuelve el token del que venga después (o
  /// `null` si ya no hay más).
  Future<String?> _continuacion(String token, List<YtTrack> acumulador) async {
    Uint8List bytes;
    try {
      bytes = await _postBytes('browse', {'continuation': token});
    } catch (_) {
      bytes = await _postBytes('browse', const {},
          query: {'ctoken': token, 'continuation': token, 'type': 'next'});
    }
    final (nuevasPistas, nuevoToken) = bytes.length >= _umbralIsolate
        ? await Isolate.run(() => _decodeAndParseContinuacion(bytes))
        : _decodeAndParseContinuacion(bytes);
    acumulador.addAll(nuevasPistas);
    return nuevoToken;
  }

  static (List<YtTrack>, String?) _continuacionDeRespuestaStatic(dynamic j) {
    final acumulador = <YtTrack>[];
    try {
      final acciones = j['onResponseReceivedActions'] as List?;
      if (acciones != null && acciones.isNotEmpty) {
        final items =
            acciones.first['appendContinuationItemsAction']['continuationItems'] as List;
        _acumularPistas(items, acumulador);
        return (acumulador, _tokenDeContinuacion(const {}, items));
      }
    } catch (_) {}
    try {
      final cont = (j['continuationContents'] as Map).values.first as Map;
      final lista = cont['contents'] ?? cont['items'];
      if (lista is List) {
        _acumularPistas(lista, acumulador);
        return (acumulador, _tokenDeContinuacion(cont, lista));
      }
    } catch (_) {}
    return (acumulador, null);
  }

  static void _acumularPistas(List lista, List<YtTrack> destino) {
    for (final item in lista) {
      final it = _parseItem(item);
      final pista = it?.comoPista;
      if (pista != null) destino.add(pista);
    }
  }

  static String? _tokenDeContinuacion(Map estante, List lista) {
    try {
      final c = estante['continuations'];
      if (c is List && c.isNotEmpty) {
        final t = c.first['nextContinuationData']?['continuation'] as String?;
        if (t != null) return t;
      }
    } catch (_) {}
    try {
      final ultimo = lista.isEmpty ? null : lista.last;
      if (ultimo is Map && ultimo.containsKey('continuationItemRenderer')) {
        return ultimo['continuationItemRenderer']['continuationEndpoint']
            ['continuationCommand']['token'] as String?;
      }
    } catch (_) {}
    return null;
  }

  /// La cola del reproductor web (`next`): el único camino para las mezclas y
  /// radios, y el plan B para todo lo demás.
  Future<YtColeccion> _colaDe({String? playlistId, String? videoId}) async {
    final bytes = await _postBytes('next', {
      'playlistId': ?playlistId,
      'videoId': ?videoId,
      'isAudioOnly': true,
      'params': 'wAEB',
    });
    return bytes.length >= _umbralIsolate
        ? Isolate.run(() => _decodeAndParseCola(bytes))
        : _decodeAndParseCola(bytes);
  }

  static YtColeccion _colaDeRespuestaStatic(dynamic j) {
    final pistas = <YtTrack>[];
    String titulo = '';
    try {
      final panel = j['contents']['singleColumnMusicWatchNextResultsRenderer']
              ['tabbedRenderer']['watchNextTabbedResultsRenderer']['tabs'][0]['tabRenderer']
          ['content']['musicQueueRenderer']['content']['playlistPanelRenderer'] as Map;
      titulo = _runsToText(panel['title']?['runs']) ?? '';
      for (final item in (panel['contents'] as List)) {
        final p = _parsePanelVideo(item['playlistPanelVideoRenderer']);
        if (p != null) pistas.add(p);
      }
    } catch (e) {
      debugPrint('[NeoTube next] no se pudo parsear la cola: $e');
    }
    if (pistas.isEmpty) {
      throw YtApiException('YouTube Music no devolvió ninguna pista para esta lista.');
    }
    return YtColeccion(
      titulo: titulo.isEmpty ? 'Mezcla' : titulo,
      subtitulo: '${pistas.length} canciones',
      miniatura: pistas.first.miniatura,
      pistas: pistas,
    );
  }

  /// La radio infinita a partir de una canción: lo que hace el botón de
  /// "reproducir" del cliente oficial cuando le das a una canción suelta, y lo
  /// que evita que la reproducción se pare en seco al acabar esa única pista.
  Future<List<YtTrack>> radioDe(String videoId) async {
    try {
      final c = await _colaDe(videoId: videoId, playlistId: 'RDAMVM$videoId');
      return c.pistas;
    } catch (e) {
      debugPrint('[NeoTube radio] $e');
      return const [];
    }
  }

  // ------------------------------------------------------------------ likes

  /// Marca la canción como favorita en la cuenta.
  ///
  /// No hay endpoint propio de "añadir a favoritos": en YouTube Music, la
  /// lista **Me gusta** (`FEmusic_liked_videos`, la que ya se enseña en
  /// Biblioteca) no es una playlist que se edite pista a pista, sino el
  /// reflejo de los "me gusta" de la cuenta. Darle like a un vídeo lo mete
  /// ahí solo, que es justo lo que hace el botón del cliente oficial.
  Future<void> darLike(String videoId) =>
      _postBytes('like/like', {'target': {'videoId': videoId}});

  /// Deshace [darLike] y, con ello, saca la canción de la lista de favoritos.
  Future<void> quitarLike(String videoId) =>
      _postBytes('like/removelike', {'target': {'videoId': videoId}});

  // -------------------------------------------------------------- navegación

  static List _bloquesDeSecciones(dynamic j) {
    for (final ruta in [_rutaUnaColumna, _rutaDosColumnasPrimaria]) {
      final r = ruta(j);
      if (r != null && r.isNotEmpty) return r;
    }
    return const [];
  }

  static List _bloquesDeContenido(dynamic j) {
    final dos = _rutaDosColumnasSecundaria(j);
    if (dos != null && dos.isNotEmpty) return dos;
    return _bloquesDeSecciones(j);
  }

  static List? _rutaUnaColumna(dynamic j) {
    try {
      final tabs = j['contents']['singleColumnBrowseResultsRenderer']['tabs'] as List;
      for (final tab in tabs) {
        final c = tab['tabRenderer']?['content']?['sectionListRenderer']?['contents'];
        if (c is List && c.isNotEmpty) return c;
      }
    } catch (_) {}
    return null;
  }

  static List? _rutaDosColumnasPrimaria(dynamic j) {
    try {
      final tabs = j['contents']['twoColumnBrowseResultsRenderer']['tabs'] as List;
      for (final tab in tabs) {
        final c = tab['tabRenderer']?['content']?['sectionListRenderer']?['contents'];
        if (c is List && c.isNotEmpty) return c;
      }
    } catch (_) {}
    return null;
  }

  static List? _rutaDosColumnasSecundaria(dynamic j) {
    try {
      final c = j['contents']['twoColumnBrowseResultsRenderer']['secondaryContents']
          ['sectionListRenderer']['contents'];
      if (c is List) return c;
    } catch (_) {}
    return null;
  }

  static (String, String, String?) _parseCabecera(dynamic j) {
    for (final candidata in [
      () => _bloquesDeSecciones(j).firstOrNull,
      () => j['header'],
    ]) {
      try {
        var h = _valorDeRenderer(candidata());
        if (h == null) continue;
        if (h.containsKey('header') && h['header'] is Map) {
          h = _valorDeRenderer(h['header']) ?? h;
        }
        final titulo = _runsToText(h['title']?['runs']) ?? '';
        if (titulo.isEmpty) continue;
        final sub = [
          _runsCompletos(h['subtitle']?['runs']),
          _runsCompletos(h['secondSubtitle']?['runs']),
        ].where((s) => s.isNotEmpty).join(' • ');
        final mini = _urlDeMiniatura(h['thumbnail']) ?? _urlDeMiniatura(h['background']);
        return (titulo, sub, mini);
      } catch (_) {}
    }
    return ('', '', null);
  }

  // ------------------------------------------------------------------ parseo

  static Map? _valorDeRenderer(dynamic bloque) {
    if (bloque is! Map || bloque.isEmpty) return null;
    final v = bloque.values.first;
    return v is Map ? v : null;
  }

  static YtSection? _parseSeccion(dynamic bloque) {
    try {
      if (bloque is! Map || bloque.isEmpty) return null;
      final clave = bloque.keys.first;
      if (clave == 'itemSectionRenderer') {
        final dentro = (bloque.values.first as Map)['contents'] as List?;
        if (dentro == null || dentro.isEmpty) return null;
        return _parseSeccion(dentro.first);
      }
      final contenido = _valorDeRenderer(bloque);
      if (contenido == null) return null;

      final lista = contenido['contents'] ?? contenido['items'];
      if (lista is! List) return null;

      final titulo = _tituloDeHeader(contenido['header']) ??
          _runsToText(contenido['title']?['runs']) ??
          '';
      final items = <YtItem>[];
      for (final it in lista) {
        final parseado = _parseItem(it);
        if (parseado != null && parseado.tieneDestino) items.add(parseado);
      }
      return YtSection(titulo: titulo, items: items);
    } catch (_) {
      return null;
    }
  }

  static YtItem? _parseItem(dynamic item) {
    if (item is! Map) return null;
    if (item.containsKey('musicTwoRowItemRenderer')) {
      return _parseTwoRowItem(item['musicTwoRowItemRenderer']);
    }
    if (item.containsKey('musicResponsiveListItemRenderer')) {
      return _parseListItem(item['musicResponsiveListItemRenderer']);
    }
    if (item.containsKey('playlistPanelVideoRenderer')) {
      final p = _parsePanelVideo(item['playlistPanelVideoRenderer']);
      return p == null
          ? null
          : YtItem(
              tipo: YtTipo.cancion,
              titulo: p.titulo,
              subtitulo: p.artista,
              videoId: p.videoId,
              miniatura: p.miniatura,
              duracion: p.duracion,
            );
    }
    return null;
  }

  static String? _tituloDeHeader(dynamic header) {
    if (header == null) return null;
    try {
      final contenido = _valorDeRenderer(header);
      if (contenido == null) return null;
      return _runsToText(contenido['title']?['runs']) ??
          _runsToText(contenido['strapline']?['runs']);
    } catch (_) {
      return null;
    }
  }

  static String? _runsToText(dynamic runs) {
    if (runs is! List || runs.isEmpty) return null;
    return runs.first['text'] as String?;
  }

  static String _runsCompletos(dynamic runs) {
    if (runs is! List) return '';
    return runs.map((r) => (r is Map ? r['text'] : null) as String? ?? '').join();
  }

  static YtItem? _parseTwoRowItem(dynamic item) {
    if (item is! Map) return null;
    try {
      final nav = item['navigationEndpoint'];
      final videoId = nav?['watchEndpoint']?['videoId'] as String?;
      final playlistId = (nav?['watchPlaylistEndpoint']?['playlistId'] ??
              nav?['watchEndpoint']?['playlistId'] ??
              _playlistDelOverlay(item)) as String?;
      var browseId = nav?['browseEndpoint']?['browseId'] as String?;
      browseId ??= item['title']?['runs']?[0]?['navigationEndpoint']?['browseEndpoint']
          ?['browseId'] as String?;

      final titulo = _runsToText(item['title']?['runs']) ?? '';
      if (titulo.isEmpty) return null;
      final subtitulo = _runsCompletos(item['subtitle']?['runs']);

      return YtItem(
        tipo: _tipoDe(videoId: videoId, playlistId: playlistId, browseId: browseId),
        titulo: titulo,
        subtitulo: subtitulo,
        videoId: videoId,
        playlistId: playlistId ??
            (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
        browseId: browseId,
        miniatura: _urlDeMiniatura(item['thumbnailRenderer']),
      );
    } catch (_) {
      return null;
    }
  }

  static String? _playlistDelOverlay(Map item) {
    try {
      return item['thumbnailOverlay']['musicItemThumbnailOverlayRenderer']['content']
              ['musicPlayButtonRenderer']['playNavigationEndpoint']['watchPlaylistEndpoint']
          ['playlistId'] as String?;
    } catch (_) {
      return null;
    }
  }

  static YtItem? _parseListItem(dynamic item) {
    if (item is! Map) return null;
    try {
      final columnas = item['flexColumns'] as List;
      final titulo = _runsToText(
              columnas[0]['musicResponsiveListItemFlexColumnRenderer']['text']['runs']) ??
          '';
      if (titulo.isEmpty) return null;
      var subtitulo = '';
      if (columnas.length > 1) {
        subtitulo = _runsCompletos(
            columnas[1]['musicResponsiveListItemFlexColumnRenderer']['text']?['runs']);
      }

      final videoId = _extraerVideoId(item);
      final nav = item['navigationEndpoint'];
      var browseId = nav?['browseEndpoint']?['browseId'] as String?;
      final playlistId = (nav?['watchPlaylistEndpoint']?['playlistId'] ??
          _playlistDelOverlay(item)) as String?;

      if (browseId == null && videoId == null && playlistId == null) {
        final runs0 = columnas[0]['musicResponsiveListItemFlexColumnRenderer']['text']?['runs'] as List?;
        browseId = runs0?[0]?['navigationEndpoint']?['browseEndpoint']?['browseId'] as String?;
        if (browseId == null && columnas.length > 1) {
          final runs1 = columnas[1]['musicResponsiveListItemFlexColumnRenderer']['text']?['runs'] as List?;
          for (final r in runs1 ?? const []) {
            if (r is Map) {
              final b = r['navigationEndpoint']?['browseEndpoint']?['browseId'] as String?;
              if (b != null) {
                browseId = b;
                break;
              }
            }
          }
        }
      }

      return YtItem(
        tipo: _tipoDe(videoId: videoId, playlistId: playlistId, browseId: browseId),
        titulo: titulo,
        subtitulo: subtitulo,
        videoId: videoId,
        playlistId: playlistId ??
            (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
        browseId: browseId,
        miniatura: _urlDeMiniatura(item['thumbnail']),
        duracion: _duracionDeFixedColumn(item),
      );
    } catch (_) {
      return null;
    }
  }

  static YtTrack? _parsePanelVideo(dynamic item) {
    if (item is! Map) return null;
    try {
      final videoId = item['videoId'] as String?;
      if (videoId == null) return null;
      return YtTrack(
        videoId: videoId,
        titulo: _runsToText(item['title']?['runs']) ?? '',
        artista: _runsCompletos(item['longBylineText']?['runs'] ??
            item['shortBylineText']?['runs']),
        miniatura: _urlDeMiniatura(item['thumbnail']),
        duracion: _parseDuracion(item['lengthText']?['runs']?[0]?['text'] as String?),
      );
    } catch (_) {
      return null;
    }
  }

  static YtTipo _tipoDe({String? videoId, String? playlistId, String? browseId}) {
    if (videoId != null) return YtTipo.cancion;
    if (browseId != null && browseId.startsWith('MPRE')) return YtTipo.album;
    if (browseId != null &&
        (browseId.startsWith('UC') ||
            browseId.startsWith('FEmusic_library_privately_owned_artist'))) {
      return YtTipo.artista;
    }
    if (playlistId != null ||
        (browseId != null && (browseId.startsWith('VL') || browseId.startsWith('RD')))) {
      return YtTipo.lista;
    }
    return YtTipo.desconocido;
  }

  static String? _extraerVideoId(dynamic item) {
    try {
      final v = item['playlistItemData']?['videoId'] as String?;
      if (v != null) return v;
    } catch (_) {}
    try {
      return item['overlay']['musicItemThumbnailOverlayRenderer']['content']
          ['musicPlayButtonRenderer']['playNavigationEndpoint']['watchEndpoint']
          ['videoId'] as String?;
    } catch (_) {}
    try {
      return item['navigationEndpoint']['watchEndpoint']['videoId'] as String?;
    } catch (_) {}
    try {
      final runs = item['flexColumns']?[0]?['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?;
      final v = runs?[0]?['navigationEndpoint']?['watchEndpoint']?['videoId'] as String?;
      if (v != null) return v;
    } catch (_) {}
    return null;
  }

  static Duration? _duracionDeFixedColumn(Map item) {
    try {
      final fijas = item['fixedColumns'] as List;
      final texto = fijas[0]['musicResponsiveListItemFixedColumnRenderer']['text']['runs'][0]
          ['text'] as String?;
      return _parseDuracion(texto);
    } catch (_) {
      return null;
    }
  }

  static Duration? _parseDuracion(String? texto) {
    if (texto == null) return null;
    final partes = texto.trim().split(':').map(int.tryParse).toList();
    if (partes.any((p) => p == null)) return null;
    final n = partes.cast<int>();
    return switch (n.length) {
      2 => Duration(minutes: n[0], seconds: n[1]),
      3 => Duration(hours: n[0], minutes: n[1], seconds: n[2]),
      _ => null,
    };
  }

  static String? _urlDeMiniatura(dynamic nodo) {
    if (nodo == null) return null;
    List? miniaturas;
    for (final ruta in [
      () => nodo['musicThumbnailRenderer']['thumbnail']['thumbnails'],
      () => nodo['croppedSquareThumbnailRenderer']['thumbnail']['thumbnails'],
      () => nodo['thumbnails'],
      () => nodo['thumbnail']['thumbnails'],
    ]) {
      try {
        final r = ruta();
        if (r is List && r.isNotEmpty) {
          miniaturas = r;
          break;
        }
      } catch (_) {}
    }
    if (miniaturas == null) return null;
    return miniaturas.last['url'] as String?;
  }
}
