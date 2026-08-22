import 'dart:async';

import 'package:flutter/foundation.dart';

import 'yt_models.dart';
import 'yt_music_api.dart';

/// Qué canciones tiene el usuario marcadas con "me gusta".
///
/// Vive aparte de `YtMusicApi` porque es **estado**, no red: la interfaz
/// necesita saber en cada fila y en la barra de abajo si una canción está en
/// favoritos, y eso no se puede resolver preguntándolo a la API cada vez que
/// se pinta un widget.
///
/// ## De dónde sale la lista
///
/// De la playlist **`LM`** (que `coleccion` traduce a `browse VLLM`), que es
/// donde YouTube Music guarda de verdad las canciones marcadas con me gusta.
///
/// ⚠️ **No vale `FEmusic_liked_videos`**, aunque el nombre lo prometa: ese es
/// el browseId que usa la pestaña Biblioteca y devuelve una *rejilla de
/// secciones*, no una lista de pistas — pedírselo a `coleccion` falla con
/// "No hay lista que abrir en este elemento" y deja los favoritos vacíos sin
/// más síntoma que unos corazones que nunca se encienden. Comprobado contra
/// la cuenta real con `tool/probe_favoritos.dart`: `VLLM` devuelve las
/// canciones y `FEmusic_liked_videos` no.
///
/// Se carga una vez, en diferido, y a partir de ahí se mantiene sola con
/// cada [alternar].
class YtFavoritos extends ChangeNotifier {
  YtFavoritos(this._api);

  final YtMusicApi _api;

  final Set<String> _ids = {};

  /// Los que están a medio camino: se pintan a medias y no aceptan otra
  /// pulsación hasta que YouTube confirme. Sin esto, pulsar dos veces
  /// deprisa manda dos peticiones contradictorias y el estado local acaba
  /// diciendo lo contrario que la cuenta.
  final Set<String> _enVuelo = {};

  bool _cargando = false;
  bool _cargado = false;

  /// El último cambio, y un número que sube con cada uno.
  ///
  /// Existen para que una lista **ya abierta** pueda reflejar el cambio sin
  /// recargarse entera: la pantalla de la lista de favoritos se apunta el
  /// número que ya aplicó y así distingue un cambio nuevo de un repintado
  /// cualquiera (`notifyListeners` se dispara varias veces por pulsación).
  ({YtTrack pista, bool anadida})? ultimoCambio;
  int revision = 0;

  bool esFavorita(String videoId) => _ids.contains(videoId);
  bool estaOcupada(String videoId) => _enVuelo.contains(videoId);

  /// Trae la lista de favoritos si no se ha traído ya. Se puede llamar tantas
  /// veces como haga falta: solo la primera hace red.
  Future<void> cargar({bool forzar = false}) async {
    if (_cargando) return;
    if (_cargado && !forzar) return;
    _cargando = true;
    try {
      final c = await _api.coleccion(playlistId: 'LM', forzar: forzar);
      _ids
        ..clear()
        ..addAll(c.pistas.map((t) => t.videoId));
      _cargado = true;
      notifyListeners();
    } catch (e) {
      // Sin favoritos cargados los corazones salen vacíos, que es mejor que
      // no dejar usar el botón: darle like funciona igual.
      debugPrint('[NeoTube favoritos] no se pudieron cargar: $e');
    } finally {
      _cargando = false;
    }
  }

  /// Da o quita el "me gusta", con la interfaz respondiendo al instante.
  ///
  /// Se pinta el cambio **antes** de que YouTube conteste y se deshace si
  /// falla: el viaje de red es de cientos de milisegundos y un corazón que
  /// tarda en encenderse se siente roto. Devuelve `false` si no se pudo.
  Future<bool> alternar(YtTrack t) async {
    final id = t.videoId;
    if (_enVuelo.contains(id)) return false;
    final eraFavorita = _ids.contains(id);
    _enVuelo.add(id);
    if (eraFavorita) {
      _ids.remove(id);
    } else {
      _ids.add(id);
    }
    ultimoCambio = (pista: t, anadida: !eraFavorita);
    revision++;
    notifyListeners();
    try {
      if (eraFavorita) {
        await _api.quitarLike(id);
      } else {
        await _api.darLike(id);
      }
      // La copia cacheada de la lista se ha quedado vieja: sin tirarla, abrir
      // Biblioteca justo después enseñaba los favoritos de antes del cambio.
      _api.cache.remove('coleccion:LM');
      return true;
    } catch (e) {
      // Se revierte: el estado local tiene que decir lo mismo que la cuenta,
      // y la lista abierta tiene que deshacer lo que ya había pintado.
      if (eraFavorita) {
        _ids.add(id);
      } else {
        _ids.remove(id);
      }
      ultimoCambio = (pista: t, anadida: eraFavorita);
      revision++;
      debugPrint('[NeoTube favoritos] no se pudo cambiar $id: $e');
      return false;
    } finally {
      _enVuelo.remove(id);
      notifyListeners();
    }
  }
}

