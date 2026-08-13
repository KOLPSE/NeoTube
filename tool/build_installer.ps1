# Compila NeoTube entero y lo empaqueta en un instalador único de Windows.
#
#   powershell -ExecutionPolicy Bypass -File tool\build_installer.ps1
#
# Deja en dist\ un solo ejecutable .exe (NeoTube-<version>-windows-x64.exe).

$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

function Paso($texto) { Write-Host "`n=== $texto ===" -ForegroundColor Cyan }

# --- 1. yt-dlp -------------------------------------------------------------
$ytdlp = "tool\ytdlp-build\bin\yt-dlp.exe"
Paso "Comprobando yt-dlp"
if (-not (Test-Path $ytdlp)) {
  & powershell -ExecutionPolicy Bypass -File tool\fetch_ytdlp.ps1
  if ($LASTEXITCODE -ne 0) { throw "Falló la descarga de yt-dlp" }
}
if (-not (Test-Path $ytdlp)) { throw "No se encuentra $ytdlp" }

# --- 2. La app -------------------------------------------------------------
Paso "Compilando NeoTube (Release)"
# Cerrar procesos que puedan bloquear la escritura de ejecutables/dlls.
foreach ($n in @('neotube', 'yt-dlp')) {
  Get-Process $n -ErrorAction SilentlyContinue | Stop-Process -Force
}

# Flutter sale del PATH, de FLUTTER_ROOT o de rutas conocidas.
$flutter = (Get-Command flutter -ErrorAction SilentlyContinue).Source
if (-not $flutter -and $env:FLUTTER_ROOT) {
  $candidato = Join-Path $env:FLUTTER_ROOT 'bin\flutter.bat'
  if (Test-Path $candidato) { $flutter = $candidato }
}
if (-not $flutter) {
  foreach ($c in @('D:\dev\flutter\bin\flutter.bat',
                   'C:\src\flutter\bin\flutter.bat',
                   'C:\flutter\bin\flutter.bat',
                   "$env:LOCALAPPDATA\flutter\bin\flutter.bat")) {
    if (Test-Path $c) { $flutter = $c; break }
  }
}
if (-not $flutter) {
  throw 'No encuentro Flutter. Añádelo al PATH o define FLUTTER_ROOT con la ' +
        'carpeta donde lo tengas (la que contiene bin\flutter.bat).'
}

Write-Host "Usando Flutter: $flutter"
& $flutter build windows --release
if ($LASTEXITCODE -ne 0) { throw "Falló la compilación de Flutter" }

$release = "build\windows\x64\runner\Release"

# Confirmar presencia de yt-dlp.exe en Release (instalado por CMake).
$ytdlpEnRelease = Join-Path $release "yt-dlp.exe"
if (-not (Test-Path $ytdlpEnRelease)) {
  Write-Host "Copiando yt-dlp.exe al directorio Release..."
  Copy-Item $ytdlp $release -Force
}

Get-ChildItem $release -Filter *.exe | ForEach-Object {
  "  {0,-24} {1,7:N1} MB" -f $_.Name, ($_.Length / 1MB)
}

# --- 3. El instalador ------------------------------------------------------
Paso "Generando el instalador con Inno Setup"
$iscc = @(
  "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
  "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
  "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc) {
  $cmdIscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
  if ($cmdIscc) { $iscc = $cmdIscc.Source }
}

if (-not $iscc) {
  throw "Falta Inno Setup 6 (ISCC.exe)."
}

Write-Host "Usando ISCC: $iscc"

# Leer versión de app_config.dart como fuente única de verdad.
$fuente = Get-Content "lib\core\app_config.dart" -Raw
if ($fuente -notmatch "kVersion\s*=\s*'([^']+)'") {
  throw "No encuentro kVersion en lib\core\app_config.dart"
}
$version = $Matches[1]
Write-Host "  Versión detectada: $version"

New-Item -ItemType Directory -Force -Path dist | Out-Null
& $iscc /Q "/DVersion=$version" "installer\neotube.iss"
if ($LASTEXITCODE -ne 0) { throw "ISCC falló al compilar el instalador." }

$instaladorFinal = "dist\NeoTube-$version-windows-x64.exe"
if (-not (Test-Path $instaladorFinal)) {
  throw "No se encontró el instalador generado en $instaladorFinal"
}

Paso "Instalador generado con éxito"
Get-ChildItem dist -Filter *.exe | ForEach-Object {
  "  {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB)
}
