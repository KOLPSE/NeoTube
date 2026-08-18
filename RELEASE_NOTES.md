## Reproducción: se encontró la causa de verdad, y no era la que se pensaba

Las versiones anteriores explicaban los cortes de reproducción como una identidad de sesión
que se iba "gastando". Era la explicación equivocada. La causa real: **YouTube corta cualquier
descarga de audio en cuanto pasa de 1 000 000 de bytes exactos**, y `libmpv` pedía el fichero
entero de un tirón al abrir una canción — así que caía del lado malo del corte prácticamente
siempre, no de vez en cuando.

La solución no era reintentar con otra identidad (eso solo cambiaba la matrícula del mismo
coche parado en el mismo sitio): ahora NeoTube monta un pequeño servidor en `127.0.0.1` que
pide el audio en trozos de 900 000 bytes y se lo va pasando a `libmpv` como si fuera un
fichero continuo. `libmpv` ya no habla con los servidores de YouTube directamente.

Probado en varias sesiones seguidas sin un solo corte. Sigue habiendo un plan B automático
(yt-dlp) por si algún día cambia el límite, pero la causa de fondo ya no depende de adivinar
identidades.

## Reintento más persistente

Si una pista aun así no arranca a la primera, ahora se reintenta hasta dos veces (antes, una)
antes de rendirse, y el error que se enseña viene traducido y resumido en vez del volcado en
inglés de yt-dlp.

## Deno viaja empaquetado

El plan B de yt-dlp necesita un runtime de JavaScript para descifrar firmas; sin él caía a una
extracción más frágil que era la que disparaba los avisos de "confirma que no eres un bot".
Ahora Deno va dentro del instalador y del paquete de Arch, así que ese plan B no depende de
tenerlo ya instalado en el sistema.
