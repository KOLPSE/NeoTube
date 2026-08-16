# NeoTube

Cliente nativo de **YouTube Music** para escritorio (Windows y Linux), en Flutter.

Habla con la API interna de YouTube Music (`youtubei/v1`) usando las cookies de una sesión de
verdad, le pide a esa misma API la URL del stream de cada pista y la reproduce con
**media_kit** (libmpv). Sin servidor propio, sin clave de API y sin OAuth: te logueas una vez
en una ventana de Google y ya.

> **Estado:** arranca de verdad, en Windows y en Linux. Instalador de Windows y paquete de
> Arch Linux publicados en [Releases](https://github.com/KOLPSE/NeoTube/releases).

Se integra además con el escritorio: controles multimedia del sistema (SMTC en Windows, MPRIS
en Linux) y **Discord Rich Presence** opcional, que se enciende en Ajustes y viene apagado.

## ⚠️ La reproducción puede cortarse de vez en cuando

**YouTube rechaza a veces la URL del stream que él mismo acaba de emitir.** Cuando pasa, esa
canción tarda unos segundos de más en arrancar. No es un fallo de NeoTube ni de tu conexión:
ocurre igual usando `yt-dlp` a pelo, sin NeoTube de por medio.

La app lo resuelve sola —reintenta por otra vía y la canción suena—, así que lo único que se
nota es que esa pista concreta empieza en ~2,5 s en vez de ~0,1 s. No hay que hacer nada; si
se repite mucho, reiniciar la app renueva la identidad con la que se piden los streams.

La causa exacta **no está identificada**. Hay una mitigación puesta (renovar esa identidad al
detectar el problema y cada 30 minutos), pero no está demostrado que lo elimine del todo.

## Antes de tocar nada, lee [`CONTEXTO.md`](CONTEXTO.md)

Ahí está cómo funciona todo y, sobre todo, **la lista de trampas de la API interna verificadas
contra la API real**. No hay documentación oficial de esta API: casi toda decisión que parece
rara en el código está explicada ahí porque la alternativa obvia no funciona.

La más cara, como aperitivo: `APISID` y `SAPISID` son cookies distintas, y firmar con la
primera no da un 401 — da un **200 con la sesión anónima**, así que todo parece funcionar
salvo lo único que depende de saber quién eres.

## Cómo instalarlo

- **Windows:** descarga el instalador `.exe` de la [última
  release](https://github.com/KOLPSE/NeoTube/releases/latest).
- **Arch Linux:** añade el repositorio a `/etc/pacman.conf`:

  ```
  [neotube]
  SigLevel = Optional TrustAll
  Server = https://github.com/KOLPSE/NeoTube/releases/download/repo
  ```

  y luego `sudo pacman -Sy neotube-bin`. A partir de ahí se actualiza solo con
  `sudo pacman -Syu`.
- **Otras distros de Linux:** el tarball genérico está también en la última release.

Los detalles del pipeline de compilación y publicación están en la sección [«Cómo se
compila y se publica»](CONTEXTO.md#9-cómo-se-compila-y-se-publica) de `CONTEXTO.md`.

## Licencia

Ver [`LICENSE`](LICENSE).
