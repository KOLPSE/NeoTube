import 'dart:io';

import 'package:flutter/material.dart';

import '../core/art_cache.dart';
import '../core/settings.dart';

/// Los tamaños a los que se pide una carátula de Google.
///
/// Se **cuantiza** en vez de pedir el ancho exacto de cada widget porque la
/// URL es la clave de la caché de disco: pedir 132 aquí y 133 allí serían dos
/// descargas y dos ficheros de la misma imagen. Con esta escalera, una
/// carátula se baja como mucho tres veces en toda la app (la fila de 40, la
/// tarjeta de 132, la cabecera de 128 en HiDPI).
/// Los últimos escalones son para **el banner del artista**, que se pinta a lo
/// ancho de la ventana: una carátula cuadrada de esta app no pasa de ~270 px
/// reales y nunca llega ahí.
const List<int> _escalones = [60, 120, 226, 320, 544, 720, 960, 1280];

/// Qué tamaño pedir para pintar [ladoEnPixeles] píxeles reales.
int _escalonPara(int ladoEnPixeles) =>
    _escalones.firstWhere((e) => e >= ladoEnPixeles, orElse: () => _escalones.last);

/// A qué ancho descodificar, que **no es lo mismo que a qué ancho se pinta**.
///
/// ⚠️ Esto existe por un cuelgue, no por ahorrar memoria de más. El banner del
/// artista se pinta a lo ancho de la ventana: maximizada son unos 2560 px
/// lógicos, ~3840 reales en una pantalla al 150 %. Pasarle ese número a
/// `cacheWidth` le pide a Flutter un bitmap de 3840 × ~1600 × 4 bytes —**unos
/// 24 MB por imagen**— y encima *ampliando*, porque lo que se ha descargado
/// mide 1280 de ancho: no se gana un solo píxel de nitidez.
///
/// Y no es una sola: al maximizar, el `LayoutBuilder` se dispara varias veces
/// con anchos intermedios, así que se encadenan varias descodificaciones de
/// ese tamaño. Eso es lo que se llevaba por delante el hilo de rasterizado —
/// una violación de acceso dentro de `flutter_windows.dll`, sin excepción de
/// Dart de por medio, que es justo lo difícil de diagnosticar de este fallo.
///
/// El tope es el escalón que de verdad se ha pedido ([_escalonPara]): por
/// debajo del máximo se descodifica al tamaño pintado (reducir está bien),
/// y por encima se para en lo que mide el original.
int anchoDeDecodificacion(double pixelesPintados) {
  final px = pixelesPintados.round();
  final descargado = _escalonPara(px);
  return px < descargado ? px : descargado;
}

final _tokenAnchoAlto = RegExp(r'=w(\d+)-h(\d+)');
final _tokenLado = RegExp(r'=s(\d+)(?=[-=]|$)');

/// Reescribe una URL de carátula de Google para que sirva la imagen al tamaño
/// al que se va a pintar de verdad.
///
/// ⚠️ **Existe porque la biblioteca llega a 120 px como mucho.** Las filas de
/// `FEmusic_liked_videos`, `FEmusic_library_corpus_track_artists` y las
/// canciones de la búsqueda traen solo dos escalones —60 y 120—, y la app las
/// pinta en tarjetas de 132 px lógicos, que en una pantalla HiDPI son 264
/// reales. El resultado era una carátula borrosa, o directamente un borrón
/// gris donde debería haber una portada.
///
/// La API **no** ofrece nada mejor en esa respuesta, pero el servidor de
/// imágenes sí: el `=w120-h120-l90-rj` del final de la URL no describe la
/// imagen, es una **petición de reescalado** que `googleusercontent` atiende
/// al vuelo. Comprobado: la misma URL con `=w544-h544-l90-rj` devuelve 200 y
/// 62 KB, frente a los 4 KB de la de 120.
///
/// Se conserva la proporción (se escala el lado mayor hasta el escalón y el
/// otro con el mismo factor) porque no todo lo que pasa por aquí es cuadrado:
/// el banner de un artista llega como `=w540-h225` y forzarlo a cuadrado lo
/// deformaría.
///
/// Lo que no lleva ninguno de esos dos tokens —las miniaturas de vídeo de
/// `i.ytimg.com`, que ya vienen a 400-800 px— se devuelve **tal cual**:
/// inventarle parámetros a una URL que no los entiende es cambiar una imagen
/// borrosa por un 404.
String urlDeCaratulaEscalada(String url, int ladoEnPixeles) {
  final destino = _escalonPara(ladoEnPixeles);

  final m = _tokenAnchoAlto.firstMatch(url);
  if (m != null) {
    final w = int.parse(m.group(1)!);
    final h = int.parse(m.group(2)!);
    final mayor = w > h ? w : h;
    if (mayor == destino || mayor == 0) return url;
    final factor = destino / mayor;
    final nw = (w * factor).round().clamp(1, 2048);
    final nh = (h * factor).round().clamp(1, 2048);
    return url.replaceRange(m.start, m.end, '=w$nw-h$nh');
  }

  final s = _tokenLado.firstMatch(url);
  if (s != null) {
    if (int.parse(s.group(1)!) == destino) return url;
    return url.replaceRange(s.start, s.end, '=s$destino');
  }

  return url;
}

