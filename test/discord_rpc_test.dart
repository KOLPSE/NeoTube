import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/discord_rpc.dart';
import 'package:neotube/core/yt_models.dart';

YtTrack _track({
  String videoId = 'dQw4w9WgXcQ',
  String titulo = 'Never Gonna Give You Up',
  String artista = 'Rick Astley',
  String? miniatura = 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
  Duration? duracion = const Duration(milliseconds: 213573),
}) =>
    YtTrack(
      videoId: videoId,
      titulo: titulo,
      artista: artista,
      miniatura: miniatura,
      duracion: duracion,
    );

void main() {
  group('DiscordRpc.construirActividad', () {
    test('con pista y siguiente pista: details es el título, state lleva el artista y "Siguiente: …"', () {
      final pistaActual = _track(titulo: 'Bohemian Rhapsody', artista: 'Queen');
      final siguientePista = _track(titulo: 'Don\'t Stop Me Now', artista: 'Queen');

      final actividad = DiscordRpc.construirActividad(
        track: pistaActual,
        siguiente: siguientePista,
        sonando: true,
        progresoMs: 30000,
      );

      expect(actividad['details'], 'Bohemian Rhapsody');
      expect(actividad['state'], 'Queen · Siguiente: Don\'t Stop Me Now');
    });

    test('sin siguiente pista todavía: state solo lleva el artista, sin "Siguiente:"', () {
      final pistaActual = _track(titulo: 'Billie Jean', artista: 'Michael Jackson');

      final actividad = DiscordRpc.construirActividad(
        track: pistaActual,
        siguiente: null,
        sonando: true,
        progresoMs: 15000,
      );

      expect(actividad['details'], 'Billie Jean');
      expect(actividad['state'], 'Michael Jackson');
      expect(actividad['state'], isNot(contains('Siguiente:')));
    });

    test('el botón de GitHub siempre está presente con la URL de NeoTube', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      final buttons = actividad['buttons'] as List<dynamic>;
      expect(buttons, isNotEmpty);
      expect(buttons.first['label'], 'GitHub');
      expect(buttons.first['url'], 'https://github.com/KOLPSE/NeoTube');
    });

    test('timestamps.start es coherente con el progreso pasado', () {
      final ahora = DateTime.now();
      const progresoMs = 45000;

      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: progresoMs,
        ahora: ahora,
      );

      final timestamps = actividad['timestamps'] as Map<String, dynamic>?;
      expect(timestamps, isNotNull);
      final start = timestamps!['start'] as int;
      expect(start, ahora.millisecondsSinceEpoch - progresoMs);
      expect(start, lessThan(DateTime.now().millisecondsSinceEpoch));
    });

    test('timestamps.end es start + duración, para que salga la barra de progreso', () {
      final ahora = DateTime.now();
      const progresoMs = 45000;
      const duracionMs = 213573;

      final actividad = DiscordRpc.construirActividad(
        track: _track(duracion: const Duration(milliseconds: duracionMs)),
        siguiente: null,
        sonando: true,
        progresoMs: progresoMs,
        ahora: ahora,
      );

      final timestamps = actividad['timestamps'] as Map<String, dynamic>?;
      expect(timestamps, isNotNull);
      final start = timestamps!['start'] as int;
      final end = timestamps['end'] as int;
      expect(end, start + duracionMs);
    });

    test('sin duración conocida, timestamps no incluye end', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(duracion: null),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      final timestamps = actividad['timestamps'] as Map<String, dynamic>?;
      expect(timestamps, isNotNull);
      expect(timestamps!.containsKey('end'), isFalse);
    });

    /// El caso que dejaba a Discord sin barra de progreso: **las tarjetas de
    /// portada y de búsqueda no traen duración**, solo la traen las listas y el
    /// panel. Una canción lanzada desde ahí llegaba con `duracion` a `null`, y
    /// sin `end` Discord enseña el tiempo corriendo pero no dibuja la barra.
    /// libmpv sí sabe cuánto dura lo que está sonando.
    test('la duración del reproductor salva a una pista que no la trae', () {
      const duracionReal = 195181;
      final actividad = DiscordRpc.construirActividad(
        track: _track(duracion: null),
        siguiente: null,
        sonando: true,
        progresoMs: 1000,
        duracionMs: duracionReal,
      );

      final timestamps = actividad['timestamps'] as Map<String, dynamic>?;
      expect(timestamps!['end'] as int, (timestamps['start'] as int) + duracionReal);
    });

    /// Y manda sobre la de la pista cuando están las dos: la del reproductor es
    /// la del audio que de verdad está sonando.
    test('la duración del reproductor manda sobre la de la pista', () {
      const duracionReal = 195181;
      final actividad = DiscordRpc.construirActividad(
        track: _track(duracion: const Duration(seconds: 999)),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
        duracionMs: duracionReal,
      );

      final timestamps = actividad['timestamps'] as Map<String, dynamic>?;
      expect(timestamps!['end'] as int, (timestamps['start'] as int) + duracionReal);
    });

    test('cuando está en pausa, no incluye timestamps para no avanzar el tiempo', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: false,
        progresoMs: 45000,
      );

      expect(actividad['timestamps'], isNull);
    });

    test('large_image es la miniatura real de la pista cuando está sonando', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      final assets = actividad['assets'] as Map<String, dynamic>;
      expect(assets['large_image'], 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg');
      expect(assets['large_text'], 'Rick Astley');
    });

    test('al pausar, large_image es el Logo de NeoTube aunque la pista tenga miniatura', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: false,
        progresoMs: 45000,
      );

      final assets = actividad['assets'] as Map<String, dynamic>;
      expect(assets['large_image'], kDiscordAssetLogo);
    });

    test('sin miniatura, large_image se cae al Logo subido', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(miniatura: null),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      final assets = actividad['assets'] as Map<String, dynamic>;
      expect(assets['large_image'], kDiscordAssetLogo);
    });

    test('small_image es siempre el asset Logo, la esquina no depende de la carátula', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      final assets = actividad['assets'] as Map<String, dynamic>;
      expect(assets['small_image'], kDiscordAssetLogo);
      expect(assets['small_text'], kDiscordNombre);
    });

    test('tipo de actividad 2 (Escuchando), no 0 (Jugando)', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      expect(actividad['type'], 2);
    });

    test('YouTube no lleva sync_id ni flags de Spotify', () {
      final actividad = DiscordRpc.construirActividad(
        track: _track(),
        siguiente: null,
        sonando: true,
        progresoMs: 0,
      );

      expect(actividad.containsKey('sync_id'), isFalse);
      expect(actividad.containsKey('flags'), isFalse);
    });
  });

  group('DiscordRpc.construirPayloadSetActivity', () {
    test('construye estructura válida para SET_ACTIVITY con actividad', () {
      final payload = DiscordRpc.construirPayloadSetActivity(
        pid: 1234,
        nonce: 'nonce-1',
        activity: {'details': 'Test'},
      );

      expect(payload['cmd'], 'SET_ACTIVITY');
      expect(payload['nonce'], 'nonce-1');
      expect(payload['args'], {
        'pid': 1234,
        'activity': {'details': 'Test'},
      });
    });

    test('permite activity null para limpiar la presencia', () {
      final payload = DiscordRpc.construirPayloadSetActivity(
        pid: 1234,
        nonce: 'nonce-2',
        activity: null,
      );

      expect(payload['cmd'], 'SET_ACTIVITY');
      expect(payload['nonce'], 'nonce-2');
      expect(payload['args'], {
        'pid': 1234,
        'activity': null,
      });
    });
  });

  group('DiscordRpc.empaquetar', () {
    test('empaqueta opcode y longitud little endian con payload UTF-8', () {
      const jsonStr = '{"v":1}';
      final bytes = DiscordRpc.empaquetar(0, jsonStr);

      expect(bytes.length, 8 + utf8.encode(jsonStr).length);

      final data = ByteData.sublistView(bytes);
      expect(data.getUint32(0, Endian.little), 0);
      expect(data.getUint32(4, Endian.little), utf8.encode(jsonStr).length);

      final payloadDecoded = utf8.decode(bytes.sublist(8));
      expect(payloadDecoded, jsonStr);
    });
  });

  group('DiscordRpc: no reenviar por deriva de posición', () {
    test('la misma pista no reenvía aunque cambie el progreso; una pista nueva sí', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(transporte: transporte);
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 1000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      final tras1a = transporte.envios;
      expect(tras1a, greaterThan(0));

      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 9000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, tras1a);

      final otraPista = _track(videoId: 'otra-cancion', titulo: 'Otra canción');
      rpc.actualizarActividad(
          track: otraPista, sonando: true, progresoMs: 0, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, greaterThan(tras1a));

      await rpc.stop();
    });
  });

  group('DiscordRpc: timeout y comportamiento en pausa', () {
    test('al pausar, se limpia la presencia tras el timeout de pausa', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        timeoutPausa: const Duration(milliseconds: 30),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 2);

      final payloadPausa = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      final activityPausa = (payloadPausa['args'] as Map<String, dynamic>)['activity'] as Map<String, dynamic>;
      expect((activityPausa['assets'] as Map<String, dynamic>)['large_image'], kDiscordAssetLogo);

      await Future<void>.delayed(const Duration(milliseconds: 10));
      expect(transporte.envios, 2);

      await Future<void>.delayed(const Duration(milliseconds: 35));
      expect(transporte.envios, 3);
      final payloadLimpio = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      expect((payloadLimpio['args'] as Map<String, dynamic>)['activity'], isNull);

      await rpc.stop();
    });

    test('si vuelve a sonar antes del timeout de pausa, el temporizador se cancela', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        timeoutPausa: const Duration(milliseconds: 40),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 2);

      await Future<void>.delayed(const Duration(milliseconds: 15));
      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 3);

      await Future<void>.delayed(const Duration(milliseconds: 50));
      expect(transporte.envios, 3);
      final payloadActual = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      expect((payloadActual['args'] as Map<String, dynamic>)['activity'], isNotNull);

      await rpc.stop();
    });

    test('actualizaciones intermedias durante la pausa no reinician el temporizador', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        timeoutPausa: const Duration(milliseconds: 50),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      await Future<void>.delayed(const Duration(milliseconds: 20));
      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      await Future<void>.delayed(const Duration(milliseconds: 15));
      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      await Future<void>.delayed(const Duration(milliseconds: 25));
      expect(transporte.envios, 2);
      final payloadLimpio = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      expect((payloadLimpio['args'] as Map<String, dynamic>)['activity'], isNull);

      await rpc.stop();
    });

    test('cuando track pasa a null, se cancela el timer de pausa y se limpia', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        timeoutPausa: const Duration(milliseconds: 40),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      rpc.actualizarActividad(
          track: null, sonando: false, progresoMs: 0, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 2);
      final payloadLimpio = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      expect((payloadLimpio['args'] as Map<String, dynamic>)['activity'], isNull);

      await Future<void>.delayed(const Duration(milliseconds: 60));
      expect(transporte.envios, 2);

      await rpc.stop();
    });

    test('stop() cancela el timer de pausa y no envía nada tras expirar', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        timeoutPausa: const Duration(milliseconds: 40),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      final pista = _track();
      rpc.actualizarActividad(
          track: pista, sonando: false, progresoMs: 10000, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      await rpc.stop();
      final enviosTrasStop = transporte.envios;

      await Future<void>.delayed(const Duration(milliseconds: 60));
      expect(transporte.envios, enviosTrasStop);
    });
  });
}

class _FakeTransport implements DiscordTransport {
  bool _conectado = false;
  int envios = 0;
  final List<String> payloads = [];

  @override
  bool get conectado => _conectado;

  @override
  Future<bool> conectar(String clientId) async {
    _conectado = true;
    return true;
  }

  @override
  Future<bool> enviar(int opcode, String jsonStr) async {
    envios++;
    payloads.add(jsonStr);
    return true;
  }

  @override
  Future<void> desconectar() async {
    _conectado = false;
  }
}
