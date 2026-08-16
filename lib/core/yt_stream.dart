// Dart puro a propósito: ni `package:flutter/...` ni `yt_auth.dart` (que
// arrastraría `desktop_webview_window`). Con un plugin de Flutter en el árbol
// de imports `dart run` no compila, y este fichero tiene que poder sondearse
// contra la API de verdad — ver `tool/probe_stream.dart`.
import 'dart:convert';

import 'package:http/http.dart' as http;

/// El resolutor no pudo sacar la URL. Quien llama decide si cae al plan B.
class YtStreamException implements Exception {
  YtStreamException(this.message, {this.reintentarConYtDlp = true, this.avisoDeBot = false});

  final String message;

  /// Si YouTube contestó "confirma que no eres un bot". Merece un reintento con
  /// un `visitorData` nuevo antes de rendirse — ver [YtStreamResolver.resolver].
  final bool avisoDeBot;

  /// Si tiene sentido probar con yt-dlp. `false` cuando el problema es del
  /// vídeo y no del método (retirado, privado, bloqueado por región): ahí
  /// lanzar un proceso más solo sirve para esperar tres segundos y volver a
  /// fallar.
  final bool reintentarConYtDlp;

  @override
  String toString() => message;
}

/// Resuelve la URL del stream de audio de un vídeo **sin subprocesos**:
/// una llamada a `youtubei/v1/player`, la misma API interna con la que
/// `yt_music_api.dart` pide la portada y la biblioteca.
///
/// ## Por qué esto y no yt-dlp
///
/// yt-dlp acaba haciendo exactamente esta petición — su URL de salida lleva
/// `c=ANDROID_VR` pegado —, pero antes paga el arranque del binario empaquetado
/// con PyInstaller y toda la maquinaria de extractores. Medido en Windows sobre
/// la misma pista: **2705 ms con yt-dlp contra 88 ms de mediana por aquí**. Esa
/// diferencia era el "se queda pensando" entre canciones.
///
/// De paso desaparecen dos fuentes de fallo que no se podían arreglar desde
/// aquí: que el binario no esté (o lo haya vaciado un antivirus), y que falte
/// el runtime de JavaScript que yt-dlp necesita desde que YouTube cifra las
/// firmas — sin Deno al lado suelta `some formats may be missing` y a veces se
/// queda sin ninguno.
///
/// ## Por qué un cliente móvil y no `WEB_REMIX`
///
/// **Este es el detalle del que depende todo lo demás.** Pedirle `player` a
/// `WEB_REMIX` (el cliente que usa el resto de la app) devuelve 200 y cuatro
/// formatos de audio perfectamente normales… todos dentro de
/// `signatureCipher`, no de `url`. Descifrarlos exige ejecutar el JavaScript
/// del reproductor de YouTube, que es justo lo que obliga a arrastrar Deno o,
/// como hace `pear-desktop`, un DOM falso con `happy-dom` más un PoToken de
/// BotGuard.
///
/// Los clientes móviles no cifran nada: los formatos llegan con `url` directa
/// y sin parámetro `n` que descifrar. De ahí la velocidad, y de ahí que no
/// haga falta ningún motor de JavaScript.
///
/// ## ⚠️ Que la URL se pueda descargar no significa que libmpv la acepte
///
/// La trampa más cara de este fichero, y la que costó una tarde: se eligió
/// primero el cliente `IOS` porque respondía igual de rápido y con URLs igual
/// de directas — comprobado además bajando los primeros megas por HTTP, que
/// llegaban a 14 MB/s. **Y aun así no sonaba nada.** googlevideo le contesta
/// `403 Forbidden` a ffmpeg con esas URLs mientras le da `206` a una petición
/// normal desde el mismo proceso, con la misma URL y en el mismo segundo.
///
/// La consecuencia práctica: **una URL de audio no se valida descargándola.**
/// Hay que abrirla con el reproductor de verdad, y eso solo lo dice
/// `flutter run` con `MPVLogLevel.debug` puesto, mirando el log de mpv — por
/// HTTP todo parecía correcto.
///
/// ## ⚠️ La sesión no se manda, y esto es lo más contraintuitivo del fichero
///
/// Esta petición va **anónima**: sin `Cookie`, sin `Authorization`, sin
/// `SAPISIDHASH`. Es lo único de toda la app que no se autentica, y no es un
/// descuido.
///
/// Mandar las cookies de la cuenta **resuelve perfectamente** —200, formatos
/// con `url` directa, todo igual de rápido— pero la URL que devuelve viene
/// envenenada: googlevideo le contesta `403 Forbidden` a ffmpeg al abrirla. La
/// hipótesis es que decir "soy una app de Oculus Quest" y adjuntar a la vez una
/// sesión web de escritorio es una contradicción que Google acepta al emitir la
/// URL y castiga al servirla. yt-dlp tampoco manda cookies con los clientes
/// móviles, por lo mismo.
///
/// **Cómo se vio, después de tres hipótesis equivocadas:** con las cookies, el
/// reproductor fallaba en todas las pistas y el reintento por yt-dlp las
/// salvaba a 2,6 s cada una; sin ellas, 7 de 7 a ~120 ms y ni un solo fallo.
/// Ninguna prueba por HTTP lo detecta —la URL se descarga igual de bien en los
/// dos casos—, exactamente igual que con el cliente `IOS`.
///
/// Perder la sesión aquí no cuesta nada: reproducir no depende de la cuenta.
/// `yt_music_api.dart` sigue firmando todo lo demás, que es donde importa.
///
/// Por eso esta petición **no reutiliza `_postBytes()` de `yt_music_api.dart`**:
/// aquella firma siempre y manda `Origin` y `X-Origin`, y aquí las tres cosas
/// sobran o hacen daño.
class YtStreamResolver {
  YtStreamResolver();

