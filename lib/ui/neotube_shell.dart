import 'dart:async';

import 'package:flutter/material.dart';

import '../core/resource_monitor.dart';
import '../core/settings.dart';
import '../core/updater.dart';
import '../core/yt_auth.dart';
import '../core/yt_home_store.dart';
import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';
import 'art_image.dart';
import 'settings_dialog.dart';
import 'yt_acciones.dart';
import 'yt_ajustes.dart';
import 'yt_browse_screen.dart';
import 'yt_login_screen.dart';
import 'yt_playlist_screen.dart';
import 'yt_search_screen.dart';

/// El shell de NeoTube: misma disposición que `AppShell` (barra lateral de
/// 244 px + contenido + barra inferior), navegación al estilo Metrolist.
enum _NeoTubeView { inicio, explorar, biblioteca, buscar, cola }

class NeoTubeShell extends StatefulWidget {
  const NeoTubeShell({
    super.key,
    required this.auth,
    required this.api,
    required this.player,
    required this.ram,
    required this.settings,
    required this.updater,
    required this.onSalirParaActualizar,
    this.onLogout,
  });

  final YtAuth auth;
  final YtMusicApi api;
  final YtPlayer player;

  /// Ajustes es el mismo diálogo en los dos modos (memoria, CPU, modo
  /// rendimiento y actualizaciones son de la app entera, no de un modo), así
  /// que NeoTube necesita las mismas piezas que NeoFy para poder abrirlo.
  final ResourceMonitor ram;
  final Settings settings;
  final Updater updater;
  final Future<void> Function() onSalirParaActualizar;

  final Future<void> Function()? onLogout;

  @override
  State<NeoTubeShell> createState() => _NeoTubeShellState();
}

class _NeoTubeShellState extends State<NeoTubeShell> {
  _NeoTubeView _view = _NeoTubeView.inicio;

  /// La lista o el álbum abierto encima de la pestaña actual, si lo hay. Es
  /// toda la pila de navegación que hace falta: desde una lista solo se
  /// vuelve atrás.
  YtItem? _abierto;

  // Un store por pestaña, creado una vez y no en cada `_content()`: así no se
  // repite la carga cada vez que se cambia de pestaña y se vuelve.
  late final YtHomeStore _inicio = YtHomeStore(
      ({bool forzar = false}) => widget.api.browseSections(YtMusicApi.browseIdInicio, forzar: forzar));
  late final YtHomeStore _explorar = YtHomeStore(
      ({bool forzar = false}) => widget.api.browseSections(YtMusicApi.browseIdExplorar, forzar: forzar));
  // La biblioteca no es un `browseId`, son cuatro: playlists, álbumes,
  // canciones y artistas viven en endpoints distintos.
  late final YtHomeStore _biblioteca = YtHomeStore.progresivo(widget.api.bibliotecaProgresiva);

  late final YtAcciones _acciones = YtAcciones(
    api: widget.api,
    player: widget.player,
    abrir: (item) => setState(() => _abierto = item),
  );

  String? _ultimoError;

  @override
  void initState() {
    super.initState();
    _ultimoError = widget.player.error;
    widget.player.addListener(_onPlayerCambio);
  }

