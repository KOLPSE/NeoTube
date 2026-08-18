## Reproducción: arreglado el corte a mitad de canción

Cualquier canción de más de un minuto largo se cortaba y saltaba sola a la siguiente. La causa
no tenía nada que ver con identidades de sesión ni con reintentos: **YouTube deja de servir el
audio de esta app pasado 1.000.000 de bytes exactos** (un límite nuevo, del protocolo de
streaming con el que está sustituyendo la descarga progresiva de toda la vida — hasta el propio
`yt-dlp`, con su cliente habitual, tropieza con el mismo muro).

La solución: en cuanto empieza a sonar una canción, NeoTube ya está bajando en segundo plano una
copia completa por otra vía. Si la vía rápida choca con el corte, la app **retoma exactamente
donde iba** con esa copia — se nota un parón de bien menos de un segundo en ese punto, pero la
canción sigue en vez de saltar. Esas copias no se acumulan: solo se guardan las dos últimas y se
borran solas.

## Reintento más persistente

Si una pista aun así no arranca a la primera, se reintenta hasta dos veces (antes, una) antes de
rendirse, y el error que se enseña viene traducido y resumido en vez del volcado en inglés de
yt-dlp.

## Deno viaja empaquetado

El plan B de yt-dlp necesita un runtime de JavaScript para descifrar firmas; sin él caía a una
extracción más frágil que era la que disparaba los avisos de "confirma que no eres un bot".
Ahora Deno va dentro del instalador y del paquete de Arch, así que ese plan B no depende de
tenerlo ya instalado en el sistema.
