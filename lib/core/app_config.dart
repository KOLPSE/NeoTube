import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Versión de NeoTube.
///
/// Esta constante es la única fuente de verdad para el instalador y el
/// actualizador.
const String kVersion = '1.1.0';

/// Repositorio de donde salen las actualizaciones.
const String kRepoGitHub = 'KOLPSE/NeoTube';

/// Client ID de la aplicación de Discord de NeoTube (Developer Portal).
const String kDiscordClientId = '1538661228040298556';

Directory? _appDataDir;
Directory? _cacheDir;

/// Dónde viven los datos que no se pueden perder: `config.json` y las cookies
/// de sesión.
///
/// - Windows: `%APPDATA%\neotube`.
/// - Linux: `$XDG_CONFIG_HOME/neotube`, o `~/.config/neotube` si no está definida.
Directory appDataDir() {
  final cached = _appDataDir;
  if (cached != null) return cached;
  final base = Platform.isWindows
      ? Platform.environment['APPDATA']
      : _xdg('XDG_CONFIG_HOME', '.config');
  final dir = Directory(p.join(base ?? Directory.systemTemp.path, 'neotube'));
  if (!dir.existsSync()) {
    final viejaCookie = File(p.join(dir.parent.path, 'neofy', 'yt_cookies.json'));
    dir.createSync(recursive: true);
    if (viejaCookie.existsSync()) {
      try {
        viejaCookie.copySync(p.join(dir.path, 'yt_cookies.json'));
      } catch (_) {
        // Conservar la carpeta antigua es más importante que forzar una copia.
      }
    }
  }
  return _appDataDir = dir;
}

/// Dónde va lo que se puede tirar sin consecuencias: las carátulas.
///
/// - Windows: la misma carpeta que [appDataDir].
/// - Linux: `$XDG_CACHE_HOME/neotube`, o `~/.cache/neotube`.
Directory cacheDir() {
  final cached = _cacheDir;
  if (cached != null) return cached;
  if (Platform.isWindows) return _cacheDir = appDataDir();
  final base = _xdg('XDG_CACHE_HOME', '.cache') ?? Directory.systemTemp.path;
  final dir = Directory(p.join(base, 'neotube'));
  if (!dir.existsSync()) dir.createSync(recursive: true);
  return _cacheDir = dir;
}

/// Dónde viven de verdad las descargas de repuesto, ya resuelto: la carpeta
/// que haya elegido el usuario en Ajustes ([AppConfig.rutaCacheContinuaciones])
/// o, si no ha elegido ninguna, la de por defecto bajo [cacheDir].
///
/// Está aquí y no dentro de `YtPlayer` porque lo necesitan los dos: el
/// reproductor para escribir, y Ajustes para **enseñar la ruta y lo que
/// ocupa**. Que cada uno lo calculara por su cuenta es justo cómo acaban
/// discrepando y enseñándole al usuario una carpeta que no es la que se usa.
String rutaEfectivaDeCacheContinuaciones(String? elegida) =>
    (elegida != null && elegida.trim().isNotEmpty)
        ? elegida.trim()
        : p.join(cacheDir().path, 'continuaciones');

/// Una ruta base de XDG: la variable si es absoluta y, si no, el respaldo bajo
/// `$HOME`.
String? _xdg(String variable, String respaldo) {
  final valor = Platform.environment[variable];
  if (valor != null && valor.isNotEmpty && p.isAbsolute(valor)) return valor;
  final home = Platform.environment['HOME'];
  return home == null || home.isEmpty ? null : p.join(home, respaldo);
}

/// Preferencias persistidas en `config.json`, dentro de [appDataDir].
class AppConfig {
  /// Modo rendimiento: sacrifica las carátulas para bajar la memoria.
  bool performanceMode;

  /// Volumen de NeoTube, 0..100.
  int volumenNeoTube;

  /// Si Discord Rich Presence está encendido.
  bool discordRpcEnabled;

  /// Client ID de la app de Discord. Por defecto el de NeoTube.
  String discordClientId;

  /// Cuántos MB puede ocupar como máximo la caché de descargas de repuesto
  /// (`YtPlayer._descargarCompletaConYtDlp`, ver ese fichero para el
  /// porqué): la copia completa que se baja por si la vía rápida choca con
  /// el corte de posición pasado el primer minuto. `0` la apaga del todo —
  /// ni se descarga ni se guarda nada. Tope: 100 000 MB (100 GB).
  int limiteCacheContinuacionesMB;

  /// Dónde vive esa caché. `null` es el valor por defecto —
  /// `cacheDir()/continuaciones`—; si el usuario elige otra carpeta (por
  /// ejemplo, otra unidad con más sitio libre) se guarda aquí la ruta
  /// absoluta. Cambiarla **no mueve** lo que ya hubiera en la carpeta
  /// anterior — son copias desechables, se vuelven a bajar si hacen falta.
  String? rutaCacheContinuaciones;

  AppConfig({
    this.performanceMode = false,
    this.volumenNeoTube = 60,
    this.discordRpcEnabled = false,
    this.discordClientId = kDiscordClientId,
    this.limiteCacheContinuacionesMB = 300,
    this.rutaCacheContinuaciones,
  });

  static File get _file => File(p.join(appDataDir().path, 'config.json'));

  static Future<AppConfig> load() async {
    try {
      final f = _file;
      if (!await f.exists()) return AppConfig();
      final map = jsonDecode(await f.readAsString()) as Map<String, dynamic>;
      return AppConfig(
        performanceMode: (map['performanceMode'] as bool?) ?? false,
        volumenNeoTube: (map['volumenNeoTube'] as int?) ?? 60,
        discordRpcEnabled: (map['discordRpcEnabled'] as bool?) ?? false,
        discordClientId: (map['discordClientId'] as String?) ?? kDiscordClientId,
        limiteCacheContinuacionesMB: (map['limiteCacheContinuacionesMB'] as int?) ?? 300,
        rutaCacheContinuaciones: map['rutaCacheContinuaciones'] as String?,
      );
    } catch (_) {
      return AppConfig();
    }
  }

  Future<void> save() async {
    await _file.writeAsString(jsonEncode({
      'performanceMode': performanceMode,
      'volumenNeoTube': volumenNeoTube,
      'discordRpcEnabled': discordRpcEnabled,
      'discordClientId': discordClientId,
      'limiteCacheContinuacionesMB': limiteCacheContinuacionesMB,
      'rutaCacheContinuaciones': rutaCacheContinuaciones,
    }));
  }
}
