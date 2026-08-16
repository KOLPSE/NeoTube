import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Versión de NeoTube.
///
/// Esta constante es la única fuente de verdad para el instalador y el
/// actualizador.
const String kVersion = '0.1.5';

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

  AppConfig({
    this.performanceMode = false,
    this.volumenNeoTube = 60,
    this.discordRpcEnabled = false,
    this.discordClientId = kDiscordClientId,
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
    }));
  }
}
