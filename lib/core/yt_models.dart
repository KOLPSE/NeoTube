/// Modelos de NeoTube. Mínimos a propósito, igual que los de Spotify
/// (`models.dart`): solo los campos que la interfaz pinta o que hacen falta
/// para reproducir.
library;

/// Qué es una tarjeta de un carrusel o una fila de la biblioteca.
///
/// La API interna mete en el mismo sitio cosas que se comportan de forma muy
/// distinta, y **distinguirlas es lo que hacía falta para poder reproducir
/// listas**: una canción se reproduce por `videoId`, una lista por
/// `playlistId` (hay que pedir sus pistas primero), un álbum por su
/// `browseId` (`MPREb_…`, que ni siquiera es un `playlistId`) y un artista no
/// se reproduce en absoluto. Antes todo se aplanaba a "pista sin videoId" y
/// por eso las listas solo se podían mirar.
enum YtTipo { cancion, lista, album, artista, desconocido }

/// Una pista concreta y reproducible: la unidad de la cola de [YtPlayer].
class YtTrack {
  const YtTrack({
    required this.videoId,
    required this.titulo,
    required this.artista,
    this.miniatura,
    this.duracion,
  });

  final String videoId;
  final String titulo;
  final String artista;
  final String? miniatura;

  /// La misma pista con carátula, si no traía.
  ///
  /// ⚠️ Existe por los **álbumes**: sus filas no traen `thumbnail` (enseñan el
  /// número de pista en su lugar), a diferencia de las de una playlist, que sí.
  /// Comprobado contra la API real — 0 de 8 filas de un álbum traen miniatura
  /// propia, y la portada del álbum sí está en la cabecera. Sin esto, abrir un
  /// álbum dejaba todas las canciones sin carátula, y con ellas la barra de
  /// reproducción y el Rich Presence de Discord.
  YtTrack conMiniatura(String? url) => miniatura != null || url == null
      ? this
      : YtTrack(
          videoId: videoId,
          titulo: titulo,
          artista: artista,
          miniatura: url,
          duracion: duracion,
        );

  /// Solo cuando la API la da (las filas de una lista traen `fixedColumns`
  /// con el `mm:ss`; las tarjetas de la portada, no).
  final Duration? duracion;

  @override
  bool operator ==(Object other) => other is YtTrack && other.videoId == videoId;

  @override
  int get hashCode => videoId.hashCode;
}

/// Un elemento de una fila de portada/explorar/biblioteca: puede ser una
/// canción suelta, una lista, un álbum o un artista.
class YtItem {
  const YtItem({
    required this.tipo,
    required this.titulo,
    required this.subtitulo,
    this.videoId,
    this.playlistId,
    this.browseId,
    this.miniatura,
    this.duracion,
  });

  final YtTipo tipo;
  final String titulo;
  final String subtitulo;

  /// Presente en las canciones sueltas.
  final String? videoId;

  /// Sin el prefijo `VL`: el que hay que pasar a `browse`/`next`. Las mezclas
  /// y radios de la portada empiezan por `RD`, las listas de verdad por `PL`
  /// o `VLPL`, y "Tu Me gusta" es `LM`.
  final String? playlistId;

  /// `MPREb_…` en los álbumes, `UC…` en los artistas.
  final String? browseId;

  final String? miniatura;
  final Duration? duracion;

  bool get esCancion => tipo == YtTipo.cancion && videoId != null;

  /// ¿Lleva a algún sitio? La biblioteca cuela como primera tarjeta el botón
  /// de "Nueva lista", que tiene título y carátula pero ningún destino:
  /// pintarlo sería una tarjeta muerta en mitad de tus playlists.
  bool get tieneDestino => videoId != null || playlistId != null || browseId != null;

  /// ¿Se puede pulsar y que suene (o que se abra algo que suena)? Un artista
  /// no: no tiene ni pistas propias ni lista que resolver.
  bool get esNavegable =>
      esCancion || playlistId != null || (tipo == YtTipo.album && browseId != null);

