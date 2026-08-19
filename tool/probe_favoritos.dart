// Sonda de la lista "Me gusta" (NeoTube).
//
//   dart run tool/probe_favoritos.dart
//
// Vino a resolver una duda concreta: **de dónde se sacan los videoId de las
// canciones marcadas con me gusta**. `FEmusic_liked_videos` es lo que usa
// Biblioteca, pero llega como rejilla de secciones; la lista de verdad, la
// que tiene todas las pistas y sus continuaciones, es la playlist `LM`.
//
// SEGURIDAD: solo lee (`browse`). No da ni quita likes.
//
// ignore_for_file: avoid_print  — es una herramienta de diagnóstico.
import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:http/http.dart' as http;
import 'package:neotube/core/app_config.dart';
import 'package:path/path.dart' as p;

const _base = 'music.youtube.com';

Future<void> main() async {
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
  if (sapisid.isEmpty) {
    print('Las cookies guardadas no traen SAPISID.');
    exit(1);
  }
  final cabeceraCookie = cookies.entries.map((e) => '${e.key}=${e.value}').join('; ');

  Future<Map<String, dynamic>> post(String endpoint, Map<String, dynamic> body) async {
    final epoch = (DateTime.now().millisecondsSinceEpoch ~/ 1000).toString();
    final origin = 'https://$_base';
    final hash = sha1.convert(utf8.encode('$epoch $sapisid $origin')).toString();
    final res = await http.post(
      Uri.https(_base, '/youtubei/v1/$endpoint'),
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
        ...body,
      }),
    );
    if (res.statusCode != 200) {
      throw 'HTTP ${res.statusCode}: ${res.body.substring(0, res.body.length.clamp(0, 300))}';
    }
    return jsonDecode(res.body) as Map<String, dynamic>;
  }

  /// Todos los `videoId` que aparezcan en cualquier parte del JSON, con el
  /// título de al lado si se puede pillar. Es a propósito tosco: la duda es
  /// *cuántas* pistas trae cada vía, no cómo están anidadas.
  int contarVideoIds(dynamic j, Set<String> vistos) {
    if (j is Map) {
      final v = j['videoId'];
      if (v is String && v.isNotEmpty) vistos.add(v);
      for (final e in j.values) {
        contarVideoIds(e, vistos);
      }
    } else if (j is List) {
      for (final e in j) {
        contarVideoIds(e, vistos);
      }
    }
    return vistos.length;
  }

  final j = await post('browse', {'browseId': 'VLLM'});
  final ids = <String>{};
  contarVideoIds(j, ids);
  print('VLLM (playlist "Me gusta"): ${ids.length} canciones\n');

  // Los títulos, para poder reconocer a ojo si una canción concreta está.
  final titulos = <String>[];
  void recogerTitulos(dynamic n) {
    if (n is Map) {
      if (n['videoId'] is String) {
        final texto = jsonEncode(n);
        final m = RegExp(r'"text"\s*:\s*"([^"]{2,80})"').firstMatch(texto);
        if (m != null) titulos.add(m.group(1)!);
      }
      for (final e in n.values) {
        recogerTitulos(e);
      }
    } else if (n is List) {
      for (final e in n) {
        recogerTitulos(e);
      }
    }
  }

  recogerTitulos(j);
  for (final t in titulos.toSet()) {
    print('  · $t');
  }
}