  final http.Client _http = http.Client();

  static const _host = 'www.youtube.com';

  /// El cliente que dice ser esta petición.
  ///
  /// ⚠️ **Tiene que ser `ANDROID_VR`, y no `IOS`.** Los dos devuelven URLs sin
  /// cifrar y a la misma velocidad, así que por la respuesta de la API parecen
  /// intercambiables — pero **las URLs del cliente `IOS` las rechaza libmpv**
  /// (ver arriba). Con `ANDROID_VR`, que es el que acaba usando yt-dlp —mira el
  /// `c=` de sus URLs—, suena.
  ///
  /// Está copiado campo a campo de lo que manda yt-dlp, que se puede ver con
  /// **`yt-dlp --print-traffic`**. No es cosmético: con una descripción
  /// "parecida" (versión 1.62.27, `osVersion` 12, sin `userAgent` ni `timeZone`
  /// dentro del contexto) la API seguía contestando con el aviso de bot.
  ///
  /// Si algún día YouTube retira esta versión, el síntoma sería un 400 o un
  /// `playabilityStatus` raro en **todas** las pistas a la vez; se arregla
  /// volviendo a mirar qué manda yt-dlp y copiándolo otra vez.
  static const _cliente = {
    'clientName': 'ANDROID_VR',
    'clientVersion': '1.65.10',
    'deviceMake': 'Oculus',
    'deviceModel': 'Quest 3',
    'androidSdkVersion': 32,
    'userAgent': _userAgent,
    'osName': 'Android',
    'osVersion': '12L',
    'timeZone': 'UTC',
    'utcOffsetMinutes': 0,
  };

  /// El `User-Agent` va emparejado a `clientVersion`: son el mismo cliente
  /// descrito dos veces, y YouTube los compara.
  static const _userAgent =
      'com.google.android.apps.youtube.vr.oculus/1.65.10 '
      '(Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip';

  /// El número de cliente de `ANDROID_VR` en la tabla interna de YouTube.
  static const _clientName = '28';

  /// Estados de `playabilityStatus` que son culpa del vídeo, no del método.
  /// Con estos no se cae a yt-dlp: fallaría igual, tres segundos más tarde.
  ///
  /// `LOGIN_REQUIRED` **no está**, aunque lo parezca: es el estado con el que
  /// llega el "confirma que no eres un bot", y ese sí lo resuelve yt-dlp.
  static const _sinRemedio = {'UNPLAYABLE', 'ERROR'};

