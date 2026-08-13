; Instalador de NeoTube (Inno Setup 6).
;
; Empaqueta el ejecutable Flutter, sus plugins, recursos y yt-dlp.
;
; Se genera con:
;   powershell -ExecutionPolicy Bypass -File tool\build_installer.ps1
;
; Es un instalador por usuario (PrivilegesRequired=lowest): no pide
; administrador, no toca Archivos de programa y no necesita firma para
; instalarse. Para una app que solo escribe en %APPDATA% y %LOCALAPPDATA% no
; hace falta más, y quita la fricción de un SmartScreen pidiendo elevación.

#define Nombre     "NeoTube"
; La versión la inyecta build_installer.ps1 leyéndola de `kVersion` en
; lib/core/app_config.dart, que es la única fuente de la verdad.
#ifndef Version
  #define Version  "0.0.0"
#endif
#define Autor      "neogex.xyz"
#define Ejecutable "neotube.exe"

[Setup]
AppId={{D3E4F5A6-8B1C-4D2E-9F0A-NEOTUBE00001}
AppName={#Nombre}
AppVersion={#Version}
AppVerName={#Nombre} {#Version}
AppPublisher={#Autor}
DefaultDirName={autopf}\{#Nombre}
DefaultGroupName={#Nombre}
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=NeoTube-{#Version}-windows-x64
SetupIconFile=..\windows\runner\resources\app_icon.ico
UninstallDisplayIcon={app}\{#Ejecutable}
UninstallDisplayName={#Nombre}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Windows 10 1809 en adelante, que es lo que pide Flutter en escritorio.
MinVersion=10.0.17763

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "startup"; Description: "Iniciar NeoTube al encender el equipo"; GroupDescription: "Inicio"; Flags: unchecked

[Files]
; Todo el contenido de la carpeta Release: el exe, flutter_windows.dll, los
; plugins, la carpeta data\ con los assets y yt-dlp.exe.
Source: "..\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#Nombre}"; Filename: "{app}\{#Ejecutable}"
Name: "{group}\{cm:UninstallProgram,{#Nombre}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#Nombre}"; Filename: "{app}\{#Ejecutable}"; Tasks: desktopicon
Name: "{userstartup}\{#Nombre}"; Filename: "{app}\{#Ejecutable}"; Tasks: startup

[Run]
; Instalación normal: la casilla de "ejecutar ahora" del final del asistente.
Filename: "{app}\{#Ejecutable}"; Description: "{cm:LaunchProgram,{#Nombre}}"; Flags: nowait postinstall skipifsilent
; ⚠️ Y en silencio hay que lanzarla igualmente, sin casilla que marcar. Una
; actualización desde la propia app va con `/SILENT` (ver `updater.dart`), y
; ahí `skipifsilent` se saltaba la línea de arriba: la app se cerraba para
; dejarse actualizar y no volvía nunca. `runasoriginaluser` evita que se ejecute
; como administrador si alguien elevó el instalador a mano.
Filename: "{app}\{#Ejecutable}"; Flags: nowait runasoriginaluser; Check: WizardSilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM neotube.exe"; Flags: runhidden; RunOnceId: "MatarApp"

[UninstallDelete]
; La caché de carátulas se puede regenerar; no tiene sentido dejarla
; ocupando disco tras desinstalar. Las cookies y configuración NO se tocan
; aquí a propósito: si el usuario reinstala, conservará su sesión.
Type: filesandordirs; Name: "{userappdata}\neotube\art"

[Code]

function ProcesoVivo(Nombre: String): Boolean;
var
  Codigo: Integer;
begin
  Result := Exec(ExpandConstant('{cmd}'),
                 '/C tasklist /FI "IMAGENAME eq ' + Nombre + '" /NH | ' +
                 'find /I "' + Nombre + '" >nul',
                 '', SW_HIDE, ewWaitUntilTerminated, Codigo) and (Codigo = 0);
end;

function EsperarAQueMuera(Nombre: String; Intentos: Integer): Boolean;
var
  i: Integer;
begin
  for i := 1 to Intentos do
  begin
    if not ProcesoVivo(Nombre) then
    begin
      Result := True;
      Exit;
    end;
    Sleep(250);
  end;
  Result := not ProcesoVivo(Nombre);
end;

function InitializeSetup(): Boolean;
var
  Codigo: Integer;
begin
  // Si la actualización sale de la propia app, se le dan 4 segundos para
  // cerrarse por las buenas y soltar la bandeja.
  EsperarAQueMuera('neotube.exe', 16);

  // Forzar cierre si continuara vivo.
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM neotube.exe', '',
       SW_HIDE, ewWaitUntilTerminated, Codigo);

  // ⚠️ Esperar a que el proceso muera de verdad para que flutter_windows.dll
  // no quede bloqueada durante la copia de ficheros.
  EsperarAQueMuera('neotube.exe', 40);
  Result := True;
end;
