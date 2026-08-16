## ⚠️ Sobre los cortes de reproducción

**YouTube rechaza de vez en cuando la URL del stream ya emitida**, y cuando pasa esa canción
tarda unos segundos de más en arrancar. No es un fallo del programa ni de tu conexión: es del
lado de YouTube, y le ocurre igual a otras herramientas —se ha comprobado que pasa también
usando `yt-dlp` directamente, sin NeoTube de por medio—.

NeoTube lo resuelve solo: cuando ocurre, reintenta por otra vía y la canción suena igual.
Lo único que notarás es que esa pista concreta tarda ~2,5 s en empezar en vez de ~0,1 s.

No hace falta que hagas nada. Si te pasa muy seguido, cerrar y abrir la app renueva la
identidad con la que se piden los streams y suele bastar.

> La causa exacta no está identificada. Hay una mitigación puesta —se renueva la identidad de
> visitante al detectar el problema y cada 30 minutos— pero no está demostrado que la elimine
> del todo, así que se prefiere avisar antes que prometer.

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
