import 'dart:io';

/// Sufijo de los ejecutables: `.exe` en Windows, nada en Linux.
final String sufijoEjecutable = Platform.isWindows ? '.exe' : '';
