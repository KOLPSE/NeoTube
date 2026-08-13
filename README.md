# NeoTube

Cliente nativo de **YouTube Music** para escritorio (Windows y Linux), en Flutter.

Habla con la API interna de YouTube Music (`youtubei/v1`) usando las cookies de una sesión de
verdad, resuelve el stream de cada pista con **yt-dlp** y lo reproduce con **media_kit**
(libmpv). Sin servidor propio, sin clave de API y sin OAuth: te logueas una vez en una ventana
de Google y ya.

> **Estado:** arranca de verdad, en Windows y en Linux. Instalador de Windows y paquete de
> Arch Linux publicados en [Releases](https://github.com/KOLPSE/NeoTube/releases).

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
