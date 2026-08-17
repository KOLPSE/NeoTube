import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:http/http.dart' as http;
import 'package:neotube/core/app_config.dart';
import 'package:path/path.dart' as p;

const _base = 'music.youtube.com';

Future<void> main(List<String> args) async {
  final f = File(p.join(appDataDir().path, 'yt_cookies.json'));
  if (!f.existsSync()) {
    print('No hay sesión de NeoTube en ${appDataDir().path}.');
    exit(1);
  }

  final crudas = (jsonDecode(await f.readAsString()) as List).cast<Map<String, dynamic>>();
  final basura = RegExp(r'[^\x21-\x7E]');
  final cookies = <String, String>{};
  for (final c in crudas) {
    final nombre = (c['name'] as String? ?? '').replaceAll(basura, '');
    final valor = (c['value'] as String? ?? '').replaceAll(basura, '');
    if (nombre.isEmpty) continue;
    cookies.putIfAbsent(nombre, () => valor);
  }
  final nombreSapisid = const ['SAPISID', '__Secure-3PAPISID', '__Secure-1PAPISID']
      .firstWhere((n) => (cookies[n] ?? '').isNotEmpty, orElse: () => '');
  final sapisid = cookies[nombreSapisid] ?? '';
  print('Sesión: ${cookies.length} cookies, firmando con $nombreSapisid.\n');

  final cabeceraCookie = cookies.entries.map((e) => '${e.key}=${e.value}').join('; ');

  Future<Map<String, dynamic>> search(String query, {String? params}) async {
    final epoch = (DateTime.now().millisecondsSinceEpoch ~/ 1000).toString();
    final origin = 'https://$_base';
    final hash = sha1.convert(utf8.encode('$epoch $sapisid $origin')).toString();
    final res = await http.post(
      Uri.https(_base, '/youtubei/v1/search'),
      headers: {
        'Content-Type': 'application/json',
        'Cookie': cabeceraCookie,
        'Authorization': 'SAPISIDHASH ${epoch}_$hash',
        'X-Goog-AuthUser': '0',
        'X-Origin': origin,
        'Origin': origin,
      },
      body: jsonEncode({
        'context': {
          'client': {
            'clientName': 'WEB_REMIX',
            'clientVersion': '1.20241201.01.00',
            'hl': 'es',
            'gl': 'ES',
          },
          'user': {'lockedSafetyMode': false},
        },
        'query': query,
        if (params != null) 'params': params,
      }),
    );
    if (res.statusCode != 200) {
      throw 'HTTP ${res.statusCode}: ${res.body.substring(0, res.body.length.clamp(0, 300))}';
    }
    return jsonDecode(res.body) as Map<String, dynamic>;
  }

  const songsParam = 'EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D';
  final queries = [
    'bad bunny',
    'lofi chill music',
    'queen',
    'taylor swift',
    'reggaeton 2025',
  ];

  for (final q in queries) {
    print('================================================================');
    print('TESTING PROPOSED buscar("$q")');
    print('================================================================');
    final sw = Stopwatch()..start();

    // 1. General search
    final fGeneral = search(q);

    // 2. Songs search + 1 continuation
    final fSongs = () async {
      final j1 = await search(q, params: songsParam);
      final tracks = <Map>[];
      String? contToken;

      try {
        final tabs = j1['contents']?['tabbedSearchResultsRenderer']?['tabs'] as List? ?? [];
        final slr = tabs.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
        final cList = slr?['contents'] as List? ?? [];
        for (final b in cList) {
          final shelf = (b as Map)['musicShelfRenderer'] as Map?;
          if (shelf != null) {
            final items = shelf['contents'] as List? ?? [];
            tracks.addAll(items.cast<Map>());
            contToken = shelf['continuations']?[0]?['nextContinuationData']?['continuation'] as String?;
          }
        }
      } catch (_) {}

      if (contToken != null) {
        try {
          final epoch = (DateTime.now().millisecondsSinceEpoch ~/ 1000).toString();
          final origin = 'https://$_base';
          final hash = sha1.convert(utf8.encode('$epoch $sapisid $origin')).toString();
          final res = await http.post(
            Uri.https(_base, '/youtubei/v1/search'),
            headers: {
              'Content-Type': 'application/json',
              'Cookie': cabeceraCookie,
              'Authorization': 'SAPISIDHASH ${epoch}_$hash',
              'X-Goog-AuthUser': '0',
              'X-Origin': origin,
              'Origin': origin,
            },
            body: jsonEncode({
              'context': {
                'client': {
                  'clientName': 'WEB_REMIX',
                  'clientVersion': '1.20241201.01.00',
                  'hl': 'es',
                  'gl': 'ES',
                },
                'user': {'lockedSafetyMode': false},
              },
              'continuation': contToken,
            }),
          );
          if (res.statusCode == 200) {
            final jCont = jsonDecode(res.body) as Map<String, dynamic>;
            final msc = jCont['continuationContents']?['musicShelfContinuation'] as Map?;
            final more = msc?['contents'] as List? ?? [];
            tracks.addAll(more.cast<Map>());
          }
        } catch (_) {}
      }

      return tracks;
    }();

    final results = await Future.wait([fGeneral, fSongs]);
    sw.stop();
    print('Total search & fetch time: ${sw.elapsedMilliseconds}ms');

    final jGeneral = results[0] as Map<String, dynamic>;
    final songsRaw = results[1] as List<Map>;

    final songsItems = songsRaw
        .map(_parseItemSimple)
        .whereType<_ItemSimple>()
        .toList();

    final seccionesGeneral = _parsearSeccionesDeGeneral(jGeneral);

    final resultadoPrincipal = seccionesGeneral
        .where((s) => s.titulo.toLowerCase().contains('principal') || s.titulo.toLowerCase().contains('top'))
        .toList();
    final otrasSecciones = seccionesGeneral
        .where((s) => !resultadoPrincipal.contains(s))
        .toList();

    final resultadoFinal = <_SeccionSimple>[];
    final idsVistos = <String>{};

    // 1. Add "Resultado principal"
    for (final rp in resultadoPrincipal) {
      if (rp.items.isNotEmpty) {
        resultadoFinal.add(rp);
        for (final it in rp.items) {
          if (it.browseId != null) idsVistos.add(it.browseId!);
        }
      }
    }

    // 2. Add "Canciones"
    if (songsItems.isNotEmpty) {
      final cancionesUnicas = <_ItemSimple>[];
      for (final s in songsItems) {
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
        resultadoFinal.add(_SeccionSimple(titulo: 'Canciones', items: cancionesUnicas));
      }
    }

    // 3. Add other sections
    for (final s in otrasSecciones) {
      if (songsItems.isNotEmpty && (s.titulo.toLowerCase() == 'canciones' || s.titulo.toLowerCase() == 'songs')) {
        continue;
      }
      final itemsFiltrados = <_ItemSimple>[];
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
        resultadoFinal.add(_SeccionSimple(titulo: s.titulo, items: itemsFiltrados));
      }
    }

    print('SECCIONES RESULTANTES CON DEDUP (${resultadoFinal.length}):');
    for (final s in resultadoFinal) {
      print('  📌 "${s.titulo}" (${s.items.length} items)');
      for (final it in s.items.take(3)) {
        print('      [${it.tipo}] "${it.titulo}" - "${it.subtitulo}" (videoId: ${it.videoId}, browseId: ${it.browseId})');
      }
    }
    print('\n');
  }
}

