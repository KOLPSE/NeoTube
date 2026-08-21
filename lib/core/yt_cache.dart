import 'dart:collection';

/// Caché en memoria de modelos ya parseados de YouTube Music.
///
/// Guarda modelos finales ([YtSection], [YtColeccion]) y no JSON crudo,
/// reduciendo el consumo de memoria y evitando re-parseos costosos.
///
/// Implementa una política LRU con un máximo de 24 entradas y TTL configurado
/// según el tipo de recurso.
class YtCache {
  YtCache({this.maxEntradas = 24});

  final int maxEntradas;
  final LinkedHashMap<String, ({Object valor, DateTime hasta})> _entradas =
      LinkedHashMap();

  static const ttlPortada = Duration(minutes: 30);
  static const ttlExplorar = Duration(hours: 6);
  static const ttlBiblioteca = Duration(minutes: 10);
  static const ttlColeccion = Duration(hours: 24);
  static const ttlBuscar = Duration(minutes: 10);
  static const ttlPorDefecto = Duration(minutes: 10);

  static Duration ttlParaClave(String clave) {
    if (clave.startsWith('browse:FEmusic_home')) return ttlPortada;
    if (clave.startsWith('browse:FEmusic_explore')) return ttlExplorar;
    if (clave.startsWith('browse:FEmusic_liked') ||
        clave.startsWith('browse:FEmusic_library') ||
        clave == 'biblioteca') {
      return ttlBiblioteca;
    }
    if (clave.startsWith('coleccion:')) return ttlColeccion;
    if (clave.startsWith('buscar:')) return ttlBuscar;
    return ttlPorDefecto;
  }

  T? get<T>(String clave) {
    final entrada = _entradas[clave];
    if (entrada == null) return null;
    if (DateTime.now().isAfter(entrada.hasta)) {
      _entradas.remove(clave);
      return null;
    }
    // Refrescar orden LRU (mover al final)
    _entradas.remove(clave);
    _entradas[clave] = entrada;
    return entrada.valor as T?;
  }

  void set(String clave, Object valor, [Duration? ttl]) {
    final duracion = ttl ?? ttlParaClave(clave);
    final hasta = DateTime.now().add(duracion);
    if (_entradas.containsKey(clave)) {
      _entradas.remove(clave);
    } else if (_entradas.length >= maxEntradas) {
      _entradas.remove(_entradas.keys.first);
    }
    _entradas[clave] = (valor: valor, hasta: hasta);
  }

  void remove(String clave) => _entradas.remove(clave);

  void clear() => _entradas.clear();

  int get length => _entradas.length;
}
