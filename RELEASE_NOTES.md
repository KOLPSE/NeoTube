## Reproducción: bastante más fiable

La causa de los cortes de reproducción de la 0.1.5 (la URL de audio se resolvía bien pero
`libmpv` la abría sin las cabeceras con las que se pidió, y a veces se la rechazaba) ya se
conoce. La URL se abre ahora con el mismo `User-Agent` y el mismo `Referer`/`Origin` con el
que se resolvió.

Medido en una sesión larga de escucha, antes fallaba prácticamente toda canción y ahora
cae, como mucho, una de cada quince o veinte — casi siempre solo la primera del arranque,
antes de que la identidad de visitante haya "calentado". Sigue habiendo un plan B automático
(yt-dlp) para cuando pase, así que una canción rara vez se queda sin sonar. No prometemos que
esto quede cerrado del todo: es la API interna de YouTube, sin documentar, y puede volver a
cambiar.

## Modo aleatorio

Botón de aleatorio en la barra inferior, como el de Spotify: se queda encendido entre
canciones y entre listas hasta que lo apagas, en vez de barajar una lista una sola vez. El
botón "Aleatorio" de una lista enciende el mismo modo.

## Buscador arreglado

La búsqueda solo miraba la pestaña "Todo" de YouTube Music, que recorta cada categoría a un
puñado de resultados de muestra — de ahí que "bad bunny" solo sacara 3 canciones y consultas
de estado de ánimo como "lofi chill music" no sacaran nada. Ahora pide también la búsqueda
filtrada por canciones y sigue su continuación, así que salen 40-60 canciones reales por
consulta.

## Controles de la barra de tareas de Windows

Al pasar el ratón por el icono de NeoTube en la barra de tareas aparece la miniatura con
anterior, pausa/reproducir y siguiente — igual que cualquier reproductor nativo de Windows.
De paso, el icono de la app (ventana, barra de tareas y el panel multimedia de Windows) ya es
el logo real de NeoTube: antes era un marcador de posición de baja resolución y Windows no
sabía identificar la app en el Centro de conexiones rápidas.

## Biblioteca más rápida

"Biblioteca" esperaba a que terminasen sus cuatro peticiones (playlists, álbumes, canciones
favoritas, artistas) antes de pintar nada. Con una lista de favoritas grande, su paginación
—que es forzosamente secuencial— bloqueaba de gratis a las otras tres, que suelen ir mucho
más rápido. Ahora cada categoría se pinta en cuanto llega la suya.

## Modo rendimiento, arreglado de verdad

Encender el modo rendimiento quitaba las carátulas pero no liberaba la RAM que ya tenía
reservada: vaciaba la caché de imágenes de Flutter, pero nunca le pedía a Windows que le
devolviera esas páginas al sistema (`EmptyWorkingSet`). Ahora sí lo hace, igual que al
esconderse en la bandeja.
