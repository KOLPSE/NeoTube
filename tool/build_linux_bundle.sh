#!/usr/bin/env bash
#
# Empaqueta NeoTube para Linux: compila la app, asegura yt-dlp y saca un
# tarball listo para el PKGBUILD (linux/packaging/PKGBUILD).
#
#   ./tool/build_linux_bundle.sh
#
# Sale dist/NeoTube-<version>-linux-x86_64.tar.gz.

set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$raiz"

# La versión sale de kVersion en lib/core/app_config.dart, la fuente única de verdad.
version="$(grep -oP "kVersion\s*=\s*'\K[^']+" lib/core/app_config.dart)"
[ -n "$version" ] || { echo "No encuentro kVersion en lib/core/app_config.dart" >&2; exit 1; }
echo "NeoTube $version"

# yt-dlp se descarga cada vez que se empaqueta para no arrastrar una versión vieja.
echo "Actualizando yt-dlp..."
bash ./tool/fetch_ytdlp.sh
[ -x 'tool/ytdlp-build/bin/yt-dlp' ] || { echo "Falta tool/ytdlp-build/bin/yt-dlp" >&2; exit 1; }

# Deno: el runtime de JavaScript que yt-dlp necesita para no caer a una
# extracción más frágil (ver linux/CMakeLists.txt).
echo "Actualizando Deno..."
bash ./tool/fetch_deno.sh
[ -x 'tool/ytdlp-build/bin/deno' ] || { echo "Falta tool/ytdlp-build/bin/deno" >&2; exit 1; }

echo "Compilando la app para Linux (Release)..."
flutter build linux --release

bundle='build/linux/x64/release/bundle'
[ -d "$bundle" ] || { echo "No aparece $bundle" >&2; exit 1; }

# CMake ya lo copia por install(PROGRAMS), pero se comprueba y se asegura.
if [ ! -f "$bundle/yt-dlp" ]; then
  cp tool/ytdlp-build/bin/yt-dlp "$bundle/"
fi
if [ ! -f "$bundle/deno" ]; then
  cp tool/ytdlp-build/bin/deno "$bundle/"
fi
chmod +x "$bundle/yt-dlp" "$bundle/deno" "$bundle/neotube"

# Ficheros de integración de escritorio para el tarball.
cp linux/packaging/xyz.neogex.neotube.desktop "$bundle/"
cp -r linux/packaging/icons "$bundle/"
cp LICENSE "$bundle/"

mkdir -p dist
nombre="NeoTube-$version-linux-x86_64"
rm -rf "dist/$nombre" "dist/$nombre.tar.gz"
cp -r "$bundle" "dist/$nombre"

tar -czf "dist/$nombre.tar.gz" -C dist --owner=0 --group=0 "$nombre"

echo
echo "Listo: dist/$nombre.tar.gz ($(du -h "dist/$nombre.tar.gz" | cut -f1))"
echo "sha256: $(sha256sum "dist/$nombre.tar.gz" | cut -d' ' -f1)"
echo "Árbol sin comprimir en dist/$nombre/"