/// Carátula con caché en disco y decodificación al tamaño de destino.
///
/// El `cacheWidth` es lo importante: sin él Flutter guarda en memoria el bitmap
/// a resolución completa aunque se pinte a 48 px, y una lista con cien filas se
/// come cientos de megas. Con él, cada miniatura ocupa lo que ocupa en pantalla.
///
/// ⚠️ Pinta desde **fichero**, no desde bytes. `Image.memory` obligaba a
/// guardar el JPEG entero además del bitmap: el `MemoryImage` es la clave de la
/// entrada en el `imageCache` de Flutter, y mientras la entrada viva su
/// `Uint8List` vive con ella. Además cada fila sujetaba su propio futuro con
/// los bytes. Con `Image.file` la clave es una ruta.
///
/// Es un `StatefulWidget` **a propósito**: el futuro de la descarga se resuelve
/// una sola vez por URL, en `initState`/`didUpdateWidget`. Pedirlo desde
/// `build()` crearía un futuro nuevo en cada repintado — y la barra de
/// reproducción se repinta en cada sondeo —, así que la imagen volvía al
/// placeholder continuamente y varias lecturas competían por el mismo fichero.
class ArtImage extends StatefulWidget {
  const ArtImage({
    super.key,
    required this.url,
    this.urlGrande,
    required this.size,
    this.ancho,
    this.radius = 4,
  });

  /// La variante pequeña (los 64 px de Spotify).
  final String? url;

  /// La variante de 300 px, si el modelo la tiene. Solo se baja cuando el
  /// tamaño real en píxeles no cabe en la de 64.
  final String? urlGrande;

  /// El **alto**. Es también el ancho salvo que se pase [ancho], porque casi
  /// todo lo que pinta esta app son carátulas cuadradas.
  final double size;

  /// El ancho, cuando no es cuadrada.
  ///
  /// ⚠️ Existe por **el banner del artista**, y no es un capricho de layout:
  /// de este número sale el tamaño que se le pide al servidor de imágenes
  /// ([urlDeCaratulaEscalada]). Metiendo un banner en el widget cuadrado se
  /// bajaba una imagen calculada para el alto —180 px— y se pintaba a lo ancho
  /// de la ventana, unos 700: se veía pixelada, y no había forma de arreglarlo
  /// desde fuera porque el widget no sabía a qué anchura lo iban a pintar.
  ///
  /// `double.infinity` vale: significa "ocupa lo que te den", y para elegir el
  /// tamaño de descarga se usa el ancho de verdad que mide el `LayoutBuilder`.
  final double? ancho;

  final double radius;

  /// El escalón de Spotify más pequeño. Por debajo de esto, pedir la de 300 es
  /// bajarse hasta 78 KB para pintar 56 px.
  static const int _escalonPequeno = 64;

  @override
  State<ArtImage> createState() => _ArtImageState();
}

class _ArtImageState extends State<ArtImage> {
  Future<File?>? _future;
  String? _elegida;

