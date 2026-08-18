import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:media_kit/media_kit.dart';
import 'package:path/path.dart' as p;

import 'procesos.dart' show sufijoEjecutable;
import 'yt_auth.dart';
import 'yt_local_proxy.dart';
import 'yt_models.dart';
import 'yt_music_api.dart' show YtMusicApi;
import 'yt_stream.dart';

class YtPlayerException implements Exception {
  final String message;
  YtPlayerException(this.message);
  @override
  String toString() => message;
}

/// Reproduce audio de YouTube sin necesitar cuenta Premium, **con cola**.
///
/// Dos piezas separadas, como en el resto de la app: **resolver** la URL del
/// stream de audio (`yt_stream.dart`, una llamada a la misma API interna que ya
/// usa el resto de la app; ver ahí por qué) y **reproducirla** (`media_kit`,
/// que envuelve libmpv y sabe abrir esa URL directamente).
///
/// A diferencia de `PlayerController` (que sondea la Web API de Spotify porque
/// el audio suena en otro proceso, `librespot`, controlado en remoto), aquí el
/// reproductor vive en este mismo proceso: no hace falta sondeo, basta con
/// escuchar sus streams.
///
/// ## Por qué hay cola aquí y no en la pantalla
///
/// La cola no es un detalle de la pantalla que la lanzó: al terminar una
/// canción tiene que empezar la siguiente aunque el usuario se haya ido a
/// buscar otra cosa, y la barra inferior necesita saber si hay "siguiente"
/// para encender su botón. Vive donde vive el audio.
///
/// ## La espera de resolver
///
/// Resolver una URL por la API cuesta unos 90 ms, pero el plan B (yt-dlp) se va
/// a casi tres segundos, así que la siguiente pista se sigue resolviendo
/// **mientras suena la actual** y se guarda en [_urls]: es barato y quita el
/// silencio entre pistas también cuando toca caer al plan B. Las URLs que
/// devuelve Google caducan (traen su propio `expire`, unas 6 h), de ahí que la
/// caché tenga fecha de caducidad propia y conservadora.
class YtPlayer extends ChangeNotifier {
  /// [auth] es opcional porque el resolutor funciona sin sesión (ver
  /// `YtStreamResolver.sesion`), y así los tests que solo comprueban la cola o
  /// la ausencia de libmpv no tienen que montar una. `main.dart` sí la pasa.
  YtPlayer({YtAuth? auth, int volumenInicial = 60})
      // PRUEBA: sin las cookies de la cuenta, exactamente como yt-dlp. Ver
      // `YtStreamResolver.sesion`.
      : _stream = YtStreamResolver(),
        _volumen = ValueNotifier<int>(volumenInicial.clamp(0, 100)),
        _mpv = libmpvDisponible ? Player() : null {
    final mpv = _mpv;
    if (mpv == null) {
      // Sin libmpv no hay reproductor, pero el objeto tiene que existir igual:
      // el shell de NeoTube se construye siempre, esté el usuario en ese modo
      // o no. Lo único que cambia es que no suena nada y se enseña el porqué.
      error = motivoSinLibmpv;
      return;
    }
    // El volumen guardado se aplica ya, antes de que suene nada: aplicarlo al
    // abrir la primera pista se oiría como un salto de volumen a mitad de la
    // primera nota.
    unawaited(mpv.setVolume(_volumen.value.toDouble()));
    _subCompletado = mpv.stream.completed.listen((terminada) {
      if (!terminada || _cambiando) return;
      final pista = actual;
      if (pista != null && _pistaConCorte == pista.videoId) {
        unawaited(_continuarTrasCorte(pista));
        return;
      }
      if (cola.isNotEmpty) unawaited(siguiente(automatico: true));
    });
    // La duración no se sabe hasta que libmpv ha abierto el medio, unos cientos
    // de milisegundos después de que la pista ya conste como "la actual". Sin
    // avisar de esto, todo lo que dependa de saber cuánto dura la canción se
    // queda con el cero de antes: el panel del sistema y, sobre todo, la barra
    // de progreso de Discord, que sin duración no llega a dibujarse.
    _subDuracion = mpv.stream.duration.listen((_) => notifyListeners());
    _subError = mpv.stream.error.listen((e) {
      debugPrint('NeoTube: libmpv informa de un error: $e '
          '(abriendo=$_pistaAbriendo, reintentosHechos=$_reintentosHechos)');
      if (_pistaAbriendo != null && _reintentosHechos < _maxReintentos) {
        _reintentosHechos++;
        final pista = actual;
        if (pista != null && pista.videoId == _pistaAbriendo) {
          unawaited(_reintentarConUrlFresca(pista));
          return; // No se enseña nada todavía: el reintento decide
        }
      }
      _pistaAbriendo = null;
      error = 'Error de reproducción: $e';
      notifyListeners();
    });
  }

