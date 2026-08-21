import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as p;

import 'yt_models.dart';

/// El asset de Rich Presence subido al Discord Developer Portal.
///
/// ⚠️ **Discord distingue mayúsculas y no avisa cuando no encuentra el asset.**
/// Simplemente no pinta la imagen: ni error, ni hueco, ni nada en el log. Aquí
/// estuvo puesto `'Logo'` mientras el asset subido se llamaba `logo`, y el
/// síntoma era exactamente ese, una presencia sin minilogo.
///
/// **Cómo comprobar el nombre de verdad**, sin entrar al portal ni autenticarse
/// —la lista de assets de una aplicación es pública—:
///
/// ```
/// curl https://discord.com/api/v9/oauth2/applications/<clientId>/assets
/// [{"id":"1538661613006094571","type":1,"name":"logo"}]
/// ```
const String kDiscordAssetLogo = 'logo';

const String kDiscordGithubUrl = 'https://github.com/KOLPSE/NeoTube';

/// El nombre de la app tal cual se escribe. Se usa en los textos de la
/// presencia; una sola fuente para no volver a escribirlo a mano.
const String kDiscordNombre = 'NeoTube';

/// Discord Rich Presence por IPC nativo, sin paquete de terceros.
///
/// Es la misma implementación que NeoFy (`spotify-native/lib/core/discord_rpc.dart`),
/// adaptada a [YtTrack]: la cola vive en este proceso, no hay `sync_id` de
/// Spotify, y el asset del minilogo es [kDiscordAssetLogo].
class DiscordRpc {
  DiscordRpc({
    DiscordTransport? transporte,
    Duration timeoutPausa = const Duration(minutes: 5),
  })  : _transporte = transporte ?? _crearTransporte(),
        _duracionTimeoutPausa = timeoutPausa;

  final DiscordTransport _transporte;
  final Duration _duracionTimeoutPausa;
  String _clientId = '';
  bool _activo = false;
  Timer? _reintentoTimer;
  Timer? _timeoutPausa;
  bool _pausaExpirada = false;
  int _nonce = 0;

  String? _ultimoVideoId;
  YtTrack? _siguienteTrack;
  bool _cargandoCola = false;

  YtTrack? _ultimoTrack;
  bool _ultimoSonando = false;
  int _ultimoProgresoMs = 0;
  int _ultimaDuracionMs = 0;
  String? _ultimaFirma;

  String get clientId => _clientId;
  bool get activo => _activo;
  bool get conectado => _transporte.conectado;

  void start(String clientId) {
    final nuevoId = clientId.trim();
    if (nuevoId.isEmpty) {
      unawaited(stop());
      return;
    }
    if (_activo && _clientId == nuevoId && _transporte.conectado) {
      return;
    }
    _clientId = nuevoId;
    _activo = true;
    _iniciarConexion();

    _reintentoTimer?.cancel();
    _reintentoTimer = Timer.periodic(const Duration(seconds: 15), (_) {
      if (_activo && !_transporte.conectado) {
        _iniciarConexion();
      }
    });
  }

  void _iniciarConexion() {
    if (!_activo || _clientId.isEmpty) return;
    unawaited(_conectar());
  }

  Future<void> _conectar() async {
    try {
      final ok = await _transporte.conectar(_clientId);
      if (ok && _activo) {
        if (_ultimoTrack != null && !_pausaExpirada) {
          await _enviarActividad(
            track: _ultimoTrack,
            siguiente: _siguienteTrack,
            sonando: _ultimoSonando,
            progresoMs: _ultimoProgresoMs,
            duracionMs: _ultimaDuracionMs,
            forzar: true,
          );
        }
      }
    } catch (_) {}
  }

  Future<void> stop() async {
    _activo = false;
    _reintentoTimer?.cancel();
    _reintentoTimer = null;
    _timeoutPausa?.cancel();
    _timeoutPausa = null;
    _pausaExpirada = false;
    _ultimoVideoId = null;
    _siguienteTrack = null;
    _ultimoTrack = null;
    _ultimaFirma = null;

    if (_transporte.conectado) {
      try {
        await _enviarPayloadLimpiar();
      } catch (_) {}
    }
    await _transporte.desconectar();
  }

  Future<void> limpiar() async {
    _timeoutPausa?.cancel();
    _timeoutPausa = null;
    _ultimoVideoId = null;
    _siguienteTrack = null;
    _ultimoTrack = null;
    _ultimaFirma = null;

    if (_transporte.conectado) {
      try {
        await _enviarPayloadLimpiar();
      } catch (_) {}
    }
  }