class _ItemSimple {
  final String tipo;
  final String titulo;
  final String subtitulo;
  final String? videoId;
  final String? browseId;
  final String? playlistId;
  final String? miniatura;

  _ItemSimple({
    required this.tipo,
    required this.titulo,
    required this.subtitulo,
    this.videoId,
    this.browseId,
    this.playlistId,
    this.miniatura,
  });
}

class _SeccionSimple {
  final String titulo;
  final List<_ItemSimple> items;
  _SeccionSimple({required this.titulo, required this.items});
}

List<_SeccionSimple> _parsearBusquedaCombinada(dynamic jGeneral, dynamic jSongs) {
  // Let's implement the full parser logic
  final secciones = <_SeccionSimple>[];

  // 1. First parse songs from jSongs
  final canciones = _parsearCancionesDeBusqueda(jSongs);

  // 2. Parse general search
  final seccionesGeneral = _parsearSeccionesDeGeneral(jGeneral);

  // 3. If jGeneral had a "Resultado principal" (cardShelf), we can put it first!
  final resultadoPrincipal = seccionesGeneral.where((s) => s.titulo.toLowerCase().contains('principal') || s.titulo.toLowerCase().contains('top')).toList();
  final otrasSeccionesGeneral = seccionesGeneral.where((s) => !resultadoPrincipal.contains(s)).toList();

  for (final rp in resultadoPrincipal) {
    secciones.add(rp);
  }

  // 4. Add Canciones section if not empty
  if (canciones.isNotEmpty) {
    secciones.add(_SeccionSimple(titulo: 'Canciones', items: canciones));
  }

  // 5. Add other sections from general search (Albums, Artists, Playlists, etc.)
  // Remove "Canciones" section from general search if we already added full songs
  for (final s in otrasSeccionesGeneral) {
    if (canciones.isNotEmpty && (s.titulo.toLowerCase() == 'canciones' || s.titulo.toLowerCase() == 'songs')) {
      continue;
    }
    if (s.items.isNotEmpty) {
      secciones.add(s);
    }
  }

  return secciones;
}

