import 'dart:async';
import 'dart:io';

import 'yt_stream.dart';

/// Retransmite en `127.0.0.1` las URLs de audio que resuelve [YtStreamResolver],
/// para que libmpv no las pida de un tirón — y avisa cuando choca con un
/// corte que no puede sortear solo.
///
/// ## Por qué existe esto
///
/// La URL que devuelve la API es válida —lo prueba `tool/probe_stream.dart`—,
/// pero **googlevideo contesta `403 Forbidden` en cuanto la posición pedida
/// pasa de un byte concreto del fichero**. Se aisló con pruebas directas,
/// quitando variables una a una: no son las cabeceras, no es la sesión, no es
/// la reutilización de la conexión, no es si la URL es nueva o gastada — es
/// la posición. Con el cliente `ANDROID_VR` el corte está justo en 1.000.000
/// de bytes, **incluso pidiéndolo con una URL recién resuelta y nunca usada**:
/// no es un cupo que se gasta, es que esa posición del fichero no se sirve
/// por ese camino, punto. Con libmpv pidiendo `Range: bytes=0-` al abrir —el
/// fichero entero de un golpe—, toda pista de más de un minuto caía del lado
/// malo del corte.
///
/// yt-dlp, con el mismo cliente, tropieza exactamente igual (se comprobó a
/// pelo, sin nada de este código de por medio). Es un límite nuevo de
/// googlevideo, no un bug de aquí — encaja con lo que yt-dlp llama SABR: un
/// protocolo de streaming por el que Google está sustituyendo la descarga
/// progresiva por rangos.
///
/// ## ⚠️ Por qué esto no intenta seguir sirviendo desde otra URL
///
/// La primera versión, al chocar con el corte, empalmaba en la misma
/// respuesta HTTP los bytes que le faltaban desde una descarga de repuesto
/// (`WEB_EMBEDDED_PLAYER`, el único cliente que sirve el fichero entero).
/// **No funciona**: aunque el tamaño total coincida byte a byte, son dos
/// descargas independientes del mismo audio, y un contenedor como WebM no es
/// una tira plana de bytes — está organizado en clusters que tienen que
/// empezar donde el códec los puso, no donde a este proxy le tocara cortar
/// (900 000, un número elegido aquí sin relación con esa estructura). El
/// resultado es un fichero con el tamaño correcto y la mitad de atrás
/// corrupta: mpv no da error, simplemente dejaba de encontrar datos válidos y
/// dab a la pista por terminada — el mismo síntoma que el bug original, solo
/// que unos segundos más tarde.
///
/// La solución de verdad no es empalmar bytes: es que quien abre el medio
/// (`YtPlayer`) **reabra en un fichero aparte y salte** a la posición donde
/// iba, en vez de seguir la misma conexión. Este proxy solo avisa con
/// [onCorte] de que ha chocado; no intenta arreglarlo él mismo.
class YtLocalProxy {
  final HttpClient _cliente = HttpClient();
  final Map<String, String> _urls = {};
  final Map<String, void Function()> _onCorte = {};
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

  /// Da de alta [urlRapida] y devuelve la URL local que hay que abrir en su
  /// lugar. Cada llamada crea una entrada nueva: no hace falta limpiarlas
  /// (son un puñado de cadenas por pista, y la app no dura semanas abierta).
  ///
  /// [onCorte], si se pasa, se llama la primera vez que un trozo posterior al
  /// primero falla: es la señal de que esta URL ya no va a dar más, y de que
  /// hace falta la descarga de repuesto que gestiona `YtPlayer`.
  Future<Uri> exponer(String urlRapida, {void Function()? onCorte}) async {
    final puerto = await _puerto();
    final token = (_siguiente++).toString();
    _urls[token] = urlRapida;
    if (onCorte != null) _onCorte[token] = onCorte;
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
        // La propia vía rápida no arrancó (URL mala, vídeo bloqueado…): esto
        // no es el corte de después, es la petición inicial fallando. Se
        // devuelve el código real para que el mecanismo de reintento de
        // `YtPlayer` (el que ve `mpv.stream.error`) siga funcionando igual.
        req.response.statusCode = trozo.statusCode;
        await req.response.close();
        return;
      }

      req.response.statusCode = rangeHeader != null ? HttpStatus.partialContent : HttpStatus.ok;
      req.response.headers.set(HttpHeaders.acceptRangesHeader, 'bytes');
      // Sin `Content-Length` a propósito: no se sabe si va a hacer falta
      // cortar antes de tiempo, y declarar uno que luego no se cumple es
      // justo el fallo que costó una tarde entera. Sin él, dart:io manda la
      // respuesta troceada (`Transfer-Encoding: chunked`) y cerrarla pronto
      // es una respuesta corta y válida, no una rota.
      final tipo = trozo.headers.contentType;
      if (tipo != null) req.response.headers.contentType = tipo;
      var total = _totalDesde(trozo);

      while (true) {
        await for (final datos in trozo) {
          req.response.add(datos);
          pos += datos.length;
        }
        if (total != null && pos >= total) break;
        trozo = await _pedirTrozo(real, pos, pos + _tamanoTrozo - 1);
        if (trozo.statusCode >= 400) {
          // La vía rápida chocó con el corte de posición: se corta aquí, en
          // limpio, y se avisa para que `YtPlayer` reabra por la otra vía.
          _onCorte[token]?.call();
          break;
        }
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
