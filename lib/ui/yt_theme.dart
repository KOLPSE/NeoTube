import 'package:flutter/material.dart';

/// Colores y tipografía de YouTube Music, sacados de la CSS del propio sitio
/// (music.youtube.com, variables `--ytmusic-*` y `--yt-sys-color-baseline-*`).
///
/// No son aproximaciones de Material: son los valores que pinta la web, con su
/// nombre de variable anotado para que se pueda volver a verificar en cualquier
/// momento. Ver `tool/` o la sección de diseño de CONTEXTO.md si algo cambia.

/// Rojo de marca (logo, acentos de identidad).
/// `--yt-sys-color-baseline--static-brand-red: #f03`.
const Color ytRojoMarca = Color(0xFFFF0033);

/// Rojo del progreso de reproducción (la parte ya escuchada de la barra).
/// `--ytmusic-color-youtubered: #f00`. Distinto del de marca: son dos rojos.
const Color ytRojoProgreso = Color(0xFFFF0000);

/// Azul de las llamadas a la acción y los enlaces.
/// `--yt-sys-color-baseline--call-to-action: #3ea6ff` (oscuro).
const Color ytAzulAccion = Color(0xFF3EA6FF);

/// Verde de los estados "correcto" (p. ej. "guardado").
/// `--yt-sys-color-baseline--themed-green: #2ba640` (oscuro).
const Color ytVerde = Color(0xFF2BA640);

/// Rojo de los errores.
/// `--ytmusic-color-lightred: #ff4e45`.
const Color ytRojoError = Color(0xFFFF4E45);

/// Fondo base de la app (la web entera).
/// `--yt-sys-color-baseline--base-background: #0f0f0f` (oscuro).
const Color _oscuroFondo = Color(0xFF0F0F0F);

/// Superficie elevada (barra del reproductor, menús, chips, hover).
/// `--yt-sys-color-baseline--raised-background: #212121` (oscuro).
const Color _oscuroElevado = Color(0xFF212121);

/// Fondo de los menús.
/// `--yt-sys-color-baseline--menu-background: #282828` (oscuro).
const Color _oscuroMenu = Color(0xFF282828);

/// Texto secundario en oscuro. `--text-secondary: #aaa`.
const Color _oscuroTextoSecundario = Color(0xFFAAAAAA);

/// Borde en oscuro. `--outline-opaque: #3f3f3f`.
const Color _oscuroBorde = Color(0xFF3F3F3F);

/// Divisor sutil en oscuro (línea de la guía, separadores).
/// `--ytmusic-divider: var(--ytmusic-color-white1-alpha10)`.
const Color _oscuroDivisor = Color(0x1AFFFFFF);

// ---------------------------------------------------------------- claro

const Color _claroFondo = Color(0xFFFFFFFF);
const Color _claroTexto = Color(0xFF030303);
const Color _claroTextoSecundario = Color(0xFF606060);
const Color _claroBorde = Color(0xFFE5E5E5);
const Color _claroDivisor = Color(0x1A000000);
const Color _claroAzulAccion = Color(0xFF065FD4);

