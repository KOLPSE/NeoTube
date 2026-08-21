import 'dart:async';

import 'package:flutter/material.dart';

import '../core/yt_favoritos.dart';
import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';
import 'art_image.dart';
import 'yt_acciones.dart';

/// El contenido de una lista, un álbum o una mezcla: cabecera con carátula,
/// botón de reproducir todo y las pistas en orden.
///
/// Es la pantalla que faltaba entera. Sin ella la portada y la biblioteca
/// pintaban listas que no llevaban a ningún sitio, y de ahí que "solo se
/// pudieran reproducir canciones sueltas".
class YtPlaylistScreen extends StatefulWidget {
  const YtPlaylistScreen({
    super.key,
    required this.item,
    required this.api,
    required this.player,
    required this.favoritos,
    required this.onVolver,
  });

  final YtItem item;
  final YtMusicApi api;
  final YtPlayer player;
  final YtFavoritos favoritos;
  final VoidCallback onVolver;

  @override
  State<YtPlaylistScreen> createState() => _YtPlaylistScreenState();
}

class _YtPlaylistScreenState extends State<YtPlaylistScreen> {
  YtColeccion? _coleccion;
  bool _cargando = true;
  bool _cargandoMas = false;
  String? _error;
  StreamSubscription<YtColeccion>? _sub;

  /// El `videoId` que acaba de entrar en la lista y aún se está animando.
  String? _recienAnadida;

  /// La última [YtFavoritos.revision] ya reflejada aquí, para no repetir el
  /// mismo cambio en cada `notifyListeners`.
  int _revisionAplicada = 0;

  /// ¿Es *esta* la lista de "Me gusta"? Solo ahí tiene sentido que dar like
  /// añada una fila: en un álbum o en una playlist cualquiera, el like no
  /// cambia lo que se está mirando.
  bool get _esListaDeFavoritos {
    final id = widget.item.playlistId ?? widget.item.browseId ?? '';
    final limpio = id.startsWith('VL') ? id.substring(2) : id;
    return limpio == 'LM';
  }

  @override
  void initState() {
    super.initState();
    _revisionAplicada = widget.favoritos.revision;
    widget.favoritos.addListener(_onFavoritosCambio);
    _cargar();
  }

  /// Refleja en la lista abierta el like que se acaba de dar o quitar.
  ///
  /// Antes había que salir de la lista y volver a entrar para verlo: la
  /// colección se pide una vez y se queda como está, y el like solo tocaba
  /// el conjunto de favoritos. Aquí se aplica el cambio sobre las pistas que
  /// ya hay en pantalla, sin volver a pedir nada.
  void _onFavoritosCambio() {
    if (!mounted || !_esListaDeFavoritos) return;
    final favs = widget.favoritos;
    if (favs.revision == _revisionAplicada) return;
    _revisionAplicada = favs.revision;
    final cambio = favs.ultimoCambio;
    final c = _coleccion;
    if (cambio == null || c == null) return;

    final pistas = [...c.pistas];
    final estaba = pistas.indexWhere((t) => t.videoId == cambio.pista.videoId);
    if (cambio.anadida) {
      if (estaba != -1) return;
      // Arriba del todo, que es donde la pone YouTube Music: lo último que
      // marcas es lo primero que ves.
      pistas.insert(0, cambio.pista);
      _recienAnadida = cambio.pista.videoId;
    } else {
      if (estaba == -1) return;
      pistas.removeAt(estaba);
      if (_recienAnadida == cambio.pista.videoId) _recienAnadida = null;
    }
    setState(() {
      _coleccion = YtColeccion(
        titulo: c.titulo,
        subtitulo: c.subtitulo,
        miniatura: c.miniatura,
        pistas: pistas,
      );
    });
  }

  @override
  void didUpdateWidget(YtPlaylistScreen old) {
    super.didUpdateWidget(old);
    if (old.item.playlistId != widget.item.playlistId ||
        old.item.browseId != widget.item.browseId) {
      _sub?.cancel();
      _sub = null;
      _cargar();
    }
  }

  @override
  void dispose() {
    widget.favoritos.removeListener(_onFavoritosCambio);
    _sub?.cancel();
    _sub = null;
    super.dispose();
  }

  void _cargar({bool forzar = false}) {
    _sub?.cancel();
    setState(() {
      _cargando = true;
      _cargandoMas = false;
      _error = null;
      _coleccion = null;
    });

    _sub = widget.api
        .coleccionProgresiva(
          playlistId: widget.item.playlistId,
          browseId: widget.item.browseId,
          forzar: forzar,
        )
        .listen(
      (c) {
        if (!mounted) return;
        setState(() {
          _coleccion = c;
          _cargando = false;
          _cargandoMas = true;
        });
      },
      onError: (e) {
        debugPrint('[NeoTube lista ${widget.item.playlistId ?? widget.item.browseId}] $e');
        if (!mounted) return;
        setState(() {
          _error = '$e';
          _cargando = false;
          _cargandoMas = false;
        });
      },
      onDone: () {
        if (!mounted) return;
        setState(() {
          _cargando = false;
          _cargandoMas = false;
        });
      },
    );
  }