List<_ItemSimple> _parsearCancionesDeBusqueda(dynamic j) {
  final items = <_ItemSimple>[];
  try {
    final tabs = j['contents']?['tabbedSearchResultsRenderer']?['tabs'] as List? ?? [];
    if (tabs.isEmpty) return items;
    final slr = tabs.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
    final contents = slr?['contents'] as List? ?? [];
    for (final b in contents) {
      if (b is! Map) continue;
      final shelf = b['musicShelfRenderer'] as Map?;
      if (shelf != null) {
        final list = shelf['contents'] as List? ?? [];
        for (final it in list) {
          final parsed = _parseItemSimple(it);
          if (parsed != null) items.add(parsed);
        }
      }
    }
  } catch (_) {}
  return items;
}

List<_SeccionSimple> _parsearSeccionesDeGeneral(dynamic j) {
  final secciones = <_SeccionSimple>[];
  try {
    final tabs = j['contents']?['tabbedSearchResultsRenderer']?['tabs'] as List? ?? [];
    if (tabs.isEmpty) return secciones;
    final slr = tabs.first['tabRenderer']?['content']?['sectionListRenderer'] as Map?;
    final contents = slr?['contents'] as List? ?? [];

    final itemsSueltos = <_ItemSimple>[];

    for (final b in contents) {
      if (b is! Map) continue;
      final key = b.keys.first;
      final val = b[key] as Map;

      if (key == 'musicCardShelfRenderer') {
        final card = _parseCardShelfSimple(val);
        if (card != null && card.items.isNotEmpty) {
          secciones.add(card);
        }
      } else if (key == 'musicShelfRenderer' || key == 'musicCarouselShelfRenderer') {
        final title = (val['title']?['runs'] as List?)?.map((x) => x['text']).join() ??
            (val['header']?['musicCarouselShelfBasicHeaderRenderer']?['title']?['runs'] as List?)?.map((x) => x['text']).join() ??
            (val['header']?['musicShelfRendererHeaderRenderer']?['title']?['runs'] as List?)?.map((x) => x['text']).join() ??
            '';
        final list = val['contents'] as List? ?? [];
        final items = <_ItemSimple>[];
        for (final it in list) {
          final p = _parseItemSimple(it);
          if (p != null) items.add(p);
        }
        if (items.isNotEmpty) {
          secciones.add(_SeccionSimple(titulo: title, items: items));
        }
      } else if (key == 'itemSectionRenderer') {
        final inside = val['contents'] as List? ?? [];
        for (final it in inside) {
          final p = _parseItemSimple(it);
          if (p != null) itemsSueltos.add(p);
        }
      }
    }

    // If there were flat items in itemSectionRenderer, group them by type if not already covered
    if (itemsSueltos.isNotEmpty) {
      // Group itemsSueltos by type
      final porTipo = <String, List<_ItemSimple>>{};
      for (final it in itemsSueltos) {
        porTipo.putIfAbsent(it.tipo, () => []).add(it);
      }
      for (final entry in porTipo.entries) {
        final nombreSeccion = switch (entry.key) {
          'cancion' => 'Otras canciones y vídeos',
          'album' => 'Álbumes',
          'artista' => 'Artistas',
          'lista' => 'Listas de reproducción',
          _ => 'Resultados',
        };
        // Only add if not already present or if it adds value
        secciones.add(_SeccionSimple(titulo: nombreSeccion, items: entry.value));
      }
    }
  } catch (_) {}
  return secciones;
}

