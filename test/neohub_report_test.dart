import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:neotube/core/neohub.dart';

// El cliente NeoHub con la red mockeada: fija que se manda un reporte `bug`
// con el id de la app, que la clave viaja en la cabecera y que los adjuntos
// (al menos el contexto del equipo) se suben al reporte recién creado.
void main() {
  test('crea un reporte bug con appId y sube adjuntos', () async {
    final peticiones = <http.Request>[];
    final cliente = MockClient((request) async {
      peticiones.add(request);
      if (request.url.path == '/api/public/reports' &&
          request.method == 'POST') {
        return http.Response(
          jsonEncode({
            'report': {'id': 'rep-123', 'type': 'bug'},
          }),
          201,
        );
      }
      if (request.url.path.endsWith('/adjuntos') && request.method == 'POST') {
        return http.Response(jsonEncode({'adjunto': {}}), 201);
      }
      return http.Response('{}', 200);
    });

    final hub = NeoHub(cliente: cliente);
    final resultado = await hub.reportarProblema(
      titulo: 'Se cierra al abrir una lista',
      descripcion: 'Al abrir cualquier lista la app se cierra sola.',
      autorNombre: 'Prueba',
    );
    hub.cerrar();

    expect(resultado.id, 'rep-123');

    final crear = peticiones.firstWhere(
      (r) => r.url.path == '/api/public/reports' && r.method == 'POST',
    );
    final cuerpo = jsonDecode(crear.body) as Map<String, dynamic>;
    expect(cuerpo['type'], 'bug');
    expect(cuerpo['appId'], 'neotube');
    expect(cuerpo['authorName'], 'Prueba');
    expect(cuerpo['title'], 'Se cierra al abrir una lista');
    expect(crear.headers['X-NeoHub-Key'], isNotEmpty);

    // El contexto del equipo siempre se adjunta; los logs, si existen.
    final subidas = peticiones.where((r) => r.url.path.endsWith('/adjuntos'));
    expect(subidas.length, greaterThanOrEqualTo(1));
    expect(
      subidas.any((r) => r.headers['X-Nombre-Fichero'] != null),
      isTrue,
    );
  });

  test('rechaza un título corto sin tocar la red', () async {
    final cliente = MockClient((_) async => http.Response('{}', 200));
    final hub = NeoHub(cliente: cliente);
    await expectLater(
      hub.reportarProblema(
        titulo: 'abc',
        descripcion: 'Una descripción suficientemente larga.',
        autorNombre: 'Pe',
      ),
      throwsA(isA<ErrorNeoHub>()),
    );
    hub.cerrar();
  });
}
