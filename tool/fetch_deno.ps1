# Descarga el runtime de Deno y lo deja donde yt-dlp (y NeoTube) lo buscan.
#
# yt-dlp necesita un runtime de JavaScript para descifrar las firmas de
# algunos formatos; sin el, avisa de "No supported JavaScript runtime could
# be found" y cae a una extraccion mas fragil (mas peticiones a paginas web
# en vez de a la API, que es justo lo que dispara los 429 y el aviso de bot
# que ven los usuarios). Deno es el runtime que yt-dlp sabe usar de serie
# (`--js-runtimes deno:<ruta>`, ya cableado en YtPlayer.findDenoBinary()).
#
#   powershell -ExecutionPolicy Bypass -File tool\fetch_deno.ps1

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$binDir = Join-Path $root 'tool\ytdlp-build\bin'
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

$exe = Join-Path $binDir 'deno.exe'
$zip = Join-Path $binDir 'deno.zip'
$url = 'https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip'

Write-Output "Descargando Deno desde $url ..."
Invoke-WebRequest -Uri $url -OutFile $zip

Expand-Archive -Path $zip -DestinationPath $binDir -Force
Remove-Item $zip

if (-not (Test-Path $exe)) {
    throw "La descarga termino pero no aparece $exe"
}

$size = [math]::Round((Get-Item $exe).Length / 1MB, 1)
Write-Output "Deno listo en $exe ($size MB)"
