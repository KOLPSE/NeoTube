import 'dart:async';

import 'package:flutter/material.dart';

import '../core/log.dart';
import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';
import 'art_image.dart';
import 'yt_acciones.dart';

/// Buscador de NeoTube. Devuelve **por categorías** (canciones, listas,
/// álbumes, artistas) y no solo canciones: antes se le pedía a la API el
/// filtro de "solo canciones", así que desde aquí tampoco había forma de
/// llegar a una playlist.
class YtSearchScreen extends StatefulWidget {
  const YtSearchScreen({
    super.key,
    required this.api,
    required this.player,
    required this.acciones,
    this.peticionExterna,
  });

  final YtMusicApi api;
  final YtPlayer player;
  final YtAcciones acciones;

  /// La consulta pedida desde la barra superior. Cambia de valor con cada
  /// Intro; la pantalla se suscribe y repite la búsqueda.
  final ValueNotifier<String?>? peticionExterna;

  @override
  State<YtSearchScreen> createState() => _YtSearchScreenState();
}

sealed class _ElementoDeBusqueda {
  const _ElementoDeBusqueda();
}

class _EncabezadoBusqueda extends _ElementoDeBusqueda {
  const _EncabezadoBusqueda(this.titulo);
  final String titulo;
}

class _ItemBusqueda extends _ElementoDeBusqueda {
  const _ItemBusqueda(this.item);
  final YtItem item;
}

class _SeparadorSeccion extends _ElementoDeBusqueda {
  const _SeparadorSeccion();
}

class _YtSearchScreenState extends State<YtSearchScreen> {
  List<_ElementoDeBusqueda> _elementos = const [];
  bool _buscando = false;
  String? _error;
  bool _buscadoAlgunaVez = false;
  YtFiltroBusqueda _filtro = YtFiltroBusqueda.todo;

  /// La consulta con la que se pintó lo que hay ahora en pantalla.
  ///
  /// Cambiar de filtro tiene que repetir **esa** búsqueda, no lo que haya en
  /// la barra superior: si el usuario dejó escrita otra cosa, pulsar un chip
  /// buscaría una palabra que nunca pidió.
  String _consultaEnPantalla = '';

  @override
  void initState() {
    super.initState();
    widget.peticionExterna?.addListener(_alPedirExterna);
    final inicial = widget.peticionExterna?.value;
    if (inicial != null && inicial.isNotEmpty) {
      unawaited(_buscar(inicial));
    }
  }

  @override
  void didUpdateWidget(YtSearchScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.peticionExterna != widget.peticionExterna) {
      oldWidget.peticionExterna?.removeListener(_alPedirExterna);
      widget.peticionExterna?.addListener(_alPedirExterna);
    }
  }

  @override
  void dispose() {
    widget.peticionExterna?.removeListener(_alPedirExterna);
    super.dispose();
  }

  void _alPedirExterna() {
    final q = widget.peticionExterna?.value;
    if (q != null && q.isNotEmpty) unawaited(_buscar(q));
  }

  static List<_ElementoDeBusqueda> _aplanarResultados(
    List<YtSection> secciones,
  ) {
    final list = <_ElementoDeBusqueda>[];
    for (final seccion in secciones) {
      if (seccion.titulo.isNotEmpty) {
        list.add(_EncabezadoBusqueda(seccion.titulo));
      }
      for (final item in seccion.items) {
        list.add(_ItemBusqueda(item));
      }
      list.add(const _SeparadorSeccion());
    }
    return list;
  }

  /// Cambia de categoría y repite la búsqueda que hay en pantalla.
  void _cambiarFiltro(YtFiltroBusqueda f) {
    if (f == _filtro) return;
    setState(() => _filtro = f);
    if (_consultaEnPantalla.isNotEmpty) unawaited(_buscar(_consultaEnPantalla));
  }

  Future<void> _buscar(String consulta) async {
    final query = consulta.trim();
    if (query.isEmpty) return;
    setState(() {
      _buscando = true;
      _error = null;
      _buscadoAlgunaVez = true;
      _consultaEnPantalla = query;
    });
    try {
      final r = await widget.api.buscar(query, filtro: _filtro);
      if (!mounted) return;
      setState(() => _elementos = _aplanarResultados(r));
    } catch (e) {
      // La API interna no tiene documentación oficial, así que conviene ver el
      // fallo completo en consola — el mensaje en pantalla se recorta para no
      // desbordar el layout.
      Log.w('buscar: fallo completo: $e');
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _buscando = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 24, 24, 8),
          child: Wrap(
            spacing: 8,
            runSpacing: 4,
            children: [
              for (final f in YtFiltroBusqueda.values)
                ChoiceChip(
                  label: Text(f.etiqueta),
                  selected: _filtro == f,
                  // Deshabilitados mientras se busca: pulsar tres chips
                  // seguidos lanzaría tres búsquedas y pintaría la que
                  // conteste antes, que no tiene por qué ser la última pulsada.
                  onSelected: _buscando
                      ? null
                      : (elegido) {
                          if (elegido) _cambiarFiltro(f);
                        },
                ),
            ],
          ),
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24),
            child: Text(
              _error!,
              // La API interna no da errores cortos y legibles — a veces es
              // una página entera. Recortado aquí; el completo va a la consola.
              maxLines: 3,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
        Expanded(child: _resultados(theme)),
      ],
    );
  }

  Widget _resultados(ThemeData theme) {
    if (_elementos.isEmpty) {
      return Center(
        child: Text(
          _buscadoAlgunaVez && !_buscando
              ? 'Sin resultados.'
              : 'Busca desde la barra superior.',
          style: theme.textTheme.bodyMedium?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      );
    }
    return AnimatedBuilder(
      animation: widget.player,
      builder: (context, _) => ListView.builder(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
        itemCount: _elementos.length,
        itemBuilder: (context, index) {
          final elem = _elementos[index];
          return switch (elem) {
            _EncabezadoBusqueda(:final titulo) => Padding(
              padding: const EdgeInsets.only(top: 8, bottom: 4),
              child: Text(titulo, style: theme.textTheme.titleSmall),
            ),
            _ItemBusqueda(:final item) => _fila(theme, item),
            _SeparadorSeccion() => const SizedBox(height: 12),
          };
        },
      ),
    );
  }

  Widget _fila(ThemeData theme, YtItem item) {
    final sonando =
        item.videoId != null && widget.player.actual?.videoId == item.videoId;
    final cargando =
        item.videoId != null && widget.player.resolviendo == item.videoId;
    return ListTile(
      dense: true,
      selected: sonando,
      leading: ArtImage(
        url: item.miniatura,
        size: 48,
        radius: item.tipo == YtTipo.artista ? 24 : 4,
      ),
      title: Text(item.titulo, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: Text(
        item.subtitulo.isEmpty ? _nombreDe(item.tipo) : item.subtitulo,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      trailing: cargando
          ? const SizedBox(
              width: 20,
              height: 20,
              child: CircularProgressIndicator(strokeWidth: 2),
            )
          : Icon(
              sonando ? Icons.graphic_eq : iconoDe(item.tipo),
              color: sonando ? theme.colorScheme.primary : null,
            ),
      onTap: cargando
          ? null
          : () => unawaited(widget.acciones.pulsar(context, item)),
    );
  }

  String _nombreDe(YtTipo t) => switch (t) {
    YtTipo.cancion => 'Canción',
    YtTipo.lista => 'Lista',
    YtTipo.album => 'Álbum',
    YtTipo.artista => 'Artista',
    YtTipo.desconocido => '',
  };
}
