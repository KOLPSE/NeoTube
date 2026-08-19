import 'dart:async';
import 'dart:io';

import 'package:desktop_webview_window/desktop_webview_window.dart';
import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart' hide Track;
import 'package:path/path.dart' as p;
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'core/app_config.dart';
import 'core/art_cache.dart';
import 'core/discord_rpc.dart';
import 'core/media_keys.dart';
import 'core/mpris.dart';
import 'core/resource_monitor.dart';
import 'core/settings.dart';
import 'core/smtc.dart';
import 'core/updater.dart';
import 'core/yt_auth.dart';
import 'core/yt_models.dart';
import 'core/yt_music_api.dart';
import 'core/yt_player.dart';
import 'ui/atajos.dart';
import 'ui/neotube_shell.dart';

const _lockPort = 53331;

/// Intenta quedarse con la instancia principal sin confundir un puerto ajeno
/// con una instancia existente de NeoTube.
Future<bool> _instanciaUnica() async {
  try {
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, _lockPort);
    server.listen((socket) {
      unawaited(socket.drain<void>().catchError((_) {}));
      unawaited(windowManager.show());
      unawaited(windowManager.focus());
      socket.destroy();
    });
    return true;
  } catch (_) {
    try {
      final socket = await Socket.connect(
        InternetAddress.loopbackIPv4,
        _lockPort,
        timeout: const Duration(seconds: 2),
      );
      socket.write('show');
      await socket.flush();
      socket.destroy();
      return false;
    } catch (_) {
      // Otro programa puede haber ocupado el puerto. Sin una instancia a la
      // que despertar, NeoTube sigue adelante en vez de desaparecer en silencio.
      return true;
    }
  }
}

Future<void> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();

  // desktop_webview_window relanza este ejecutable para la barra de título de
  // su ventana de login; no es una segunda instancia de NeoTube.
  if (runWebViewTitleBarWidget(args)) return;

  try {
    MediaKit.ensureInitialized();
  } catch (e) {
    YtPlayer.libmpvDisponible = false;
    // Esta cadena la comprueba el CI de Linux.
    debugPrint('[NeoTube] sin libmpv, no se podra reproducir: $e');
  }

  if (!await _instanciaUnica()) exit(0);

  await windowManager.ensureInitialized();
  const options = WindowOptions(
    size: Size(1000, 680),
    minimumSize: Size(820, 560),
    center: true,
    title: 'NeoTube',
  );
  await windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.show();
    await windowManager.focus();
  });
  await windowManager.setPreventClose(true);

  final config = await AppConfig.load();
  unawaited(ArtCache.prune());

  final auth = YtAuth();
  await auth.loadStored();

  runApp(NeoTubeApp(config: config, auth: auth));
}

const _seedNeoTube = Color(0xFFE53935);

class NeoTubeApp extends StatelessWidget {
  const NeoTubeApp({super.key, required this.config, required this.auth});

  final AppConfig config;
  final YtAuth auth;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'NeoTube',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: _seedNeoTube),
        useMaterial3: true,
      ),
      darkTheme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: _seedNeoTube,
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
      themeMode: ThemeMode.system,
      home: _NeoTubeRoot(config: config, auth: auth),
    );
  }
}

class _NeoTubeRoot extends StatefulWidget {
  const _NeoTubeRoot({required this.config, required this.auth});

  final AppConfig config;
  final YtAuth auth;

  @override
  State<_NeoTubeRoot> createState() => _NeoTubeRootState();
}

class _NeoTubeRootState extends State<_NeoTubeRoot> with WindowListener, TrayListener {
  late final YtMusicApi _api = YtMusicApi(widget.auth);
  late final YtPlayer _player = YtPlayer(
    volumenInicial: widget.config.volumenNeoTube,
    config: widget.config,
  );
  late final ResourceMonitor _ram = ResourceMonitor(
    pidsDeSidecars: () => const <int?>[],
  );
  late final Settings _settings = Settings(widget.config)
    ..onCambioDiscord = _alCambiarDiscord;
  late final DiscordRpc _discordRpc = DiscordRpc();
  final Updater _updater = Updater();