  @override
  void didUpdateWidget(NeoTubeShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.player != widget.player) {
      oldWidget.player.removeListener(_onPlayerCambio);
      widget.player.addListener(_onPlayerCambio);
      _ultimoError = widget.player.error;
    }
  }

  @override
  void dispose() {
    widget.player.removeListener(_onPlayerCambio);
    _inicio.dispose();
    _explorar.dispose();
    _biblioteca.dispose();
    super.dispose();
  }

  void _onPlayerCambio() {
    final err = widget.player.error;
    if (err != null && err != _ultimoError) {
      _ultimoError = err;
      if (mounted) {
        final mensaje = err.startsWith('YtPlayerException: ')
            ? err.substring('YtPlayerException: '.length)
            : err;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('No se pudo reproducir: $mensaje'),
            duration: const Duration(seconds: 4),
            behavior: SnackBarBehavior.floating,
          ),
        );
      }
    } else if (err == null) {
      _ultimoError = null;
    }
  }

  void _irA(_NeoTubeView v) => setState(() {
        _view = v;
        _abierto = null;
      });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          // Escucha al reproductor: "En cola" aparece en cuanto hay algo
          // que enseñar, sin esperar a que algo más redibuje el shell.
          AnimatedBuilder(
            animation: widget.player,
            builder: (context, _) => _Sidebar(
              view: _view,
              hayCola: widget.player.cola.isNotEmpty,
              onSelectView: _irA,
              onLogout: widget.auth.isLoggedIn ? widget.onLogout : null,
              ram: widget.ram,
              settings: widget.settings,
              updater: widget.updater,
              onSalirParaActualizar: widget.onSalirParaActualizar,
            ),
          ),
          const VerticalDivider(width: 1),
          Expanded(child: _content()),
        ],
      ),
      bottomNavigationBar: _BarraInferior(player: widget.player),
    );
  }

  Widget _content() {
    // Y esto se antepone incluso al login: sin libmpv no va a sonar nada,
    // así que mandar al usuario a iniciar sesión sería hacerle perder el
    // tiempo. Solo puede pasar en Linux; ver `YtPlayer.libmpvDisponible`.
    if (!widget.player.disponible) return const _SinLibmpv();
    // Nada de esto sirve sin sesión, así que el login se antepone a la
    // navegación en vez de repetirlo en cada pestaña.
    if (!widget.auth.isLoggedIn) {
      return YtLoginScreen(auth: widget.auth, onLoggedIn: () async => setState(() {}));
    }
    final abierto = _abierto;
    if (abierto != null) {
      return YtPlaylistScreen(
        // La clave evita que al abrir otra lista se reutilice el estado (y las
        // pistas) de la anterior mientras carga la nueva.
        key: ValueKey(abierto.playlistId ?? abierto.browseId),
        item: abierto,
        api: widget.api,
        player: widget.player,
        onVolver: () => setState(() => _abierto = null),
      );
    }
    return switch (_view) {
      _NeoTubeView.inicio => YtBrowseScreen(
          key: const PageStorageKey('inicio'),
          store: _inicio,
          player: widget.player,
          acciones: _acciones,
          tituloVacio: 'No se ha podido traer la portada todavía.',
        ),
      _NeoTubeView.explorar => YtBrowseScreen(
          key: const PageStorageKey('explorar'),
          store: _explorar,
          player: widget.player,
          acciones: _acciones,
          tituloVacio: 'No se ha podido traer nada para explorar todavía.',
        ),
      _NeoTubeView.biblioteca => YtBrowseScreen(
          key: const PageStorageKey('biblioteca'),
          store: _biblioteca,
          player: widget.player,
          acciones: _acciones,
          tituloVacio: 'Tu biblioteca está vacía, o no se ha podido traer.',
        ),
      _NeoTubeView.buscar =>
        YtSearchScreen(api: widget.api, player: widget.player, acciones: _acciones),
      _NeoTubeView.cola => _ColaScreen(player: widget.player),
    };
  }
}

/// Lo que se enseña en NeoTube cuando falta libmpv en el sistema.
///
/// Antes esto no existía porque el caso no llegaba a verse: la app moría en
/// `main()` al inicializar `media_kit` y no aparecía ni la ventana. Ver el
/// try/catch de `main()` y `YtPlayer.libmpvDisponible`.
class _SinLibmpv extends StatelessWidget {
  const _SinLibmpv();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.volume_off_outlined,
                  size: 48, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(height: 16),
              Text('NeoTube no puede reproducir audio',
                  style: theme.textTheme.titleMedium, textAlign: TextAlign.center),
              const SizedBox(height: 12),
              Text(
                YtPlayer.motivoSinLibmpv,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              Text(
                'Sin libmpv NeoTube no puede emitir ningún sonido.',
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.view,
    required this.hayCola,
    required this.onSelectView,
    required this.ram,
    required this.settings,
    required this.updater,
    required this.onSalirParaActualizar,
    this.onLogout,
  });

