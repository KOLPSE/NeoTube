import 'dart:async';

import 'package:flutter/material.dart';

import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';
import 'art_image.dart';
import 'tira_horizontal.dart';
import 'yt_acciones.dart';
import 'yt_browse_screen.dart';

/// La página de un artista: cabecera con su foto y sus secciones —"Canciones
/// más escuchadas", "Álbumes", "Singles y EPs", "Vídeos", "Puede que también
/// te guste"—.
///
/// Es una pantalla propia y no otra [YtBrowseScreen] por dos motivos, y solo
/// dos: la cabecera (foto, oyentes y los botones de Aleatorio y Mix) y que
/// **las secciones de canciones se pintan como filas y no como tarjetas**. Una
/// tira horizontal de cinco carátulas cuadradas es un mal sitio para leer
/// "canciones más escuchadas": lo que se quiere ver ahí es un top, en orden y
/// numerado. Todo lo demás —los carruseles de álbumes, singles y similares—
/// es exactamente la misma [TarjetaDeYtItem] del resto de la app.
class YtArtistaScreen extends StatefulWidget {
  const YtArtistaScreen({
    super.key,
    required this.item,
    required this.api,
    required this.player,
    required this.acciones,
    required this.onVolver,
  });

  /// La tarjeta que se pulsó. Su título y su miniatura se enseñan mientras
  /// llega la página de verdad, para que la pantalla no salga en blanco.
  final YtItem item;

  final YtMusicApi api;
  final YtPlayer player;
  final YtAcciones acciones;
  final VoidCallback onVolver;

  @override
  State<YtArtistaScreen> createState() => _YtArtistaScreenState();
}