  /// Si libmpv se pudo cargar al arrancar. Lo fija `main()`, que es quien
  /// llama a `MediaKit.ensureInitialized()`.
  ///
  /// **Por qué esto existe.** `media_kit` no empaqueta libmpv en Linux: la
  /// carga del sistema con `dlopen` al llamar a `ensureInitialized()`, y si no
  /// está, lanza. Como esa llamada va en `main()` antes de `runApp()`, una
  /// distribución sin libmpv instalado hacía que la app arrancara y se cerrara
  /// **sin ventana ni mensaje**: era el fallo que se veía en Arch, donde
  /// `mpv` no viene de serie.
  ///
  /// Los paquetes ya declaran la dependencia, así que un usuario de `.deb`,
  /// `.rpm` o `pacman` no debería llegar aquí nunca. Queda por los del
  /// tarball, que no tiene forma de exigir nada: mejor una NeoFy entera con
  /// NeoTube apagado que ninguna app.
  ///
  /// En Windows no aplica: allí `libmpv-2.dll` viaja dentro del paquete.
  static bool libmpvDisponible = true;

  /// Qué decirle al usuario cuando [libmpvDisponible] es `false`.
  ///
  /// Es un mensaje nuestro y no el de la excepción de `media_kit`: aquel está
  /// en inglés y solo sabe recomendar `apt install libmpv-dev`, que no le sirve
  /// de nada a quien está en Arch o en Fedora. El original se queda en el log.
  static String motivoSinLibmpv =
      'No se encuentra libmpv, la librería que reproduce el audio de NeoTube. '
      'Instala el paquete de tu distribución (Arch: mpv · Debian/Ubuntu: '
      'libmpv2 · Fedora: mpv-libs) y vuelve a abrir NeoTube.';

  /// El reproductor de verdad, o `null` si falta libmpv. Privado a propósito:
  /// fuera se usan [sonando], [posicion] y compañía, que ya contemplan que no
  /// haya reproductor.
  final Player? _mpv;

  /// Quien resuelve las URLs por la vía rápida. yt-dlp se queda de plan B, más
  /// abajo.
  final YtStreamResolver _stream;

  /// Por dónde pasan todas las URLs de audio antes de llegar a libmpv. Ver
  /// `yt_local_proxy.dart` para el porqué: ffmpeg recibe `403` de googlevideo
  /// pidiendo la misma URL que un cliente HTTP normal sí consigue.
  final YtLocalProxy _proxy = YtLocalProxy();

  StreamSubscription<bool>? _subCompletado;
  StreamSubscription<String>? _subError;
  StreamSubscription<Duration>? _subDuracion;
  StreamSubscription<PlayerLog>? _subLog;

  /// ¿Puede sonar algo? `false` solo cuando falta libmpv (ver
  /// [libmpvDisponible]). Con esto puesto a `false`, NeoTube se enseña
  /// explicando qué falta en vez de aceptar pulsaciones que no harían nada.
  bool get disponible => _mpv != null;

  bool get sonando => _mpv?.state.playing ?? false;

  /// Cambios de play/pausa. Hace falta aparte de [notifyListeners] porque
  /// pausar no cambia nada del estado que lleva esta clase.
  Stream<bool> get cambiosDeSonando => _mpv?.stream.playing ?? const Stream<bool>.empty();

  Duration get posicion => _mpv?.state.position ?? Duration.zero;

  Stream<Duration> get cambiosDePosicion =>
      _mpv?.stream.position ?? const Stream<Duration>.empty();

  Duration get duracion => _mpv?.state.duration ?? Duration.zero;

  /// Se llama justo antes de empezar a sonar algo. Es el enganche con el que
  /// `main.dart` para NeoFy: los dos modos comparten altavoces y sonar a la
  /// vez no es una opción.
  Future<void> Function()? alEmpezarAReproducir;

  // --------------------------------------------------------------------- cola

  List<YtTrack> cola = const [];
  int indice = -1;

  /// De dónde salió la cola (el `playlistId`), para poder marcar en la
  /// interfaz qué lista está sonando.
  String? contexto;

  /// Modo aleatorio, como el de Spotify: una vez activado, se queda activado
  /// (no es "barajar esta lista una vez") y afecta a la próxima lista que se
  /// ponga a sonar también. [_colaSinBarajar] guarda el orden real para poder
  /// volver a él al desactivarlo, sin tener que volver a pedir nada a la API.
  bool _aleatorio = false;
  bool get aleatorio => _aleatorio;
  List<YtTrack> _colaSinBarajar = const [];

  /// Enciende o apaga el aleatorio. La pista que suena ahora se queda donde
  /// está (no se corta lo que se está oyendo); lo que cambia es qué viene
  /// después.
  void alternarAleatorio() {
    _aleatorio = !_aleatorio;
    if (cola.isEmpty) {
      notifyListeners();
      return;
    }
    final pistaActual = actual;
    if (_aleatorio) {
      _colaSinBarajar = List.unmodifiable(cola);
      final resto = [...cola];
      if (indice >= 0 && indice < resto.length) resto.removeAt(indice);
      resto.shuffle();
      cola = List.unmodifiable(pistaActual == null ? resto : [pistaActual, ...resto]);
      indice = pistaActual == null ? -1 : 0;
    } else {
      if (_colaSinBarajar.isNotEmpty) {
        cola = _colaSinBarajar;
        indice = pistaActual == null
            ? -1
            : cola.indexWhere((t) => t.videoId == pistaActual.videoId);
        if (indice < 0) indice = 0;
      }
      _colaSinBarajar = const [];
    }
    notifyListeners();
  }