  /// La identidad de visitante con la que se pide el stream.
  ///
  /// ⚠️ **Sin esto no se resuelve ni una pista.** Es la diferencia entre 0 de 4
  /// y 4 de 4 en las pruebas: sin `visitorData`, `ANDROID_VR` contesta
  /// `LOGIN_REQUIRED` con el motivo "Inicia sesión para confirmar que no eres
  /// un bot" — y lo hace **tenga o no tenga las cookies de la cuenta delante**,
  /// así que el mensaje engaña: no le falta tu sesión, le falta saber que hay
  /// un visitante detrás.
  ///
  /// Se saca como lo saca yt-dlp: de la página de un vídeo cualquiera, que trae
  /// el valor incrustado en su JavaScript.
  ///
  /// **No se guarda para siempre**, y ahí hay una diferencia con yt-dlp que
  /// importa: él pide uno nuevo *en cada invocación*, porque cada pista es un
  /// proceso distinto. Aquí el proceso dura horas, así que una identidad
  /// reutilizada acumula cientos de peticiones — y la sospecha es que
  /// googlevideo la va quemando y acaba negándole las URLs a ffmpeg. Encaja con
  /// lo medido: una sesión recién arrancada hizo 17 resoluciones sin un fallo, y
  /// la siguiente, con la misma identidad ya rodada, falló 9 veces.
  ///
  /// Por eso se renueva sola: por edad ([_vidaDelVisitante]) y, sobre todo,
  /// cuando quien la usa avisa de que la URL no se pudo abrir
  /// ([renovarVisitante]).
  String? _visitorData;
  DateTime? _visitanteDesde;

  /// Cuánto se reutiliza una identidad de visitante antes de pedir otra. Es un
  /// número elegido a ojo: lo bastante largo para que pedirla no cueste (una
  /// petición extra cada media hora) y lo bastante corto para no arrastrarla
  /// toda una tarde de escucha.
  static const _vidaDelVisitante = Duration(minutes: 30);

  /// Tira la identidad actual: la siguiente pista pedirá una nueva.
  ///
  /// La llama `YtPlayer` cuando libmpv rechaza una URL. El razonamiento: esa
  /// URL la emitió esta identidad, así que si el reproductor no la puede abrir,
  /// la identidad es sospechosa y sale más a cuenta cambiarla que seguir
  /// pidiéndole URLs que no sirven.
  void renovarVisitante() {
    _visitorData = null;
    _visitanteDesde = null;
  }

  Future<String> resolver(String videoId, {String hl = 'en', String gl = 'US'}) async {
    try {
      return await _pedir(videoId, hl: hl, gl: gl);
    } on YtStreamException catch (e) {
      // Un `visitorData` caducado se ve exactamente igual que no tener ninguno:
      // el aviso de bot. Se renueva y se prueba una segunda vez antes de darlo
      // por perdido, que es más barato que caer a yt-dlp.
      if (!e.avisoDeBot) rethrow;
      renovarVisitante();
      return _pedir(videoId, hl: hl, gl: gl);
    }
  }

  Future<String> _pedir(String videoId, {required String hl, required String gl}) async {
    final visitante = await _obtenerVisitorData();

    final cuerpo = <String, dynamic>{
      'context': {
        'client': {
          ..._cliente,
          'hl': hl,
          'gl': gl,
          'visitorData': ?visitante,
        },
        'user': {'lockedSafetyMode': false},
      },
      'videoId': videoId,
      'playbackContext': {
        'contentPlaybackContext': {'html5Preference': 'HTML5_PREF_WANTS'},
      },
      // Sin estos dos, un vídeo con aviso de contenido contesta que hace falta
      // confirmar la edad en vez de devolver los formatos.
      'contentCheckOk': true,
      'racyCheckOk': true,
    };

    final http.Response res;
    try {
      res = await _http
          .post(
            // `prettyPrint=false` como yt-dlp: la respuesta sin sangrar es la
            // mitad de bytes, y aquí se piden una por pista.
            Uri.https(_host, '/youtubei/v1/player', {'prettyPrint': 'false'}),
            headers: {
              'Content-Type': 'application/json',
              'User-Agent': _userAgent,
              'X-Youtube-Client-Name': _clientName,
              'X-Youtube-Client-Version': _cliente['clientVersion']! as String,
              'Origin': 'https://$_host',
              'X-Goog-Visitor-Id': ?visitante,
              // Sin `Cookie` ni `Authorization` — ver "La sesión no se manda".
            },
            body: jsonEncode(cuerpo),
          )
          .timeout(const Duration(seconds: 10));
    } catch (e) {
      throw YtStreamException('No se pudo hablar con YouTube: $e');
    }

    if (res.statusCode != 200) {
      throw YtStreamException('YouTube devolvió ${res.statusCode} al pedir el stream.');
    }

    final Map<String, dynamic> json;
    try {
      json = jsonDecode(utf8.decode(res.bodyBytes)) as Map<String, dynamic>;
    } catch (e) {
      throw YtStreamException('Respuesta ilegible de YouTube: $e');
    }

    return interpretar(json);
  }