/// Qué álbumes y listas tiene el usuario guardados en su biblioteca.
///
/// Hermano de [YtFavoritos], pero **no comparte su forma de enterarse**, y esa
/// es toda la razón de que sea una clase aparte:
///
/// - Los "me gusta" de canciones se saben de golpe: una sola petición a la
///   lista `LM` trae todos los `videoId` marcados, así que [YtFavoritos] puede
///   cargarlos al arrancar y responder por cualquier canción.
/// - Para un álbum o una lista **no hay lista equivalente que valga**. Lo que
///   sí llega, gratis, es el `isToggled` del botón de marcador que viene
///   dentro de la cabecera de cada colección al abrirla
///   ([YtColeccion.guardada]). Así que aquí el estado no se carga: se
///   **siembra** ([sembrar]) según se van abriendo colecciones.
///
/// Por eso [estado] devuelve `bool?`: `null` es "todavía no lo sé", que es
/// distinto de "no está guardado" y no debe pintarse igual que un corazón
/// apagado con certeza.
class YtColeccionesGuardadas extends ChangeNotifier {
  YtColeccionesGuardadas(this._api);

  final YtMusicApi _api;

  final Set<String> _guardadas = {};
  final Set<String> _conocidas = {};
  final Set<String> _enVuelo = {};

  /// `true` guardada, `false` no guardada, `null` sin saber todavía.
  bool? estado(String playlistId) =>
      _conocidas.contains(playlistId) ? _guardadas.contains(playlistId) : null;

  bool estaOcupada(String playlistId) => _enVuelo.contains(playlistId);

  /// Anota lo que dice la cabecera recién traída de la API.
  ///
  /// **No pisa lo que ya se sepa de este id.** La colección puede venir de la
  /// caché en memoria, con el `isToggled` de antes de que el usuario tocara el
  /// corazón hace diez segundos; dejar que ese valor viejo mande sería ver el
  /// corazón apagarse solo al volver a entrar.
  void sembrar(String playlistId, bool guardada) {
    if (_conocidas.contains(playlistId)) return;
    _conocidas.add(playlistId);
    if (guardada) _guardadas.add(playlistId);
    notifyListeners();
  }

  /// Guarda o saca de la biblioteca, pintando el cambio antes de que YouTube
  /// conteste y deshaciéndolo si falla — mismo criterio que
  /// [YtFavoritos.alternar]. Devuelve `false` si no se pudo.
  Future<bool> alternar(String playlistId) async {
    if (_enVuelo.contains(playlistId)) return false;
    final estaba = _guardadas.contains(playlistId);
    _enVuelo.add(playlistId);
    _conocidas.add(playlistId);
    if (estaba) {
      _guardadas.remove(playlistId);
    } else {
      _guardadas.add(playlistId);
    }
    notifyListeners();
    try {
      if (estaba) {
        await _api.quitarColeccion(playlistId);
      } else {
        await _api.guardarColeccion(playlistId);
      }
      // La biblioteca cacheada acaba de quedarse vieja: sin tirarla, entrar en
      // Biblioteca justo después seguiría sin enseñar el álbum recién
      // guardado, y parecería que el corazón no hizo nada.
      _api.cache
        ..remove('biblioteca')
        ..remove('browse:${YtMusicApi.browseIdAlbumes}')
        ..remove('browse:${YtMusicApi.browseIdListas}');
      return true;
    } catch (e) {
      if (estaba) {
        _guardadas.add(playlistId);
      } else {
        _guardadas.remove(playlistId);
      }
      debugPrint('[NeoTube biblioteca] no se pudo cambiar $playlistId: $e');
      return false;
    } finally {
      _enVuelo.remove(playlistId);
      notifyListeners();
    }
  }
}
