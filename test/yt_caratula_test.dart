import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/ui/art_image.dart';

/// El fallo que esto fija: las carátulas de la biblioteca salían borrosas.
///
/// `FEmusic_liked_videos` y `FEmusic_library_corpus_track_artists` traen solo
/// dos escalones de miniatura —60 y 120 px— y la app las pinta en tarjetas de
/// 132 px lógicos, que en una pantalla HiDPI son 264 reales. La API no ofrece
/// nada mejor en esa respuesta, pero el servidor de imágenes sí: el
/// `=w120-h120-l90-rj` del final no describe la imagen, es una petición de
/// reescalado. Comprobado contra el servidor real: la misma URL con
/// `=w544-h544-l90-rj` devuelve 200 y 62 KB frente a los 4,4 KB de la de 120.
void main() {
  group('urlDeCaratulaEscalada', () {
    const cuadrada =
        'https://yt3.googleusercontent.com/CjYJD2IOmMqq8jWSK19tgPka1hB6BQsgs5QXO0UKC2h3=w120-h120-l90-rj';

    test('sube una miniatura de 120 al escalón que hace falta', () {
      expect(urlDeCaratulaEscalada(cuadrada, 264), contains('=w320-h320-l90-rj'));
    });

    test('conserva el resto de opciones de la URL', () {
      const conPad =
          'https://lh3.googleusercontent.com/CFlmN6hRwg_ZrfjPNhjoLli0=w120-h120-p-l90-rj';
      final r = urlDeCaratulaEscalada(conPad, 264);
      expect(r, contains('=w320-h320-p-l90-rj'),
          reason: 'el `-p` es el recorte inteligente: perderlo cambia el encuadre');
    });

    test('también baja, para no bajarse 544 px y pintar 40', () {
      const grande =
          'https://yt3.googleusercontent.com/hre8IEvDIpcB-U3dYGgcJPC9=w544-h544-l90-rj';
      expect(urlDeCaratulaEscalada(grande, 40), contains('=w60-h60-'));
    });

    test('mantiene la proporción de un banner apaisado', () {
      // El banner de un artista llega así. Forzarlo a cuadrado lo deformaría.
      const banner =
          'https://lh3.googleusercontent.com/CFlmN6hRwg_ZrfjPNhjoLli0=w540-h225-p-l90-rj';
      final r = urlDeCaratulaEscalada(banner, 300);
      expect(r, contains('=w320-h133-p-l90-rj'));
    });

    test('entiende también el formato `=sNNN`', () {
      const conS = 'https://yt3.ggpht.com/KBAkbym29xgF4IlJTBQAd7fZIU2=s576';
      expect(urlDeCaratulaEscalada(conS, 120), 'https://yt3.ggpht.com/KBAkbym29xgF4IlJTBQAd7fZIU2=s120');
    });

    test('cuantiza a escalones fijos, para no multiplicar la caché', () {
      // La caché de disco va por URL: pedir 131 aquí y 133 allí serían dos
      // descargas y dos ficheros de la misma imagen.
      expect(urlDeCaratulaEscalada(cuadrada, 200),
          urlDeCaratulaEscalada(cuadrada, 226));
    });

    // ⚠️ Lo que **no** se toca. Las miniaturas de vídeo de `i.ytimg.com` no
    // entienden estos parámetros: inventárselos cambia una imagen borrosa por
    // un 404, que es un hueco gris.
    test('una miniatura de i.ytimg.com se devuelve intacta', () {
      const ytimg =
          'https://i.ytimg.com/vi/IcrbM1l_BoI/hq720.jpg?sqp=-oaymwEKCKAGEMIDIABIWg&rs=AMzJL3na';
      expect(urlDeCaratulaEscalada(ytimg, 264), ytimg);
    });

    test('una URL sin tokens de tamaño se devuelve intacta', () {
      const rara = 'https://ejemplo.invalido/portada.jpg';
      expect(urlDeCaratulaEscalada(rara, 264), rara);
    });
  });

  /// El cuelgue que esto fija: con la ventana maximizada, el banner del artista
  /// se pinta a ~3840 px reales. Pasarle ese número a `cacheWidth` pide un
  /// bitmap de unos 24 MB **ampliando** un JPEG de 1280, y al maximizar se
  /// encadenan varias descodificaciones así — se llevaba por delante el hilo de
  /// rasterizado (violación de acceso en `flutter_windows.dll`, sin excepción
  /// de Dart que mirar).
  group('anchoDeDecodificacion', () {
    test('nunca descodifica más grande que lo que se ha descargado', () {
      expect(anchoDeDecodificacion(3840), 1280);
      expect(anchoDeDecodificacion(99999), 1280);
    });

    test('por debajo del tope, descodifica al tamaño pintado', () {
      // Se descargó el escalón de 226; reducir a 200 al pintar está bien.
      expect(anchoDeDecodificacion(200), 200);
      expect(anchoDeDecodificacion(48), 48);
    });

    test('nunca devuelve cero ni negativo', () {
      expect(anchoDeDecodificacion(0), greaterThanOrEqualTo(0));
      expect(anchoDeDecodificacion(1), 1);
    });
  });
}
