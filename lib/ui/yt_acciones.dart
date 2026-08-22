import 'dart:async';

import 'package:flutter/material.dart';

import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';

/// Qué pasa al pulsar un elemento de NeoTube, en un solo sitio.
///
/// La portada, "Explorar", la biblioteca, la búsqueda y el contenido de una
/// lista pintan **los mismos** elementos ([YtItem]), y antes cada pantalla
/// decidía por su cuenta qué hacer con un tap — que era, en todas, "si no es
/// una canción suelta, avisar de que no se puede reproducir". Centralizarlo es
/// lo que permite que una playlist se comporte igual se pulse donde se pulse.
class YtAcciones {
  const YtAcciones({
    required this.api,
    required this.player,
    required this.abrir,
    this.abrirArtista,
  });

  final YtMusicApi api;
  final YtPlayer player;

  /// Navegar al detalle de una lista o un álbum. Lo resuelve el shell, que es
  /// quien tiene la pila de navegación.
  final void Function(YtItem) abrir;

  /// Navegar a la página de un artista. Opcional para que los tests que solo
  /// comprueban qué pasa al pulsar una canción o una lista no tengan que
  /// montar una pila de navegación entera; sin él, un artista avisa en vez de
  /// abrir nada.
  final void Function(YtItem)? abrirArtista;

  /// El tap normal: una canción suena; una lista o un álbum se abren.
  Future<void> pulsar(
    BuildContext context,
    YtItem item, {
    List<YtItem>? hermanas,
  }) async {
    if (item.esCancion) {
      await reproducirCancion(context, item.comoPista!, hermanas: hermanas);
      return;
    }
    // ⚠️ El artista va **antes** que la comprobación de `browseId`, y ahí está
    // el fallo que esto arregla: un artista de la biblioteca sí trae
    // `browseId` (un `MPLAUC…`), así que caía en la rama de abajo y se
    // intentaba abrir como si fuera una lista — de donde salía el «No hay
    // lista que abrir en este elemento» encima de una tarjeta que decía tener
    // canciones.
    if (item.esArtista) {
      final ir = abrirArtista;
      if (ir == null) {
        _avisar(context, 'No se puede abrir la página de este artista.');
      } else {
        ir(item);
      }
      return;
    }
    if (item.playlistId != null || item.browseId != null) {
      abrir(item);
      return;
    }
    _avisar(context, 'Este elemento no trae nada que abrir.');
  }

  /// Una canción suelta (o dentro de una sección/carrusel de [hermanas]):
  /// Si se pasa [hermanas], se construye una cola con todas las canciones
  /// de esa sección y se arranca desde la pulsada. Si no se pasa, suena la pista
  /// suelta y se le engancha su radio detrás.
  Future<void> reproducirCancion(
    BuildContext context,
    YtTrack t, {
    List<YtItem>? hermanas,
  }) async {
    if (hermanas != null && hermanas.isNotEmpty) {
      final pistas = hermanas
          .where((it) => it.esCancion && it.comoPista != null)
          .map((it) => it.comoPista!)
          .toList();
      if (pistas.isNotEmpty) {
        final indice = pistas.indexWhere((p) => p.videoId == t.videoId);
        final desde = indice >= 0 ? indice : 0;
        try {
          await player.reproducirLista(pistas, desde: desde);
        } catch (_) {}
        return;
      }
    }

    try {
      await player.reproducirPista(t);
    } catch (_) {
      // El error queda en player.error y el ScaffoldMessenger del shell raíz
      // se encarga de mostrarlo siempre, aunque este widget se haya desmontado.
      return;
    }
    unawaited(api.radioDe(t.videoId).then((radio) {
      // Solo si sigue sonando la misma: si el usuario ya cambió de canción,
      // engancharle la radio de la anterior sería sabotear su cola.
      if (player.actual?.videoId == t.videoId) player.anexar(radio);
    }).catchError((_) {}));
  }

  /// Reproducir una lista o un álbum de principio a fin.
  Future<void> reproducirColeccion(BuildContext context, YtItem item) async {
    try {
      final c = await api.coleccion(playlistId: item.playlistId, browseId: item.browseId);
      if (c.pistas.isEmpty) {
        if (context.mounted) _avisar(context, 'Esta lista no tiene canciones que reproducir.');
        return;
      }
      await player.reproducirLista(c.pistas, contexto: item.playlistId ?? item.browseId);
    } catch (e) {
      // Si el error fue al reproducir la pista dentro de la lista, ya lo
      // gestiona player.error y el shell. Si fue al traer la lista de la API:
      if (player.error == null && context.mounted) {
        _avisar(context, 'No se pudo reproducir la lista: $e');
      }
    }
  }

  void _avisar(BuildContext context, String mensaje) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(mensaje), duration: const Duration(seconds: 3)),
    );
  }
}

/// El icono que distingue de un vistazo qué es cada tarjeta: sin él, una lista
/// y una canción se ven idénticas y no hay forma de saber cuál va a sonar y
/// cuál va a abrirse.
IconData iconoDe(YtTipo tipo) => switch (tipo) {
      YtTipo.cancion => Icons.play_arrow,
      YtTipo.lista => Icons.queue_music,
      YtTipo.album => Icons.album,
      YtTipo.artista => Icons.person,
      YtTipo.desconocido => Icons.more_horiz,
    };

String formatoDuracion(Duration d) {
  final h = d.inHours;
  final m = d.inMinutes.remainder(60);
  final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
  return h > 0 ? '$h:${m.toString().padLeft(2, '0')}:$s' : '$m:$s';
}