  /// Cuál de las dos variantes hace falta **en píxeles reales**.
  ///
  /// Spotify sirve cada carátula en 640/300/64. La barra de reproducción pinta
  /// 56 px lógicos: en una pantalla al 100 % son 56 físicos y la de 64 sobra,
  /// pero en una HiDPI al 200 % son 112 y se vería blanda. Elegir por el
  /// tamaño lógico (como se hacía) obliga a acertar a ciegas para todas las
  /// pantallas; elegir por píxeles acierta en cada una.
  String? _urlPara(double pixeles) {
    final elegida = pixeles <= ArtImage._escalonPequeno
        ? (widget.url ?? widget.urlGrande)
        : (widget.urlGrande ?? widget.url);
    // Y, sea cual sea, se le pide al servidor de imágenes el tamaño que de
    // verdad se va a pintar: lo que manda la API interna se queda corto en
    // media app. Ver [urlDeCaratulaEscalada].
    return elegida == null ? null : urlDeCaratulaEscalada(elegida, pixeles.round());
  }

  @override
  void didUpdateWidget(ArtImage oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Solo cuando cambia la URL o la medida de verdad; un repintado cualquiera
    // no debe volver a lanzar la descarga. Se olvida lo elegido y el `build`
    // de después decide de nuevo.
    if (oldWidget.url != widget.url ||
        oldWidget.urlGrande != widget.urlGrande ||
        oldWidget.size != widget.size ||
        oldWidget.ancho != widget.ancho) {
      _elegida = null;
      _future = null;
    }
  }

  /// Deja [_future] apuntando a la carátula del tamaño que hace falta.
  ///
  /// Se llama desde `build` y **sin `setState` a propósito**: no está pidiendo
  /// un repintado, está decidiendo qué pintar en este mismo. El tamaño de
  /// destino no se conoce hasta que se mide el widget (un banner puede ocupar
  /// lo que le den), así que no hay un sitio más temprano donde hacerlo.
  ///
  /// ⚠️ Lo que hace que esto no sea una descarga por fotograma es la guarda de
  /// abajo, junto con que [urlDeCaratulaEscalada] **cuantiza** el tamaño: dos
  /// anchos parecidos dan la misma URL, así que a partir del primer `build` la
  /// respuesta es siempre "ya la tienes".
  void _resolverPara(double pixeles) {
    // En modo rendimiento no se pide nada: ni red, ni disco, ni decodificar.
    // El mosaico se dibuja con la propia URL como semilla.
    if (modoRendimiento.value) {
      _elegida = null;
      _future = null;
      return;
    }
    final url = _urlPara(pixeles);
    if (url == _elegida && _future != null) return;
    _elegida = url;
    _future = url == null ? null : ArtCache.file(url);
  }

  @override
  void initState() {
    super.initState();
    // Encender o apagar el modo tiene que redibujar todas las carátulas que
    // haya en pantalla en ese momento, no solo las que se construyan después.
    modoRendimiento.addListener(_alCambiarElModo);
  }

  @override
  void dispose() {
    modoRendimiento.removeListener(_alCambiarElModo);
    super.dispose();
  }