  YtTrack? get actual => (indice >= 0 && indice < cola.length) ? cola[indice] : null;
  bool get hayNada => actual == null;
  bool get puedeSaltar => indice + 1 < cola.length;
  bool get puedeVolver => cola.isNotEmpty;

  /// Qué pista se está resolviendo ahora mismo (su `videoId`), para que la
  /// tarjeta pulsada pueda enseñar su ruedecita. `null` si no hay ninguna.
  String? resolviendo;
  String? error;

  bool _cambiando = false;

  /// El videoId que se acaba de mandar a abrir y del que aún no hay
  /// confirmación de que suene: mientras esté puesto, un fallo de [_subError]
  /// se trata como "no se pudo abrir" (con un reintento silencioso) y no como
  /// un fallo de reproducción en mitad de una canción que ya sonaba.
  String? _pistaAbriendo;

  /// Cuántas veces se ha reintentado la pista que se está abriendo.
  ///
  /// Antes era un booleano y se rendía al primer fallo del reintento. En la
  /// práctica una pista concreta a veces rechaza **dos** URLs seguidas —dos
  /// identidades distintas, dos servidores de CDN distintos, con segundos de
  /// diferencia— y la tercera sí suena: es justo lo que hacía manualmente
  /// quien cambiaba de canción y volvía a la de antes. [_maxReintentos] deja
  /// que la app haga ese mismo tercer intento sola antes de rendirse.
  int _reintentosHechos = 0;
  static const _maxReintentos = 2;
  Timer? _temporizadorAbriendo;

  /// URLs ya resueltas, con su caducidad. Se limita a un puñado: no es una
  /// caché de verdad, es el adelanto de las siguientes pistas.
  final Map<String, ({String url, DateTime hasta})> _urls = {};
  final Map<String, Future<String>> _enVuelo = {};
  static const _vidaDeUrl = Duration(minutes: 45);

  /// La descarga completa (ver [_descargarCompletaConYtDlp]) de cada pista
  /// que se ha abierto, por si hace falta cuando la vía rápida choque con el
  /// corte de posición. Vive mientras viva `YtPlayer`: son Futures, no
  /// cuestan nada hasta que alguien las espera.
  final Map<String, Future<File?>> _continuaciones = {};

  /// El videoId de la pista que acaba de chocar con el corte de la vía
  /// rápida (ver `yt_local_proxy.dart`). Lo pone [YtLocalProxy.onCorte] y lo
  /// consume el listener de `mpv.stream.completed`: si coincide con la pista
  /// actual, ese "completado" no es el final de verdad, es el corte.
  String? _pistaConCorte;

  // ------------------------------------------------------------------ yt-dlp

  /// Busca el binario junto al ejecutable (release) y, si no, en el árbol de
  /// desarrollo — mismo patrón que `LibrespotManager.findBinary()`.
  ///
  /// Y, como último recurso, **en el PATH**: a diferencia de librespot (que se
  /// compila con opciones concretas para esta app), yt-dlp es el mismo binario
  /// que empaqueta cualquier distribución, y YouTube le rompe los extractores
  /// cada pocas semanas. Si el usuario tiene uno instalado y más reciente que
  /// el que vino en el paquete, es mejor que el nuestro; y si el suyo es lo
  /// único que hay, NeoTube funciona igual en vez de no reproducir nada.
  static File? _bin;
  static bool _buscado = false;

  static File? findYtDlpBinary({bool refrescar = false}) {
    if (refrescar) {
      _bin = null;
      _buscado = false;
    }
    if (_buscado) return _bin;
    _buscado = true;
    final nombre = 'yt-dlp$sufijoEjecutable';
    for (final c in [
      p.join(p.dirname(Platform.resolvedExecutable), nombre),
      p.join(Directory.current.path, 'tool', 'ytdlp-build', 'bin', nombre),
    ]) {
      final f = File(c);
      if (f.existsSync()) return _bin = f;
    }
    return _bin = _enElPath(nombre);
  }

  static File? _denoBin;
  static bool _denoBuscado = false;

  /// Busca el runtime de JavaScript (Deno) para yt-dlp.
  ///
  /// Si está presente (junto al ejecutable, en tool/ytdlp-build/bin/ o en el
  /// PATH), se le pasa a yt-dlp con `--js-runtimes deno:<ruta>` para resolver
  /// el desafío de JavaScript sin advertencias de deprecación ni throttling.
  static File? findDenoBinary({bool refrescar = false}) {
    if (refrescar) {
      _denoBin = null;
      _denoBuscado = false;
    }
    if (_denoBuscado) return _denoBin;
    _denoBuscado = true;
    final nombre = 'deno$sufijoEjecutable';
    for (final c in [
      p.join(p.dirname(Platform.resolvedExecutable), nombre),
      p.join(Directory.current.path, 'tool', 'ytdlp-build', 'bin', nombre),
    ]) {
      final f = File(c);
      if (f.existsSync()) return _denoBin = f;
    }
    return _denoBin = _enElPath(nombre);
  }