  Future<void> _reproducirDesde(int i) async {
    final pistas = _coleccion?.pistas;
    if (pistas == null || pistas.isEmpty) return;
    try {
      await widget.player.reproducirLista(
        pistas,
        desde: i,
        contexto: widget.item.playlistId ?? widget.item.browseId,
      );
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('No se pudo reproducir: $e')));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final c = _coleccion;
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
                  c?.titulo.isNotEmpty == true ? c!.titulo : widget.item.titulo,
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
                  ? _mensaje(theme, _error!, esError: true)
                  : c == null || c.pistas.isEmpty
                      ? _mensaje(theme, 'Esta lista no tiene canciones.')
                      : _lista(theme, c),
        ),
      ],
    );
  }

  Widget _mensaje(ThemeData theme, String texto, {bool esError = false}) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                texto,
                textAlign: TextAlign.center,
                maxLines: 5,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: esError ? theme.colorScheme.error : theme.colorScheme.onSurfaceVariant,
                ),
              ),
              if (esError) ...[
                const SizedBox(height: 16),
                OutlinedButton.icon(
                  onPressed: () => _cargar(forzar: true),
                  icon: const Icon(Icons.refresh),
                  label: const Text('Reintentar'),
                ),
              ],
            ],
          ),
        ),
      );

  Widget _lista(ThemeData theme, YtColeccion c) {
    return AnimatedBuilder(
      animation: widget.player,
      builder: (context, _) {
        final sonando = widget.player.actual?.videoId;
        final totalItems = c.pistas.length + 1 + (_cargandoMas ? 1 : 0);
        return ListView.builder(
          padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
          // Uno más para la cabecera (+1 si aún carga más trozos)
          itemCount: totalItems,
          itemBuilder: (context, i) {
            if (i == 0) return _cabecera(theme, c);
            if (_cargandoMas && i == totalItems - 1) {
              return const Padding(
                padding: EdgeInsets.symmetric(vertical: 16),
                child: Center(
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                      SizedBox(width: 8),
                      Text('Cargando más canciones...'),
                    ],
                  ),
                ),
              );
            }
            final t = c.pistas[i - 1];
            final activa = t.videoId == sonando;
            final fila = ListTile(
              dense: true,
              selected: activa,
              leading: SizedBox(
                width: 44,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    if (widget.player.resolviendo == t.videoId)
                      const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2))
                    else if (activa)
                      Icon(Icons.graphic_eq, size: 18, color: theme.colorScheme.primary)
                    else
                      Text('$i',
                          style: theme.textTheme.bodySmall
                              ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
                  ],
                ),
              ),
              title: Text(t.titulo, maxLines: 1, overflow: TextOverflow.ellipsis),
              subtitle: Text(t.artista, maxLines: 1, overflow: TextOverflow.ellipsis),
              trailing: t.duracion == null
                  ? null
                  : Text(formatoDuracion(t.duracion!),
                      style: theme.textTheme.bodySmall
                          ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
              onTap: () => unawaited(_reproducirDesde(i - 1)),
            );
            if (t.videoId != _recienAnadida) return fila;
            // La recién añadida entra creciendo desde altura cero y bajando
            // un poco: al ocupar sitio empuja al resto hacia abajo sola, que
            // es el efecto que se busca sin tener que animar las demás filas
            // ni cambiar el `ListView.builder` por un `AnimatedList`.
            return TweenAnimationBuilder<double>(
              key: ValueKey('nueva-${t.videoId}'),
              tween: Tween(begin: 0, end: 1),
              duration: const Duration(milliseconds: 420),
              curve: Curves.easeOutCubic,
              onEnd: () {
                if (mounted && _recienAnadida == t.videoId) {
                  setState(() => _recienAnadida = null);
                }
              },
              builder: (context, v, child) => Align(
                alignment: Alignment.bottomCenter,
                heightFactor: v,
                child: Opacity(
                  opacity: v,
                  child: Transform.translate(
                    offset: Offset(0, -16 * (1 - v)),
                    child: child,
                  ),
                ),
              ),
              child: fila,
            );
          },
        );
      },
    );
  }

  Widget _cabecera(ThemeData theme, YtColeccion c) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 20),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ArtImage(url: c.miniatura ?? widget.item.miniatura, size: 128, radius: 10),
          const SizedBox(width: 20),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(c.titulo.isEmpty ? widget.item.titulo : c.titulo,
                    style: theme.textTheme.headlineSmall,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis),
                const SizedBox(height: 4),
                Text(
                  c.subtitulo.isEmpty ? '${c.pistas.length} canciones' : c.subtitulo,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                ),
                const SizedBox(height: 16),
                Row(
                  children: [
                    FilledButton.icon(
                      onPressed: () => unawaited(_reproducirDesde(0)),
                      icon: const Icon(Icons.play_arrow),
                      label: const Text('Reproducir'),
                    ),
                    const SizedBox(width: 12),
                    OutlinedButton.icon(
                      onPressed: () {
                        // El aleatorio es un modo persistente (yt_player.dart),
                        // no "barajar esta lista una vez": si no está encendido,
                        // se enciende, y reproducirLista baraja el resto sola.
                        if (!widget.player.aleatorio) widget.player.alternarAleatorio();
                        unawaited(_reproducirDesde(0));
                      },
                      icon: const Icon(Icons.shuffle),
                      label: const Text('Aleatorio'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
