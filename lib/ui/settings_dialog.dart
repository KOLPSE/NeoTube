import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';

import '../core/app_config.dart' show rutaEfectivaDeCacheContinuaciones;
import '../core/resource_monitor.dart';
import '../core/settings.dart';
import '../core/updater.dart';

/// Ajustes: lo que gasta la app, el modo rendimiento y las actualizaciones.
///
/// El consumo se enseña aquí y no en una barra permanente porque es un dato de
/// diagnóstico, no algo que haga falta mirar mientras escuchas música.
///
/// **El mismo diálogo sirve a los dos modos.** Casi todo lo que hay dentro es
/// de la app entera y no de uno de ellos: la memoria y la CPU las gasta un
/// único proceso, el modo rendimiento toca la caché de imágenes que comparten
/// los dos, y la actualización trae un solo binario. Duplicar el diálogo
/// habría significado mantener dos copias de eso para que solo cambiara un
/// bloque — de ahí [propiosDelModo], que es justo ese bloque: en NeoFy,
/// reiniciar la salida de audio de librespot; en NeoTube, el estado de yt-dlp.
Future<void> mostrarAjustes(
  BuildContext context, {
  required ResourceMonitor monitor,
  required Settings settings,
  required Updater updater,
  required Future<void> Function() onSalirParaActualizar,
  List<Widget> propiosDelModo = const [],
}) {
  return showDialog<void>(
    context: context,
    builder: (context) => _DialogoAjustes(
      monitor: monitor,
      settings: settings,
      updater: updater,
      onSalirParaActualizar: onSalirParaActualizar,
      propiosDelModo: propiosDelModo,
    ),
  );
}

class _DialogoAjustes extends StatelessWidget {
  const _DialogoAjustes({
    required this.monitor,
    required this.settings,
    required this.updater,
    required this.onSalirParaActualizar,
    required this.propiosDelModo,
  });

  final ResourceMonitor monitor;
  final Settings settings;
  final Updater updater;

  /// Cerrar la app en cuanto arranque el instalador: no puede sobrescribir un
  /// ejecutable en uso.
  final Future<void> Function() onSalirParaActualizar;

  /// Los bloques que solo tienen sentido en el modo desde el que se abrió.
  final List<Widget> propiosDelModo;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AlertDialog(
      title: const Text('Ajustes'),
      content: SizedBox(
        width: 380,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Se repinta con cada muestra del monitor (cada 3 s); es lo único
              // vivo del diálogo.
              AnimatedBuilder(
                animation: monitor,
                builder: (context, _) {
                  final uso = monitor.uso;
                  return Row(
                    children: [
                      Expanded(
                        child: _Medida(
                          icono: Icons.memory,
                          etiqueta: 'Memoria',
                          valor: UsoDeRecursos.mb(uso.total),
                        ),
                      ),
                      Expanded(
                        child: _Medida(
                          icono: Icons.speed,
                          etiqueta: 'CPU',
                          valor: '${uso.cpu.toStringAsFixed(1)} %',
                        ),
                      ),
                    ],
                  );
                },
              ),
              const Divider(height: 28),
              AnimatedBuilder(
                animation: settings,
                builder: (context, _) => Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SwitchListTile(
                      contentPadding: EdgeInsets.zero,
                      value: settings.performanceMode,
                      onChanged: (v) => unawaited(settings.setPerformanceMode(v)),
                      title: const Text('Modo rendimiento'),
                      subtitle: Text(
                        'Sustituye las carátulas por mosaicos de color y apaga el '
                        'lector de metadatos. El audio no se toca.',
                        style: theme.textTheme.bodySmall
                            ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                      ),
                    ),
                    const Divider(height: 12),
                    _CacheDeContinuaciones(settings: settings),
                    const Divider(height: 12),
                    SwitchListTile(
                      contentPadding: EdgeInsets.zero,
                      value: settings.discordRpcEnabled,
                      onChanged: (v) => unawaited(settings.setDiscordRpcEnabled(v)),
                      title: const Text('Mostrar en Discord (Rich Presence)'),
                      subtitle: Text(
                        'Enseña en tu perfil de Discord la canción que suena, la '
                        'siguiente de la cola y un botón a GitHub.',
                        style: theme.textTheme.bodySmall
                            ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                      ),
                    ),
                    if (settings.discordRpcEnabled) ...[
                      const SizedBox(height: 8),
                      _CampoDiscordClientId(settings: settings),
                    ],
                  ],
                ),
              ),
              for (final bloque in propiosDelModo) ...[
                const Divider(height: 12),
                bloque,
              ],
              const Divider(height: 12),
              _Actualizaciones(
                updater: updater,
                onSalirParaActualizar: onSalirParaActualizar,
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cerrar'),
        ),
      ],
    );
  }
}