  final _NeoTubeView view;
  final bool hayCola;
  final void Function(_NeoTubeView) onSelectView;
  final Future<void> Function()? onLogout;
  final ResourceMonitor ram;
  final Settings settings;
  final Updater updater;
  final Future<void> Function() onSalirParaActualizar;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 244,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 16, 16, 8),
            child: Text('NeoTube', style: Theme.of(context).textTheme.titleLarge),
          ),
          _NavTile(
            icon: Icons.home,
            label: 'Inicio',
            selected: view == _NeoTubeView.inicio,
            onTap: () => onSelectView(_NeoTubeView.inicio),
          ),
          _NavTile(
            icon: Icons.explore,
            label: 'Explorar',
            selected: view == _NeoTubeView.explorar,
            onTap: () => onSelectView(_NeoTubeView.explorar),
          ),
          _NavTile(
            icon: Icons.library_music,
            label: 'Biblioteca',
            selected: view == _NeoTubeView.biblioteca,
            onTap: () => onSelectView(_NeoTubeView.biblioteca),
          ),
          _NavTile(
            icon: Icons.search,
            label: 'Buscar',
            selected: view == _NeoTubeView.buscar,
            onTap: () => onSelectView(_NeoTubeView.buscar),
          ),
          if (hayCola)
            _NavTile(
              icon: Icons.playlist_play,
              label: 'En cola',
              selected: view == _NeoTubeView.cola,
              onTap: () => onSelectView(_NeoTubeView.cola),
            ),
          const Spacer(),
          const Divider(height: 1),
          // Mismo pie que la barra lateral de NeoFy: los dos modos se manejan
          // igual, y Ajustes abre el mismo diálogo con su bloque propio.
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
            child: Row(
              children: [
                Expanded(
                  child: TextButton.icon(
                    onPressed: () => unawaited(mostrarAjustes(
                      context,
                      monitor: ram,
                      settings: settings,
                      updater: updater,
                      onSalirParaActualizar: onSalirParaActualizar,
                      // Lo único que cambia entre los dos modos: aquí no hay
                      // salida de audio que reabrir, hay un yt-dlp que puede
                      // faltar.
                      propiosDelModo: const [EstadoDeYtDlp()],
                    )),
                    icon: const Icon(Icons.tune, size: 18),
                    label: const Text('Ajustes'),
                  ),
                ),
                Expanded(
                  child: TextButton.icon(
                    onPressed: onLogout == null ? null : () => unawaited(onLogout!()),
                    icon: const Icon(Icons.logout, size: 18),
                    label: const Text('Salir'),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 4),
        ],
      ),
    );
  }
}

class _NavTile extends StatelessWidget {
  const _NavTile({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ListTile(
      dense: true,
      selected: selected,
      leading: Icon(icon, size: 20),
      title: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis),
      onTap: onTap,
    );
  }
}

/// Lo que hay en la cola y por dónde va. Existe porque desde que se reproduce
/// una lista entera hay algo que enseñar: antes solo había "la canción suelta
/// que has pulsado".
class _ColaScreen extends StatelessWidget {
  const _ColaScreen({required this.player});

