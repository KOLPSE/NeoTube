// Enviar reportes de errores al servidor Neo (NeoHub).
//
// La app no lista ni responde reportes: eso es del panel de administración.
// Aquí solo se CREA un reporte de tipo bug y se le adjuntan los logs de la
// app y el contexto del equipo. La API es la pública — `https://neogex.xyz`
// con la cabecera `X-NeoHub-Key`, y de respaldo la LAN
// (`http://192.168.1.20:6070`) — porque en casa responde la LAN y fuera,
// internet, sin configurar nada.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:http/http.dart' as http;
import 'package:path/path.dart' as p;

import 'app_config.dart';
import 'log.dart';

/// Clave de aplicación, compartida por todas las apps Neo. Identifica al
/// cliente, no a la persona: quién envía cada reporte viaja en `authorName`.
const String _kClaveApp = 'f448f2a0f22661230e348b1891baa2812c210c8a01496b24';

const String _kApiPublica = 'https://neogex.xyz';
const String _kApiLan = 'http://192.168.1.20:6070';

/// Id con el que el panel agrupa los reportes de esta app.
const String kAppIdNeoTube = 'neotube';

class ErrorNeoHub implements Exception {
  const ErrorNeoHub(this.mensaje);

  final String mensaje;

  @override
  String toString() => mensaje;
}

/// Un fichero listo para adjuntar a un reporte.
class AdjuntoPendiente {
  const AdjuntoPendiente({
    required this.nombre,
    required this.bytes,
    required this.mime,
  });

  final String nombre;
  final Uint8List bytes;
  final String mime;
}

/// Lo que deja un envío: el id del reporte y qué adjuntos no llegaron.
class ResultadoReporte {
  const ResultadoReporte({
    required this.id,
    required this.subidos,
    required this.fallidos,
  });

  final String id;
  final int subidos;
  final List<String> fallidos;
}

class NeoHub {
  NeoHub({http.Client? cliente}) : _cliente = cliente ?? http.Client();

  final http.Client _cliente;

  String? _baseBuena;
  DateTime? _comprobadaEl;

  static const Duration _kTimeout = Duration(seconds: 7);
  static const Duration _kTimeoutSubida = Duration(seconds: 60);
  static const Duration _kValidezBase = Duration(seconds: 45);

  Map<String, String> get _cabeceras => {
    'X-NeoHub-Key': _kClaveApp,
    'Accept': 'application/json',
    'User-Agent': 'NeoTube/$kVersion',
  };

  /// Direcciones en orden: la que ya funcionó (mientras siga fresca), la
  /// pública y la LAN.
  List<String> get _candidatas {
    final lista = <String>[];
    final recordada = _baseBuena;
    final comprobada = _comprobadaEl;
    if (recordada != null &&
        comprobada != null &&
        DateTime.now().difference(comprobada) < _kValidezBase) {
      lista.add(recordada);
    }
    for (final base in const [_kApiPublica, _kApiLan]) {
      if (!lista.contains(base)) lista.add(base);
    }
    return lista;
  }

  void _apuntarBase(String base) {
    _baseBuena = base;
    _comprobadaEl = DateTime.now();
  }

  /// ¿Hay servidor? Una lista corta basta para saber que quien contesta es la
  /// API de NeoHub y no otra cosa.
  Future<bool> comprobar() async {
    for (final base in _candidatas) {
      try {
        final res = await _cliente
            .get(
              Uri.parse('$base/api/public/reports?limit=1'),
              headers: _cabeceras,
            )
            .timeout(_kTimeout);
        if (res.statusCode == 200) {
          _apuntarBase(base);
          return true;
        }
      } catch (_) {
        // Sin red o servidor apagado: se prueba la otra dirección.
      }
    }
    return false;
  }

  Future<String> _crearReporte({
    required String titulo,
    required String descripcion,
    required String autorNombre,
  }) async {
    Object? ultimo;
    for (final base in _candidatas) {
      try {
        final res = await _cliente
            .post(
              Uri.parse('$base/api/public/reports'),
              headers: {..._cabeceras, 'Content-Type': 'application/json'},
              body: jsonEncode({
                'type': 'bug',
                'title': titulo,
                'description': descripcion,
                'priority': 'media',
                'category': 'general',
                'authorName': autorNombre,
                'appId': kAppIdNeoTube,
              }),
            )
            .timeout(_kTimeout);
        final json = _comoJson(res);
        if (res.statusCode >= 400) {
          final mensaje = json['error'] is String
              ? json['error'] as String
              : 'Error ${res.statusCode}';
          throw ErrorNeoHub(mensaje);
        }
        _apuntarBase(base);
        final reporte = json['report'];
        if (reporte is Map && reporte['id'] is String) {
          return reporte['id'] as String;
        }
        throw const ErrorNeoHub('El servidor no devolvió el reporte.');
      } on ErrorNeoHub {
        // El servidor respondió con un error: no tiene sentido reintentar en
        // otra dirección.
        rethrow;
      } catch (e) {
        ultimo = e;
      }
    }
    Log.w('NeoHub: no se pudo crear el reporte${ultimo == null ? '' : ' ($ultimo)'}');
    throw const ErrorNeoHub('No se pudo contactar con el servidor.');
  }