/// Versión instalada y actualización en un clic.
class _Actualizaciones extends StatelessWidget {
  const _Actualizaciones({
    required this.updater,
    required this.onSalirParaActualizar,
  });

  final Updater updater;
  final Future<void> Function() onSalirParaActualizar;

  Future<void> _actualizar(BuildContext context) async {
    await updater.descargar();
    if (updater.estado != EstadoActualizacion.listaParaInstalar) return;
    if (await updater.instalar()) {
      // El instalador ya está corriendo; la app tiene que quitarse de en medio.
      await onSalirParaActualizar();
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return AnimatedBuilder(
      animation: updater,
      builder: (context, _) {
        final estado = updater.estado;
        return Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  // El nombre del modo activo, no siempre "NeoFy": el binario
                  // y la versión son los mismos para los dos, pero la app se
                  // presenta con la identidad del modo en el que estás (es lo
                  // que hace también el título de la barra lateral).
                  Text(
                    'NeoTube ${updater.versionActual}',
                    style: theme.textTheme.bodyMedium,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    switch (estado) {
                      EstadoActualizacion.buscando => 'Buscando…',
                      EstadoActualizacion.alDia => 'Estás al día',
                      // Donde no se instala sola (Linux), el aviso tiene que
                      // decir qué hacer: si no, informa de una novedad y deja
                      // al usuario sin ninguna forma de cogerla.
                      EstadoActualizacion.disponible => Updater.seInstalaSolo
                          ? 'Hay una versión nueva: ${updater.versionDisponible}'
                          : 'Hay una versión nueva (${updater.versionDisponible}). '
                              'Actualiza con ${Updater.comandoDeActualizacion}',
                      EstadoActualizacion.descargando =>
                        'Descargando… ${(updater.progreso * 100).round()} %',
                      EstadoActualizacion.listaParaInstalar =>
                        'Instalando; NeoTube se reiniciará',
                      EstadoActualizacion.fallo =>
                        updater.error ?? 'No se pudo comprobar',
                      EstadoActualizacion.reposo => 'Comprobar si hay novedades',
                    },
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: estado == EstadoActualizacion.disponible
                          ? theme.colorScheme.primary
                          : theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                  if (estado == EstadoActualizacion.descargando) ...[
                    const SizedBox(height: 6),
                    LinearProgressIndicator(
                      minHeight: 3,
                      value: updater.progreso > 0 ? updater.progreso : null,
                    ),
                  ],
                ],
              ),
            ),
            const SizedBox(width: 12),
            if (estado == EstadoActualizacion.disponible &&
                Updater.seInstalaSolo)
              FilledButton(
                onPressed: () => _actualizar(context),
                child: const Text('Actualizar'),
              )
            else if (estado != EstadoActualizacion.descargando &&
                estado != EstadoActualizacion.listaParaInstalar)
              OutlinedButton(
                onPressed: estado == EstadoActualizacion.buscando
                    ? null
                    : () => unawaited(updater.buscar()),
                child: const Text('Buscar'),
              ),
          ],
        );
      },
    );
  }
}

class _Medida extends StatelessWidget {
  const _Medida({
    required this.icono,
    required this.etiqueta,
    required this.valor,
  });

  final IconData icono;
  final String etiqueta;
  final String valor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      children: [
        Icon(icono, size: 20, color: theme.colorScheme.onSurfaceVariant),
        const SizedBox(height: 6),
        Text(valor, style: theme.textTheme.titleLarge),
        Text(
          etiqueta,
          style: theme.textTheme.labelSmall
              ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
      ],
    );
  }
}