  /// Recorre el PATH a mano en vez de dejar que lo resuelva el sistema al
  /// lanzar el proceso: así Ajustes puede decir **dónde** está el que se va a
  /// usar, que es la mitad de lo que hace falta cuando algo no reproduce.
  static File? _enElPath(String nombre) {
    final path = Platform.environment['PATH'];
    if (path == null) return null;
    for (final dir in path.split(Platform.isWindows ? ';' : ':')) {
      if (dir.trim().isEmpty) continue;
      try {
        final f = File(p.join(dir, nombre));
        if (f.existsSync()) return f;
      } catch (_) {
        // Una entrada del PATH con caracteres inválidos no debe tirar la
        // búsqueda entera: se salta y se sigue con las demás.
      }
    }
    return null;
  }

  /// Una ruta limpia para los subprocesos. Sin esto, `Process.run` hereda el
  /// PATH entero del sistema, que en la máquina de un desarrollador puede
  /// tener Python 3.13, msys64 o binarios incompatibles por delante de lo
  /// nuestro.
  static final String _pathMinimo = () {
    final dirs = <String>[];
    if (Platform.isWindows) {
      final sys = Platform.environment['SystemRoot'] ?? r'C:\Windows';
      dirs.addAll([
        p.join(sys, 'System32'),
        sys,
        p.join(sys, 'System32', 'Wbem'),
      ]);
    } else {
      dirs.addAll(['/usr/bin', '/bin', '/usr/local/bin']);
    }
    return dirs.join(Platform.isWindows ? ';' : ':');
  }();

  static Future<ProcessResult> _ejecutar(String exe, List<String> args) {
    return Process.run(
      exe,
      args,
      // Solo se pisa PATH: el resto del entorno (SystemRoot, TEMP…) sigue
      // haciendo falta, y sin él ni siquiera arranca el intérprete embebido.
      environment: {'PATH': _pathMinimo},
    ).timeout(const Duration(seconds: 25));
  }

  /// La versión de yt-dlp instalada, o `null` si no responde.
  ///
  /// Sirve para que Ajustes pueda decir algo mejor que "está el fichero": un
  /// binario presente pero roto (descarga a medias, antivirus que lo ha
  /// vaciado) se ve exactamente igual desde fuera, y el síntoma sería que
  /// ninguna canción arranca sin más pista que un error por pista.
  static Future<String?> versionDeYtDlp() async {
    final bin = findYtDlpBinary();
    if (bin == null) return null;
    try {
      final res = await _ejecutar(bin.path, ['--version']);
      if (res.exitCode != 0) return null;
      final v = (res.stdout as String).trim();
      return v.isEmpty ? null : v;
    } catch (_) {
      return null;
    }
  }

  /// Resuelve la URL del stream de solo-audio de un vídeo, con caché.
  ///
  /// Deduplica resoluciones concurrentes mediante [_enVuelo]: el adelanto de
  /// [_adelantarSiguiente] y una pulsación del usuario sobre la misma pista no
  /// deben salir dos veces a la vez.
  Future<String> _resolverUrl(String videoId) async {
    final guardada = _urls[videoId];
    if (guardada != null && guardada.hasta.isAfter(DateTime.now())) return guardada.url;

    final enMarcha = _enVuelo[videoId];
    if (enMarcha != null) return enMarcha;

    final future = _resolverUrlDirecto(videoId);
    _enVuelo[videoId] = future;
    try {
      final url = await future;
      if (_urls.length > 24) _urls.remove(_urls.keys.first);
      _urls[videoId] = (url: url, hasta: DateTime.now().add(_vidaDeUrl));
      return url;
    } finally {
      _enVuelo.remove(videoId);
    }
  }

  /// Primero la API, y solo si falla, yt-dlp.
  ///
  /// El plan B se conserva por lo que la vía rápida no cubre: vídeos con
  /// restricción de edad, o el día que YouTube cambie algo y el cliente `IOS`
  /// deje de servir. Cuesta unos 2,6 s frente a los ~90 ms de la vía rápida,
  /// así que es exactamente eso, un plan B, y no un camino que se tome a
  /// menudo.
  ///
  /// Cuando el problema es del vídeo y no del método (retirado, privado,
  /// bloqueado), [YtStreamException.reintentarConYtDlp] viene a `false` y no se
  /// lanza el proceso: fallaría igual, tres segundos más tarde.
  Future<String> _resolverUrlDirecto(String videoId) async {
    try {
      return await _stream.resolver(videoId, hl: YtMusicApi.hl, gl: YtMusicApi.gl);
    } on YtStreamException catch (e) {
      if (!e.reintentarConYtDlp) throw YtPlayerException('$e');
      if (findYtDlpBinary() == null) throw YtPlayerException('$e');
      debugPrint('NeoTube: la vía rápida falló ($e). Probando con yt-dlp.');
      return _resolverConYtDlp(videoId);
    }
  }