  Future<void> _subirAdjunto(String reporteId, AdjuntoPendiente adjunto) async {
    Object? ultimo;
    for (final base in _candidatas) {
      try {
        final res = await _cliente
            .post(
              Uri.parse(
                '$base/api/public/reports/${Uri.encodeComponent(reporteId)}'
                '/adjuntos',
              ),
              headers: {
                ..._cabeceras,
                'Content-Type': adjunto.mime,
                'X-Nombre-Fichero': Uri.encodeComponent(adjunto.nombre),
              },
              body: adjunto.bytes,
            )
            .timeout(_kTimeoutSubida);
        if (res.statusCode >= 400) {
          final json = _comoJson(res);
          final mensaje = json['error'] is String
              ? json['error'] as String
              : 'Error ${res.statusCode}';
          throw ErrorNeoHub(mensaje);
        }
        _apuntarBase(base);
        return;
      } on ErrorNeoHub {
        rethrow;
      } catch (e) {
        ultimo = e;
      }
    }
    Log.w('NeoHub: no se pudo subir ${adjunto.nombre}${ultimo == null ? '' : ' ($ultimo)'}');
    throw const ErrorNeoHub('No se pudo subir el adjunto.');
  }

  Map<String, dynamic> _comoJson(http.Response res) {
    try {
      final decodificado = jsonDecode(
        utf8.decode(res.bodyBytes, allowMalformed: true),
      );
      return decodificado is Map
          ? Map<String, dynamic>.from(decodificado)
          : const {};
    } catch (_) {
      return const {};
    }
  }

  void cerrar() => _cliente.close();

  // ------------------------------------------------------------- el envío --

  /// Arma y envía un reporte de error con los logs de la app y el contexto del
  /// equipo. El reporte se crea primero y los adjuntos van detrás: si un
  /// adjunto falla, el reporte no se pierde.
  Future<ResultadoReporte> reportarProblema({
    required String titulo,
    required String descripcion,
    required String autorNombre,
  }) async {
    if (titulo.trim().length < 4) {
      throw const ErrorNeoHub('El título necesita al menos 4 caracteres.');
    }
    if (descripcion.trim().length < 8) {
      throw const ErrorNeoHub('La descripción necesita al menos 8 caracteres.');
    }
    final nombre = autorNombre.trim();
    if (nombre.length < 2) {
      throw const ErrorNeoHub('Escribe el nombre con el que firmas el reporte.');
    }

    final id = await _crearReporte(
      titulo: titulo.trim(),
      descripcion: descripcion.trim(),
      autorNombre: nombre,
    );

    final adjuntos = <AdjuntoPendiente>[
      ...await _logsDeNeoTube(),
      _contextoDelEquipo(),
    ];

    final fallidos = <String>[];
    var subidos = 0;
    for (final a in adjuntos) {
      try {
        await _subirAdjunto(id, a);
        subidos++;
      } on ErrorNeoHub catch (e) {
        fallidos.add('${a.nombre}: ${e.mensaje}');
      }
    }
    Log.i('NeoHub: reporte $id enviado ($subidos adjuntos)');
    return ResultadoReporte(id: id, subidos: subidos, fallidos: fallidos);
  }

  /// Los ficheros `.log` que escribe [Log]. Solo se adjunta lo que no esté
  /// vacío; nunca configuración ni cookies de sesión.
  Future<List<AdjuntoPendiente>> _logsDeNeoTube() async {
    final dir = Directory(p.join(appDataDir().path, 'logs'));
    final salida = <AdjuntoPendiente>[];
    if (!dir.existsSync()) return salida;
    for (final nombre in const ['neotube.log', 'neotube.1.log']) {
      final f = File(p.join(dir.path, nombre));
      try {
        if (!f.existsSync()) continue;
        final bytes = f.readAsBytesSync();
        if (bytes.isEmpty) continue;
        salida.add(
          AdjuntoPendiente(
            nombre: 'neotube-$nombre',
            bytes: bytes,
            mime: 'text/plain',
          ),
        );
      } catch (_) {
        // Un log que no se puede leer no es motivo para no enviar el reporte.
      }
    }
    return salida;
  }

  /// El contexto técnico del envío, como fichero de texto adjunto: sistema,
  /// arquitectura y versión, que es lo que hace falta para reproducir el fallo.
  AdjuntoPendiente _contextoDelEquipo() {
    final cuando = DateTime.now();
    final lineas = <String>[
      'NeoTube $kVersion · contexto del envío',
      'Sistema: ${Platform.operatingSystem} ${Platform.operatingSystemVersion}',
      'Arquitectura: ${_arquitectura()}',
      'Núcleos: ${Platform.numberOfProcessors}',
      'Idioma: ${Platform.localeName}',
      'Zona: ${cuando.timeZoneName}',
      'Enviado: ${cuando.toIso8601String()}',
    ];
    return AdjuntoPendiente(
      nombre: 'contexto-neotube.txt',
      bytes: Uint8List.fromList(utf8.encode('${lineas.join('\n')}\n')),
      mime: 'text/plain',
    );
  }

  String _arquitectura() {
    if (!Platform.isWindows) return Platform.version;
    // Un proceso de 32 bits sobre Windows de 64 explica que un fallo aparezca
    // en un equipo y no en otro.
    return Platform.environment.containsKey('PROCESSOR_ARCHITEW6432')
        ? 'x64 (proceso de 32 bits sobre Windows de 64)'
        : 'x64';
  }
}
