// Sonda del CORTE de posición de googlevideo (ver `yt_local_proxy.dart`).
//
//   dart run tool/probe_corte.dart [VIDEOID…]
//
// Resuelve la URL por la vía rápida (igual que la app) y mide, con `dart:io`
// puro —el mismo cliente que usa ffmpeg por debajo—, en qué byte exacto deja
// de servir. Es la comprobación que decide si el troceado de 900 000 bytes del
// proxy sigue teniendo sentido o se ha quedado corto.
//
// SEGURIDAD: solo lee. No toca la sesión.
//
// ignore_for_file: avoid_print — es una herramienta de diagnóstico.
import 'dart:io';

import 'package:neotube/core/yt_stream.dart';

const _porDefecto = ['dQw4w9WgXcQ', 'mFFJzJvIQyU'];

Future<void> main(List<String> args) async {
  final ids = args.where((a) => !a.startsWith('--')).toList();
  if (ids.isEmpty) ids.addAll(_porDefecto);

  final resolutor = YtStreamResolver();
  final cliente = HttpClient();

  for (final id in ids) {
    print('=== $id ===');
    final String url;
    try {
      url = await resolutor.resolver(id);
    } on YtStreamException catch (e) {
      print('  no se pudo resolver: $e\n');
      continue;
    }

    // 1. Lo que pide el proxy: primer trozo de 900 000 y el siguiente.
    final primero = await _pedir(cliente, url, 0, 899999);
    print('  bytes=0-899999        -> ${_resumen(primero)}');
    final segundo = await _pedir(cliente, url, 900000, 1799999);
    print('  bytes=900000-1799999  -> ${_resumen(segundo)}');

    // 2. Lo que pide libmpv al abrir: el fichero de un tirón.
    final entero = await _pedir(cliente, url, 0, null);
    print('  bytes=0-              -> ${_resumen(entero)}');

    // 3. Búsqueda binaria del byte exacto donde corta.
    final corte = await _buscarCorte(cliente, url);
    print('  el corte está en: ${corte ?? 'no se encontró (sirve entero)'}');
    print('');
  }

  cliente.close(force: true);
  resolutor.dispose();
}

/// Pide un `Range` y devuelve `(status, bytesLeidos, total)`.
Future<({int status, int leidos, int? total})> _pedir(
  HttpClient cliente,
  String url,
  int inicio,
  int? fin,
) async {
  try {
    final req = await cliente.getUrl(Uri.parse(url));
    for (final e in YtStreamResolver.cabecerasDeReproduccion.entries) {
      req.headers.set(e.key, e.value);
    }
    req.headers.set(
        HttpHeaders.rangeHeader, fin == null ? 'bytes=$inicio-' : 'bytes=$inicio-$fin');
    final res = await req.close().timeout(const Duration(seconds: 20));
    var leidos = 0;
    await for (final datos in res) {
      leidos += datos.length;
      // No hace falta el audio entero: con el primer bloque ya se sabe si sirve.
      if (leidos > 400000) break;
    }
    final cr = res.headers.value('content-range');
    final m = cr == null ? null : RegExp(r'/(\d+)$').firstMatch(cr);
    return (
      status: res.statusCode,
      leidos: leidos,
      total: m == null ? null : int.tryParse(m.group(1)!),
    );
  } catch (e) {
    return (status: -1, leidos: 0, total: null);
  }
}

String _resumen(({int status, int leidos, int? total}) r) {
  if (r.status == -1) return 'excepción';
  final mb = r.total == null ? '?' : (r.total! / 1048576).toStringAsFixed(2);
  return 'HTTP ${r.status} · ${r.leidos} bytes leídos · total ${mb} MB';
}

/// El byte exacto a partir del cual ya no sirve. `null` si sirve todo.
Future<int?> _buscarCorte(HttpClient cliente, String url) async {
  // Primero, ¿sirve el final del fichero? Si sí, no hay corte.
  final sondeo = await _pedir(cliente, url, 0, null);
  final total = sondeo.total;
  if (total == null) return null;
  final ultimo = await _pedir(cliente, url, total - 1000, total - 1);
  if (ultimo.status < 400) return null;

  var bajo = 0; // sirve
  var alto = total; // no sirve
  while (alto - bajo > 1) {
    final medio = (bajo + alto) ~/ 2;
    final r = await _pedir(cliente, url, medio, medio + 1000);
    if (r.status < 400) {
      bajo = medio;
    } else {
      alto = medio;
    }
  }
  return alto;
}