  void actualizarActividad({
    required YtTrack? track,
    required bool sonando,
    required int progresoMs,
    required int duracionMs,
    required Future<List<YtTrack>> Function() obtenerCola,
  }) {
    _ultimoTrack = track;
    _ultimoSonando = sonando;
    _ultimoProgresoMs = progresoMs;
    _ultimaDuracionMs = duracionMs;

    if (track == null) {
      _timeoutPausa?.cancel();
      _timeoutPausa = null;
      _pausaExpirada = false;
      unawaited(limpiar());
      return;
    }

    if (!_activo || _clientId.isEmpty) return;

    if (track.videoId != _ultimoVideoId) {
      _pausaExpirada = false;
      _timeoutPausa?.cancel();
      _timeoutPausa = null;
    }

    if (sonando) {
      _pausaExpirada = false;
      _timeoutPausa?.cancel();
      _timeoutPausa = null;
    } else if (!_pausaExpirada && _timeoutPausa == null) {
      _timeoutPausa = Timer(_duracionTimeoutPausa, () {
        _timeoutPausa = null;
        _pausaExpirada = true;
        unawaited(limpiar());
      });
    }

    if (_pausaExpirada) return;

    if (track.videoId != _ultimoVideoId) {
      _ultimoVideoId = track.videoId;
      _siguienteTrack = null;
      if (!_cargandoCola) {
        _cargandoCola = true;
        obtenerCola().then((cola) {
          _cargandoCola = false;
          if (_ultimoVideoId == track.videoId) {
            _siguienteTrack = cola.isNotEmpty ? cola.first : null;
            if (!_pausaExpirada) {
              unawaited(_enviarActividad(
                track: track,
                siguiente: _siguienteTrack,
                sonando: _ultimoSonando,
                progresoMs: _ultimoProgresoMs,
                duracionMs: _ultimaDuracionMs,
              ));
            }
          }
        }).catchError((_) {
          _cargandoCola = false;
        });
      }
    }

    unawaited(_enviarActividad(
      track: track,
      siguiente: _siguienteTrack,
      sonando: sonando,
      progresoMs: progresoMs,
      duracionMs: duracionMs,
    ));
  }

  Future<void> _enviarActividad({
    required YtTrack? track,
    YtTrack? siguiente,
    required bool sonando,
    required int progresoMs,
    int duracionMs = 0,
    bool forzar = false,
  }) async {
    if (!_activo || !_transporte.conectado || track == null) return;

    final ahora = DateTime.now();
    // La duración entra en la firma porque **llega tarde**: cuando cambia la
    // pista, libmpv todavía no la sabe y se manda un 0. Sin tenerla en cuenta,
    // la actualización que sí la trae se descartaba por repetida y la barra de
    // progreso de Discord no aparecía nunca.
    final firma =
        '${track.videoId}|$sonando|${siguiente?.videoId}|${duracionMs > 0}';

    if (!forzar && firma == _ultimaFirma) {
      return;
    }

    _ultimaFirma = firma;

    final actividad = construirActividad(
      track: track,
      siguiente: siguiente,
      sonando: sonando,
      progresoMs: progresoMs,
      duracionMs: duracionMs,
      ahora: ahora,
    );

    final payload = construirPayloadSetActivity(
      pid: pid,
      nonce: (++_nonce).toString(),
      activity: actividad,
    );

    await _transporte.enviar(1, jsonEncode(payload));
  }

  Future<void> _enviarPayloadLimpiar() async {
    final payload = construirPayloadSetActivity(
      pid: pid,
      nonce: (++_nonce).toString(),
      activity: null,
    );
    await _transporte.enviar(1, jsonEncode(payload));
  }

