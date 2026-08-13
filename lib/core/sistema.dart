import 'dart:async';
import 'dart:io';

import 'art_cache.dart';
import 'yt_models.dart';

/// Lo que los controles del sistema (MPRIS en Linux, SMTC en Windows) necesitan
/// saber del reproductor.
class EstadoDelSistema {
  const EstadoDelSistema({
    required this.track,
    required this.sonando,
    required this.posicionMs,
    required this.puedeSaltar,
    required this.puedeVolver,
    required this.volumen,
  });

  /// Sin nada sonando. Es lo que se manda al salir, para que el escritorio no se
  /// quede enseñando la última canción de una app que ya no está.
  static const vacio = EstadoDelSistema(
    track: null,
    sonando: false,
    posicionMs: 0,
    puedeSaltar: false,
    puedeVolver: false,
    volumen: null,
  );

  final YtTrack? track;
  final bool sonando;
  final int posicionMs;
  final bool puedeSaltar;
  final bool puedeVolver;

  /// 0..100. MPRIS lo quiere de 0.0 a 1.0; el panel de Windows no lo usa
  /// (allí el volumen es el del sistema).
  final int? volumen;

  /// `Playing`, `Paused` o `Stopped`, que son los tres valores que distingue MPRIS
  /// y Windows.
  String get estadoDeReproduccion {
    if (track == null) return 'Stopped';
    return sonando ? 'Playing' : 'Paused';
  }
}

/// El fichero de la carátula de [track] que **ya esté descargado**, o `null`.
///
/// Se sirve desde disco para que el escritorio la pinte sin bajar nada y sin
/// gastar cuota: no se puede esperar a una descarga para contestar por D-Bus, y
/// dar la url `http` haría que el escritorio (o Windows) se la bajara por su
/// cuenta.
File? ficheroDeCaratula(YtTrack track) {
  final url = track.miniatura;
  if (url == null) return null;
  return ArtCache.ficheroSiEstaEnDisco(url);
}

/// Lo mismo, como url `file://`, que es la forma en la que MPRIS quiere la
/// carátula. Windows, en cambio, quiere la ruta a secas.
String? caratulaEnDisco(YtTrack track) =>
    ficheroDeCaratula(track)?.uri.toString();

/// Se trae la carátula que falta y avisa cuando ya está en disco.
class DescargadorDeCaratula {
  DescargadorDeCaratula(this._videoIdQueSuena);

  /// Qué está sonando **ahora mismo**.
  final String? Function() _videoIdQueSuena;

  /// VideoId de la pista cuya carátula se está bajando.
  String? _bajando;

  void asegurar(YtTrack track, void Function() alLlegar) {
    final url = track.miniatura;
    if (url == null || _bajando == track.videoId) return;
    _bajando = track.videoId;
    unawaited(ArtCache.file(url).then((_) {
      _bajando = null;
      if (_videoIdQueSuena() != track.videoId) return;
      alLlegar();
    }).catchError((_) {
      _bajando = null;
    }));
  }
}