  /// ¿Lleva a una **página de artista**? Se mira aparte de [tipo] porque el
  /// destino no es el mismo widget que el de una lista: lo abre
  /// `YtArtistaScreen`, no `YtPlaylistScreen`.
  bool get esArtista => tipo == YtTipo.artista && browseId != null;

  YtTrack? get comoPista => esCancion
      ? YtTrack(
          videoId: videoId!,
          titulo: titulo,
          artista: subtitulo,
          miniatura: miniatura,
          duracion: duracion,
        )
      : null;
}

/// Una fila de la portada/explorar/biblioteca: su título ("Escuchado
/// recientemente", "Tus playlists"…) y lo que trae dentro.
class YtSection {
  const YtSection({required this.titulo, required this.items});

  final String titulo;
  final List<YtItem> items;
}

/// El resultado de abrir una lista o un álbum: la cabecera y sus pistas.
class YtColeccion {
  /// Las pistas que no traigan carátula heredan la de la colección — ver
  /// [YtTrack.conMiniatura], que explica por qué hace falta. Se hace aquí, en
  /// el constructor, para que no haya forma de armar una colección saltándoselo.
  YtColeccion({
    required this.titulo,
    required this.subtitulo,
    required List<YtTrack> pistas,
    this.miniatura,
    this.idParaGuardar,
    this.guardada,
  }) : pistas = miniatura == null
            ? pistas
            : pistas.map((p) => p.conMiniatura(miniatura)).toList(growable: false);

  final String titulo;
  final String subtitulo;
  final List<YtTrack> pistas;
  final String? miniatura;

  /// El `playlistId` que hay que mandarle a `like/like` para meter este álbum
  /// o esta lista en la biblioteca de la cuenta.
  ///
  /// ⚠️ **No es el `browseId` con el que se abrió.** Un álbum se abre por
  /// `MPREb_…` pero se guarda por su `OLAK5uy_…`, y ese id solo aparece dentro
  /// del `toggleButtonRenderer` de la cabecera. Mandar el `MPREb_…` no da
  /// error: contesta 200 y no guarda nada, que es la peor forma de fallar.
  /// `null` en lo que no se puede guardar (una mezcla `RD…` recién generada,
  /// o una cabecera con otra forma).
  final String? idParaGuardar;

  /// Si ya está en la biblioteca, según el `isToggled` que manda la propia
  /// cabecera. `null` cuando no hay botón que mirar — ahí el corazón sale
  /// apagado, que es mejor que inventarse un estado.
  final bool? guardada;
}

/// Una página de artista: la cabecera y sus secciones ("Canciones más
/// escuchadas", "Álbumes", "Singles y EPs"…).
///
/// Las secciones son [YtSection] igual que las de la portada porque **son la
/// misma forma**: `browse UC…` devuelve un `sectionListRenderer` con
/// `musicShelfRenderer` y `musicCarouselShelfRenderer` dentro, exactamente lo
/// que ya parsea `browseSections`. Lo único propio del artista es la cabecera.
class YtArtista {
  const YtArtista({
    required this.nombre,
    required this.secciones,
    this.miniatura,
    this.subtitulo = '',
    this.descripcion,
    this.playlistAleatorio,
    this.playlistRadio,
  });

  final String nombre;
  final List<YtSection> secciones;

  /// El banner del artista (`musicImmersiveHeaderRenderer.thumbnail`). Llega
  /// apaisado, no cuadrado: es una cabecera, no una carátula.
  final String? miniatura;

  /// Oyentes mensuales o suscriptores, tal cual lo manda Google ya traducido.
  final String subtitulo;

  final String? descripcion;

  /// `RDAO…`: lo que reproduce el botón "Aleatorio" de la página oficial.
  final String? playlistAleatorio;

  /// `RDEM…`: el "Mix" del artista.
  final String? playlistRadio;
}
