// Registro de actividad de NeoTube en un fichero, para poder adjuntarlo a un
// reporte de NeoHub cuando algo falla.
//
// - Es un append síncrono a `logs/neotube.log`, que rota a `neotube.1.log` al
//   pasar de 1 MB. Síncrono a propósito: los logs que de verdad importan son
//   los de un cuelgue, y un buffer asíncrono sin vaciar se pierde justo cuando
//   más falta hace.
// - Además se vuelca por consola con `debugPrint` (que imprime también en
//   release), para que el smoke test de CI y `flutter run` sigan viéndolo.
// - Nada de lo que se escribe aquí sale del equipo por sí solo: adjuntarlo a
//   un reporte es cosa de `neohub.dart`, y el usuario lo ve antes de enviar.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;

import 'app_config.dart';

enum LogNivel { debug, info, aviso, error }

class Log {
  Log._();

  static const String _nombre = 'neotube.log';
  static const String _nombreAnterior = 'neotube.1.log';
  static const int _maxBytes = 1024 * 1024; // 1 MB: entra entero en un adjunto.

  static Directory get _directorio =>
      Directory(p.join(appDataDir().path, 'logs'));

  static File get _fichero => File(p.join(_directorio.path, _nombre));

  /// Mensaje que solo se registra en desarrollo.
  static void d(String mensaje) {
    if (kDebugMode) _escribir(LogNivel.debug, mensaje);
  }

  static void i(String mensaje) => _escribir(LogNivel.info, mensaje);

  static void w(String mensaje) => _escribir(LogNivel.aviso, mensaje);

  /// Error con, si vienen, la excepción y la pila. La pila se guarda en varias
  /// líneas para que un lector humano (o una IA) pueda seguirla.
  static void e(String mensaje, [Object? error, StackTrace? pila]) {
    final buffer = StringBuffer(mensaje);
    if (error != null) buffer.write(' :: $error');
    if (pila != null) buffer.write('\n$pila');
    _escribir(LogNivel.error, buffer.toString());
  }

  static void _escribir(LogNivel nivel, String mensaje) {
    final linea = _formatear(nivel, mensaje);
    // Por consola siempre (release incluido), para CI y desarrollo.
    debugPrint(linea.trimRight());
    try {
      _directorio.createSync(recursive: true);
      _rotarSiHaceFalta(linea.length);
      _fichero.writeAsStringSync(linea, mode: FileMode.append, flush: true);
    } catch (_) {
      // Sin permiso o sin disco: no hay dónde guardarlo, y no se puede hacer
      // ruido por no poder hacer ruido.
    }
  }

  static String _formatear(LogNivel nivel, String mensaje) {
    final etiqueta = switch (nivel) {
      LogNivel.debug => 'DEBUG',
      LogNivel.info => 'INFO ',
      LogNivel.aviso => 'AVISO',
      LogNivel.error => 'ERROR',
    };
    return '${DateTime.now().toIso8601String()} $etiqueta $mensaje\n';
  }

  static void _rotarSiHaceFalta(int extra) {
    try {
      final f = _fichero;
      if (!f.existsSync() || f.lengthSync() + extra <= _maxBytes) return;
      final anterior = File(p.join(_directorio.path, _nombreAnterior));
      if (anterior.existsSync()) anterior.deleteSync();
      f.renameSync(anterior.path);
    } catch (_) {
      // Si no se puede rotar, se sigue escribiendo en el mismo fichero.
    }
  }

  /// Ruta del fichero activo, para enseñarla en el formulario de reporte.
  static String get ruta => _fichero.path;

  /// Ficheros `.log` que hay ahora mismo, del más nuevo al más viejo. Son los
  /// que se adjuntan a un reporte (ver `neohub.dart`).
  static List<String> get ficheros {
    final activo = _fichero;
    final anterior = File(p.join(_directorio.path, _nombreAnterior));
    return [
      if (activo.existsSync()) activo.path,
      if (anterior.existsSync()) anterior.path,
    ];
  }
}
