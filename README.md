# NeoTube

Cliente nativo de **YouTube Music** para escritorio (Windows y Linux), en Flutter.

Habla con la API interna de YouTube Music (`youtubei/v1`) usando las cookies de una sesión de
verdad, resuelve el stream de cada pista con **yt-dlp** y lo reproduce con **media_kit**
(libmpv). Sin servidor propio, sin clave de API y sin OAuth: te logueas una vez en una ventana
de Google y ya.

> **Estado:** este repositorio es el código recién separado de
> [NeoFy](https://github.com/KOLPSE/NeoFy), donde NeoTube era el segundo modo de la app.
> Todavía **no arranca por su cuenta**: falta `lib/main.dart`, los runners de Windows y Linux
> y el empaquetado.

## Antes de tocar nada, lee [`CONTEXTO.md`](CONTEXTO.md)

Ahí está cómo funciona todo y, sobre todo, **la lista de trampas de la API interna verificadas
contra la API real**. No hay documentación oficial de esta API: casi toda decisión que parece
rara en el código está explicada ahí porque la alternativa obvia no funciona.

La más cara, como aperitivo: `APISID` y `SAPISID` son cookies distintas, y firmar con la
primera no da un 401 — da un **200 con la sesión anónima**, así que todo parece funcionar
salvo lo único que depende de saber quién eres.

## Qué falta

La lista exacta está en la sección [«Lo que falta para que
arranque»](CONTEXTO.md#9-lo-que-falta-para-que-arranque) de `CONTEXTO.md`.

## Licencia

Ver [`LICENSE`](LICENSE).