  /// El plan B de toda la vida: `yt-dlp -f bestaudio -g` imprime la URL
  /// resuelta en stdout y termina solo. `Process.run` (sin `runInShell`, que es
  /// el que de verdad abriría una shell) no pasa por ninguna shell del sistema.
  Future<String> _resolverConYtDlp(String videoId) async {
    final bin = findYtDlpBinary();
    if (bin == null) {
      throw YtPlayerException(
          'No se encuentra yt-dlp. Ejecuta tool/fetch_ytdlp.ps1 (o .sh en Linux).');
    }
    final deno = findDenoBinary();
    final args = <String>[
      if (deno != null) ...['--js-runtimes', 'deno:${deno.path}'],
      '-f',
      'bestaudio',
      '-g',
      'https://www.youtube.com/watch?v=$videoId',
    ];
    final res = await _ejecutar(bin.path, args);
    if (res.exitCode != 0) {
      final stderr = '${res.stderr}';
      debugPrint('NeoTube: yt-dlp falló (exit ${res.exitCode}):\n$stderr');
      throw YtPlayerException(_motivoDeFalloYtDlp(stderr));
    }
    final url = (res.stdout as String).trim().split('\n').first.trim();
    if (url.isEmpty) throw YtPlayerException('yt-dlp no devolvió ninguna URL.');
    return url;
  }

  /// Descarga **la pista entera** a un fichero, para cuando la vía rápida
  /// (`ANDROID_VR`) se quede corta pasado el primer megabyte — ver
  /// `yt_local_proxy.dart` para el porqué del corte, y [_continuarTrasCorte]
  /// para qué se hace con este fichero. Se lanza en paralelo con la
  /// reproducción, no antes: se llama sin esperar el resultado.
  ///
  /// `WEB_EMBEDDED_PLAYER` es el único cliente probado que no se corta ahí,
  /// pero **no vale con resolver su URL y pedirla desde aquí**: esa URL solo
  /// la sirve un cliente con la huella TLS de un navegador de verdad
  /// (`curl_cffi`, que usa yt-dlp por debajo), y `dart:io HttpClient` no la
  /// replica — da `403` aunque las cabeceras coincidan al carácter. Por eso
  /// tiene que ser yt-dlp quien la baje de verdad, no solo quien la resuelva.
  ///
  /// Devuelve `null` en vez de lanzar: quien llama ya está sonando por la vía
  /// rápida, y esto es un complemento, no algo de lo que dependa arrancar.
  Future<File?> _descargarCompletaConYtDlp(String videoId) async {
    final bin = findYtDlpBinary();
    if (bin == null) return null;
    final deno = findDenoBinary();
    final destino = File(p.join((await _dirDeContinuaciones()).path, '$videoId.audio'));
    final args = <String>[
      if (deno != null) ...['--js-runtimes', 'deno:${deno.path}'],
      '-f',
      'bestaudio',
      '--extractor-args',
      'youtube:player_client=web_embedded',
      '-o',
      destino.path,
      'https://www.youtube.com/watch?v=$videoId',
    ];
    try {
      final res = await Process.run(
        bin.path,
        args,
        environment: {'PATH': _pathMinimo},
      ).timeout(const Duration(seconds: 40));
      if (res.exitCode != 0 || !destino.existsSync()) {
        debugPrint('NeoTube: descarga completa de $videoId falló '
            '(exit ${res.exitCode}): ${res.stderr}');
        return null;
      }
      debugPrint('NeoTube: descarga completa de $videoId lista '
          '(${destino.lengthSync()} bytes)');
      _registrarContinuacion(destino);
      return destino;
    } catch (e) {
      debugPrint('NeoTube: descarga completa de $videoId lanzó: $e');
      return null;
    }
  }

  Directory? _carpetaDeContinuaciones;

  Future<Directory> _dirDeContinuaciones() async {
    final actual = _carpetaDeContinuaciones;
    if (actual != null) return actual;
    final dir = Directory(p.join(Directory.systemTemp.path, 'neotube_continuaciones'));
    if (!dir.existsSync()) dir.createSync(recursive: true);
    return _carpetaDeContinuaciones = dir;
  }

  /// Los ficheros de [_descargarCompletaConYtDlp] que están vivos ahora
  /// mismo, del más viejo al más nuevo. Cada pista de más de un minuto deja
  /// uno, y son varios MB cada uno — sin límite, sería una fuga de disco en
  /// una sesión de escucha larga. Se quedan solo los dos últimos: la pista
  /// actual y la anterior, por si un salto hacia atrás la necesita todavía.
  final List<File> _continuacionesGuardadas = [];
  static const _maxContinuacionesGuardadas = 2;

  void _registrarContinuacion(File f) {
    _continuacionesGuardadas.add(f);
    while (_continuacionesGuardadas.length > _maxContinuacionesGuardadas) {
      final viejo = _continuacionesGuardadas.removeAt(0);
      try {
        if (viejo.existsSync()) viejo.deleteSync();
      } catch (_) {
        // Se queda en disco hasta el próximo reinicio; no es grave.
      }
    }
  }