  bool _bandejaDisponible = false;

  late final MediaKeys _mediaKeys = MediaKeys(
    onPlayPause: () => _mandoDeFuera('playPause', _alternar),
    onNext: () => _mandoDeFuera('next', _siguiente),
    onPrevious: () => _mandoDeFuera('previous', _anterior),
    onPause: () => _mandoDeFuera('pause', _pausar),
  );

  Future<void> _alternar() => _player.alternar();
  Future<void> _siguiente() => _player.siguiente();
  Future<void> _anterior() => _player.anterior();
  Future<void> _pausar() => _player.pause();
  Future<void> _reproducirSiEstaParado() async {
    if (_player.sonando) return;
    await _player.alternar();
  }
  Future<void> _saltarA(int ms) => _player.seek(Duration(milliseconds: ms));

  late final SmtcService _smtc = SmtcService(
    onPlayPause: () => _mandoDeFuera('playPause', _alternar),
    onPlay: () => _mandoDeFuera('play', _reproducirSiEstaParado),
    onPause: () => _mandoDeFuera('pause', _pausar),
    onNext: () => _mandoDeFuera('next', _siguiente),
    onPrevious: () => _mandoDeFuera('previous', _anterior),
    onSeek: (ms) => _mandoDeFuera('seek', () => _saltarA(ms)),
    estado: _estadoParaElSistema,
  );

  late final MprisService _mpris = MprisService(
    onPlayPause: _alternar,
    onPlay: _reproducirSiEstaParado,
    onPause: _pausar,
    onNext: _siguiente,
    onPrevious: _anterior,
    onSeek: (us) => _saltarA(us ~/ 1000),
    onRaise: () {
      unawaited(windowManager.show());
      unawaited(windowManager.focus());
    },
    estado: _estadoParaElSistema,
  );

  EstadoDelSistema _estadoParaElSistema() => EstadoDelSistema(
        track: _player.actual,
        sonando: _player.sonando,
        posicionMs: _player.posicion.inMilliseconds,
        puedeSaltar: _player.puedeSaltar,
        puedeVolver: _player.puedeVolver,
        volumen: _player.volumen,
      );

  DateTime _ultimoMando = DateTime.fromMillisecondsSinceEpoch(0);
  String? _ultimoMandoQue;

  Future<void> _mandoDeFuera(String que, Future<void> Function() accion) async {
    final ahora = DateTime.now();
    if (que == _ultimoMandoQue &&
        ahora.difference(_ultimoMando) < const Duration(milliseconds: 250)) {
      return;
    }
    _ultimoMandoQue = que;
    _ultimoMando = ahora;
    await accion();
  }

  void _avisarAlSistema() {
    _mpris.notificarCambio();
    _smtc.notificarCambio();
    _discordRpc.actualizarActividad(
      track: _player.actual,
      sonando: _player.sonando,
      progresoMs: _player.posicion.inMilliseconds,
      duracionMs: _player.duracion.inMilliseconds,
      obtenerCola: _colaDiscord,
    );
  }

  Future<List<YtTrack>> _colaDiscord() async {
    final i = _player.indice;
    if (i < 0 || i + 1 >= _player.cola.length) return const [];
    return _player.cola.sublist(i + 1);
  }

  void _alCambiarDiscord(bool activo, String clientId) {
    if (activo && clientId.isNotEmpty) {
      _discordRpc.start(clientId);
      _avisarAlSistema();
    } else {
      unawaited(_discordRpc.stop());
    }
  }

