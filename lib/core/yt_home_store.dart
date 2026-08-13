import 'package:flutter/foundation.dart';

import 'yt_models.dart';

/// Carga y guarda las secciones de una pantalla de NeoTube (portada, explorar
/// o biblioteca).
///
/// Recibe **cómo** cargar y no un `browseId`: la biblioteca no es un solo
/// `browseId` sino cuatro (playlists, álbumes, canciones y artistas viven en
/// endpoints distintos), y encajar eso en un campo de texto obligaba a poner
/// la excepción dentro del store. Con una función, cada pantalla trae la suya.
class YtHomeStore extends ChangeNotifier {
  YtHomeStore(this.cargarSecciones);

  final Function cargarSecciones;

  List<YtSection> secciones = const [];
  bool cargando = false;
  String? error;

  bool get vacio => secciones.isEmpty;

  Future<void> cargar({bool forzar = true}) async {
    cargando = true;
    error = null;
    notifyListeners();
    try {
      final fn = cargarSecciones;
      if (fn is Future<List<YtSection>> Function({bool forzar})) {
        secciones = await fn(forzar: forzar);
      } else if (fn is Future<List<YtSection>> Function(bool forzar)) {
        secciones = await fn(forzar);
      } else if (fn is Future<List<YtSection>> Function()) {
        secciones = await fn();
      } else {
        secciones = await (fn as dynamic)();
      }
    } catch (e) {
      error = '$e';
    } finally {
      cargando = false;
      notifyListeners();
    }
  }

  /// Carga solo si no hay nada todavía: al volver a una pestaña ya vista no
  /// se repite el viaje y aprovecha la caché.
  Future<void> cargarSiHaceFalta() async {
    if (secciones.isNotEmpty || cargando) return;
    await cargar(forzar: false);
  }
}
