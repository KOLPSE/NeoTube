import 'dart:async';
import 'dart:io';

import 'yt_stream.dart';

/// Retransmite en `127.0.0.1` las URLs de audio que resuelve [YtStreamResolver],
/// para que libmpv no las pida de un tirón.
///
/// ## Por qué existe esto
///
/// La URL que devuelve la API es válida —lo prueba `tool/probe_stream.dart`—,
/// pero **googlevideo contesta `403 Forbidden` en cuanto el trozo pedido pasa
/// de un tamaño**. Se aisló con `tool/probe_headers.dart`, quitando variables
/// una a una: no son las cabeceras (todas las combinaciones probadas pasan),
/// no es la sesión, no es la identidad — es el tamaño del `Range` pedido. La
/// frontera está justo en 1.000.000 de bytes: `bytes=0-900000` sirve 206 y
/// `bytes=0-1000000` sirve 403, con la misma URL, la misma sesión, en la misma
/// prueba. Y libmpv, al abrir un stream, pide `Range: bytes=0-` sin más —todo
/// el fichero de un golpe—, así que **siempre** cae del lado malo del corte.
///
/// yt-dlp no lo sufre porque trocea sus descargas por su cuenta
/// (`--http-chunk-size`, activo por defecto en las URLs de este tipo de
/// cliente); aquí no hay yt-dlp de por medio en la vía rápida, así que hay que
/// trocear a mano. Es la misma idea que un reproductor adaptativo de verdad:
/// pedir el audio en tajadas, no entero.
///
/// ## Por qué no `package:http`
///
/// `http.Client.get()` carga la respuesta entera en memoria antes de
/// devolverla. `dart:io HttpClient` entrega cada trozo según llega, que es lo
/// que hace falta para ir encadenando peticiones sin acumular el audio
/// completo en RAM.
class YtLocalProxy {
  final HttpClient _cliente = HttpClient();
  final Map<String, String> _urls = {};
  HttpServer? _server;
  int _siguiente = 0;

  /// El límite real está en 1.000.000; se deja un margen para no bailar justo
  /// en el borde si googlevideo lo mueve un byte.
  static const _tamanoTrozo = 900000;

  Future<int> _puerto() async {
    final actual = _server;
    if (actual != null) return actual.port;
    final servidor = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    _server = servidor;
    unawaited(_atender(servidor));
    return servidor.port;
  }

  /// Da de alta [urlReal] y devuelve la URL local que hay que abrir en su
  /// lugar. Cada llamada crea una entrada nueva: no hace falta limpiarlas
  /// (son un puñado de cadenas por pista, y la app no dura semanas abierta).
  Future<Uri> exponer(String urlReal) async {
    final puerto = await _puerto();
    final token = (_siguiente++).toString();
    _urls[token] = urlReal;
    return Uri(scheme: 'http', host: '127.0.0.1', port: puerto, path: '/s/$token');
  }

  Future<void> _atender(HttpServer servidor) async {
    await for (final req in servidor) {
      unawaited(_manejar(req));
    }
  }

  Future<void> _manejar(HttpRequest req) async {
    final token = req.uri.pathSegments.isNotEmpty ? req.uri.pathSegments.last : '';
    final real = _urls[token];
    if (real == null) {
      req.response.statusCode = HttpStatus.notFound;
      await req.response.close();
      return;
    }

    // libmpv puede pedir desde un punto concreto (al buscar en la canción);
    // se respeta el inicio y se ignora el final que mande, que es justo lo
    // que hay que trocear.
    var inicio = 0;
    final rangeHeader = req.headers.value(HttpHeaders.rangeHeader);
    if (rangeHeader != null) {
      final m = RegExp(r'bytes=(\d+)-').firstMatch(rangeHeader);
      if (m != null) inicio = int.parse(m.group(1)!);
    }

    try {
      var pos = inicio;
      var trozo = await _pedirTrozo(real, pos, pos + _tamanoTrozo - 1);
      if (trozo.statusCode >= 400) {
        req.response.statusCode = trozo.statusCode;
        await req.response.close();
        return;
      }

      final total = _totalDesde(trozo);
      req.response.statusCode = rangeHeader != null ? HttpStatus.partialContent : HttpStatus.ok;
      if (total != null) {
        if (rangeHeader != null) {
          req.response.headers.set('content-range', 'bytes $inicio-${total - 1}/$total');
        }
        req.response.headers.set(HttpHeaders.contentLengthHeader, (total - inicio).toString());
      }
      req.response.headers.set(HttpHeaders.acceptRangesHeader, 'bytes');
      final tipo = trozo.headers.contentType;
      if (tipo != null) req.response.headers.contentType = tipo;

      while (true) {
        await for (final datos in trozo) {
          req.response.add(datos);
          pos += datos.length;
        }
        if (total != null && pos >= total) break;
        trozo = await _pedirTrozo(real, pos, pos + _tamanoTrozo - 1);
        // Un 403/416 aquí es fin de fichero o que la URL caducó a media
        // descarga: se corta limpio en vez de reventar la respuesta.
        if (trozo.statusCode >= 400) break;
      }
      await req.response.close();
    } catch (_) {
      // Lo normal: el usuario saltó de pista y libmpv cerró la conexión a
      // media descarga. No es un fallo que haya que enseñar.
      try {
        await req.response.close();
      } catch (_) {}
    }
  }

  Future<HttpClientResponse> _pedirTrozo(String urlReal, int inicio, int fin) async {
    final upstream = await _cliente.getUrl(Uri.parse(urlReal));
    for (final entrada in YtStreamResolver.cabecerasDeReproduccion.entries) {
      upstream.headers.set(entrada.key, entrada.value);
    }
    upstream.headers.set(HttpHeaders.rangeHeader, 'bytes=$inicio-$fin');
    return upstream.close();
  }

  /// El tamaño total del fichero, sacado de `Content-Range: bytes a-b/total`.
  int? _totalDesde(HttpClientResponse res) {
    final cr = res.headers.value('content-range');
    if (cr == null) return null;
    final m = RegExp(r'/(\d+)$').firstMatch(cr);
    return m == null ? null : int.tryParse(m.group(1)!);
  }

  void dispose() {
    unawaited(_server?.close(force: true));
    _cliente.close(force: true);
  }
}