  static Map<String, dynamic> construirActividad({
    required YtTrack track,
    YtTrack? siguiente,
    required bool sonando,
    required int progresoMs,
    int duracionMs = 0,
    DateTime? ahora,
  }) {
    final stateStr = siguiente != null && siguiente.titulo.isNotEmpty
        ? '${track.artista} · Siguiente: ${siguiente.titulo}'
        : track.artista;
    final startEpoch =
        (ahora ?? DateTime.now()).millisecondsSinceEpoch - progresoMs;

    // ⚠️ La duración del reproductor manda sobre la de la pista, y por eso se
    // pasa desde fuera: **`YtTrack.duracion` viene vacía en media app**. Solo
    // la rellenan el parseo de listas y el del panel; las tarjetas de portada y
    // de búsqueda no la traen, así que una canción lanzada desde ahí llegaba
    // aquí sin duración.
    //
    // Y sin duración no hay `end`, y sin `end` **Discord no pinta la barra de
    // progreso**: enseña el tiempo corriendo hacia arriba y ya. libmpv sí sabe
    // cuánto dura lo que está sonando, así que se usa eso y la pista queda de
    // reserva.
    final duracionFinal =
        duracionMs > 0 ? duracionMs : (track.duracion?.inMilliseconds ?? 0);

    final caratula = sonando ? track.miniatura : null;

    return {
      'type': 2,
      'details': track.titulo,
      'state': stateStr,
      if (sonando)
        'timestamps': {
          'start': startEpoch,
          if (duracionFinal > 0) 'end': startEpoch + duracionFinal,
        },
      'assets': {
        'large_image': caratula ?? kDiscordAssetLogo,
        'large_text': track.artista.isNotEmpty ? track.artista : kDiscordNombre,
        'small_image': kDiscordAssetLogo,
        // Solo el nombre, no la URL: Discord **recorta el tooltip del minilogo
        // por ancho**, no por el límite de la API (que son 128 caracteres). Con
        // `github.com/KOLPSE/NeoTube` se cortaba a media palabra y se leía
        // «…NeoTub», que parece una errata nuestra. La URL entera ya la lleva el
        // botón de GitHub, así que aquí no aporta nada.
        'small_text': kDiscordNombre,
      },
      'buttons': const [
        {
          'label': 'GitHub',
          'url': kDiscordGithubUrl,
        },
      ],
    };
  }

  static Map<String, dynamic> construirPayloadSetActivity({
    required int pid,
    required String nonce,
    Map<String, dynamic>? activity,
  }) =>
      {
        'cmd': 'SET_ACTIVITY',
        'nonce': nonce,
        'args': {
          'pid': pid,
          'activity': activity,
        },
      };

  static Uint8List empaquetar(int opcode, String jsonStr) {
    final payloadBytes = utf8.encode(jsonStr);
    final bytes = Uint8List(8 + payloadBytes.length);
    final data = ByteData.sublistView(bytes);
    data.setUint32(0, opcode, Endian.little);
    data.setUint32(4, payloadBytes.length, Endian.little);
    bytes.setRange(8, 8 + payloadBytes.length, payloadBytes);
    return bytes;
  }

  void dispose() {
    unawaited(stop());
  }
}

abstract class DiscordTransport {
  bool get conectado;
  Future<bool> conectar(String clientId);
  Future<bool> enviar(int opcode, String jsonStr);
  Future<void> desconectar();
}

DiscordTransport _crearTransporte() {
  if (Platform.isWindows) return _WindowsPipeTransport();
  if (Platform.isLinux) return _LinuxSocketTransport();
  return _NoopTransport();
}

class _NoopTransport implements DiscordTransport {
  @override
  bool get conectado => false;

  @override
  Future<bool> conectar(String clientId) async => false;

  @override
  Future<bool> enviar(int opcode, String jsonStr) async => false;

  @override
  Future<void> desconectar() async {}
}

typedef _CreateFileWC = IntPtr Function(
  Pointer<Utf16> lpFileName,
  Uint32 dwDesiredAccess,
  Uint32 dwShareMode,
  Pointer<Void> lpSecurityAttributes,
  Uint32 dwCreationDisposition,
  Uint32 dwFlagsAndAttributes,
  IntPtr hTemplateFile,
);
typedef _CreateFileWDart = int Function(
  Pointer<Utf16> lpFileName,
  int dwDesiredAccess,
  int dwShareMode,
  Pointer<Void> lpSecurityAttributes,
  int dwCreationDisposition,
  int dwFlagsAndAttributes,
  int hTemplateFile,
);

typedef _ReadFileC = Int32 Function(
  IntPtr hFile,
  Pointer<Uint8> lpBuffer,
  Uint32 nNumberOfBytesToRead,
  Pointer<Uint32> lpNumberOfBytesRead,
  Pointer<Void> lpOverlapped,
);
typedef _ReadFileDart = int Function(
  int hFile,
  Pointer<Uint8> lpBuffer,
  int nNumberOfBytesToRead,
  Pointer<Uint32> lpNumberOfBytesRead,
  Pointer<Void> lpOverlapped,
);

