import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'sistema.dart';

export 'sistema.dart' show EstadoDelSistema;

/// Integración con los controles multimedia de Windows (SMTC).
///
/// Permite que NeoTube aparezca en el panel multimedia de Windows (centro de control,
/// teclas de volumen, pantalla de bloqueo) con carátula, título, artista, barra de
/// progreso y controles.
class SmtcService {
  SmtcService({
    required this.onPlayPause,
    required this.onPlay,
    required this.onPause,
    required this.onNext,
    required this.onPrevious,
    required this.onSeek,
    required this.estado,
  });

  static const _canal = MethodChannel('neotube/system_media');

  final Future<void> Function() onPlayPause;
  final Future<void> Function() onPlay;
  final Future<void> Function() onPause;
  final Future<void> Function() onNext;
  final Future<void> Function() onPrevious;

  /// Salto absoluto, en **milisegundos**.
  final Future<void> Function(int milisegundos) onSeek;

  /// Función para leer el estado actual del reproductor.
  final EstadoDelSistema Function() estado;

  bool _activo = false;

  bool get activo => _activo;

  void start() {
    if (!Platform.isWindows || _activo) return;
    _canal.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'playPause':
          await onPlayPause();
        case 'play':
          await onPlay();
        case 'pause':
          await onPause();
        case 'stop':
          await onPause();
        case 'next':
          await onNext();
        case 'previous':
          await onPrevious();
        case 'seek':
          final ms = call.arguments;
          if (ms is int) await onSeek(ms < 0 ? 0 : ms);
      }
      return null;
    });
    _activo = true;
    notificarCambio();
  }

  Future<void> stop() async {
    if (!_activo) return;
    _activo = false;
    _canal.setMethodCallHandler(null);
    await _mandar(EstadoDelSistema.vacio, null);
  }

  String? _ultimaFirma;

  late final DescargadorDeCaratula _caratulas =
      DescargadorDeCaratula(() => estado().track?.videoId);

  /// Avisa al sistema de que ha cambiado la canción o el estado.
  void notificarCambio() {
    if (!_activo) return;
    final e = estado();
    final track = e.track;
    final caratula = track == null ? null : ficheroDeCaratula(track)?.path;
    final firma = '${track?.videoId}|${e.estadoDeReproduccion}|'
        '${e.puedeSaltar}|${e.puedeVolver}|$caratula';
    if (firma != _ultimaFirma) {
      _ultimaFirma = firma;
      unawaited(_mandar(e, caratula));
    }
    if (track != null && caratula == null) {
      _caratulas.asegurar(track, notificarCambio);
    }
  }

  /// Anuncia un salto de posición al panel del sistema.
  void notificarSalto(int posicionMs) {
    if (!_activo) return;
    final e = estado();
    final track = e.track;
    final caratula = track == null ? null : ficheroDeCaratula(track)?.path;
    unawaited(_mandar(e, caratula, posicionMs: posicionMs));
  }

  Future<void> _mandar(EstadoDelSistema e, String? caratula,
      {int? posicionMs}) async {
    final track = e.track;
    try {
      await _canal.invokeMethod<void>('update', <String, Object?>{
        'hayCancion': track != null,
        'sonando': e.sonando,
        'titulo': track?.titulo ?? '',
        'artista': track?.artista ?? '',
        'album': '',
        'caratula': caratula ?? '',
        'puedeSaltar': e.puedeSaltar,
        'puedeVolver': e.puedeVolver,
        'duracionMs': track?.duracion?.inMilliseconds ?? 0,
        'posicionMs': posicionMs ?? e.posicionMs,
      });
    } on MissingPluginException {
      // El runner no implementa el canal (tests o compilaciones previas).
    } on PlatformException catch (error) {
      debugPrint('Controles del sistema: $error');
    }
  }
}
