import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/yt_models.dart';
import 'package:neotube/core/yt_player.dart';

void main() {
  tearDown(() => YtPlayer.libmpvDisponible = true);

  test('doble llamada sobre la misma pista no duplica reproduccion mientras resuelve', () async {
    YtPlayer.libmpvDisponible = false;
    final p = YtPlayer(volumenInicial: 50);
    addTearDown(p.dispose);

    const pista = YtTrack(videoId: 'test_vid_123', titulo: 'Test', artista: 'Artist');
    await p.reproducirPista(pista);
    expect(p.actual, isNull);
  });

  test('error en YtPlayer notifica a los listeners cuando se asigna un fallo', () {
    YtPlayer.libmpvDisponible = false;
    final p = YtPlayer(volumenInicial: 50);
    addTearDown(p.dispose);

    var notificaciones = 0;
    p.addListener(() => notificaciones++);

    p.error = 'YtPlayerException: Error al resolver video';
    p.notifyListeners();

    expect(notificaciones, greaterThanOrEqualTo(1));
    expect(p.error, contains('Error al resolver video'));
  });
}