typedef _WriteFileC = Int32 Function(
  IntPtr hFile,
  Pointer<Uint8> lpBuffer,
  Uint32 nNumberOfBytesToWrite,
  Pointer<Uint32> lpNumberOfBytesWritten,
  Pointer<Void> lpOverlapped,
);
typedef _WriteFileDart = int Function(
  int hFile,
  Pointer<Uint8> lpBuffer,
  int nNumberOfBytesToWrite,
  Pointer<Uint32> lpNumberOfBytesWritten,
  Pointer<Void> lpOverlapped,
);

typedef _CloseHandleC = Int32 Function(IntPtr hObject);
typedef _CloseHandleDart = int Function(int hObject);

typedef _PeekNamedPipeC = Int32 Function(
  IntPtr hNamedPipe,
  Pointer<Void> lpBuffer,
  Uint32 nBufferSize,
  Pointer<Uint32> lpBytesRead,
  Pointer<Uint32> lpTotalBytesAvail,
  Pointer<Uint32> lpBytesLeftThisMessage,
);
typedef _PeekNamedPipeDart = int Function(
  int hNamedPipe,
  Pointer<Void> lpBuffer,
  int nBufferSize,
  Pointer<Uint32> lpBytesRead,
  Pointer<Uint32> lpTotalBytesAvail,
  Pointer<Uint32> lpBytesLeftThisMessage,
);

class _WindowsPipeTransport implements DiscordTransport {
  int _handle = 0;
  bool _conectado = false;

  static final _kernel32 =
      Platform.isWindows ? DynamicLibrary.open('kernel32.dll') : null;

  static final _createFile = _kernel32
      ?.lookupFunction<_CreateFileWC, _CreateFileWDart>('CreateFileW');
  static final _readFile =
      _kernel32?.lookupFunction<_ReadFileC, _ReadFileDart>('ReadFile');
  static final _writeFile =
      _kernel32?.lookupFunction<_WriteFileC, _WriteFileDart>('WriteFile');
  static final _closeHandle =
      _kernel32?.lookupFunction<_CloseHandleC, _CloseHandleDart>('CloseHandle');
  static final _peekNamedPipe = _kernel32
      ?.lookupFunction<_PeekNamedPipeC, _PeekNamedPipeDart>('PeekNamedPipe');

  @override
  bool get conectado => _conectado && _handle != 0 && _handle != -1;

  @override
  Future<bool> conectar(String clientId) async {
    await desconectar();

    final createFile = _createFile;
    final writeFile = _writeFile;
    final closeHandle = _closeHandle;
    final peekNamedPipe = _peekNamedPipe;
    final readFile = _readFile;

    if (createFile == null ||
        writeFile == null ||
        closeHandle == null ||
        peekNamedPipe == null ||
        readFile == null) {
      return false;
    }

    for (var i = 0; i < 10; i++) {
      final nombrePipe = r'\\.\pipe\discord-ipc-' + i.toString();
      final pathPtr = nombrePipe.toNativeUtf16();
      final handle = createFile(pathPtr, 0xC0000000, 0, nullptr, 3, 0, 0);
      calloc.free(pathPtr);

      if (handle == 0 || handle == -1) {
        continue;
      }

      final handshakeJson = jsonEncode({'v': 1, 'client_id': clientId});
      final paquete = DiscordRpc.empaquetar(0, handshakeJson);
      final buf = calloc<Uint8>(paquete.length);
      buf.asTypedList(paquete.length).setAll(0, paquete);
      final escritos = calloc<Uint32>();

      final okEscritura =
          writeFile(handle, buf, paquete.length, escritos, nullptr);
      calloc.free(buf);
      calloc.free(escritos);

      if (okEscritura == 0) {
        closeHandle(handle);
        continue;
      }

      var listo = false;
      final bytesDisp = calloc<Uint32>();
      try {
        for (var intento = 0; intento < 40; intento++) {
          final okPeek =
              peekNamedPipe(handle, nullptr, 0, nullptr, bytesDisp, nullptr);
          if (okPeek != 0 && bytesDisp.value >= 8) {
            final totalDisp = bytesDisp.value;
            final bufLectura = calloc<Uint8>(totalDisp);
            final leidos = calloc<Uint32>();
            final okRead =
                readFile(handle, bufLectura, totalDisp, leidos, nullptr);
            if (okRead != 0 && leidos.value >= 8) {
              final data = ByteData.sublistView(
                  bufLectura.asTypedList(leidos.value));
              final opcode = data.getUint32(0, Endian.little);
              final len = data.getUint32(4, Endian.little);
              if (leidos.value >= 8 + len) {
                final jsonStr = utf8.decode(
                    bufLectura.asTypedList(leidos.value).sublist(8, 8 + len));
                final decoded = jsonDecode(jsonStr);
                if (decoded is Map &&
                    (decoded['evt'] == 'READY' || opcode == 1)) {
                  listo = true;
                }
              }
            }
            calloc.free(bufLectura);
            calloc.free(leidos);
            break;
          }
          await Future<void>.delayed(const Duration(milliseconds: 50));
        }
      } catch (_) {
      } finally {
        calloc.free(bytesDisp);
      }

      if (listo) {
        _handle = handle;
        _conectado = true;
        return true;
      } else {
        closeHandle(handle);
      }
    }

    return false;
  }

