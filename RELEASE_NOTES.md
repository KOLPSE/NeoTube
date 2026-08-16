## ⚠️ Sobre los cortes de reproducción — léelo antes de instalar

**YouTube rechaza a veces la URL del stream que él mismo acaba de emitir.** Cuando pasa, esa
canción tarda **~2,5 s en arrancar en vez de ~0,1 s**. Suena igual: NeoTube lo detecta y
reintenta solo por otra vía.

**Va a rachas, y las malas son bastante malas.** Medido durante el desarrollo: hay tramos de
17 canciones seguidas sin un solo fallo, y tramos donde falla **una de cada dos**. No es un
goteo constante; es a temporadas.

No es un fallo de NeoTube ni de tu conexión: ocurre igual usando `yt-dlp` directamente, sin
NeoTube de por medio. Pero **la causa exacta no está identificada**, así que tampoco vamos a
jurar que sea solo cosa de YouTube.

Hay una mitigación puesta —renovar la identidad de visitante al detectar el problema y cada
30 minutos— y hay que decir que **no funcionó**: medida contra el mismo escenario, la tasa de
fallos no bajó. Se queda porque no hace daño, no porque lo arregle.

Si te toca una racha mala, cerrar y abrir la app a veces la corta.

---

## Reproducción: de ~2,7 s a ~0,12 s por canción

Las canciones ya no pasan por un subproceso. La URL del audio se le pide directamente a la
misma API interna con la que NeoTube habla para todo lo demás, en la misma aplicación.

- **~120 ms de mediana** contra los ~2700 ms de antes: el silencio entre pistas desaparece.
- Deja de hacer falta un runtime de JavaScript al lado, que era otra fuente de fallos.
- `yt-dlp` sigue incluido como plan B y entra solo cuando la vía rápida no puede.

## Discord Rich Presence

Nuevo, y **apagado por defecto**: se enciende en Ajustes. Muestra qué estás escuchando, la
carátula, la barra de progreso, cuál es la siguiente y un botón a GitHub.

## Arreglos

- Las canciones dentro de un **álbum** ya salen con su carátula. Las filas de un álbum no la
  traen (enseñan el número de pista), así que ahora heredan la portada del álbum.
- La barra de progreso de Discord aparece siempre, también en canciones abiertas desde la
  portada o desde el buscador.