  /// Traduce el `stderr` de yt-dlp (que puede ser un vertedero de varias
  /// líneas de WARNING/ERROR en inglés) a algo corto y en español que tenga
  /// sentido en un SnackBar. El texto completo se queda en el log
  /// (`debugPrint` en [_resolverConYtDlp]) para quien necesite mirarlo de
  /// verdad; esto es solo lo que ve el usuario.
  static String _motivoDeFalloYtDlp(String stderr) {
    final texto = stderr.toLowerCase();
    if (texto.contains('429') || texto.contains('too many requests')) {
      return 'YouTube está limitando las peticiones ahora mismo. '
          'Prueba de nuevo en unos minutos.';
    }
    if (texto.contains('confirm you’re not a bot') ||
        texto.contains("confirm you're not a bot") ||
        texto.contains('sign in to confirm')) {
      return 'YouTube ha pedido confirmar que no eres un robot. '
          'Suele pasar solo, prueba de nuevo en un rato.';
    }
    if (texto.contains('private video')) {
      return 'Ese vídeo es privado.';
    }
    if (texto.contains('video unavailable') || texto.contains('no longer available')) {
      return 'Ese vídeo ya no está disponible.';
    }
    if (texto.contains('not available in your country') ||
        texto.contains('not available on this app')) {
      return 'YouTube no deja reproducir ese vídeo desde aquí.';
    }
    return 'yt-dlp no pudo resolver el vídeo. Consulta la consola para el detalle.';
  }

  /// Adelanta la resolución de las siguientes dos pistas mientras suena la actual.
  /// Falla en silencio a propósito: es un adelanto, y si no llega a tiempo se
  /// resolverá cuando toque como se hacía antes.
  /// Limita a un máximo de 2 resoluciones concurrentes para no saturar procesos ni red.
  void _adelantarSiguiente() {
    if (indice < 0) return;
    for (var offset = 1; offset <= 2; offset++) {
      final pos = indice + offset;
      if (pos >= cola.length) break;
      final proxima = cola[pos];
      final guardada = _urls[proxima.videoId];
      if (guardada != null && guardada.hasta.isAfter(DateTime.now())) continue;
      if (_enVuelo.containsKey(proxima.videoId)) continue;
      if (_enVuelo.length >= 2) break;
      unawaited(_resolverUrl(proxima.videoId).catchError((_) => ''));
    }
  }

  // ------------------------------------------------------------- reproducción

  /// Pone una cola entera a sonar desde [desde].
  Future<void> reproducirLista(
    List<YtTrack> pistas, {
    int desde = 0,
    String? contexto,
  }) async {
    if (pistas.isEmpty || !disponible) return;
    final indiceDeseado = desde.clamp(0, pistas.length - 1);
    final pistaDeseada = pistas[indiceDeseado];
    // Evita doble clic impaciente sobre la misma pista que ya se está abriendo
    if (_cambiando && resolviendo == pistaDeseada.videoId && actual?.videoId == pistaDeseada.videoId) {
      return;
    }
    this.contexto = contexto;
    if (_aleatorio) {
      _colaSinBarajar = List.unmodifiable(pistas);
      final resto = [...pistas]..removeAt(indiceDeseado);
      resto.shuffle();
      cola = List.unmodifiable([pistaDeseada, ...resto]);
      indice = 0;
    } else {
      cola = List.unmodifiable(pistas);
      indice = indiceDeseado;
    }
    await _abrirActual();
  }

  /// Una canción suelta: empieza a sonar ya, ella sola.
  ///
  /// Quien la lanza suele pedir después su radio y engancharla con [anexar].
  /// Se hace en dos pasos y no en uno a propósito: esperar a la radio antes de
  /// abrir el audio le sumaba a cada pulsación el viaje entero a la API
  /// **antes** del primer sonido, y eso se nota mucho más que quedarse sin
  /// continuación al terminar.
  Future<void> reproducirPista(YtTrack t) => reproducirLista([t]);

  /// Añade pistas al final de la cola actual, sin repetir las que ya estén.
  void anexar(List<YtTrack> pistas) {
    if (pistas.isEmpty || cola.isEmpty) return;
    final vistos = cola.map((t) => t.videoId).toSet();
    final nuevas = pistas.where((t) => vistos.add(t.videoId)).toList();
    if (nuevas.isEmpty) return;
    cola = List.unmodifiable([...cola, ...nuevas]);
    // Si el aleatorio está activo, [_colaSinBarajar] es lo que se restaura al
    // apagarlo: sin esto, las pistas añadidas aquí (la radio de una canción
    // suelta) desaparecerían de golpe la próxima vez que se apague.
    if (_aleatorio && _colaSinBarajar.isNotEmpty) {
      _colaSinBarajar = List.unmodifiable([..._colaSinBarajar, ...nuevas]);
    }
    notifyListeners();
    _adelantarSiguiente();
  }

