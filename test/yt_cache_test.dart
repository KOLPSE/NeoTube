import 'package:flutter_test/flutter_test.dart';
import 'package:neotube/core/yt_cache.dart';
import 'package:neotube/core/yt_models.dart';

void main() {
  group('YtCache', () {
    test('guarda y recupera elementos por clave', () {
      final cache = YtCache();
      const seccion = YtSection(titulo: 'Prueba', items: []);
      cache.set('browse:FEmusic_home', [seccion]);

      final recuperado = cache.get<List<YtSection>>('browse:FEmusic_home');
      expect(recuperado, isNotNull);
      expect(recuperado!.length, 1);
      expect(recuperado.first.titulo, 'Prueba');
    });

    test('aplica límite LRU de 24 entradas descartando la más antigua', () {
      final cache = YtCache(maxEntradas: 3);
      cache.set('clave:1', 'valor1');
      cache.set('clave:2', 'valor2');
      cache.set('clave:3', 'valor3');

      expect(cache.length, 3);
      expect(cache.get<String>('clave:1'), 'valor1'); // clave:1 pasa a ser la más reciente

      cache.set('clave:4', 'valor4'); // Debe expulsar clave:2 (la más antigua)

      expect(cache.length, 3);
      expect(cache.get<String>('clave:2'), isNull);
      expect(cache.get<String>('clave:1'), 'valor1');
      expect(cache.get<String>('clave:3'), 'valor3');
      expect(cache.get<String>('clave:4'), 'valor4');
    });

    test('respeta el TTL de expiración', () {
      final cache = YtCache();
      cache.set('buscar:test', 'resultado', const Duration(milliseconds: 10));

      expect(cache.get<String>('buscar:test'), 'resultado');

      // Esperar a que expire
      Future.delayed(const Duration(milliseconds: 20), () {
        expect(cache.get<String>('buscar:test'), isNull);
      });
    });

    test('asigna TTLs correctos según el prefijo de la clave', () {
      expect(YtCache.ttlParaClave('browse:FEmusic_home'), YtCache.ttlPortada);
      expect(YtCache.ttlParaClave('browse:FEmusic_explore'), YtCache.ttlExplorar);
      expect(YtCache.ttlParaClave('biblioteca'), YtCache.ttlBiblioteca);
      expect(YtCache.ttlParaClave('browse:FEmusic_liked_playlists'), YtCache.ttlBiblioteca);
      expect(YtCache.ttlParaClave('coleccion:PL123'), YtCache.ttlColeccion);
      expect(YtCache.ttlParaClave('coleccion:MPRE123'), YtCache.ttlColeccion);
      expect(YtCache.ttlParaClave('buscar:rock'), YtCache.ttlBuscar);
      expect(YtCache.ttlParaClave('otra:cosa'), YtCache.ttlPorDefecto);
    });
  });
}