class _CampoDiscordClientId extends StatefulWidget {
  const _CampoDiscordClientId({required this.settings});

  final Settings settings;

  @override
  State<_CampoDiscordClientId> createState() => _CampoDiscordClientIdState();
}

class _CampoDiscordClientIdState extends State<_CampoDiscordClientId> {
  late final TextEditingController _controller;
  late final FocusNode _foco;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.settings.discordClientId);
    _foco = FocusNode()..addListener(_alPerderElFoco);
  }

  void _alPerderElFoco() {
    if (!_foco.hasFocus) _guardar();
  }

  void _guardar() => unawaited(widget.settings.setDiscordClientId(_controller.text));

  @override
  void didUpdateWidget(covariant _CampoDiscordClientId oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.settings.discordClientId != _controller.text) {
      _controller.text = widget.settings.discordClientId;
    }
  }

  @override
  void dispose() {
    _foco.removeListener(_alPerderElFoco);
    _foco.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return TextField(
      controller: _controller,
      focusNode: _foco,
      decoration: InputDecoration(
        labelText: 'Client ID de Discord',
        hintText: 'Pega aquí el Client ID de tu aplicación',
        helperText:
            'Crea una app en el Discord Developer Portal con un asset llamado "logo" '
            '(en minuscula: Discord distingue mayusculas)',
        helperMaxLines: 2,
        helperStyle: theme.textTheme.bodySmall
            ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      onSubmitted: (_) => _guardar(),
    );
  }
}

/// Cuánto puede ocupar en disco la caché de descargas de repuesto — la copia
/// completa que se baja por si la vía rápida choca con el corte de posición
/// pasado el primer minuto (ver `YtPlayer._descargarCompletaConYtDlp`). En
/// `0 MB` se apaga del todo: ni se descarga ni se guarda nada.
class _CacheDeContinuaciones extends StatefulWidget {
  const _CacheDeContinuaciones({required this.settings});

  final Settings settings;

  @override
  State<_CacheDeContinuaciones> createState() => _CacheDeContinuacionesState();
}

class _CacheDeContinuacionesState extends State<_CacheDeContinuaciones> {
  double? _arrastrando;
  late final TextEditingController _rutaController;
  late final FocusNode _rutaFoco;

  /// Lo que ocupa ahora mismo la carpeta, y cuántas canciones hay. Se mide al
  /// abrir Ajustes y tras vaciarla: es una lectura de disco, no algo que
  /// tenga sentido repetir en cada repintado.
  int? _bytesEnDisco;
  int? _cancionesEnDisco;

  /// 100 GB exactos, en MB — que es la unidad en la que se guarda de verdad.
  static const _maxMB = 100 * 1024.0;

  @override
  void initState() {
    super.initState();
    _rutaController = TextEditingController(text: widget.settings.rutaCacheContinuaciones ?? '');
    _rutaFoco = FocusNode()..addListener(_alPerderElFocoDeRuta);
    unawaited(_medir());
  }

  String get _rutaEfectiva =>
      rutaEfectivaDeCacheContinuaciones(widget.settings.rutaCacheContinuaciones);

  Future<void> _medir() async {
    var bytes = 0;
    var cuenta = 0;
    try {
      final dir = Directory(_rutaEfectiva);
      if (dir.existsSync()) {
        await for (final e in dir.list()) {
          if (e is! File) continue;
          bytes += await e.length();
          cuenta++;
        }
      }
    } catch (_) {
      // Carpeta inaccesible: se enseña como vacía, que es lo único honesto
      // que se puede decir sin poder mirarla.
    }
    if (!mounted) return;
    setState(() {
      _bytesEnDisco = bytes;
      _cancionesEnDisco = cuenta;
    });
  }