  /// Consigue (y cachea) la identidad de visitante. Ver [_visitorData].
  ///
  /// Devuelve `null` en vez de lanzar si no se puede sacar: sin él la petición
  /// fallará con el aviso de bot y de ahí se cae a yt-dlp, que es mejor final
  /// que no intentarlo. La página se pide con `User-Agent` de navegador porque
  /// es a un navegador a quien se le sirve el HTML que trae el valor dentro.
  Future<String?> _obtenerVisitorData() async {
    final guardado = _visitorData;
    final desde = _visitanteDesde;
    if (guardado != null &&
        desde != null &&
        DateTime.now().difference(desde) < _vidaDelVisitante) {
      return guardado;
    }
    try {
      final res = await _http.get(
        Uri.https(_host, '/watch', {
          'v': 'dQw4w9WgXcQ',
          // Los mismos dos de yt-dlp: saltan el intersticial de consentimiento,
          // que si aparece devuelve una página sin `visitorData` dentro.
          'bpctr': '9999999999',
          'has_verified': '1',
        }),
        headers: const {
          'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) '
              'AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.5 Safari/605.1.15',
          'Accept-Language': 'en-us,en;q=0.5',
          'Cookie': 'PREF=hl=en&tz=UTC; SOCS=CAI',
        },
      ).timeout(const Duration(seconds: 10));
      if (res.statusCode != 200) return null;
      final m = RegExp(r'"visitorData":\s*"([^"]+)"')
          .firstMatch(utf8.decode(res.bodyBytes, allowMalformed: true));
      if (m == null) return null;
      // Viene escapado a la manera de JavaScript (`=` y compañía), y hay
      // que deshacerlo o la cabecera viaja con las barras invertidas dentro.
      _visitanteDesde = DateTime.now();
      return _visitorData = jsonDecode('"${m.group(1)}"') as String;
    } catch (_) {
      return null;
    }
  }

  /// Saca la URL de una respuesta ya descargada, o lanza explicando por qué no.
  ///
  /// Está separado de [resolver] para poder probarlo: `flutter test` no puede
  /// hacer red (está mockeada a 400), así que la única forma de cubrir esto es
  /// dándole recortes de JSON con la misma forma que devuelve la API. La red en
  /// sí se comprueba aparte, con `tool/probe_stream.dart`.
  static String interpretar(Map<String, dynamic> json) {
    // Se lee con `is` y no con `as`: un casteo que falla lanza `TypeError`, que
    // no es `YtStreamException` y por tanto se escapa del `catch` que decide si
    // caer a yt-dlp. Mismo criterio defensivo que el parseo de
    // `yt_music_api.dart`.
    final estado = json['playabilityStatus'];
    final status = estado is Map ? estado['status'] : null;
    if (status != 'OK') {
      final motivo = estado is Map ? estado['reason'] : null;
      final razon = (motivo is String ? motivo : null) ??
          (status is String ? status : null) ??
          'sin motivo';
      throw YtStreamException(
        'YouTube no deja reproducir esta pista: $razon',
        reintentarConYtDlp: !_sinRemedio.contains(status),
        avisoDeBot: status == 'LOGIN_REQUIRED',
      );
    }

    final url = _mejorAudio(json);
    if (url == null) {
      throw YtStreamException('La respuesta no traía ningún formato de audio con URL.');
    }
    return url;
  }

  /// El formato de solo-audio de más bitrate que venga con `url` servida.
  ///
  /// Se ignoran los que traigan `signatureCipher`: con el cliente `IOS` no
  /// aparece ninguno, y si algún día apareciera, no sabríamos descifrarlo — es
  /// preferible caer a yt-dlp que devolver una URL que libmpv no puede abrir.
  ///
  /// No se prefiere ningún contenedor: libmpv reproduce igual el Opus en WebM
  /// (itag 251) que el AAC en MP4 (itag 140), y el primero suele ganar en
  /// bitrate.
  static String? _mejorAudio(Map<String, dynamic> json) {
    final streaming = json['streamingData'];
    final formatos = streaming is Map ? streaming['adaptiveFormats'] : null;
    if (formatos is! List) return null;

    String? mejor;
    var mejorBitrate = -1;
    for (final f in formatos) {
      if (f is! Map) continue;
      final mime = f['mimeType'];
      if (mime is! String || !mime.startsWith('audio/')) continue;
      final url = f['url'];
      if (url is! String || url.isEmpty) continue;
      final bitrate = f['bitrate'];
      final valor = bitrate is num ? bitrate.toInt() : 0;
      if (valor > mejorBitrate) {
        mejorBitrate = valor;
        mejor = url;
      }
    }
    return mejor;
  }

  void dispose() => _http.close();
}
