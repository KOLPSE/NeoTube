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
      final rpc = DiscordRpc(
        transporte: transporte,
        intervaloMinimo: Duration.zero,
      );
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
        intervaloMinimo: Duration.zero,
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
        intervaloMinimo: Duration.zero,
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
        intervaloMinimo: Duration.zero,
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
        intervaloMinimo: Duration.zero,
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
        intervaloMinimo: Duration.zero,
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

  group('DiscordRpc: saltar de canción deprisa', () {
    // El fallo que esto fija: cada cambio de estado escribía en el pipe, así
    // que saltar deprisa gastaba el cupo de Discord (unas 5 llamadas por 20 s,
    // que descarta sin avisar) en pistas ya descartadas. La presencia se
    // quedaba en una de en medio y la actualización con la duración —la única
    // que dibuja la barra de progreso— llegaba tarde o se perdía.
    test('cinco saltos seguidos se funden, y el último estado es el que se manda',
        () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        intervaloMinimo: const Duration(milliseconds: 60),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      for (var i = 0; i < 5; i++) {
        rpc.actualizarActividad(
          track: _track(videoId: 'pista-$i', titulo: 'Pista $i'),
          sonando: true,
          progresoMs: 0,
          duracionMs: 0,
          obtenerCola: colaVacia,
        );
        await Future<void>.delayed(Duration.zero);
      }

      // La primera sale al instante (un cambio de canción normal no se
      // retrasa); las otras cuatro se funden en el envío diferido.
      expect(transporte.envios, 1);

      await Future<void>.delayed(const Duration(milliseconds: 90));
      expect(transporte.envios, 2);

      final ultimo = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      final actividad =
          (ultimo['args'] as Map<String, dynamic>)['activity'] as Map<String, dynamic>;
      expect(actividad['details'], 'Pista 4',
          reason: 'el envío diferido describe lo que suena cuando le toca '
              'salir, no lo que sonaba cuando se programó');

      await rpc.stop();
    });

    test('la duración que llega tarde acaba mandándose, con su `end`', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        intervaloMinimo: const Duration(milliseconds: 60),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      // Al cambiar de pista, libmpv todavía no sabe cuánto dura: se manda un 0.
      final pista = _track(duracion: null);
      rpc.actualizarActividad(
          track: pista, sonando: true, progresoMs: 0, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      expect(transporte.envios, 1);

      final primero = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      final sinDuracion =
          (primero['args'] as Map<String, dynamic>)['activity'] as Map<String, dynamic>;
      expect((sinDuracion['timestamps'] as Map<String, dynamic>)['end'], isNull);

      // Y cuando la sabe, dentro de la ventana de agrupado.
      rpc.actualizarActividad(
          track: pista,
          sonando: true,
          progresoMs: 500,
          duracionMs: 213573,
          obtenerCola: colaVacia);
      await Future<void>.delayed(const Duration(milliseconds: 90));

      expect(transporte.envios, 2);
      final segundo = jsonDecode(transporte.payloads.last) as Map<String, dynamic>;
      final conDuracion =
          (segundo['args'] as Map<String, dynamic>)['activity'] as Map<String, dynamic>;
      expect((conDuracion['timestamps'] as Map<String, dynamic>)['end'], isNotNull,
          reason: 'sin `end` Discord no dibuja la barra de progreso');

      await rpc.stop();
    });

    test('parar cancela el envío que estuviera esperando', () async {
      final transporte = _FakeTransport();
      final rpc = DiscordRpc(
        transporte: transporte,
        intervaloMinimo: const Duration(milliseconds: 60),
      );
      Future<List<YtTrack>> colaVacia() async => const [];

      rpc.start('123456');
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);

      rpc.actualizarActividad(
          track: _track(), sonando: true, progresoMs: 0, duracionMs: 0, obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);
      rpc.actualizarActividad(
          track: _track(videoId: 'otra'),
          sonando: true,
          progresoMs: 0,
          duracionMs: 0,
          obtenerCola: colaVacia);
      await Future<void>.delayed(Duration.zero);

      await rpc.stop();
      final trasParar = transporte.envios;

      // Sin cancelar el temporizador, el envío pendiente resucitaría la
      // presencia justo después de haberla limpiado.
      await Future<void>.delayed(const Duration(milliseconds: 90));
      expect(transporte.envios, trasParar);
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