_SeccionSimple? _parseCardShelfSimple(Map val) {
  final headerTitle = (val['header']?['musicCardShelfHeaderBasicRenderer']?['title']?['runs'] as List?)?.map((x) => x['text']).join() ?? 'Resultado principal';
  final items = <_ItemSimple>[];

  // 1. The main card item itself
  final cardTitle = (val['title']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
  final cardSubtitle = (val['subtitle']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
  final nav = val['onTap'] ?? val['title']?['runs']?[0]?['navigationEndpoint'];
  final videoId = nav?['watchEndpoint']?['videoId'] as String?;
  var browseId = nav?['browseEndpoint']?['browseId'] as String?;
  final playlistId = nav?['watchPlaylistEndpoint']?['playlistId'] as String?;
  final mini = val['thumbnailRenderer']?['musicThumbnailRenderer']?['thumbnail']?['thumbnails']?.last?['url'] as String?;

  if (cardTitle.isNotEmpty) {
    String tipo = 'desconocido';
    if (videoId != null) tipo = 'cancion';
    else if (browseId != null && browseId.startsWith('MPRE')) tipo = 'album';
    else if (browseId != null && browseId.startsWith('UC')) tipo = 'artista';
    else if (playlistId != null || (browseId != null && browseId.startsWith('VL'))) tipo = 'lista';

    items.add(_ItemSimple(
      tipo: tipo,
      titulo: cardTitle,
      subtitulo: cardSubtitle,
      videoId: videoId,
      browseId: browseId,
      playlistId: playlistId ?? (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
      miniatura: mini,
    ));
  }

  // 2. Subcontents (e.g. top 3 tracks)
  final subContents = val['contents'] as List? ?? [];
  for (final sub in subContents) {
    final p = _parseItemSimple(sub);
    if (p != null) items.add(p);
  }

  return items.isEmpty ? null : _SeccionSimple(titulo: headerTitle, items: items);
}

_ItemSimple? _parseItemSimple(dynamic it) {
  if (it is! Map) return null;
  if (it.containsKey('musicResponsiveListItemRenderer')) {
    final item = it['musicResponsiveListItemRenderer'] as Map;
    final columnas = item['flexColumns'] as List? ?? [];
    if (columnas.isEmpty) return null;
    final titulo = (columnas[0]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    if (titulo.isEmpty) return null;
    final subtitulo = columnas.length > 1
        ? (columnas[1]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?)?.map((x) => x['text']).join() ?? ''
        : '';
    final videoId = item['playlistItemData']?['videoId'] as String? ??
        item['overlay']?['musicItemThumbnailOverlayRenderer']?['content']?['musicPlayButtonRenderer']?['playNavigationEndpoint']?['watchEndpoint']?['videoId'] as String? ??
        item['navigationEndpoint']?['watchEndpoint']?['videoId'] as String?;
    final nav = item['navigationEndpoint'];
    var browseId = nav?['browseEndpoint']?['browseId'] as String?;
    final playlistId = nav?['watchPlaylistEndpoint']?['playlistId'] as String?;

    String tipo = 'desconocido';
    if (videoId != null) tipo = 'cancion';
    else if (browseId != null && browseId.startsWith('MPRE')) tipo = 'album';
    else if (browseId != null && browseId.startsWith('UC')) tipo = 'artista';
    else if (playlistId != null || (browseId != null && browseId.startsWith('VL'))) tipo = 'lista';

    // If browseId is missing but subtitulo indicates artist or playlist or album
    // (e.g., subtitle starts with "Artista", "Álbum", "Lista")
    // Let's check navigation endpoints in flexColumns runs if nav was not on item level!
    if (browseId == null) {
      try {
        final runs0 = columnas[0]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?;
        browseId ??= runs0?[0]?['navigationEndpoint']?['browseEndpoint']?['browseId'] as String?;
        if (columnas.length > 1) {
          final runs1 = columnas[1]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?;
          for (final r in runs1 ?? []) {
            final b = r['navigationEndpoint']?['browseEndpoint']?['browseId'] as String?;
            if (b != null) {
              browseId ??= b;
              break;
            }
          }
        }
      } catch (_) {}
    }

    return _ItemSimple(
      tipo: tipo,
      titulo: titulo,
      subtitulo: subtitulo,
      videoId: videoId,
      browseId: browseId,
      playlistId: playlistId ?? (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
    );
  }

  if (it.containsKey('musicTwoRowItemRenderer')) {
    final item = it['musicTwoRowItemRenderer'] as Map;
    final titulo = (item['title']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    if (titulo.isEmpty) return null;
    final subtitulo = (item['subtitle']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    final nav = item['navigationEndpoint'];
    final videoId = nav?['watchEndpoint']?['videoId'] as String?;
    final playlistId = nav?['watchPlaylistEndpoint']?['playlistId'] as String?;
    var browseId = nav?['browseEndpoint']?['browseId'] as String?;
    browseId ??= item['title']?['runs']?[0]?['navigationEndpoint']?['browseEndpoint']?['browseId'] as String?;

    String tipo = 'desconocido';
    if (videoId != null) tipo = 'cancion';
    else if (browseId != null && browseId.startsWith('MPRE')) tipo = 'album';
    else if (browseId != null && browseId.startsWith('UC')) tipo = 'artista';
    else if (playlistId != null || (browseId != null && browseId.startsWith('VL'))) tipo = 'lista';

    return _ItemSimple(
      tipo: tipo,
      titulo: titulo,
      subtitulo: subtitulo,
      videoId: videoId,
      browseId: browseId,
      playlistId: playlistId ?? (browseId != null && browseId.startsWith('VL') ? browseId.substring(2) : null),
    );
  }

  return null;
}

void _analizarSearchJson(Map<String, dynamic> j) {
  print('Top-level keys: ${j.keys.toList()}');
  final contents = j['contents'] as Map<String, dynamic>?;
  if (contents == null) {
    print('Sin contents!');
    return;
  }
  print('contents keys: ${contents.keys.toList()}');

  final header = j['header'];
  if (header != null) {
    print('Header keys: ${(header as Map).keys.toList()}');
    final chipCloud = (header as Map)['chipCloudRenderer'] as Map<String, dynamic>?;
    if (chipCloud != null) {
      final chips = chipCloud['chips'] as List? ?? [];
      print('  ChipCloud chips (${chips.length}):');
      for (final chip in chips) {
        final cr = chip['chipCloudChipRenderer'] as Map<String, dynamic>?;
        if (cr != null) {
          final text = cr['text']?['runs']?[0]?['text'];
          final isSelected = cr['isSelected'] ?? false;
          final params = cr['navigationEndpoint']?['searchEndpoint']?['params'];
          print('    Chip: text="$text", isSelected=$isSelected, params="$params"');
        }
      }
    }
  }

  final tabbed = contents['tabbedSearchResultsRenderer'];
  if (tabbed != null) {
    final tabs = tabbed['tabs'] as List? ?? [];
    print('tabbedSearchResultsRenderer.tabs length: ${tabs.length}');
    for (var i = 0; i < tabs.length; i++) {
      final tab = tabs[i]['tabRenderer'] as Map<String, dynamic>?;
      final title = tab?['title'] ?? '(sin titulo)';
      final selected = tab?['selected'] ?? false;
      print('  Tab $i: title="$title", selected=$selected');
      final content = tab?['content'] as Map<String, dynamic>?;
      if (content != null) {
        print('    content keys: ${content.keys.toList()}');
        final slr = content['sectionListRenderer'] as Map<String, dynamic>?;
        if (slr != null) {
          final cList = slr['contents'] as List? ?? [];
          print('    sectionListRenderer.contents length: ${cList.length}');
          for (var j = 0; j < cList.length; j++) {
            final bloque = cList[j] as Map<String, dynamic>;
            final key = bloque.keys.first;
            final val = bloque[key] as Map<String, dynamic>? ?? {};
            if (key == 'musicShelfRenderer') {
              final titleRuns = val['title']?['runs'] ?? val['header']?['musicShelfRendererHeaderRenderer']?['title']?['runs'];
              final title = (titleRuns as List?)?.map((r) => r['text']).join() ?? '(sin titulo)';
              final subContents = val['contents'] as List? ?? [];
              final contToken = val['continuations'] != null;
              final bottomEndpoint = val['bottomEndpoint'];
              print('      [$j] musicShelfRenderer "$title" (items: ${subContents.length}, continuations: $contToken, bottomEndpoint: $bottomEndpoint)');
              for (var k = 0; k < subContents.take(3).length; k++) {
                final it = subContents[k] as Map<String, dynamic>;
                print('        item $k: ${_describirItem(it)}');
              }
            } else if (key == 'musicCardShelfRenderer') {
              final titleRuns = val['title']?['runs'] ?? val['header']?['musicCardShelfHeaderBasicRenderer']?['title']?['runs'];
              final title = (titleRuns as List?)?.map((r) => r['text']).join() ?? '(sin titulo)';
              final subtitle = (val['subtitle']?['runs'] as List?)?.map((r) => r['text']).join() ?? '';
              final subContents = val['contents'] as List? ?? [];
              print('      [$j] musicCardShelfRenderer "$title" subtitle="$subtitle" (contents: ${subContents.length})');
              print('          buttons: ${val['buttons']}');
              print('          onTap: ${val['onTap']}');
              for (var k = 0; k < subContents.length; k++) {
                final it = subContents[k] as Map<String, dynamic>;
                print('          subItem $k: ${_describirItem(it)}');
              }
            } else if (key == 'musicCarouselShelfRenderer') {
              final titleRuns = val['header']?['musicCarouselShelfBasicHeaderRenderer']?['title']?['runs'];
              final title = (titleRuns as List?)?.map((r) => r['text']).join() ?? '(sin titulo)';
              final subContents = val['contents'] as List? ?? [];
              print('      [$j] musicCarouselShelfRenderer "$title" (items: ${subContents.length})');
              for (var k = 0; k < subContents.take(3).length; k++) {
                final it = subContents[k] as Map<String, dynamic>;
                print('        item $k: ${_describirItem(it)}');
              }
            } else if (key == 'itemSectionRenderer') {
              final inside = val['contents'] as List? ?? [];
              print('      [$j] itemSectionRenderer (inside count: ${inside.length})');
              for (var k = 0; k < inside.length; k++) {
                final it = inside[k] as Map<String, dynamic>;
                print('        inside $k: ${it.keys.first} -> ${_describirItem(it)}');
              }
            } else {
              print('      [$j] OTRO: $key -> ${val.keys.toList()}');
            }
          }
        }
      }
    }
  }
}

String _describirItem(Map it) {
  if (it.containsKey('musicResponsiveListItemRenderer')) {
    final r = it['musicResponsiveListItemRenderer'] as Map;
    final cols = r['flexColumns'] as List? ?? [];
    final col0 = cols.isNotEmpty ? (cols[0]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?)?.map((x) => x['text']).join() : '';
    final col1 = cols.length > 1 ? (cols[1]['musicResponsiveListItemFlexColumnRenderer']?['text']?['runs'] as List?)?.map((x) => x['text']).join() : '';
    final videoId = r['playlistItemData']?['videoId'] ?? r['navigationEndpoint']?['watchEndpoint']?['videoId'];
    final browseId = r['navigationEndpoint']?['browseEndpoint']?['browseId'];
    return 'musicResponsiveListItem: "$col0" / "$col1" (videoId: $videoId, browseId: $browseId)';
  }
  if (it.containsKey('musicTwoRowItemRenderer')) {
    final r = it['musicTwoRowItemRenderer'] as Map;
    final title = (r['title']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    final sub = (r['subtitle']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    final videoId = r['navigationEndpoint']?['watchEndpoint']?['videoId'];
    final browseId = r['navigationEndpoint']?['browseEndpoint']?['browseId'];
    return 'musicTwoRowItem: "$title" / "$sub" (videoId: $videoId, browseId: $browseId)';
  }
  if (it.containsKey('messageRenderer')) {
    final r = it['messageRenderer'] as Map;
    final t = (r['text']?['runs'] as List?)?.map((x) => x['text']).join() ?? '';
    return 'messageRenderer: "$t"';
  }
  return '${it.keys.toList()}';
}