  Future<void> _abrirActual() async {
    final t = actual;
    final mpv = _mpv;
    if (t == null || mpv == null) return;
    _temporizadorAbriendo?.cancel();
    _pistaAbriendo = t.videoId;
    _reintentosHechos = 0;
    _cambiando = true;
    resolviendo = t.videoId;
    error = null;
    notifyListeners();
    final reloj = Stopwatch()..start();
    try {
      await alEmpezarAReproducir?.call();
      final url = await _resolverUrl(t.videoId);
      // Una línea por cambio de pista, con el número que justifica todo este
      // fichero: si algún día vuelve a marcar segundos en vez de milisegundos,
      // es que se está cayendo al plan B en cada pista y nadie se ha enterado.
      debugPrint('NeoTube: ${t.videoId} resuelto en ${reloj.elapsedMilliseconds}ms');
      _pistaConCorte = null;
      _continuaciones.putIfAbsent(t.videoId, () => _descargarCompletaConYtDlp(t.videoId));
      final urlLocal = await _proxy.exponer(
        url,
        onCorte: () => _pistaConCorte = t.videoId,
      );
      await mpv.open(Media(urlLocal.toString()));
      _adelantarSiguiente();
      _temporizadorAbriendo = Timer(const Duration(seconds: 7), () {
        if (_pistaAbriendo == t.videoId) _pistaAbriendo = null;
      });
    } catch (e, pila) {
      // Sin esto, una pista que no arranca no deja **ni una línea** en el log:
      // el error acaba en `error` y de ahí a la interfaz, y quien mira la
      // consola no ve nada de nada. Es exactamente cómo se perdió una tarde.
      debugPrint('NeoTube: no se pudo abrir ${t.videoId}: $e\n$pila');
      _pistaAbriendo = null;
      error = '$e';
      rethrow;
    } finally {
      resolviendo = null;
      _cambiando = false;
      notifyListeners();
    }
  }

  /// Reintenta abrir la pista **por la otra vía** cuando libmpv rechaza la URL.
  ///
  /// Pedirle otra URL al mismo sitio no sirve de nada cuando el problema no es
  /// que la URL esté caducada sino que googlevideo le contesta `403` a ffmpeg:
  /// la siguiente sale igual y falla igual, que es exactamente lo que se veía
  /// —`Failed to open` seguidos por pista, con `reintentosHechos` subiendo de
  /// uno en uno—.
  ///
  /// Así que el reintento va por yt-dlp. Cuesta sus ~2,6 s, pero solo se paga
  /// cuando la vía rápida ya ha fallado, y es la que se ha visto sonar cuando
  /// la otra no. Si tampoco hay yt-dlp, se vuelve a intentar como antes: es
  /// mejor que rendirse sin probar.
  Future<void> _reintentarConUrlFresca(YtTrack t) async {
    _urls.remove(t.videoId);
    _enVuelo.remove(t.videoId);
    // La URL que acaba de rechazar libmpv la emitió la identidad de visitante
    // actual, así que se tira: si sigue en pie, las siguientes pistas saldrían
    // con el mismo problema y todas acabarían pagando el plan B. Las URLs ya
    // adelantadas también se van, que se emitieron con esa misma identidad.
    _stream.renovarVisitante();
    _urls.clear();
    try {
      final urlFresca = findYtDlpBinary() != null
          ? await _resolverConYtDlp(t.videoId)
          : await _resolverUrl(t.videoId);
      debugPrint('NeoTube: reintento de ${t.videoId} por '
          '${findYtDlpBinary() != null ? 'yt-dlp' : 'la vía rápida'}');
      // Si el usuario saltó a otra pista mientras se resolvía la URL fresca,
      // no abrimos ni modificamos el estado de la pista nueva.
      if (actual?.videoId != t.videoId) return;
      _continuaciones.putIfAbsent(t.videoId, () => _descargarCompletaConYtDlp(t.videoId));
      final urlLocal = await _proxy.exponer(
        urlFresca,
        onCorte: () => _pistaConCorte = t.videoId,
      );
      await _mpv?.open(Media(urlLocal.toString()));
      _temporizadorAbriendo?.cancel();
      _temporizadorAbriendo = Timer(const Duration(seconds: 7), () {
        if (_pistaAbriendo == t.videoId) _pistaAbriendo = null;
      });
    } catch (e) {
      if (actual?.videoId == t.videoId) {
        _pistaAbriendo = null;
        error = 'Error de reproducción: $e';
        notifyListeners();
      }
    }
  }

  /// Retoma una pista que se quedó a medias por el corte de posición de la
  /// vía rápida (ver `yt_local_proxy.dart`) — lo llama el listener de
  /// `mpv.stream.completed` cuando ese "completado" no era el final de
  /// verdad. **No sigue la misma conexión**: eso es justo lo que no funciona
  /// (empalmar bytes de dos descargas independientes corrompe el contenedor,
  /// ver el porqué en `yt_local_proxy.dart`). En vez de eso, reabre en el
  /// fichero de la descarga completa y salta a donde iba con `Media.start`,
  /// que dos ficheros de verdad sí soportan sin problema.
  Future<void> _continuarTrasCorte(YtTrack t) async {
    _pistaConCorte = null;
    final posicion = _mpv?.state.position ?? Duration.zero;
    final futuro = _continuaciones[t.videoId];
    final archivo = futuro == null ? null : await futuro;
    // Si mientras se esperaba la descarga el usuario ya saltó de pista, no
    // hay nada que retomar: la pista actual es otra.
    if (actual?.videoId != t.videoId) return;
    if (archivo == null || !archivo.existsSync()) {
      // No hay descarga de repuesto (falló, o no dio tiempo): se acepta el
      // corte como si de verdad hubiera sido el final.
      if (cola.isNotEmpty) unawaited(siguiente(automatico: true));
      return;
    }
    debugPrint('NeoTube: retomando ${t.videoId} desde $posicion tras el corte');
    _cambiando = true;
    notifyListeners();
    try {
      await _mpv?.open(Media(archivo.path, start: posicion));
    } catch (e) {
      debugPrint('NeoTube: no se pudo retomar ${t.videoId}: $e');
      if (actual?.videoId == t.videoId && cola.isNotEmpty) {
        unawaited(siguiente(automatico: true));
      }
    } finally {
      _cambiando = false;
      notifyListeners();
    }
  }

