#!/usr/bin/env bash
#
# Descarga el runtime de Deno y lo deja donde yt-dlp (y NeoTube) lo buscan.
# Equivalente de tool/fetch_deno.ps1, que es el de Windows.
#
# yt-dlp necesita un runtime de JavaScript para descifrar las firmas de
# algunos formatos; sin el, avisa de "No supported JavaScript runtime could
# be found" y cae a una extraccion mas fragil (mas peticiones a paginas web
# en vez de a la API, que es justo lo que dispara los 429 y el aviso de bot
# que ven los usuarios). Deno es el runtime que yt-dlp sabe usar de serie
# (`--js-runtimes deno:<ruta>`, ya cableado en YtPlayer.findDenoBinary()).
#
#   ./tool/fetch_deno.sh

set -euo pipefail

raiz="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
destino="$raiz/tool/ytdlp-build/bin"
mkdir -p "$destino"

bin="$destino/deno"
zip="$destino/deno.zip"
url='https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip'

echo "Descargando Deno desde $url ..."
curl -L --fail -o "$zip" "$url"
unzip -o "$zip" -d "$destino"
rm "$zip"
chmod +x "$bin"

[ -x "$bin" ] || { echo "La descarga termino pero no aparece $bin" >&2; exit 1; }
echo "Deno listo en $bin ($(du -h "$bin" | cut -f1))"
