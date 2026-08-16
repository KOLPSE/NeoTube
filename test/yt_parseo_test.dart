import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/yt_auth.dart';
import 'package:neotube/core/yt_models.dart';
import 'package:neotube/core/yt_music_api.dart';

/// Lo que se prueba aquí es el **parseo** de las respuestas de YouTube Music,
/// no la red: los JSON son recortes con la misma forma que devuelve la API
/// real, reducidos a los campos que se leen.
///
/// Es la parte que de verdad se rompió: la biblioteca llegaba entera y se
/// descartaba al leerla, con lo que "no aparecen mis playlists" no era un
/// problema de sesión ni de permisos sino de dos líneas de parseo.
void main() {
  final api = YtMusicApi(YtAuth());

  Map<String, dynamic> unaColumna(List<dynamic> bloques) => {
        'contents': {
          'singleColumnBrowseResultsRenderer': {
            'tabs': [
              {
                'tabRenderer': {
                  'content': {
                    'sectionListRenderer': {'contents': bloques},
                  },
                },
              },
            ],
          },
        },
      };

  Map<String, dynamic> tarjeta({
    required String titulo,
    String subtitulo = 'Lista',
    String? videoId,
    String? browseId,
    String? playlistIdDelOverlay,
  }) =>
      {
        'musicTwoRowItemRenderer': {
          'title': {
            'runs': [
              {'text': titulo},
            ],
          },
          'subtitle': {
            'runs': [
              {'text': subtitulo},
            ],
          },
          'thumbnailRenderer': {
            'musicThumbnailRenderer': {
              'thumbnail': {
                'thumbnails': [
                  {'url': 'https://ejemplo/pequena.jpg'},
                  {'url': 'https://ejemplo/grande.jpg'},
                ],
              },
            },
          },
          'navigationEndpoint': {
            if (videoId != null) 'watchEndpoint': {'videoId': videoId},
            if (browseId != null) 'browseEndpoint': {'browseId': browseId},
          },
          if (playlistIdDelOverlay != null)
            'thumbnailOverlay': {
              'musicItemThumbnailOverlayRenderer': {
                'content': {
                  'musicPlayButtonRenderer': {
                    'playNavigationEndpoint': {
                      'watchPlaylistEndpoint': {'playlistId': playlistIdDelOverlay},
                    },
                  },
                },
              },
            },
        },
      };

  group('biblioteca', () {
    test('las playlists de un gridRenderer se leen de `items`, no de `contents`',
        () {
      // ⚠️ Este es el fallo original. La biblioteca del usuario llega dentro
      // de un `gridRenderer`, que guarda sus elementos en `items`; el parseo
      // solo miraba `contents` y por eso la pantalla salía vacía aunque la
      // respuesta viniera llena.
      final j = unaColumna([
        {
          'itemSectionRenderer': {
            'contents': [
              {
                'gridRenderer': {
                  'header': {
                    'gridHeaderRenderer': {
                      'title': {
                        'runs': [
                          {'text': 'Playlists'},
                        ],
                      },
                    },
                  },
                  'items': [
                    tarjeta(titulo: 'Rock de casa', browseId: 'VLPLabc123'),
                    tarjeta(titulo: 'Para dormir', browseId: 'VLPLxyz789'),
                  ],
                },
              },
            ],
          },
        },
      ]);

      final secciones = api.seccionesDeRespuesta(j);
      expect(secciones, hasLength(1));
      expect(secciones.first.titulo, 'Playlists');
      expect(secciones.first.items.map((i) => i.titulo),
          ['Rock de casa', 'Para dormir']);
    });

    test('una lista trae su playlistId sin el prefijo VL, listo para browse', () {
      final j = unaColumna([
        {
          'gridRenderer': {
            'items': [tarjeta(titulo: 'Rock de casa', browseId: 'VLPLabc123')],
          },
        },
      ]);

      final item = api.seccionesDeRespuesta(j).single.items.single;
      expect(item.tipo, YtTipo.lista);
      // Sin quitar el `VL`, `browse` sobre `VLVLPLabc123` da 400.
      expect(item.playlistId, 'PLabc123');
      expect(item.esNavegable, isTrue);
      expect(item.esCancion, isFalse);
    });
  });

  group('tipos', () {
    test('un álbum se distingue por su browseId MPRE', () {
      final j = unaColumna([
        {
          'gridRenderer': {
            'items': [tarjeta(titulo: 'Un disco', browseId: 'MPREb_xxxx')],
          },
        },
      ]);
      final item = api.seccionesDeRespuesta(j).single.items.single;
      expect(item.tipo, YtTipo.album);
      expect(item.browseId, 'MPREb_xxxx');
      expect(item.esNavegable, isTrue);
    });

    test('un artista se distingue por su browseId UC y no es navegable', () {
      final j = unaColumna([
        {
          'gridRenderer': {
            'items': [tarjeta(titulo: 'Alguien', browseId: 'UC1234')],
          },
        },
      ]);
      final item = api.seccionesDeRespuesta(j).single.items.single;
      expect(item.tipo, YtTipo.artista);
      // No tiene pistas propias ni lista que resolver: pulsarlo no reproduce.
      expect(item.esNavegable, isFalse);
    });

    test('una canción suelta se convierte en pista reproducible', () {
      final j = unaColumna([
        {
          'musicCarouselShelfRenderer': {
            'contents': [
              tarjeta(titulo: 'Una canción', subtitulo: 'Un grupo', videoId: 'abc-_123'),
            ],
          },
        },
      ]);
      final item = api.seccionesDeRespuesta(j).single.items.single;
      expect(item.tipo, YtTipo.cancion);
      expect(item.comoPista?.videoId, 'abc-_123');
      expect(item.comoPista?.artista, 'Un grupo');
      // La API manda las miniaturas de menor a mayor: se coge la última.
      expect(item.miniatura, 'https://ejemplo/grande.jpg');
    });

    test('una mezcla de la portada saca su playlistId del botón de play', () {
      // Las mezclas (`RD…`) no traen endpoint propio en la tarjeta: el
      // `playlistId` solo está en el play que se superpone a la carátula.
      final j = unaColumna([
        {
          'musicCarouselShelfRenderer': {
            'contents': [
              tarjeta(titulo: 'Mi mezcla', playlistIdDelOverlay: 'RDAMVMabc'),
            ],
          },
        },
      ]);
      final item = api.seccionesDeRespuesta(j).single.items.single;
      expect(item.tipo, YtTipo.lista);
      expect(item.playlistId, 'RDAMVMabc');
    });
  });

  group('robustez', () {
    test('un bloque con forma desconocida no se lleva por delante a los demás', () {
      final j = unaColumna([
        {'algoQueNoConocemos': <String, dynamic>{}},
        {
          'gridRenderer': {
            'items': [tarjeta(titulo: 'Sobrevive', browseId: 'VLPL1')],
          },
        },
      ]);
      expect(api.seccionesDeRespuesta(j).single.items.single.titulo, 'Sobrevive');
    });

    test('el botón de "Nueva lista" no se cuela como si fuera una playlist', () {
      // La biblioteca lo mete como primera tarjeta de la rejilla: tiene título
      // y carátula pero no lleva a ningún sitio.
      final j = unaColumna([
        {
          'gridRenderer': {
            'items': [
              tarjeta(titulo: 'Nueva lista'),
              tarjeta(titulo: 'Una de verdad', browseId: 'VLPL1'),
            ],
          },
        },
      ]);
      expect(api.seccionesDeRespuesta(j).single.items.map((i) => i.titulo),
          ['Una de verdad']);
    });

    test('una respuesta sin la forma esperada devuelve vacío, no lanza', () {
      expect(api.seccionesDeRespuesta({'nada': 'de nada'}), isEmpty);
      expect(api.seccionesDeRespuesta(null), isEmpty);
    });

    test('el subtítulo se une entero, no solo su primer trozo', () {
      // "Lista • 42 canciones" llega repartido en varios `runs`; quedarse con
      // el primero dejaba las tarjetas diciendo solo "Lista".
      final j = unaColumna([
        {
          'gridRenderer': {
            'items': [
              {
                'musicTwoRowItemRenderer': {
                  'title': {
                    'runs': [
                      {'text': 'Mi lista'},
                    ],
                  },
                  'subtitle': {
                    'runs': [
                      {'text': 'Lista'},
                      {'text': ' • '},
                      {'text': '42 canciones'},
                    ],
                  },
                  'navigationEndpoint': {
                    'browseEndpoint': {'browseId': 'VLPL1'},
                  },
                },
              },
            ],
          },
        },
      ]);
      expect(api.seccionesDeRespuesta(j).single.items.single.subtitulo,
          'Lista • 42 canciones');
    });
  });

  group('contenido de una lista', () {
    /// La forma exacta que devuelve la API para una playlist propia,
    /// comprobada con `tool/probe_yt.dart`: la cabecera va **dentro de la
    /// pestaña** (envuelta en `musicEditablePlaylistDetailHeaderRenderer`
    /// porque es editable) y las pistas en `secondaryContents`.
    Map<String, dynamic> listaPropia(List<dynamic> pistas, {List<dynamic>? extra}) => {
          'contents': {
            'twoColumnBrowseResultsRenderer': {
              'tabs': [
                {
                  'tabRenderer': {
                    'content': {
                      'sectionListRenderer': {
                        'contents': [
                          {
                            'musicEditablePlaylistDetailHeaderRenderer': {
                              'playlistId': 'PLabc',
                              'header': {
                                'musicResponsiveHeaderRenderer': {
                                  'title': {
                                    'runs': [
                                      {'text': 'C'},
                                    ],
                                  },
                                  'subtitle': {
                                    'runs': [
                                      {'text': 'Lista'},
                                    ],
                                  },
                                  'secondSubtitle': {
                                    'runs': [
                                      {'text': '2 canciones'},
                                    ],
                                  },
                                  'thumbnail': {
                                    'musicThumbnailRenderer': {
                                      'thumbnail': {
                                        'thumbnails': [
                                          {'url': 'https://ejemplo/portada.jpg'},
                                        ],
                                      },
                                    },
                                  },
                                },
                              },
                            },
                          },
                        ],
                      },
                    },
                  },
                },
              ],
              'secondaryContents': {
                'sectionListRenderer': {
                  'contents': [
                    {
                      'musicPlaylistShelfRenderer': {
                        'contents': [...pistas, ...?extra],
                      },
                    },
                  ],
                },
              },
            },
          },
        };

    Map<String, dynamic> fila(String titulo, String artista, String videoId,
            {String? duracion}) =>
        {
          'musicResponsiveListItemRenderer': {
            'playlistItemData': {'videoId': videoId},
            'flexColumns': [
              {
                'musicResponsiveListItemFlexColumnRenderer': {
                  'text': {
                    'runs': [
                      {'text': titulo},
                    ],
                  },
                },
              },
              {
                'musicResponsiveListItemFlexColumnRenderer': {
                  'text': {
                    'runs': [
                      {'text': artista},
                    ],
                  },
                },
              },
            ],
            if (duracion != null)
              'fixedColumns': [
                {
                  'musicResponsiveListItemFixedColumnRenderer': {
                    'text': {
                      'runs': [
                        {'text': duracion},
                      ],
                    },
                  },
                },
              ],
          },
        };

    test('las pistas se leen de secondaryContents, no de la pestaña', () {
      // ⚠️ La pestaña solo trae la cabecera. Quedarse con ella y dar la lista
      // por vacía es el error fácil aquí.
      final (c, _) = api.coleccionDeRespuesta(listaPropia([
        fila('Take On Me', 'a-ha', 'djV11Xbc914', duracion: '3:47'),
        fila('Hola', 'Alguien', 'K4xmzYN3k50'),
      ]));
      expect(c.pistas.map((t) => t.videoId), ['djV11Xbc914', 'K4xmzYN3k50']);
      expect(c.pistas.first.artista, 'a-ha');
      expect(c.pistas.first.duracion, const Duration(minutes: 3, seconds: 47));
      expect(c.pistas.last.duracion, isNull);
    });

    test('la cabecera de una lista propia va envuelta y aun así se lee', () {
      final (c, _) = api.coleccionDeRespuesta(
          listaPropia([fila('Take On Me', 'a-ha', 'djV11Xbc914')]));
      expect(c.titulo, 'C');
      expect(c.subtitulo, 'Lista • 2 canciones');
      expect(c.miniatura, 'https://ejemplo/portada.jpg');
    });

    test('una lista larga devuelve el token del siguiente trozo', () {
      // Sin seguir la continuación, una playlist de 800 canciones se
      // reproduciría entera... hasta la 100.
      final (c, token) = api.coleccionDeRespuesta(listaPropia(
        [fila('Take On Me', 'a-ha', 'djV11Xbc914')],
        extra: [
          {
            'continuationItemRenderer': {
              'continuationEndpoint': {
                'continuationCommand': {'token': 'sigue-por-aqui'},
              },
            },
          },
        ],
      ));
      expect(c.pistas, hasLength(1));
      expect(token, 'sigue-por-aqui');
    });

    test('una lista que cabe entera no pide continuación', () {
      final (_, token) = api.coleccionDeRespuesta(
          listaPropia([fila('Take On Me', 'a-ha', 'djV11Xbc914')]));
      expect(token, isNull);
    });
  });

  group('dos columnas', () {
    test('las listas modernas llegan en twoColumnBrowseResultsRenderer', () {
      final j = {
        'contents': {
          'twoColumnBrowseResultsRenderer': {
            'tabs': [
              {
                'tabRenderer': {
                  'content': {
                    'sectionListRenderer': {
                      'contents': [
                        {
                          'gridRenderer': {
                            'items': [tarjeta(titulo: 'Desde dos columnas', browseId: 'VLPL9')],
                          },
                        },
                      ],
                    },
                  },
                },
              },
            ],
          },
        },
      };
      expect(api.seccionesDeRespuesta(j).single.items.single.titulo,
          'Desde dos columnas');
    });
  });

  group('continuación de portada', () {
    Map<String, dynamic> portadaConContinuacion(
      List<dynamic> bloques, {
      String? continuationToken,
    }) =>
        {
          'contents': {
            'singleColumnBrowseResultsRenderer': {
              'tabs': [
                {
                  'tabRenderer': {
                    'content': {
                      'sectionListRenderer': {
                        'contents': bloques,
                        if (continuationToken != null)
                          'continuations': [
                            {
                              'nextContinuationData': {
                                'continuation': continuationToken,
                              },
                            },
                          ],
                      },
                    },
                  },
                },
              ],
            },
          },
        };

    Map<String, dynamic> respuestaContinuacion(
      List<dynamic> bloques, {
      String? nuevoToken,
    }) =>
        {
          'continuationContents': {
            'sectionListContinuation': {
              'contents': bloques,
              if (nuevoToken != null)
                'continuations': [
                  {
                    'nextContinuationData': {
                      'continuation': nuevoToken,
                    },
                  },
                ],
            },
          },
        };

    Map<String, dynamic> carrusel(String titulo, List<dynamic> items) => {
          'musicCarouselShelfRenderer': {
            'header': {
              'musicCarouselShelfBasicHeaderRenderer': {
                'title': {
                  'runs': [
                    {'text': titulo},
                  ],
                },
              },
            },
            'contents': items,
          },
        };

    test('la portada con continuations devuelve su token para la siguiente tanda', () {
      final j = portadaConContinuacion(
        [
          carrusel('Primer bloque', [tarjeta(titulo: 'Canción 1', videoId: 'v1')]),
        ],
        continuationToken: 'token-tanda-2',
      );

      final secciones = api.seccionesDeRespuesta(j);
      final token = YtMusicApi.extraerTokenContinuacionDeBrowse(j);

      expect(secciones, hasLength(1));
      expect(secciones.first.titulo, 'Primer bloque');
      expect(token, 'token-tanda-2');
    });

    test('la segunda tanda de portada parsea sus secciones y detecta fin de lista', () {
      final jCont = respuestaContinuacion(
        [
          carrusel('Selecciones rápidas', [tarjeta(titulo: 'Canción 2', videoId: 'v2')]),
        ],
        nuevoToken: null,
      );

      final (seccionesCont, siguienteToken) =
          YtMusicApi.seccionesContinuationDeRespuesta(jCont);

      expect(seccionesCont, hasLength(1));
      expect(seccionesCont.first.titulo, 'Selecciones rápidas');
      expect(seccionesCont.first.items.single.titulo, 'Canción 2');
      expect(siguienteToken, isNull);
    });

    test('las secciones de ambas tandas se juntan y conservan todos los elementos', () {
      final tanda1 = portadaConContinuacion(
        [
          carrusel('Populares', [tarjeta(titulo: 'Hit 1', videoId: 'h1')]),
          carrusel('Novedades', [tarjeta(titulo: 'Nuevo 1', videoId: 'n1')]),
        ],
        continuationToken: 'token-siguiente',
      );

      final tanda2 = respuestaContinuacion(
        [
          carrusel('Selecciones rápidas', [tarjeta(titulo: 'Mix 1', videoId: 'm1')]),
        ],
      );

      final sec1 = api.seccionesDeRespuesta(tanda1);
      final token1 = YtMusicApi.extraerTokenContinuacionDeBrowse(tanda1);
      expect(token1, 'token-siguiente');

      final (sec2, token2) = YtMusicApi.seccionesContinuationDeRespuesta(tanda2);
      expect(token2, isNull);

      final totalSecciones = [...sec1, ...sec2];
      expect(totalSecciones, hasLength(3));
      expect(totalSecciones.map((s) => s.titulo),
          ['Populares', 'Novedades', 'Selecciones rápidas']);
    });

    test('una portada sin continuations devuelve token nulo', () {
      final j = portadaConContinuacion([
        carrusel('Solo una', [tarjeta(titulo: 'Única', videoId: 'u1')]),
      ]);

      expect(YtMusicApi.extraerTokenContinuacionDeBrowse(j), isNull);
    });
  });
group('carátulas de un álbum', () {
    /// Las filas de un **álbum** no traen `thumbnail` —enseñan el número de
    /// pista—, a diferencia de las de una playlist. Comprobado contra la API
    /// real: 0 de 8 filas de un álbum traen miniatura propia, y la portada solo
    /// está en la cabecera. Sin heredarla, abrir un álbum dejaba todas las
    /// canciones sin carátula, y con ellas la barra de reproducción y Discord.
    test('las pistas sin carátula heredan la de la colección', () {
      final c = YtColeccion(
        titulo: 'MOTOMAMI',
        subtitulo: 'ROSALÍA',
        miniatura: 'https://ejemplo/portada.jpg',
        pistas: const [
          YtTrack(videoId: 'aaaaaaaaaaa', titulo: 'SAOKO', artista: 'ROSALÍA'),
          YtTrack(videoId: 'bbbbbbbbbbb', titulo: 'CANDY', artista: 'ROSALÍA'),
        ],
      );

      expect(c.pistas.every((p) => p.miniatura == 'https://ejemplo/portada.jpg'), isTrue);
    });

    test('la carátula propia de una pista gana a la de la colección', () {
      final c = YtColeccion(
        titulo: 'Mi lista',
        subtitulo: '',
        miniatura: 'https://ejemplo/portada.jpg',
        pistas: const [
          YtTrack(
            videoId: 'aaaaaaaaaaa',
            titulo: 'Una',
            artista: 'Alguien',
            miniatura: 'https://ejemplo/suya.jpg',
          ),
        ],
      );

      expect(c.pistas.single.miniatura, 'https://ejemplo/suya.jpg');
    });

    test('sin portada de colección, las pistas se quedan como estaban', () {
      final c = YtColeccion(
        titulo: 'Mezcla',
        subtitulo: '',
        pistas: const [
          YtTrack(videoId: 'aaaaaaaaaaa', titulo: 'Una', artista: 'Alguien'),
        ],
      );

      expect(c.pistas.single.miniatura, isNull);
    });
  });
}
