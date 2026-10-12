## Features

- **Rediseño a YouTube Music.** Tema, colores y tipografía tomados de la web del cliente oficial: rojo de marca, fondo oscuro y Roboto en toda la app.
- **Sidebar plegable.** La hamburguesa contrae la barra lateral a una guía de iconos y la vuelve a desplegar.
- **Buscador en la barra superior.** La búsqueda sale del sidebar y vive arriba, con los filtros por categoría debajo.
- **Tarjetas 16:9.** Portada, explorar y biblioteca enseñan miniaturas anchas de vídeo, con el play al pasar el ratón.
- **Reproducir todo.** Cada sección de la portada trae su botón para poner la cola entera.
- **Ajustes como página.** Los ajustes dejan de ser un diálogo flotante y pasan a una vista del sidebar.
- **Registro en fichero.** La app guarda actividad y errores en `neotube.log` (rota a 1 MB), listo para adjuntar a un reporte.

## Fixes

- **Overflow al plegar el sidebar.** El logo desbordaba durante la animación y pintaba la franja de error; corregido en raíz.
- **Texto de Ajustes.** Fuera el microcopy que explicaba lo obvio o justificaba opciones.
- **Las canciones ya no se saltan solas.** Pasado el primer minuto, la reproducción depende de una copia completa de la pista que se descarga en paralelo; cuando esa descarga fallaba, la canción saltaba a la siguiente sin decir nada. La descarga se reintenta ahora tres veces —y una más al llegar al corte—, y si aun así no hay forma, se avisa en pantalla en vez de saltar en silencio.
- **Arch: el paquete ya arranca.** `libayatana-appindicator` dejó de arrastrar `libayatana-indicator` y `ayatana-ido`, y el `PKGBUILD` no los declaraba: el binario se quedaba sin dos librerías al abrir. Están añadidas a `depends=` y el pipeline lo comprueba antes de publicar.