  StreamSubscription<bool>? _subSonando;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    trayManager.addListener(this);
    _mediaKeys.start();
    unawaited(_mpris.start());
    _smtc.start();
    if (_settings.discordRpcEnabled && _settings.discordClientId.isNotEmpty) {
      _discordRpc.start(_settings.discordClientId);
    }
    _player.addListener(_avisarAlSistema);
    _subSonando = _player.cambiosDeSonando.listen((_) => _avisarAlSistema());
    _player.onSalto = (ms) {
      _mpris.notificarSalto(ms * 1000);
      _smtc.notificarSalto(ms);
    };
    _player.onVolumenFijado = (volumen) {
      widget.config.volumenNeoTube = volumen;
      unawaited(widget.config.save());
    };
    // No hay otro reproductor que pausar en esta aplicación independiente.
    _player.alEmpezarAReproducir = null;
    _ram.start();
    unawaited(_setupTray());
  }

  Future<void> _logout() async {
    await _player.stop();
    await widget.auth.logout();
    if (mounted) setState(() {});
  }

  Future<void> _quit() async {
    await _player.stop();
    await _discordRpc.stop();
    await _mpris.stop();
    await _smtc.stop();
    try {
      await trayManager.destroy();
    } catch (_) {}
    await windowManager.setPreventClose(false);
    await windowManager.destroy();
  }

  // ------------------------------------------------------------------ bandeja

  Future<void> _setupTray() async {
    try {
      final icon = _trayIconPath();
      if (icon == null) {
        debugPrint('Bandeja: no se encuentra el icono; la X cerrará la app.');
        return;
      }
      await trayManager.setIcon(icon);
      await _refreshTrayMenu();
      if (mounted) setState(() => _bandejaDisponible = true);
    } catch (e) {
      debugPrint('Bandeja no disponible ($e); la X cerrará la app.');
      return;
    }

    if (Platform.isWindows) {
      try {
        await trayManager.setToolTip('NeoTube');
      } catch (_) {}
    }
  }

  String? _trayIconPath() {
    final nombre = Platform.isWindows ? 'tray.ico' : 'tray.png';
    final exeDir = p.dirname(Platform.resolvedExecutable);
    for (final c in [
      p.join(exeDir, 'data', 'flutter_assets', 'assets', nombre),
      p.join(Directory.current.path, 'assets', nombre),
    ]) {
      if (File(c).existsSync()) return c;
    }
    return null;
  }

  Future<void> _refreshTrayMenu() async {
    try {
      await trayManager.setContextMenu(Menu(items: [
        MenuItem(key: 'show', label: 'Mostrar NeoTube'),
        MenuItem(key: 'exit', label: 'Salir'),
      ]));
    } catch (_) {}
  }

  @override
  void onTrayIconMouseDown() {
    windowManager.show();
    windowManager.focus();
  }

  @override
  void onTrayIconRightMouseDown() {
    if (!Platform.isWindows) return;
    trayManager.popUpContextMenu();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case 'show':
        windowManager.show();
        windowManager.focus();
      case 'exit':
        unawaited(_quit());
    }
  }

  // ------------------------------------------------------------------ ventana

  @override
  void onWindowClose() {
    if (!_bandejaDisponible) {
      unawaited(_quit());
      return;
    }
    unawaited(windowManager.hide());
    _alEsconderse();
  }

  @override
  void onWindowMinimize() => _alEsconderse();

  void _alEsconderse() {
    final cache = PaintingBinding.instance.imageCache;
    cache.clear();
    cache.clearLiveImages();
    ResourceMonitor.devolverMemoriaAlSistema(const <int?>[]);
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    _mediaKeys.stop();
    _player.removeListener(_avisarAlSistema);
    _player.onSalto = null;
    unawaited(_subSonando?.cancel());
    unawaited(_mpris.stop());
    unawaited(_smtc.stop());
    _player.dispose();
    _ram.dispose();
    _settings.dispose();
    _discordRpc.dispose();
    _updater.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AtajosDeReproduccion(
      onPlayPause: () => unawaited(_player.alternar()),
      onNext: () => unawaited(_player.siguiente()),
      onPrevious: () => unawaited(_player.anterior()),
      child: NeoTubeShell(
        auth: widget.auth,
        api: _api,
        player: _player,
        ram: _ram,
        settings: _settings,
        updater: _updater,
        onSalirParaActualizar: _quit,
        onLogout: _logout,
      ),
    );
  }
}
