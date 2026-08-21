import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/yt_auth.dart';
import 'package:neotube/core/yt_models.dart';
import 'package:neotube/core/yt_music_api.dart';
import 'package:neotube/core/yt_player.dart';
import 'package:neotube/ui/yt_acciones.dart';

class _FakePlayer extends YtPlayer {
  _FakePlayer() : super(volumenInicial: 50);

  List<YtTrack>? listaReproducida;
  int? desdeReproducido;

  @override
  bool get disponible => true;

  @override
  Future<void> reproducirLista(
    List<YtTrack> pistas, {
    int desde = 0,
    String? contexto,
  }) async {
    listaReproducida = pistas;
    desdeReproducido = desde;
    cola = pistas;
    indice = desde;
  }
}

void main() {
  tearDown(() => YtPlayer.libmpvDisponible = true);

  testWidgets('reproducirCancion con hermanas monta la cola con todas las canciones y el indice correcto', (tester) async {
    YtPlayer.libmpvDisponible = false;
    final fakePlayer = _FakePlayer();
    addTearDown(fakePlayer.dispose);

    final api = YtMusicApi(YtAuth());
    final acciones = YtAcciones(
      api: api,
      player: fakePlayer,
      abrir: (_) {},
    );

    const items = [
      YtItem(tipo: YtTipo.cancion, titulo: 'Track 1', subtitulo: 'Artist 1', videoId: 'vid1'),
      YtItem(tipo: YtTipo.lista, titulo: 'Playlist', subtitulo: 'Sub', playlistId: 'pl1'),
      YtItem(tipo: YtTipo.cancion, titulo: 'Track 2', subtitulo: 'Artist 2', videoId: 'vid2'),
      YtItem(tipo: YtTipo.cancion, titulo: 'Track 3', subtitulo: 'Artist 3', videoId: 'vid3'),
    ];

    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (context) {
          return ElevatedButton(
            onPressed: () => acciones.pulsar(context, items[2], hermanas: items),
            child: const Text('Play'),
          );
        },
      ),
    ));

    await tester.tap(find.text('Play'));
    await tester.pump();

    expect(fakePlayer.listaReproducida?.length, 3);
    expect(fakePlayer.listaReproducida?[0].videoId, 'vid1');
    expect(fakePlayer.listaReproducida?[1].videoId, 'vid2');
    expect(fakePlayer.listaReproducida?[2].videoId, 'vid3');
    expect(fakePlayer.desdeReproducido, 1);
  });

  testWidgets('reproducirCancion sin hermanas solo reproduce la pista suelta', (tester) async {
    YtPlayer.libmpvDisponible = false;
    final fakePlayer = _FakePlayer();
    addTearDown(fakePlayer.dispose);

    final api = YtMusicApi(YtAuth());
    final acciones = YtAcciones(
      api: api,
      player: fakePlayer,
      abrir: (_) {},
    );

    const item = YtItem(tipo: YtTipo.cancion, titulo: 'Track 2', subtitulo: 'Artist 2', videoId: 'vid2');

    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (context) {
          return ElevatedButton(
            onPressed: () => acciones.pulsar(context, item),
            child: const Text('Play'),
          );
        },
      ),
    ));

    await tester.tap(find.text('Play'));
    await tester.pump();

    expect(fakePlayer.listaReproducida?.length, 1);
    expect(fakePlayer.listaReproducida?[0].videoId, 'vid2');
    expect(fakePlayer.desdeReproducido, 0);
  });
}