/// Tema de YouTube Music para la claridad pedida.
///
/// El [ColorScheme] se construye con `fromSeed` y luego se **clava** con
/// `copyWith`: el rojo de marca no es el rojo tonal de Material y el fondo no
/// es el gris claro de Material, así que dejar que `fromSeed` decida era
/// exactamente el "morado de framework" que no queremos.
ThemeData ytTheme(Brightness brightness) {
  final oscuro = brightness == Brightness.dark;

  final base =
      ColorScheme.fromSeed(
        seedColor: ytRojoMarca,
        brightness: brightness,
      ).copyWith(
        primary: ytRojoMarca,
        onPrimary: const Color(0xFFFFFFFF),
        secondary: oscuro ? ytAzulAccion : _claroAzulAccion,
        onSecondary: oscuro ? const Color(0xFF030303) : const Color(0xFFFFFFFF),
        tertiary: ytVerde,
        onTertiary: const Color(0xFFFFFFFF),
        error: ytRojoError,
        onError: const Color(0xFFFFFFFF),
        surface: oscuro ? _oscuroFondo : _claroFondo,
        onSurface: oscuro ? const Color(0xFFFFFFFF) : _claroTexto,
        onSurfaceVariant: oscuro
            ? _oscuroTextoSecundario
            : _claroTextoSecundario,
        outline: oscuro ? _oscuroBorde : _claroBorde,
        outlineVariant: oscuro ? _oscuroDivisor : _claroDivisor,
        surfaceContainerLowest: oscuro ? _oscuroFondo : _claroFondo,
        surfaceContainerLow: oscuro
            ? const Color(0xFF181818)
            : const Color(0xFFF9F9F9),
        surfaceContainer: oscuro ? _oscuroElevado : const Color(0xFFF1F1F1),
        surfaceContainerHigh: oscuro ? _oscuroElevado : const Color(0xFFF1F1F1),
        surfaceContainerHighest: oscuro ? _oscuroMenu : const Color(0xFFE9E9E9),
        surfaceTint: Colors.transparent,
        scrim: Colors.black,
        shadow: Colors.black,
      );

  final colorDeTexto = base.onSurface;
  final colorDeTextoSecundario = base.onSurfaceVariant;

  final texto = TextTheme(
    // La web usa Roboto (400/500/700) con YouTube Sans para los títulos de
    // display. Roboto es lo que ya trae Flutter, así que se conserva.
    displaySmall: TextStyle(
      fontSize: 36,
      fontWeight: FontWeight.w400,
      color: colorDeTexto,
    ),
    // Título de sección de la portada ("Vídeos musicales para ti"): grande y
    // en negrita, como en la web (~34 px / 700).
    headlineLarge: TextStyle(
      fontSize: 34,
      fontWeight: FontWeight.w700,
      color: colorDeTexto,
    ),
    headlineMedium: TextStyle(
      fontSize: 28,
      fontWeight: FontWeight.w700,
      color: colorDeTexto,
    ),
    headlineSmall: TextStyle(
      fontSize: 24,
      fontWeight: FontWeight.w700,
      color: colorDeTexto,
    ),
    titleLarge: TextStyle(
      fontSize: 20,
      fontWeight: FontWeight.w500,
      color: colorDeTexto,
    ),
    titleMedium: TextStyle(
      fontSize: 16,
      fontWeight: FontWeight.w500,
      color: colorDeTexto,
    ),
    titleSmall: TextStyle(
      fontSize: 14,
      fontWeight: FontWeight.w500,
      color: colorDeTexto,
    ),
    bodyLarge: TextStyle(
      fontSize: 16,
      fontWeight: FontWeight.w400,
      color: colorDeTexto,
    ),
    bodyMedium: TextStyle(
      fontSize: 14,
      fontWeight: FontWeight.w400,
      color: colorDeTexto,
    ),
    // ⚠️ `bodySmall` lleva el color primario a propósito: es el estilo de los
    // títulos de tarjeta, que no tienen `copyWith` propio. El secundario se
    // aplica explícitamente donde toca (subtítulos, duraciones).
    bodySmall: TextStyle(
      fontSize: 12,
      fontWeight: FontWeight.w400,
      color: colorDeTexto,
    ),
    labelLarge: TextStyle(
      fontSize: 14,
      fontWeight: FontWeight.w500,
      color: colorDeTexto,
    ),
    labelMedium: TextStyle(
      fontSize: 12,
      fontWeight: FontWeight.w500,
      color: colorDeTexto,
    ),
    labelSmall: TextStyle(
      fontSize: 11,
      fontWeight: FontWeight.w400,
      color: colorDeTextoSecundario,
    ),
  );

  return ThemeData(
    useMaterial3: true,
    brightness: brightness,
    colorScheme: base,
    scaffoldBackgroundColor: oscuro ? _oscuroFondo : _claroFondo,
    textTheme: texto,
    dividerTheme: DividerThemeData(
      color: base.outlineVariant,
      thickness: 1,
      space: 1,
    ),
    iconTheme: IconThemeData(color: colorDeTexto),
    listTileTheme: ListTileThemeData(
      iconColor: colorDeTexto,
      textColor: colorDeTexto,
      selectedColor: colorDeTexto,
      selectedTileColor: const Color(0x14FFFFFF),
    ),
    chipTheme: ChipThemeData(
      backgroundColor: const Color(0x1AFFFFFF),
      selectedColor: const Color(0x33FFFFFF),
      labelStyle: TextStyle(fontSize: 14, color: colorDeTexto),
      side: BorderSide.none,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
    ),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: oscuro ? _oscuroElevado : const Color(0xFF212121),
      contentTextStyle: const TextStyle(color: Color(0xFFFFFFFF), fontSize: 14),
      behavior: SnackBarBehavior.floating,
    ),
    tooltipTheme: TooltipThemeData(
      waitDuration: const Duration(milliseconds: 600),
      decoration: BoxDecoration(
        color: oscuro ? _oscuroElevado : const Color(0xFF212121),
        borderRadius: BorderRadius.circular(4),
      ),
      textStyle: const TextStyle(color: Color(0xFFFFFFFF), fontSize: 12),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: base.primary,
        foregroundColor: base.onPrimary,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
    ),
    // Los botones de texto (Ajustes, Salir) van neutros, no rojos: en la web
    // solo el rojo de marca es el que acentúa, y estos son acciones planas.
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(foregroundColor: colorDeTexto),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: colorDeTexto,
        side: BorderSide(color: base.outline),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: const Color(0x1AFFFFFF),
      hintStyle: TextStyle(color: colorDeTextoSecundario, fontSize: 14),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: BorderSide.none,
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: BorderSide.none,
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
        borderSide: const BorderSide(color: ytAzulAccion, width: 1),
      ),
    ),
  );
}

/// El logotipo de YouTube Music: el círculo rojo con el triángulo de play y la
/// palabra "Music", como en la esquina superior izquierda de la web.
///
/// Es un widget compartido para que la barra superior y el login enseñen
/// exactamente la misma marca, sin que cada sitio dibuje la suya por su cuenta.
class YtMusicLogo extends StatelessWidget {
  const YtMusicLogo({super.key, this.size = 34});

  /// El alto del icono. El texto escala en proporción.
  final double size;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Icons.play_circle_fill, color: ytRojoMarca, size: size),
        SizedBox(width: size * 0.24),
        Text(
          'Music',
          style: theme.textTheme.titleLarge?.copyWith(
            fontWeight: FontWeight.w700,
            fontSize: size * 0.65,
          ),
        ),
      ],
    );
  }
}