  final YtPlayer player;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AnimatedBuilder(
      animation: player,
      builder: (context, _) {
        if (player.cola.isEmpty) {
          return Center(
            child: Text('No hay nada en la cola.',
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          );
        }
        return ListView.builder(
          padding: const EdgeInsets.all(24),
          itemCount: player.cola.length,
          itemBuilder: (context, i) {
            final t = player.cola[i];
            final activa = i == player.indice;
            return ListTile(
              dense: true,
              selected: activa,
              leading: ArtImage(url: t.miniatura, size: 40),
              title: Text(t.titulo, maxLines: 1, overflow: TextOverflow.ellipsis),
              subtitle: Text(t.artista, maxLines: 1, overflow: TextOverflow.ellipsis),
              trailing: activa
                  ? Icon(Icons.graphic_eq, color: theme.colorScheme.primary)
                  : t.duracion == null
                      ? null
                      : Text(formatoDuracion(t.duracion!),
                          style: theme.textTheme.bodySmall
                              ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
              onTap: () => unawaited(player.saltarA(i)),
            );
          },
        );
      },
    );
  }
}

/// Barra inferior con carátula, anterior/play-pausa/siguiente y barra de
/// progreso arrastrable.
class _BarraInferior extends StatelessWidget {
  const _BarraInferior({required this.player});

  final YtPlayer player;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AnimatedBuilder(
      animation: player,
      builder: (context, _) {
        final t = player.actual;
        if (t == null) {
          return Material(
            color: theme.colorScheme.surfaceContainer,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Divider(height: 1),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 20),
                  child: Row(
                    children: [
                      Icon(Icons.music_off,
                          size: 20, color: theme.colorScheme.onSurfaceVariant),
                      const SizedBox(width: 8),
                      Text('Nada sonando en NeoTube',
                          style: theme.textTheme.bodySmall
                              ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
                    ],
                  ),
                ),
              ],
            ),
          );
        }
        final resolviendo = player.resolviendo != null;
        return Material(
          elevation: 8,
          color: theme.colorScheme.surfaceContainerHigh,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Divider(height: 1),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 8, 12, 4),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                Row(
                  children: [
                    Stack(
                      children: [
                        ArtImage(url: t.miniatura, size: 48, radius: 6),
                        if (resolviendo)
                          Positioned.fill(
                            child: Container(
                              decoration: BoxDecoration(
                                color: Colors.black45,
                                borderRadius: BorderRadius.circular(6),
                              ),
                              child: const Center(
                                child: SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                    valueColor: AlwaysStoppedAnimation(Colors.white),
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(t.titulo, maxLines: 1, overflow: TextOverflow.ellipsis),
                          Text(
                            resolviendo ? '${t.artista} • Cargando audio...' : t.artista,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: resolviendo
                                  ? theme.colorScheme.primary
                                  : theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 12),
                    IconButton(
                      icon: const Icon(Icons.shuffle),
                      tooltip: player.aleatorio ? 'Aleatorio: activado' : 'Aleatorio',
                      color: player.aleatorio ? theme.colorScheme.primary : null,
                      onPressed: player.alternarAleatorio,
                    ),
                    IconButton(
                      icon: const Icon(Icons.skip_previous),
                      tooltip: 'Anterior',
                      onPressed:
                          player.puedeVolver ? () => unawaited(player.anterior()) : null,
                    ),
                    if (resolviendo)
                      const Padding(
                        padding: EdgeInsets.all(12),
                        child: SizedBox(
                            width: 24,
                            height: 24,
                            child: CircularProgressIndicator(strokeWidth: 2)),
                      )
                    else
                      StreamBuilder<bool>(
                        stream: player.cambiosDeSonando,
                        initialData: player.sonando,
                        builder: (context, snap) {
                          final sonando = snap.data ?? false;
                          return IconButton.filled(
                            icon: Icon(sonando ? Icons.pause : Icons.play_arrow),
                            tooltip: sonando ? 'Pausar' : 'Reproducir',
                            onPressed: () => unawaited(player.alternar()),
                          );
                        },
                      ),
                    IconButton(
                      icon: const Icon(Icons.skip_next),
                      tooltip: 'Siguiente',
                      onPressed:
                          player.puedeSaltar ? () => unawaited(player.siguiente()) : null,
                    ),
                    const SizedBox(width: 8),
                    _Volumen(player: player),
                  ],
                ),
                _Progreso(player: player),
              ],
            ),
          ),
        ],
      ),
    );
  },
);
  }
}

