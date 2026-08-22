import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;

import 'app_config.dart';
import 'yt_models.dart';
import 'yt_player.dart';

/// Lo que estaba sonando la última vez que se cerró NeoTube.
///
/// ## Por qué no vive en `config.json`
///
/// [AppConfig] son **preferencias** —volumen, modo rendimiento, Discord—: se
/// escriben cuando el usuario decide algo y se leen una vez al arrancar. Esto
/// es un *snapshot* que se reescribe solo cada pocos segundos y que puede
/// ocupar cientos de veces más (una cola de 300 canciones). Mezclarlos
/// significaría reescribir las preferencias enteras en cada cambio de pista,
/// y un fichero a medio escribir por un cierre brusco se llevaría por delante
/// también los ajustes.
///
/// ## Lo que **no** hace
///
/// No reproduce nada al abrir. Lo único que hace [YtPlayer.restaurar] es
/// dejar la cola puesta y la pista seleccionada; el audio no se abre hasta
/// que el usuario le da al play. Es deliberado: una app que se pone a sonar
/// sola al arrancar es una app que hay que correr a silenciar, y además
/// gastaría una petición a YouTube por cada arranque, la escuche alguien o no.
class SesionDeReproduccion {
  const SesionDeReproduccion({
    required this.cola,
    required this.indice,
    this.posicion = Duration.zero,
    this.contexto,
    this.aleatorio = false,
    this.repeticion = YtRepeticion.ninguna,
  });

  final List<YtTrack> cola;
  final int indice;
  final Duration posicion;
  final String? contexto;
  final bool aleatorio;
  final YtRepeticion repeticion;

  /// Cuántas pistas se guardan como mucho.
  ///
  /// Una radio larga puede pasar de mil, y el fichero se reescribe cada vez
  /// que cambia la canción: sin tope, cerrar la app después de una tarde de
  /// escucha significa escribir megabytes por gusto. Se recorta **alrededor de
  /// la pista actual** (ver [_recortada]) y no por el principio, porque lo que
  /// hace falta al volver es lo que venía después.
  static const _maxPistas = 400;

  static File get _fichero =>
      File(p.join(appDataDir().path, 'sesion.json'));

  /// La sesión guardada, o `null` si no hay ninguna o el fichero no sirve.
  ///
  /// Nunca lanza: un `sesion.json` corrupto (un cierre a mitad de escritura,
  /// un formato de una versión futura) tiene que costar la cola de la vez
  /// pasada, no el arranque de la app.
  static Future<SesionDeReproduccion?> cargar() async {
    try {
      final f = _fichero;
      if (!await f.exists()) return null;
      final mapa = jsonDecode(await f.readAsString()) as Map<String, dynamic>;

      final pistas = <YtTrack>[];
      for (final bruta in (mapa['cola'] as List? ?? const [])) {
        if (bruta is! Map) continue;
        final id = bruta['videoId'] as String?;
        if (id == null || id.isEmpty) continue;
        final ms = bruta['duracionMs'] as int?;
        pistas.add(YtTrack(
          videoId: id,
          titulo: (bruta['titulo'] as String?) ?? '',
          artista: (bruta['artista'] as String?) ?? '',
          miniatura: bruta['miniatura'] as String?,
          duracion: ms == null || ms <= 0 ? null : Duration(milliseconds: ms),
        ));
      }
      if (pistas.isEmpty) return null;

      final indice = ((mapa['indice'] as int?) ?? 0).clamp(0, pistas.length - 1);
      return SesionDeReproduccion(
        cola: pistas,
        indice: indice,
        posicion: Duration(milliseconds: (mapa['posicionMs'] as int?) ?? 0),
        contexto: mapa['contexto'] as String?,
        aleatorio: (mapa['aleatorio'] as bool?) ?? false,
        repeticion: YtRepeticion.values.firstWhere(
          (r) => r.name == mapa['repeticion'],
          orElse: () => YtRepeticion.ninguna,
        ),
      );
    } catch (e) {
      debugPrint('[NeoTube sesión] no se pudo leer la sesión guardada: $e');
      return null;
    }
  }

  /// Guarda el estado del reproductor, o borra el fichero si no hay nada
  /// sonando (así "parar" no deja una sesión fantasma que resucite al abrir).
  static Future<void> guardarDe(YtPlayer player) async {
    final actual = player.actual;
    if (actual == null || player.cola.isEmpty) {
      await borrar();
      return;
    }
    final (pistas, indice) = _recortada(player.cola, player.indice);
    await SesionDeReproduccion(
      cola: pistas,
      indice: indice,
      posicion: player.posicion,
      contexto: player.contexto,
      aleatorio: player.aleatorio,
      repeticion: player.repeticion,
    ).guardar();
  }

  /// Deja como mucho [_maxPistas] alrededor de la actual, y devuelve el índice
  /// ya recolocado en la lista recortada.
  static (List<YtTrack>, int) _recortada(List<YtTrack> cola, int indice) {
    if (cola.length <= _maxPistas) return (cola, indice);
    // Un poco de historia por detrás (para que "anterior" siga sirviendo) y el
    // resto por delante, que es lo que se va a escuchar.
    const detras = 50;
    final desde = (indice - detras).clamp(0, cola.length - _maxPistas);
    return (cola.sublist(desde, desde + _maxPistas), indice - desde);
  }

  Future<void> guardar() async {
    try {
      final mapa = {
        'indice': indice,
        'posicionMs': posicion.inMilliseconds,
        'contexto': contexto,
        'aleatorio': aleatorio,
        'repeticion': repeticion.name,
        'cola': [
          for (final t in cola)
            {
              'videoId': t.videoId,
              'titulo': t.titulo,
              'artista': t.artista,
              'miniatura': t.miniatura,
              'duracionMs': t.duracion?.inMilliseconds,
            },
        ],
      };
      // Escritura atómica, igual que `ArtCache`: si la app muere a mitad, lo
      // que queda es la sesión anterior entera y no un JSON truncado que la
      // próxima carga tendría que descartar.
      final destino = _fichero;
      final tmp = File('${destino.path}.tmp');
      await tmp.writeAsString(jsonEncode(mapa), flush: true);
      await tmp.rename(destino.path);
    } catch (e) {
      debugPrint('[NeoTube sesión] no se pudo guardar: $e');
    }
  }

  static Future<void> borrar() async {
    try {
      final f = _fichero;
      if (await f.exists()) await f.delete();
    } catch (_) {
      // Que no se pueda borrar no es motivo para molestar a nadie.
    }
  }
}