  Future<void> _vaciar() async {
    try {
      final dir = Directory(_rutaEfectiva);
      if (dir.existsSync()) {
        await for (final e in dir.list()) {
          if (e is File) await e.delete().catchError((_) => e);
        }
      }
    } catch (_) {
      // Lo que no se pueda borrar se queda; la medición de después lo dirá.
    }
    await _medir();
  }

  void _alPerderElFocoDeRuta() {
    if (!_rutaFoco.hasFocus) _guardarRuta();
  }

  void _guardarRuta() {
    unawaited(
      widget.settings.setRutaCacheContinuaciones(_rutaController.text).then((_) => _medir()),
    );
  }

  @override
  void didUpdateWidget(covariant _CacheDeContinuaciones oldWidget) {
    super.didUpdateWidget(oldWidget);
    final ruta = widget.settings.rutaCacheContinuaciones ?? '';
    if (ruta != _rutaController.text) _rutaController.text = ruta;
  }

  @override
  void dispose() {
    _rutaFoco.removeListener(_alPerderElFocoDeRuta);
    _rutaFoco.dispose();
    _rutaController.dispose();
    super.dispose();
  }

  static String _etiqueta(double mb) {
    if (mb == 0) return 'Apagada';
    if (mb < 1024) return '${mb.round()} MB';
    return '${(mb / 1024).toStringAsFixed(mb % 1024 == 0 ? 0 : 1)} GB';
  }

  static String _enMB(int bytes) {
    final mb = bytes / (1024 * 1024);
    if (mb < 1024) return '${mb.toStringAsFixed(mb < 10 ? 1 : 0)} MB';
    return '${(mb / 1024).toStringAsFixed(1)} GB';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final valor =
        (_arrastrando ?? widget.settings.limiteCacheContinuacionesMB.toDouble())
            .clamp(0.0, _maxMB);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Caché de reproducción', style: theme.textTheme.titleSmall),
        Text(
          'Una caché para que la reproducción sea fluida. En 0 se apaga.',
          style: theme.textTheme.bodySmall
              ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
        Row(
          children: [
            Expanded(
              child: Slider(
                value: valor,
                max: _maxMB,
                divisions: 100, // pasos de 1 GB
                label: _etiqueta(valor),
                onChanged: (v) => setState(() => _arrastrando = v),
                onChangeEnd: (v) {
                  unawaited(widget.settings.setLimiteCacheContinuaciones(v.round()));
                  setState(() => _arrastrando = null);
                },
              ),
            ),
            SizedBox(
              width: 64,
              child: Text(
                _etiqueta(valor),
                textAlign: TextAlign.end,
                style: theme.textTheme.bodySmall,
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        TextField(
          controller: _rutaController,
          focusNode: _rutaFoco,
          style: theme.textTheme.bodySmall,
          decoration: InputDecoration(
            labelText: 'Carpeta de la caché',
            hintText: 'Vacío = la de por defecto',
            helperText: 'No mueve lo que ya haya en la carpeta anterior.',
            helperMaxLines: 2,
            helperStyle: theme.textTheme.bodySmall
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            isDense: true,
            border: const OutlineInputBorder(),
          ),
          onSubmitted: (_) => _guardarRuta(),
        ),
        const SizedBox(height: 8),
        // La ruta de verdad, siempre: con el campo de arriba vacío no había
        // manera de saber **dónde** se está guardando, que es justo lo
        // primero que uno quiere comprobar de una caché en disco.
        Text('Se guarda en', style: theme.textTheme.labelSmall),
        SelectableText(
          _rutaEfectiva,
          style: theme.textTheme.bodySmall
              ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
        const SizedBox(height: 4),
        Row(
          children: [
            Expanded(
              child: Text(
                _bytesEnDisco == null
                    ? 'Midiendo…'
                    : '${_enMB(_bytesEnDisco!)} en uso · '
                        '${_cancionesEnDisco == 1 ? "1 canción" : "$_cancionesEnDisco canciones"}',
                style: theme.textTheme.bodySmall,
              ),
            ),
            TextButton(
              onPressed: (_bytesEnDisco ?? 0) == 0 ? null : () => unawaited(_vaciar()),
              child: const Text('Vaciar'),
            ),
          ],
        ),
      ],
    );
  }
}