/// El mando del volumen, con el mismo aspecto que el de NeoFy.
///
/// A diferencia de aquel, este **sí** aplica el cambio en cada paso del
/// arrastre: allí cada paso era una petición a la Web API de Spotify y
/// saturarla acababa en 429 con el volumen parado en un valor al azar; aquí es
/// libmpv en este mismo proceso, instantáneo y gratis. Lo que se retrasa hasta
/// soltar es únicamente **guardarlo** en el config.
class _Volumen extends StatelessWidget {
  const _Volumen({required this.player});

  final YtPlayer player;

  static IconData _icono(int v) {
    if (v <= 0) return Icons.volume_off;
    if (v < 50) return Icons.volume_down;
    return Icons.volume_up;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ValueListenableBuilder<int>(
      valueListenable: player.cambiosDeVolumen,
      builder: (context, vol, _) {
        return SizedBox(
          width: 150,
          child: Row(
            children: [
              Icon(_icono(vol), size: 18),
              Expanded(
                child: SliderTheme(
                  data: SliderTheme.of(context).copyWith(
                    trackHeight: 3,
                    thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
                    overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
                  ),
                  child: Slider(
                    value: vol.toDouble(),
                    max: 100,
                    onChanged: (v) => unawaited(player.setVolumen(v.round())),
                    onChangeEnd: (v) => player.onVolumenFijado?.call(v.round()),
                  ),
                ),
              ),
              SizedBox(
                width: 26,
                child: Text('$vol', style: theme.textTheme.bodySmall),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// La barra de progreso. `StatefulWidget` porque mientras se arrastra manda la
/// posición del dedo y no la del reproductor: si no, el pulgar se vuelve al
/// sitio en cada evento de posición que llega de libmpv.
class _Progreso extends StatefulWidget {
  const _Progreso({required this.player});

  final YtPlayer player;

  @override
  State<_Progreso> createState() => _ProgresoState();
}

class _ProgresoState extends State<_Progreso> {
  double? _arrastrando;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return StreamBuilder<Duration>(
      stream: widget.player.cambiosDePosicion,
      initialData: widget.player.posicion,
      builder: (context, snapPos) {
        final total = widget.player.duracion.inMilliseconds.toDouble();
        final pos = (snapPos.data ?? Duration.zero).inMilliseconds.toDouble();
        if (total <= 0) {
          if (widget.player.resolviendo != null) {
            return const SizedBox(
              height: 24,
              child: Center(
                child: Padding(
                  padding: EdgeInsets.symmetric(horizontal: 16),
                  child: LinearProgressIndicator(minHeight: 2),
                ),
              ),
            );
          }
          return const SizedBox(height: 24);
        }
        final valor = (_arrastrando ?? pos).clamp(0, total).toDouble();
        return Row(
          children: [
            const SizedBox(width: 8),
            Text(formatoDuracion(Duration(milliseconds: valor.round())),
                style: theme.textTheme.labelSmall),
            Expanded(
              child: SliderTheme(
                data: SliderTheme.of(context).copyWith(
                  trackHeight: 3,
                  thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
                  overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
                ),
                child: Slider(
                  value: valor,
                  max: total,
                  onChanged: (v) => setState(() => _arrastrando = v),
                  onChangeEnd: (v) {
                    unawaited(widget.player.seek(Duration(milliseconds: v.round())));
                    setState(() => _arrastrando = null);
                  },
                ),
              ),
            ),
            Text(formatoDuracion(Duration(milliseconds: total.round())),
                style: theme.textTheme.labelSmall),
            const SizedBox(width: 8),
          ],
        );
      },
    );
  }
}
