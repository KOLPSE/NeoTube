import 'dart:async';

import 'package:flutter/material.dart';

import '../core/resource_monitor.dart';
import '../core/settings.dart';
import '../core/updater.dart';
import '../core/yt_auth.dart';
import '../core/yt_favoritos.dart';
import '../core/yt_home_store.dart';
import '../core/yt_models.dart';
import '../core/yt_music_api.dart';
import '../core/yt_player.dart';
import 'art_image.dart';
import 'settings_dialog.dart';
import 'yt_acciones.dart';
import 'yt_artista_screen.dart';
import 'yt_browse_screen.dart';
import 'yt_login_screen.dart';
import 'yt_playlist_screen.dart';
import 'yt_search_screen.dart';
import 'yt_theme.dart';

/// El shell de NeoTube: barra superior (logo + buscador), barra lateral de
/// navegación, contenido y barra de reproducción abajo, como YouTube Music.
enum _NeoTubeView { inicio, explorar, biblioteca, buscar, cola, ajustes }

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

  /// Si la barra lateral está contraída a la guía de iconos (72 px) o abierta
  /// con sus etiquetas. La hamburguesa la conmuta, como en la web.
  bool _sidebarColapsada = false;

  void _alternarSidebar() =>
      setState(() => _sidebarColapsada = !_sidebarColapsada);

  /// Lo que hay abierto encima de la pestaña actual: listas, álbumes y páginas
  /// de artista, en el orden en que se entró.
  ///
  /// Era un solo elemento hasta que existieron las páginas de artista. Ahora
  /// hace falta una pila de verdad porque desde un artista se entra a sus
  /// álbumes, y "volver" desde ahí tiene que devolver **al artista**, no a la
  /// pestaña de la que se venía.
  final List<YtItem> _pila = [];

  // Un store por pestaña, creado una vez y no en cada `_content()`: así no se
  // repite la carga cada vez que se cambia de pestaña y se vuelve.
  late final YtHomeStore _inicio = YtHomeStore(
    ({bool forzar = false}) =>
        widget.api.browseSections(YtMusicApi.browseIdInicio, forzar: forzar),
  );
  late final YtHomeStore _explorar = YtHomeStore(
    ({bool forzar = false}) =>
        widget.api.browseSections(YtMusicApi.browseIdExplorar, forzar: forzar),
  );
  // La biblioteca no es un `browseId`, son cuatro: playlists, álbumes,
  // canciones y artistas viven en endpoints distintos.
  late final YtHomeStore _biblioteca = YtHomeStore.progresivo(
    widget.api.bibliotecaProgresiva,
  );

  late final YtAcciones _acciones = YtAcciones(
    api: widget.api,
    player: widget.player,
    abrir: _apilar,
    abrirArtista: _apilar,
  );

  late final YtFavoritos _favoritos = YtFavoritos(widget.api);
  late final YtColeccionesGuardadas _guardadas = YtColeccionesGuardadas(
    widget.api,
  );

  void _apilar(YtItem item) => setState(() => _pila.add(item));

  void _volver() => setState(() {
    if (_pila.isNotEmpty) _pila.removeLast();
  });

  /// La consulta que se escribe en la barra superior.
  final TextEditingController _busqueda = TextEditingController();

  /// La última consulta pedida desde la barra superior. La pantalla de buscar
  /// se suscribe a esto: cada Intro en la barra dispara una búsqueda nueva.
  final ValueNotifier<String?> _peticionDeBusqueda = ValueNotifier(null);

  void _buscarDesdeLaBarra(String consulta) {
    final limpia = consulta.trim();
    if (limpia.isEmpty) return;
    _peticionDeBusqueda.value = limpia;
    _irA(_NeoTubeView.buscar);
  }

  String? _ultimoError;

  @override
  void initState() {
    super.initState();
    _ultimoError = widget.player.error;
    widget.player.addListener(_onPlayerCambio);
    // En diferido y sin esperarla: el corazón sale vacío hasta que llegue, y
    // llega mucho antes de que dé tiempo a mirarlo.
    unawaited(_favoritos.cargar());
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
    _busqueda.dispose();
    _peticionDeBusqueda.dispose();
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
    _pila.clear();
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          _BarraSuperior(controlador: _busqueda, onBuscar: _buscarDesdeLaBarra),
          Expanded(
            child: Row(
              children: [
                // Escucha al reproductor: "En cola" aparece en cuanto hay algo
                // que enseñar, sin esperar a que algo más redibuje el shell.
                AnimatedBuilder(
                  animation: widget.player,
                  builder: (context, _) => _Sidebar(
                    view: _view,
                    hayCola: widget.player.cola.isNotEmpty,
                    colapsada: _sidebarColapsada,
                    onToggle: _alternarSidebar,
                    onSelectView: _irA,
                    onLogout: widget.auth.isLoggedIn ? widget.onLogout : null,
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(child: _content()),
              ],
            ),
          ),
        ],
      ),
      bottomNavigationBar: _BarraInferior(
        player: widget.player,
        favoritos: _favoritos,
      ),
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
      return YtLoginScreen(
        auth: widget.auth,
        onLoggedIn: () async => setState(() {}),
      );
    }
    if (_pila.isNotEmpty) {
      final abierto = _pila.last;
      // La clave evita que al abrir otra lista se reutilice el estado (y las
      // pistas) de la anterior mientras carga la nueva.
      final clave = ValueKey(
        '${_pila.length}:'
        '${abierto.playlistId ?? abierto.browseId}',
      );
      if (abierto.esArtista) {
        return YtArtistaScreen(
          key: clave,
          item: abierto,
          api: widget.api,
          player: widget.player,
          acciones: _acciones,
          onVolver: _volver,
        );
      }
      return YtPlaylistScreen(
        key: clave,
        item: abierto,
        api: widget.api,
        player: widget.player,
        favoritos: _favoritos,
        guardadas: _guardadas,
        onVolver: _volver,
      );
    }
    return switch (_view) {
      _NeoTubeView.inicio => YtBrowseScreen(
        key: const PageStorageKey('inicio'),
        store: _inicio,
        player: widget.player,
        acciones: _acciones,
        tituloVacio: 'No se ha podido traer la portada.',
      ),
      _NeoTubeView.explorar => YtBrowseScreen(
        key: const PageStorageKey('explorar'),
        store: _explorar,
        player: widget.player,
        acciones: _acciones,
        tituloVacio: 'No se ha podido traer nada para explorar.',
      ),
      _NeoTubeView.biblioteca => YtBrowseScreen(
        key: const PageStorageKey('biblioteca'),
        store: _biblioteca,
        player: widget.player,
        acciones: _acciones,
        tituloVacio: 'Tu biblioteca está vacía, o no se ha podido traer.',
      ),
      _NeoTubeView.buscar => YtSearchScreen(
        api: widget.api,
        player: widget.player,
        acciones: _acciones,
        peticionExterna: _peticionDeBusqueda,
      ),
      _NeoTubeView.cola => _ColaScreen(player: widget.player),
      _NeoTubeView.ajustes => YtAjustesScreen(
        monitor: widget.ram,
        settings: widget.settings,
        updater: widget.updater,
        onSalirParaActualizar: widget.onSalirParaActualizar,
      ),
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
              Icon(
                Icons.volume_off_outlined,
                size: 48,
                color: theme.colorScheme.onSurfaceVariant,
              ),
              const SizedBox(height: 16),
              Text(
                'NeoTube no puede reproducir audio',
                style: theme.textTheme.titleMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 12),
              Text(
                YtPlayer.motivoSinLibmpv,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              Text(
                'Sin libmpv NeoTube no puede emitir ningún sonido.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// El logotipo de YouTube Music: el círculo rojo con el triángulo de play y la
/// palabra "Music", como en la esquina superior izquierda de la web.
class _LogoDeYtMusic extends StatelessWidget {
  const _LogoDeYtMusic();

  @override
  Widget build(BuildContext context) {
    return const YtMusicLogo();
  }
}

/// La barra superior, como en YouTube Music: la caja de búsqueda. El logo vive
/// en el encabezado de la barra lateral, y aquí no hay iconos de adorno.
class _BarraSuperior extends StatelessWidget {
  const _BarraSuperior({required this.controlador, required this.onBuscar});

  final TextEditingController controlador;
  final ValueChanged<String> onBuscar;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      color: theme.colorScheme.surface,
      child: SizedBox(
        height: 64,
        child: Row(
          children: [
            const SizedBox(width: 24),
            Expanded(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 560),
                child: TextField(
                  controller: controlador,
                  onSubmitted: onBuscar,
                  textInputAction: TextInputAction.search,
                  decoration: const InputDecoration(
                    hintText: 'Buscar canciones, álbumes, artistas o podcasts',
                    prefixIcon: Icon(Icons.search),
                  ),
                ),
              ),
            ),
            const SizedBox(width: 24),
          ],
        ),
      ),
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.view,
    required this.hayCola,
    required this.colapsada,
    required this.onToggle,
    required this.onSelectView,
    this.onLogout,
  });

  final _NeoTubeView view;
  final bool hayCola;
  final bool colapsada;
  final VoidCallback onToggle;
  final void Function(_NeoTubeView) onSelectView;
  final Future<void> Function()? onLogout;

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      curve: Curves.easeOutCubic,
      width: colapsada ? 72 : 240,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Encabezado: hamburguesa en una columna FIJA de 72 px (siempre
          // centrada) y el logo "Music" fundido en un `Expanded`. La columna no
          // anima su ancho: solo lo hace el sidebar entero, así la hamburguesa
          // se queda quieta y centrada y no hay nada que desborde a mitad de la
          // transición.
          SizedBox(
            height: 56,
            child: Row(
              children: [
                SizedBox(
                  width: 72,
                  child: Center(
                    child: IconButton(
                      icon: const Icon(Icons.menu, size: 24),
                      onPressed: onToggle,
                    ),
                  ),
                ),
                Expanded(
                  child: ClipRect(
                    child: AnimatedOpacity(
                      opacity: colapsada ? 0 : 1,
                      duration: const Duration(milliseconds: 150),
                      child: const OverflowBox(
                        alignment: Alignment.centerLeft,
                        minWidth: 0,
                        maxWidth: double.infinity,
                        child: Align(
                          alignment: Alignment.centerLeft,
                          child: _LogoDeYtMusic(),
                        ),
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
          _NavTile(
            icon: Icons.home,
            label: 'Inicio',
            selected: view == _NeoTubeView.inicio,
            colapsada: colapsada,
            onTap: () => onSelectView(_NeoTubeView.inicio),
          ),
          _NavTile(
            icon: Icons.explore,
            label: 'Explorar',
            selected: view == _NeoTubeView.explorar,
            colapsada: colapsada,
            onTap: () => onSelectView(_NeoTubeView.explorar),
          ),
          _NavTile(
            icon: Icons.library_music,
            label: 'Mi biblioteca',
            selected: view == _NeoTubeView.biblioteca,
            colapsada: colapsada,
            onTap: () => onSelectView(_NeoTubeView.biblioteca),
          ),
          if (hayCola)
            _NavTile(
              icon: Icons.playlist_play,
              label: 'En cola',
              selected: view == _NeoTubeView.cola,
              colapsada: colapsada,
              onTap: () => onSelectView(_NeoTubeView.cola),
            ),
          const Spacer(),
          const Divider(height: 1),
          _NavTile(
            icon: Icons.tune,
            label: 'Ajustes',
            selected: view == _NeoTubeView.ajustes,
            colapsada: colapsada,
            onTap: () => onSelectView(_NeoTubeView.ajustes),
          ),
          // Salir es una acción, no una vista, pero se dibuja como una fila
          // más para que quede alineado con el resto de la barra lateral.
          _NavTile(
            icon: Icons.logout,
            label: 'Salir',
            selected: false,
            colapsada: colapsada,
            onTap: onLogout == null ? null : () => unawaited(onLogout!()),
          ),
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
    this.colapsada = false,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback? onTap;
  final bool colapsada;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Una sola estructura para los dos estados: el icono vive en una columna
    // FIJA de 72 px (siempre centrado) y la etiqueta se funde dentro de un
    // `Expanded`. Solo el ancho del sidebar anima, así los hijos nunca suman
    // más que el contenedor y no hay desbordamiento en ningún fotograma.
    final tile = InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Container(
          height: 44,
          decoration: BoxDecoration(
            color: selected ? const Color(0x14FFFFFF) : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            children: [
              SizedBox(
                width: 72,
                child: Center(
                  child: Icon(
                    icon,
                    size: 22,
                    color: theme.colorScheme.onSurface,
                  ),
                ),
              ),
              Expanded(
                // `ClipRect`: al contraer, este `Expanded` mide 0 px y el
                // `Padding(right: 12)` de dentro forzaba un mínimo de 12 px que
                // desbordaba el `Row` (el "RIGHT-OVERFLOW"). Recortado, la
                // etiqueta se corta limpia en vez de reventar.
                child: ClipRect(
                  child: AnimatedOpacity(
                    opacity: colapsada ? 0 : 1,
                    duration: const Duration(milliseconds: 150),
                    child: Padding(
                      padding: const EdgeInsets.only(right: 12),
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodyMedium,
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    // Contraída no hay etiqueta a la vista: el tooltip dice qué es cada icono.
    if (colapsada) {
      return Tooltip(
        message: label,
        waitDuration: const Duration(milliseconds: 400),
        child: tile,
      );
    }
    return tile;
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
            child: Text(
              'No hay nada en la cola.',
              style: theme.textTheme.bodyMedium?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
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
              title: Text(
                t.titulo,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              subtitle: Text(
                t.artista,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              trailing: activa
                  ? Icon(Icons.graphic_eq, color: theme.colorScheme.primary)
                  : t.duracion == null
                  ? null
                  : Text(
                      formatoDuracion(t.duracion!),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
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
  const _BarraInferior({required this.player, required this.favoritos});

  final YtPlayer player;
  final YtFavoritos favoritos;

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
                  padding: const EdgeInsets.symmetric(
                    horizontal: 16,
                    vertical: 20,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        Icons.music_off,
                        size: 20,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                      const SizedBox(width: 8),
                      Text(
                        'Nada sonando en NeoTube',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
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
                                        valueColor: AlwaysStoppedAnimation(
                                          Colors.white,
                                        ),
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
                              Text(
                                t.titulo,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                              Text(
                                resolviendo
                                    ? '${t.artista} • Cargando audio...'
                                    : t.artista,
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
                        _BotonFavorita(favoritos: favoritos, pista: t),
                        const SizedBox(width: 4),
                        IconButton(
                          icon: const Icon(Icons.shuffle),
                          tooltip: player.aleatorio
                              ? 'Aleatorio: activado'
                              : 'Aleatorio',
                          color: player.aleatorio
                              ? theme.colorScheme.primary
                              : null,
                          onPressed: player.alternarAleatorio,
                        ),
                        _BotonRepetir(player: player),
                        IconButton(
                          icon: const Icon(Icons.skip_previous),
                          tooltip: 'Anterior',
                          onPressed: player.puedeVolver
                              ? () => unawaited(player.anterior())
                              : null,
                        ),
                        if (resolviendo)
                          const Padding(
                            padding: EdgeInsets.all(12),
                            child: SizedBox(
                              width: 24,
                              height: 24,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          )
                        else
                          StreamBuilder<bool>(
                            stream: player.cambiosDeSonando,
                            initialData: player.sonando,
                            builder: (context, snap) {
                              final sonando = snap.data ?? false;
                              return IconButton.filled(
                                icon: Icon(
                                  sonando ? Icons.pause : Icons.play_arrow,
                                ),
                                tooltip: sonando ? 'Pausar' : 'Reproducir',
                                onPressed: () => unawaited(player.alternar()),
                              );
                            },
                          ),
                        IconButton(
                          icon: const Icon(Icons.skip_next),
                          tooltip: 'Siguiente',
                          onPressed: player.puedeSaltar
                              ? () => unawaited(player.siguiente())
                              : null,
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

/// El botón de repetir: un solo icono que recorre los tres modos.
///
/// Se distinguen como los distingue cualquier reproductor y como los distingue
/// el ojo sin leer nada: **apagado** en gris (no repetir), **encendido** en el
/// color de marca (repetir la cola) y encendido **con el icono del «1»**
/// (repetir esta canción). Sin ese cambio de icono, "repetir todo" y "repetir
/// una" se ven idénticos y hay que pulsar para averiguar en cuál estás.
class _BotonRepetir extends StatelessWidget {
  const _BotonRepetir({required this.player});

  final YtPlayer player;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final modo = player.repeticion;
    final activo = modo != YtRepeticion.ninguna;
    return IconButton(
      icon: Icon(modo == YtRepeticion.pista ? Icons.repeat_one : Icons.repeat),
      color: activo ? theme.colorScheme.primary : null,
      tooltip: switch (modo) {
        YtRepeticion.ninguna => 'Repetir: desactivado',
        YtRepeticion.cola => 'Repetir: toda la cola',
        YtRepeticion.pista => 'Repetir: esta canción',
      },
      onPressed: player.alternarRepeticion,
    );
  }
}

/// El corazón de "me gusta", a la derecha del nombre de la canción.
///
/// Lo que marca aquí va a la lista **Me gusta** de la cuenta de YouTube
/// Music — la que se genera sola y ya se ve en Biblioteca —, no a una lista
/// aparte de NeoTube: es el mismo like del cliente oficial.
class _BotonFavorita extends StatelessWidget {
  const _BotonFavorita({required this.favoritos, required this.pista});

  final YtFavoritos favoritos;
  final YtTrack pista;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AnimatedBuilder(
      animation: favoritos,
      builder: (context, _) {
        final marcada = favoritos.esFavorita(pista.videoId);
        final ocupada = favoritos.estaOcupada(pista.videoId);
        return IconButton(
          icon: Icon(marcada ? Icons.favorite : Icons.favorite_border),
          color: marcada ? theme.colorScheme.primary : null,
          tooltip: marcada ? 'Quitar de Me gusta' : 'Me gusta',
          onPressed: ocupada
              ? null
              : () async {
                  final ok = await favoritos.alternar(pista);
                  if (ok || !context.mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('No se pudo cambiar Me gusta.'),
                    ),
                  );
                },
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
                    thumbShape: const RoundSliderThumbShape(
                      enabledThumbRadius: 6,
                    ),
                    overlayShape: const RoundSliderOverlayShape(
                      overlayRadius: 12,
                    ),
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

  /// El rojo de lo ya escuchado. Es el rojo de progreso de YouTube Music
  /// (`--ytmusic-color-youtubered: #f00`), distinto del rojo de marca con el
  /// que se siembra el tema: la barra necesita leerse como roja en claro y en
  /// oscuro para distinguirse del gris de la descarga.
  static const _rojoEscuchado = ytRojoProgreso;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // `AnimatedBuilder` además del `StreamBuilder` de más abajo: la posición
    // llega por su propio stream, pero `progresoDescargaRepuesto` se avisa
    // con el `notifyListeners()` normal de `YtPlayer`, y sin escucharlo aquí
    // la franja de la descarga de repuesto no se repintaría sola.
    return AnimatedBuilder(
      animation: widget.player,
      builder: (context, _) {
        return StreamBuilder<Duration>(
          stream: widget.player.cambiosDePosicion,
          initialData: widget.player.posicion,
          builder: (context, snapPos) {
            final total = widget.player.duracion.inMilliseconds.toDouble();
            final pos = (snapPos.data ?? Duration.zero).inMilliseconds
                .toDouble();
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
            final progresoDescarga = widget.player.progresoDescargaRepuesto;
            return Row(
              children: [
                const SizedBox(width: 8),
                Text(
                  formatoDuracion(Duration(milliseconds: valor.round())),
                  style: theme.textTheme.labelSmall,
                ),
                Expanded(
                  child: Stack(
                    alignment: Alignment.centerLeft,
                    children: [
                      // La franja de la descarga de repuesto, en gris claro
                      // para que **no se confunda con el rojo de lo ya
                      // escuchado**: son dos cosas distintas (cuánto hay a
                      // salvo en disco contra por dónde va la canción) y
                      // antes las dos iban del color de marca. La pinta esta
                      // capa de abajo porque el `Slider` de encima, con su
                      // tramo inactivo transparente, deja verla en la parte
                      // que aún no ha sonado y la tapa sola en la que ya sonó.
                      if (progresoDescarga != null)
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                          // `progresoDescargaRepuesto` ya llega suave (una
                          // estimación por tiempo, no el tamaño real
                          // descargado — ver el porqué en
                          // `YtPlayer._descargarCompletaConYtDlp`), a pasos
                          // de 150 ms; este `TweenAnimationBuilder` solo
                          // interpola entre esos pasos para que no se note
                          // el escalón.
                          child: TweenAnimationBuilder<double>(
                            tween: Tween(begin: 0, end: progresoDescarga),
                            duration: const Duration(milliseconds: 150),
                            curve: Curves.linear,
                            builder: (context, valorAnimado, _) =>
                                FractionallySizedBox(
                                  alignment: Alignment.centerLeft,
                                  widthFactor: valorAnimado,
                                  child: Container(
                                    height: 4,
                                    decoration: BoxDecoration(
                                      color: theme.colorScheme.onSurface
                                          .withValues(alpha: 0.38),
                                      borderRadius: BorderRadius.circular(2),
                                    ),
                                  ),
                                ),
                          ),
                        ),
                      SliderTheme(
                        data: SliderTheme.of(context).copyWith(
                          trackHeight: 3,
                          thumbShape: const RoundSliderThumbShape(
                            enabledThumbRadius: 6,
                          ),
                          overlayShape: const RoundSliderOverlayShape(
                            overlayRadius: 12,
                          ),
                          // Rojo explícito para lo ya escuchado, no el
                          // `primary` del tema: en claro y en oscuro el tema
                          // lo tiñe distinto, y aquí importa que se lea como
                          // rojo siempre para contrastar con el gris de la
                          // descarga.
                          activeTrackColor: _rojoEscuchado,
                          thumbColor: _rojoEscuchado,
                          inactiveTrackColor: progresoDescarga == null
                              ? null
                              : theme.colorScheme.onSurface.withValues(
                                  alpha: 0.08,
                                ),
                        ),
                        child: Slider(
                          value: valor,
                          max: total,
                          onChanged: (v) => setState(() => _arrastrando = v),
                          onChangeEnd: (v) {
                            unawaited(
                              widget.player.seek(
                                Duration(milliseconds: v.round()),
                              ),
                            );
                            setState(() => _arrastrando = null);
                          },
                        ),
                      ),
                    ],
                  ),
                ),
                Text(
                  formatoDuracion(Duration(milliseconds: total.round())),
                  style: theme.textTheme.labelSmall,
                ),
                const SizedBox(width: 8),
              ],
            );
          },
        );
      },
    );
  }
}
