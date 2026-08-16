// Sonda del resolutor de streams de NeoTube (`lib/core/yt_stream.dart`).
//
//   dart run tool/probe_stream.dart                    # una pista conocida
//   dart run tool/probe_stream.dart VIDEOID [VIDEOID…] # las que le pases
//   dart run tool/probe_stream.dart --rondas 3 ID      # repite, para ver rachas
//
// Existe por lo mismo que `probe_yt.dart`: `flutter test` no puede hacer red
// (está mockeada a 400), y esta API no tiene documentación oficial. Lo único
// que demuestra que el cliente `ANDROID_VR` sigue devolviendo URLs sin cifrar
// es pedírselas a YouTube de verdad.
//
// ⚠️ Ojo con lo que esta sonda **no** puede demostrar: que la URL se descargue
// bien aquí no significa que libmpv pueda abrirla. Ese fallo (403 solo a
// ffmpeg) se ha dado ya dos veces y solo se ve con `flutter run`.
//
// Lo que se vino a comprobar aquí: **que los formatos llegan con `url` y no con
// `signatureCipher`**. Ese es el detalle del que depende que no haga falta ni
// yt-dlp, ni Deno, ni un PoToken. Si algún día se rompe, se romperá por ahí, y
// el síntoma en la app sería volver a tardar tres segundos por pista (el plan B
// de yt-dlp) sin ningún error visible.
//
// Mide además el tiempo de resolución, que es la razón de ser del cambio, y
// baja los primeros bytes del stream para confirmar que la URL sirve audio de
// verdad y no un 403 amable.
//
// SEGURIDAD: solo lee. No toca la sesión ni la reescribe, así que se puede
// ejecutar con la app abierta. No imprime cookies ni URLs completas.
//
// ignore_for_file: avoid_print  — es una herramienta de diagnóstico.
import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:neotube/core/yt_stream.dart';

/// Rick Astley, que lleva ahí desde antes que esta API.
const _porDefecto = 'dQw4w9WgXcQ';

Future<void> main(List<String> args) async {
  var rondas = 1;
  final i = args.indexOf('--rondas');
  if (i >= 0 && i + 1 < args.length) rondas = int.tryParse(args[i + 1]) ?? 1;

  final ids = <String>[];
  for (var j = 0; j < args.length; j++) {
    final a = args[j];
    if (a.startsWith('--')) continue;
    if (j > 0 && args[j - 1] == '--rondas') continue;
    ids.add(a);
  }
  if (ids.isEmpty) ids.add(_porDefecto);

  // Se pide anónima, igual que la app: mandar las cookies de la cuenta hace que
  // googlevideo emita URLs que luego libmpv no puede abrir. Ver `yt_stream.dart`.
  print('Petición anónima, sin cookies de cuenta (a propósito).\n');

  final resolutor = YtStreamResolver();
  final tiempos = <int>[];
  var ok = 0;
  var total = 0;

  for (var ronda = 1; ronda <= rondas; ronda++) {
    if (rondas > 1) print('--- ronda $ronda ---');
    for (final id in ids) {
      total++;
      final reloj = Stopwatch()..start();
      try {
        final url = await resolutor.resolver(id);
        reloj.stop();
        tiempos.add(reloj.elapsedMilliseconds);
        ok++;
        print('  $id  ${reloj.elapsedMilliseconds}ms  ${_describir(url)}');
        print('        ${await _comprobarQueSirve(url)}');
      } on YtStreamException catch (e) {
        reloj.stop();
        tiempos.add(reloj.elapsedMilliseconds);
        print('  $id  ${reloj.elapsedMilliseconds}ms  FALLO: $e');
        print('        ¿caería a yt-dlp?: ${e.reintentarConYtDlp ? 'sí' : 'no'}');
      }
      await Future<void>.delayed(const Duration(milliseconds: 250));
    }
  }

  resolutor.dispose();

  tiempos.sort();
  print('\n$ok/$total resueltas.');
  if (tiempos.isNotEmpty) {
    print('mediana ${tiempos[tiempos.length ~/ 2]}ms · máximo ${tiempos.last}ms');
    print('(referencia: yt-dlp tardaba ~2700ms por pista en Windows)');
  }
  if (ok < total) exitCode = 1;
}

/// Qué formato tocó, sin escupir la URL entera (lleva la firma dentro).
String _describir(String url) {
  final u = Uri.parse(url);
  final itag = u.queryParameters['itag'] ?? '?';
  final mime = Uri.decodeComponent(u.queryParameters['mime'] ?? '?');
  final expira = int.tryParse(u.queryParameters['expire'] ?? '');
  final horas = expira == null
      ? '?'
      : ((expira * 1000 - DateTime.now().millisecondsSinceEpoch) / 3600000)
          .toStringAsFixed(1);
  return 'itag $itag ($mime) · host ${u.host.split('.').first} · caduca en ${horas}h';
}

/// La prueba que de verdad importa: ¿sirve bytes de audio a quien no dijo ser
/// un iPhone? Es lo que hará libmpv, que manda su propio `User-Agent`.
Future<String> _comprobarQueSirve(String url) async {
  try {
    final reloj = Stopwatch()..start();
    final res = await http
        .get(Uri.parse(url), headers: {'Range': 'bytes=0-500000', 'User-Agent': 'libmpv'})
        .timeout(const Duration(seconds: 15));
    reloj.stop();
    if (res.statusCode != 206 && res.statusCode != 200) {
      return 'la URL NO sirve: HTTP ${res.statusCode}';
    }
    final kb = res.bodyBytes.length / 1024;
    final mbs = res.bodyBytes.length / 1048576 / (reloj.elapsedMilliseconds / 1000);
    return 'sirve ${kb.toStringAsFixed(0)}KB a ${mbs.toStringAsFixed(1)}MB/s '
        '(HTTP ${res.statusCode}, User-Agent de libmpv)';
  } catch (e) {
    return 'la URL NO sirve: $e';
  }
}