class _YtArtistaScreenState extends State<YtArtistaScreen> {
  YtArtista? _artista;
  bool _cargando = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_cargar());
  }

  @override
  void didUpdateWidget(YtArtistaScreen old) {
    super.didUpdateWidget(old);
    if (old.item.browseId != widget.item.browseId) unawaited(_cargar());
  }

  Future<void> _cargar({bool forzar = false}) async {
    final id = widget.item.browseId;
    if (id == null) {
      setState(() {
        _cargando = false;
        _error = 'Este artista no trae página que abrir.';
      });
      return;
    }
    setState(() {
      _cargando = true;
      _error = null;
    });
    try {
      final a = await widget.api.artista(id, forzar: forzar);
      if (!mounted) return;
      setState(() {
        _artista = a;
        _cargando = false;
      });
    } catch (e) {
      debugPrint('[NeoTube artista $id] $e');
      if (!mounted) return;
      setState(() {
        _error = '$e';
        _cargando = false;
      });
    }
  }

  /// Reproduce una de las listas del artista (su aleatorio o su mix).
  Future<void> _reproducirLista(String playlistId) async {
    await widget.acciones.reproducirColeccion(
      context,
      YtItem(
        tipo: YtTipo.lista,
        titulo: _artista?.nombre ?? widget.item.titulo,
        subtitulo: '',
        playlistId: playlistId,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final a = _artista;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
          child: Row(
            children: [
              IconButton(
                icon: const Icon(Icons.arrow_back),
                tooltip: 'Volver',
                onPressed: widget.onVolver,
              ),
              const SizedBox(width: 4),
              Expanded(
                child: Text(
                  a?.nombre.isNotEmpty == true ? a!.nombre : widget.item.titulo,
                  style: theme.textTheme.titleMedium,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
        Expanded(
          child: _cargando
              ? const Center(child: CircularProgressIndicator())
              : _error != null
                  ? _mensajeDeError(theme, _error!)
                  : a == null || a.secciones.isEmpty
                      ? _mensajeDeError(
                          theme, 'No hay nada que enseñar de este artista.')
                      : _cuerpo(theme, a),
        ),
      ],
    );
  }

  Widget _mensajeDeError(ThemeData theme, String texto) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                texto,
                textAlign: TextAlign.center,
                maxLines: 4,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
              ),
              const SizedBox(height: 16),
              OutlinedButton.icon(
                onPressed: () => unawaited(_cargar(forzar: true)),
                icon: const Icon(Icons.refresh),
                label: const Text('Reintentar'),
              ),
            ],
          ),
        ),
      );

  Widget _cuerpo(ThemeData theme, YtArtista a) {
    final estilo = theme.textTheme.bodySmall ?? const TextStyle(fontSize: 12);
    final altoTira = altoDeTiraDeTarjetas(context, estilo, 3);

    return AnimatedBuilder(
      animation: widget.player,
      builder: (context, _) => ListView.builder(
        padding: const EdgeInsets.only(bottom: 24),
        itemCount: a.secciones.length + 1,
        itemBuilder: (context, i) {
          if (i == 0) return _cabecera(theme, a);
          final seccion = a.secciones[i - 1];
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              if (seccion.titulo.isNotEmpty) ...[
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 24),
                  child: Text(seccion.titulo, style: theme.textTheme.titleMedium),
                ),
                const SizedBox(height: 12),
              ],
              if (_esDeCanciones(seccion))
                _filasDeCanciones(theme, seccion)
              else
                TiraHorizontal(
                  alto: altoTira,
                  centroDeFlechas: margenDeTarjeta + ladoDeTarjeta / 2,
                  itemCount: seccion.items.length,
                  itemBuilder: (context, j) {
                    final item = seccion.items[j];
                    return TarjetaDeYtItem(
                      item: item,
                      cargando: item.videoId != null &&
                          widget.player.resolviendo == item.videoId,
                      sonando: item.videoId != null &&
                          widget.player.actual?.videoId == item.videoId,
                      onTap: () => unawaited(widget.acciones
                          .pulsar(context, item, hermanas: seccion.items)),
                      onReproducir: item.esCancion || !item.esNavegable
                          ? null
                          : () => unawaited(
                              widget.acciones.reproducirColeccion(context, item)),
                    );
                  },
                ),
              const SizedBox(height: 28),
            ],
          );
        },
      ),
    );
  }

  /// Una sección "de canciones" es la que trae **solo** canciones sueltas.
  /// Con mezclar una sola tarjeta de álbum ya no lo es: la fila numerada
  /// dejaría de tener sentido para ese elemento.
  static bool _esDeCanciones(YtSection s) =>
      s.items.isNotEmpty && s.items.every((it) => it.esCancion);

  Widget _filasDeCanciones(ThemeData theme, YtSection seccion) {
    final sonando = widget.player.actual?.videoId;
    return Column(
      children: [
        for (var i = 0; i < seccion.items.length; i++)
          Builder(builder: (context) {
            final item = seccion.items[i];
            final activa = item.videoId == sonando;
            return Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: ListTile(
                dense: true,
                selected: activa,
                leading: SizedBox(
                  width: 76,
                  child: Row(
                    children: [
                      SizedBox(
                        width: 24,
                        child: widget.player.resolviendo == item.videoId
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(strokeWidth: 2))
                            : activa
                                ? Icon(Icons.graphic_eq,
                                    size: 16, color: theme.colorScheme.primary)
                                : Text('${i + 1}',
                                    textAlign: TextAlign.center,
                                    style: theme.textTheme.bodySmall?.copyWith(
                                        color: theme.colorScheme.onSurfaceVariant)),
                      ),
                      const SizedBox(width: 8),
                      ArtImage(url: item.miniatura, size: 44, radius: 4),
                    ],
                  ),
                ),
                title: Text(item.titulo, maxLines: 1, overflow: TextOverflow.ellipsis),
                subtitle: Text(item.subtitulo,
                    maxLines: 1, overflow: TextOverflow.ellipsis),
                trailing: item.duracion == null
                    ? null
                    : Text(formatoDuracion(item.duracion!),
                        style: theme.textTheme.bodySmall
                            ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
                // `hermanas` es la sección entera: pulsar la tercera de "más
                // escuchadas" pone a sonar esa y deja las demás detrás, en vez
                // de dejar el reproductor con una canción suelta.
                onTap: () => unawaited(widget.acciones
                    .pulsar(context, item, hermanas: seccion.items)),
              ),
            );
          }),
      ],
    );
  }

  Widget _cabecera(ThemeData theme, YtArtista a) {
    // El banner llega apaisado (`=w540-h225`), así que se pinta como banda y
    // no como carátula cuadrada: recortarlo a cuadrado corta caras.
    final miniatura = a.miniatura ?? widget.item.miniatura;
    return Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (miniatura != null)
            // `ancho: double.infinity` no es solo layout: es lo que hace que
            // `ArtImage` mida el hueco real y pida la imagen a esa anchura. Con
            // el widget cuadrado se bajaba una calculada para 180 px de alto y
            // se pintaba a lo ancho de la ventana — se veía pixelada.
            ArtImage(
              url: miniatura,
              size: 180,
              ancho: double.infinity,
              radius: 12,
            ),
          const SizedBox(height: 16),
          Text(a.nombre.isEmpty ? widget.item.titulo : a.nombre,
              style: theme.textTheme.headlineSmall,
              maxLines: 2,
              overflow: TextOverflow.ellipsis),
          if (a.subtitulo.isNotEmpty) ...[
            const SizedBox(height: 4),
            Text(a.subtitulo,
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          ],
          if (a.descripcion != null && a.descripcion!.isNotEmpty) ...[
            const SizedBox(height: 12),
            Text(a.descripcion!,
                maxLines: 4,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          ],
          if (a.playlistAleatorio != null || a.playlistRadio != null) ...[
            const SizedBox(height: 16),
            Row(
              children: [
                if (a.playlistAleatorio != null)
                  FilledButton.icon(
                    onPressed: () =>
                        unawaited(_reproducirLista(a.playlistAleatorio!)),
                    icon: const Icon(Icons.shuffle),
                    label: const Text('Aleatorio'),
                  ),
                if (a.playlistAleatorio != null && a.playlistRadio != null)
                  const SizedBox(width: 12),
                if (a.playlistRadio != null)
                  OutlinedButton.icon(
                    onPressed: () => unawaited(_reproducirLista(a.playlistRadio!)),
                    icon: const Icon(Icons.radio),
                    label: const Text('Mix'),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}