  void _alCambiarElModo() {
    if (!mounted) return;
    setState(() {
      _elegida = null;
      _future = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    final anchoPedido = widget.ancho ?? widget.size;
    // Un ancho concreto se sabe ya; `double.infinity` ("ocupa lo que te den")
    // hay que medirlo, porque de ese número sale el tamaño que se descarga.
    if (anchoPedido.isFinite) return _conAncho(context, anchoPedido);
    return LayoutBuilder(
      builder: (context, restricciones) => _conAncho(
        context,
        restricciones.maxWidth.isFinite ? restricciones.maxWidth : widget.size,
      ),
    );
  }

  Widget _conAncho(BuildContext context, double ancho) {
    final dpr = MediaQuery.maybeDevicePixelRatioOf(context) ?? 1.0;
    // Manda el lado **mayor**: es el que decide si la imagen se ve nítida o
    // estirada. Pedirla por el alto era justo lo que dejaba el banner del
    // artista pixelado, porque se pinta mucho más ancho que alto.
    final ladoMayor = ancho > widget.size ? ancho : widget.size;
    final pixeles = ladoMayor * dpr;
    _resolverPara(pixeles);

    return ClipRRect(
      borderRadius: BorderRadius.circular(widget.radius),
      child: SizedBox(
        width: ancho,
        height: widget.size,
        child: _future == null
            ? _placeholder(context)
            : FutureBuilder<File?>(
                future: _future,
                builder: (context, snap) {
                  final fichero = snap.data;
                  if (fichero == null) return _placeholder(context);
                  return Image.file(
                    fichero,
                    width: ancho,
                    height: widget.size,
                    fit: BoxFit.cover,
                    cacheWidth: anchoDeDecodificacion(ancho * dpr),
                    // ⚠️ El `cacheHeight` **solo se omite en las cajas que no
                    // son cuadradas**, y la diferencia se ve.
                    //
                    // Con las dos medidas puestas, `dart:ui` descodifica a esas
                    // dos exactas sin respetar la proporción. Suena mal, pero
                    // en una caja cuadrada es lo correcto: muchas miniaturas de
                    // YouTube son 16:9 (`i.ytimg.com`, vídeos, listas), y lo
                    // que se pinta es un cuadrado.
                    //
                    // Omitiéndolo, el alto sale proporcional —132 × 74 para una
                    // tarjeta de 132— y entonces `BoxFit.cover` tiene que
                    // **ampliar 1,8×** para llenar el cuadrado: se ve pixelada.
                    // Es exactamente lo que pasó al quitarlo de todas.
                    //
                    // En la caja apaisada del banner sí sobra: la imagen ya
                    // viene apaisada, así que descodificar por el ancho deja
                    // alto de sobra y `cover` solo recorta.
                    cacheHeight: (ancho - widget.size).abs() < 1
                        ? anchoDeDecodificacion(widget.size * dpr)
                        : null,
                    gaplessPlayback: true,
                    // Una imagen que no decodifica no debe pintar el cuadro de
                    // error de Flutter en mitad de una lista: se tira la copia
                    // de disco (que puede estar corrupta) y se deja el hueco.
                    // Se tira **la URL que se pidió de verdad** (`_elegida`, ya
                    // reescalada), no `widget.url`: la caché de disco va por
                    // URL, así que borrar la original dejaría intacto el
                    // fichero corrupto que acaba de no decodificar.
                    errorBuilder: (context, error, stack) {
                      final url = _elegida;
                      if (url != null) ArtCache.evict(url);
                      return _placeholder(context);
                    },
                  );
                },
              ),
      ),
    );
  }

  /// Lo que se pinta cuando no hay imagen: sin carátula (modo rendimiento) o
  /// mientras se descarga.
  ///
  /// No es un cuadro gris. El tono sale de la propia URL, así que **cada disco
  /// tiene siempre el mismo color** y una lista sigue leyéndose de un vistazo:
  /// el ojo distingue las filas por color aunque no haya portadas. Cuesta cero
  /// bytes — es un degradado, no una imagen.
  Widget _placeholder(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final semilla = widget.url ?? widget.urlGrande;

    if (semilla == null) {
      return ColoredBox(
        color: scheme.surfaceContainerHighest,
        child: Icon(
          Icons.music_note,
          size: widget.size * 0.5,
          color: scheme.onSurfaceVariant.withValues(alpha: 0.5),
        ),
      );
    }

    final tono = (semilla.hashCode % 360).abs().toDouble();
    final oscuro = Theme.of(context).brightness == Brightness.dark;
    // Saturación contenida y luminosidad acorde al tema: colores de fondo, no
    // pegatinas fosforitas que compitan con el texto de al lado.
    final a = HSLColor.fromAHSL(1, tono, 0.42, oscuro ? 0.30 : 0.68).toColor();
    final b = HSLColor.fromAHSL(1, (tono + 28) % 360, 0.42, oscuro ? 0.20 : 0.56)
        .toColor();

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [a, b],
        ),
      ),
      child: Icon(
        Icons.music_note,
        size: widget.size * 0.45,
        color: Colors.white.withValues(alpha: 0.55),
      ),
    );
  }
}
