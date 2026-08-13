import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:media_kit/media_kit.dart';
import 'package:path/path.dart' as p;

import 'procesos.dart' show sufijoEjecutable;
import 'yt_models.dart';

class YtPlayerException implements Exception {
  final String message;
  YtPlayerException(this.message);
  @override
  String toString() => message;
}

/// Reproduce audio de YouTube sin necesitar cuenta Premium, **con cola**.
///
/// Dos piezas separadas, como en el resto de la app: **resolver** la URL del
/// stream de audio (yt-dlp, un binario que se invoca una vez por pista y no un
/// servidor persistente como `metadata-sidecar`) y **reproducirla**
/// (`media_kit`, que envuelve libmpv y sabe abrir esa URL directamente).
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
/// ## La espera de yt-dlp
///
/// Resolver una URL tarda entre medio segundo y dos. Encadenado a cada cambio
/// de canción eso es un silencio audible entre pistas, así que la siguiente se
/// resuelve **mientras suena la actual** y se guarda en [_urls]. Las URLs que
/// devuelve Google caducan (traen su propio `expire`), de ahí que la caché
/// tenga fecha de caducidad propia y conservadora.
class YtPlayer extends ChangeNotifier {
  YtPlayer({int volumenInicial = 60})
      : _volumen = ValueNotifier<int>(volumenInicial.clamp(0, 100)),
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
    // Cuando una pista termina sola, sigue la cola. `completed` también se
    // emite al abrir un medio nuevo en algunas versiones, de ahí el control
    // de que haya sonado algo de verdad y no estemos ya cambiando de pista.
    _subCompletado = mpv.stream.completed.listen((terminada) {
      if (terminada && !_cambiando && cola.isNotEmpty) unawaited(siguiente(automatico: true));
    });
    _subError = mpv.stream.error.listen((e) {
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

  StreamSubscription<bool>? _subCompletado;
  StreamSubscription<String>? _subError;

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

  YtTrack? get actual => (indice >= 0 && indice < cola.length) ? cola[indice] : null;
  bool get hayNada => actual == null;
  bool get puedeSaltar => indice + 1 < cola.length;
  bool get puedeVolver => cola.isNotEmpty;

  /// Qué pista se está resolviendo ahora mismo (su `videoId`), para que la
  /// tarjeta pulsada pueda enseñar su ruedecita. `null` si no hay ninguna.
  String? resolviendo;
  String? error;

  bool _cambiando = false;

  /// URLs ya resueltas, con su caducidad. Se limita a un puñado: no es una
  /// caché de verdad, es el adelanto de las siguientes pistas.
  final Map<String, ({String url, DateTime hasta})> _urls = {};
  final Map<String, Future<String>> _enVuelo = {};
  static const _vidaDeUrl = Duration(minutes: 45);

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

  /// Resuelve la URL del stream de solo-audio de un vídeo con yt-dlp.
  ///
  /// Una invocación, no un proceso vigilado: `yt-dlp -f bestaudio -g` imprime
  /// la URL resuelta en stdout y termina solo. `Process.run` (sin
  /// `runInShell`, que es el que de verdad abriría una shell) no pasa por
  /// ninguna shell del sistema.
  ///
  /// Deduplica resoluciones concurrentes mediante [_enVuelo] para no lanzar
  /// múltiples procesos de yt-dlp para la misma pista.
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

  Future<String> _resolverUrlDirecto(String videoId) async {
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
      throw YtPlayerException('yt-dlp no pudo resolver el vídeo: ${res.stderr}');
    }
    final url = (res.stdout as String).trim().split('\n').first.trim();
    if (url.isEmpty) throw YtPlayerException('yt-dlp no devolvió ninguna URL.');
    return url;
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
    cola = List.unmodifiable(pistas);
    this.contexto = contexto;
    indice = indiceDeseado;
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
    notifyListeners();
    _adelantarSiguiente();
  }

  Future<void> _abrirActual() async {
    final t = actual;
    final mpv = _mpv;
    if (t == null || mpv == null) return;
    _cambiando = true;
    resolviendo = t.videoId;
    error = null;
    notifyListeners();
    try {
      await alEmpezarAReproducir?.call();
      final url = await _resolverUrl(t.videoId);
      try {
        await mpv.open(Media(url));
      } catch (_) {
        // libmpv no pudo abrir esta URL en concreto (hipo de red, borde de CDN
        // o bloqueo transitorio de YouTube, no necesariamente que el vídeo
        // esté roto): se invalida la URL en caché y se pide una fresca a
        // yt-dlp antes de rendirse. "Siguiente canción" arreglaba esto a mano
        // justamente porque disparaba una resolución nueva.
        _urls.remove(t.videoId);
        _enVuelo.remove(t.videoId);
        final urlFresca = await _resolverUrl(t.videoId);
        await mpv.open(Media(urlFresca));
      }
      _adelantarSiguiente();
    } catch (e) {
      error = '$e';
      rethrow;
    } finally {
      resolviendo = null;
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
    await _mpv?.stop();
    cola = const [];
    indice = -1;
    contexto = null;
    error = null;
    notifyListeners();
  }

  @override
  void dispose() {
    unawaited(_subCompletado?.cancel());
    unawaited(_subError?.cancel());
    unawaited(_mpv?.dispose());
    _volumen.dispose();
    super.dispose();
  }
}