  /// Se avisa cuando la cola se acaba **sola** (no por un salto del usuario).
  ///
  /// NeoTube lo deja a `null`: allí la cola de este reproductor es toda la cola
  /// que hay. La vía libre de NeoFy sí lo usa, porque allí la cola de verdad es
  /// la de canciones de Spotify y esta solo tiene la que suena ahora.
  Future<void> Function()? alAcabarLaCola;

  /// Salta a la siguiente. Cuando lo pide la cola sola ([automatico]) y la
  /// pista falla, se sigue bajando en vez de parar: un vídeo bloqueado por
  /// región en mitad de una playlist de 300 no debe terminar la escucha.
  Future<void> siguiente({bool automatico = false}) async {
    final mpv = _mpv;
    if (mpv == null) return;
    if (!puedeSaltar) {
      if (!automatico) return;
      if (alAcabarLaCola != null) {
        await alAcabarLaCola!();
        return;
      }
      // Fin de la cola: se para, pero se deja la pista puesta para que el
      // botón de play la pueda volver a arrancar.
      await mpv.pause();
      await mpv.seek(Duration.zero);
      return;
    }
    indice++;
    try {
      await _abrirActual();
    } catch (_) {
      if (automatico && puedeSaltar) await siguiente(automatico: true);
    }
  }

  Future<void> anterior() async {
    final mpv = _mpv;
    if (mpv == null) return;
    // Igual que en NeoFy: pasados 3 s, "anterior" reinicia la canción.
    if (mpv.state.position.inSeconds > 3 || indice <= 0) {
      await mpv.seek(Duration.zero);
      return;
    }
    indice--;
    await _abrirActual();
  }

  /// Salta a una posición concreta de la cola (la lista de la pantalla de
  /// reproducción, o pulsar una fila de una playlist ya sonando).
  Future<void> saltarA(int i) async {
    if (i < 0 || i >= cola.length) return;
    indice = i;
    await _abrirActual();
  }

  Future<void> alternar() async {
    final mpv = _mpv;
    if (mpv == null) return;
    // Si la cola llegó al final, un `play` pelado no arranca nada: se vuelve a
    // empezar por donde se quedó. Mismo criterio que `PlayerController`.
    if (!mpv.state.playing &&
        actual != null &&
        mpv.state.position >= mpv.state.duration &&
        mpv.state.duration > Duration.zero) {
      await mpv.seek(Duration.zero);
    }
    await (mpv.state.playing ? mpv.pause() : mpv.play());
  }

  /// Un salto de la barra hay que anunciarlo aparte al panel del sistema: los
  /// dos reproductores del escritorio extrapolan la posición por su cuenta y
  /// sin esto se quedan enseñando el minuto de antes. Mismo enganche que el
  /// `onSalto` de `PlayerController`.
  void Function(int ms)? onSalto;

  /// Volumen actual, 0..100. Se guarda aquí y no se lee de
  /// `player.state.volume` porque la barra tiene que poder pintarlo antes de
  /// que haya sonado nada.
  final ValueNotifier<int> _volumen;
  int get volumen => _volumen.value;
  ValueListenable<int> get cambiosDeVolumen => _volumen;

  /// Se llama al soltar el mando del volumen, para persistirlo. No en cada
  /// paso del arrastre: eso serían decenas de escrituras del config por cada
  /// vez que alguien mueve el mando.
  void Function(int volumen)? onVolumenFijado;

  /// Aplicar el volumen es instantáneo y local (libmpv, en este mismo
  /// proceso), así que **sí** se hace en cada paso del arrastre — al revés que
  /// en NeoFy, donde cada paso sería una petición a la Web API y Spotify
  /// acababa devolviendo 429.
  Future<void> setVolumen(int v) async {
    final nuevo = v.clamp(0, 100);
    if (nuevo == _volumen.value) return;
    _volumen.value = nuevo;
    await _mpv?.setVolume(nuevo.toDouble());
  }

  Future<void> pause() async => _mpv?.pause();
  Future<void> resume() async => _mpv?.play();

  Future<void> seek(Duration d) async {
    if (_mpv == null) return;
    await _mpv.seek(d);
    onSalto?.call(d.inMilliseconds);
  }

  Future<void> stop() async {
    _temporizadorAbriendo?.cancel();
    _pistaAbriendo = null;
    _reintentosHechos = 0;
    _pistaConCorte = null;
    await _mpv?.stop();
    cola = const [];
    indice = -1;
    contexto = null;
    error = null;
    _colaSinBarajar = const [];
    notifyListeners();
  }

  @override
  void dispose() {
    _temporizadorAbriendo?.cancel();
    unawaited(_subCompletado?.cancel());
    unawaited(_subError?.cancel());
    unawaited(_subDuracion?.cancel());
    unawaited(_subLog?.cancel());
    _stream.dispose();
    _proxy.dispose();
    unawaited(_mpv?.dispose());
    _volumen.dispose();
    super.dispose();
  }
}