  @override
  Future<bool> enviar(int opcode, String jsonStr) async {
    final handle = _handle;
    final writeFile = _writeFile;
    if (!_conectado || handle == 0 || handle == -1 || writeFile == null) {
      return false;
    }

    final paquete = DiscordRpc.empaquetar(opcode, jsonStr);
    final buf = calloc<Uint8>(paquete.length);
    buf.asTypedList(paquete.length).setAll(0, paquete);
    final escritos = calloc<Uint32>();

    try {
      final ok = writeFile(handle, buf, paquete.length, escritos, nullptr);
      if (ok == 0) {
        await desconectar();
        return false;
      }
      return true;
    } catch (_) {
      await desconectar();
      return false;
    } finally {
      calloc.free(buf);
      calloc.free(escritos);
    }
  }

  @override
  Future<void> desconectar() async {
    _conectado = false;
    final handle = _handle;
    final closeHandle = _closeHandle;
    if (handle != 0 && handle != -1 && closeHandle != null) {
      closeHandle(handle);
    }
    _handle = 0;
  }
}

class _LinuxSocketTransport implements DiscordTransport {
  Socket? _socket;
  bool _conectado = false;

  @override
  bool get conectado => _conectado && _socket != null;

  @override
  Future<bool> conectar(String clientId) async {
    await desconectar();

    final directorios = [
      Platform.environment['XDG_RUNTIME_DIR'],
      Platform.environment['TMPDIR'],
      '/tmp',
    ].whereType<String>().where((d) => d.isNotEmpty && Directory(d).existsSync());

    for (final dir in directorios) {
      for (var i = 0; i < 10; i++) {
        final ruta = p.join(dir, 'discord-ipc-$i');
        if (!File(ruta).existsSync()) continue;

        try {
          final s = await Socket.connect(
            InternetAddress(ruta, type: InternetAddressType.unix),
            0,
            timeout: const Duration(seconds: 2),
          );

          final completer = Completer<bool>();
          final sub = s.listen(
            (bytes) {
              if (!completer.isCompleted && bytes.length >= 8) {
                try {
                  final data = ByteData.sublistView(Uint8List.fromList(bytes));
                  final len = data.getUint32(4, Endian.little);
                  if (bytes.length >= 8 + len) {
                    final jsonStr = utf8.decode(bytes.sublist(8, 8 + len));
                    final decoded = jsonDecode(jsonStr);
                    if (decoded is Map &&
                        (decoded['evt'] == 'READY' ||
                            decoded['cmd'] == 'DISPATCH')) {
                      completer.complete(true);
                      return;
                    }
                  }
                } catch (_) {}
              }
            },
            onDone: () => desconectar(),
            onError: (_) => desconectar(),
          );

          final paquete = DiscordRpc.empaquetar(
              0, jsonEncode({'v': 1, 'client_id': clientId}));
          s.add(paquete);
          await s.flush();

          final ok = await completer.future.timeout(
            const Duration(seconds: 2),
            onTimeout: () => false,
          );

          if (ok) {
            _socket = s;
            _conectado = true;
            return true;
          } else {
            await sub.cancel();
            s.destroy();
          }
        } catch (_) {}
      }
    }

    return false;
  }

  @override
  Future<bool> enviar(int opcode, String jsonStr) async {
    final s = _socket;
    if (s == null || !_conectado) return false;
    try {
      final paquete = DiscordRpc.empaquetar(opcode, jsonStr);
      s.add(paquete);
      await s.flush();
      return true;
    } catch (_) {
      await desconectar();
      return false;
    }
  }

  @override
  Future<void> desconectar() async {
    _conectado = false;
    try {
      _socket?.destroy();
    } catch (_) {}
    _socket = null;
  }
}
