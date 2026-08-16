import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/yt_stream.dart';

/// Lo que se prueba aquí es **cómo se lee la respuesta de `youtubei/v1/player`**,
/// no la red: `flutter test` la tiene mockeada a 400. Los JSON son recortes con
/// la misma forma que devuelve la API real, reducidos a los campos que se leen.
/// Que YouTube siga contestando así se comprueba con `tool/probe_stream.dart`.
void main() {
  Map<String, dynamic> respuesta(List<Map<String, dynamic>> formatos) => {
        'playabilityStatus': {'status': 'OK'},
        'streamingData': {'adaptiveFormats': formatos},
      };

  Map<String, dynamic> audio(int itag, int bitrate, {String? url, String? cifrado}) => {
        'itag': itag,
        'mimeType': 'audio/webm; codecs="opus"',
        'bitrate': bitrate,
        'url': ?url,
        'signatureCipher': ?cifrado,
      };

  group('elección de formato', () {
    test('se queda con el audio de más bitrate', () {
      final url = YtStreamResolver.interpretar(respuesta([
        audio(249, 49000, url: 'https://ejemplo/baja'),
        audio(251, 137000, url: 'https://ejemplo/alta'),
        audio(250, 66000, url: 'https://ejemplo/media'),
      ]));
      expect(url, 'https://ejemplo/alta');
    });

    test('ignora el vídeo aunque venga con más bitrate', () {
      final url = YtStreamResolver.interpretar(respuesta([
        {
          'itag': 137,
          'mimeType': 'video/mp4; codecs="avc1.640028"',
          'bitrate': 4000000,
          'url': 'https://ejemplo/video',
        },
        audio(251, 137000, url: 'https://ejemplo/audio'),
      ]));
      expect(url, 'https://ejemplo/audio');
    });

    /// La regresión que importa. Si YouTube empieza a cifrar también los
    /// formatos del cliente `IOS`, hay que caer a yt-dlp (que sí sabe
    /// descifrarlos, con Deno al lado) y **no** devolver una URL que libmpv no
    /// puede abrir: eso se vería como "salta un error" sin más explicación.
    test('descarta los formatos cifrados: no sabemos abrirlos', () {
      expect(
        () => YtStreamResolver.interpretar(respuesta([
          audio(251, 137000, cifrado: 's=FIRMA&sp=sig&url=https%3A%2F%2Fejemplo'),
        ])),
        throwsA(isA<YtStreamException>()
            .having((e) => e.reintentarConYtDlp, 'cae a yt-dlp', isTrue)),
      );
    });

    test('un formato sin bitrate no tumba la elección', () {
      final url = YtStreamResolver.interpretar(respuesta([
        {'itag': 140, 'mimeType': 'audio/mp4', 'url': 'https://ejemplo/sin-bitrate'},
        audio(251, 137000, url: 'https://ejemplo/con-bitrate'),
      ]));
      expect(url, 'https://ejemplo/con-bitrate');
    });
  });

  group('respuestas que no sirven', () {
    /// Con un vídeo retirado o privado, lanzar yt-dlp solo sirve para esperar
    /// tres segundos y volver a fallar. Por eso el fallo viene marcado.
    test('un vídeo imposible no manda lanzar yt-dlp', () {
      for (final status in ['UNPLAYABLE', 'ERROR']) {
        expect(
          () => YtStreamResolver.interpretar({
            'playabilityStatus': {'status': status, 'reason': 'No disponible'},
          }),
          throwsA(isA<YtStreamException>()
              .having((e) => e.reintentarConYtDlp, 'cae a yt-dlp ($status)', isFalse)),
          reason: status,
        );
      }
    });

    /// `LOGIN_REQUIRED` es el estado con el que llega "confirma que no eres un
    /// bot", y ese **no** es culpa del vídeo: es que falta el `visitorData` o
    /// se ha quedado viejo. Se marca aparte para que el resolutor lo reintente
    /// con uno nuevo, y si aun así falla, yt-dlp sabe resolverlo.
    test('el aviso de bot se marca y sí cae a yt-dlp', () {
      expect(
        () => YtStreamResolver.interpretar({
          'playabilityStatus': {
            'status': 'LOGIN_REQUIRED',
            'reason': 'Inicia sesión para confirmar que no eres un bot',
          },
        }),
        throwsA(isA<YtStreamException>()
            .having((e) => e.avisoDeBot, 'aviso de bot', isTrue)
            .having((e) => e.reintentarConYtDlp, 'cae a yt-dlp', isTrue)),
      );
    });

    test('un fallo normal no se confunde con el aviso de bot', () {
      expect(
        () => YtStreamResolver.interpretar({
          'playabilityStatus': {'status': 'UNPLAYABLE', 'reason': 'No disponible'},
        }),
        throwsA(isA<YtStreamException>()
            .having((e) => e.avisoDeBot, 'aviso de bot', isFalse)),
      );
    });

    /// Cualquier otra cosa sí: puede ser que el cliente haya dejado de valer, y
    /// ahí yt-dlp es justamente la red de seguridad.
    test('un estado desconocido sí cae a yt-dlp', () {
      expect(
        () => YtStreamResolver.interpretar({
          'playabilityStatus': {'status': 'AGE_VERIFICATION_REQUIRED'},
        }),
        throwsA(isA<YtStreamException>()
            .having((e) => e.reintentarConYtDlp, 'cae a yt-dlp', isTrue)),
      );
    });

    test('el motivo de YouTube llega al mensaje, que es lo que ve el usuario', () {
      expect(
        () => YtStreamResolver.interpretar({
          'playabilityStatus': {
            'status': 'UNPLAYABLE',
            'reason': 'Este vídeo no está disponible en tu país',
          },
        }),
        throwsA(isA<YtStreamException>()
            .having((e) => e.message, 'mensaje', contains('no está disponible en tu país'))),
      );
    });

    test('una respuesta sin la forma esperada lanza, no devuelve basura', () {
      for (final json in <Map<String, dynamic>>[
        const {},
        const {'playabilityStatus': {'status': 'OK'}},
        const {'playabilityStatus': {'status': 'OK'}, 'streamingData': {}},
        const {
          'playabilityStatus': {'status': 'OK'},
          'streamingData': {'adaptiveFormats': 'esto no es una lista'},
        },
      ]) {
        expect(() => YtStreamResolver.interpretar(json), throwsA(isA<YtStreamException>()));
      }
    });
  });
}
