# desktop_webview_window (fork NeoTube)

Copia de `desktop_webview_window` 0.3.0 con dos cambios solo en Windows:

1. **No cancelar navegaciones** al notificar `onUrlRequested` (igual que Linux).
   El 0.3.0 de pub cancelaba cada URL y la relanzaba por MethodChannel; con el
   login actual de Google la WebView quedaba en blanco.
2. **User-Agent de Chrome de escritorio** (sin `Edg/`), para que Google no
   sirva página vacía / bloqueo de WebView embebido.
3. `put_IsVisible(TRUE)` tras crear el controlador, por si los bounds iniciales
   dejaban la vista gris.

El resto del paquete es el upstream. No publicar; solo path dependency de NeoTube.
