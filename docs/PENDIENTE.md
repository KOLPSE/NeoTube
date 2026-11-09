# NeoTube 2.0 — Lo que falta

Fecha: 9 de noviembre de 2026. Estado: `v2.1.0` publicado en `main`.

Este documento es la lista de lo que **no** está hecho o no está verificado. Lo que
funciona está en `README.md` y en `PLAN.md`.

---

## 1. Bloqueante para seguir: falta probar con sesión real

Todo lo que depende de la cuenta está **implementado pero sin verificar**. Sin
cookies reales no se puede distinguir «compila» de «funciona», y esa diferencia es
justo la que importa.

| Función | Estado | Qué falta |
|---|---|---|
| Biblioteca | implementada | abrirla con sesión y ver que salen las listas del usuario |
| Historial | implementada | comprobar que respeta el historial desactivado |
| Me gusta / no me gusta | implementada | dar uno y ver que persiste al recargar |
| Suscribirse | implementada | suscribirse y ver el estado en el canal |
| Comentar | implementada, **endpoint sin verificar** | `comment/create_comment` necesita un `createCommentParams` que solo aparece en la respuesta en vivo; si falta, la app dice la verdad en vez de fingir éxito |
| Valorar comentario | implementada, **sin verificar** | `comment/perform_comment_action` |
| Notificaciones | implementada, **sin verificar** | el endpoint `notification` con `NOTIFICATIONS_MENU_REQUEST_TYPE_INBOX` no se ha probado nunca |

**Cómo cerrarlo:** abrir la ventana de login (`cargo run --example login_probe
--features webview-login`), entrar, y recorrer esa tabla apuntando qué falla.

### Cómo se captura la sesión (ya implementado)

Se descartaron dos vías antes de elegir:

- **Leer las cookies de Chrome/Edge del disco: no funciona.** Desde Chrome 127 usan
  `app_bound_encrypted_key` (`APPB`), una clave ligada al binario firmado del
  navegador y al TPM. No se descifra desde fuera, ni con DPAPI. El fichero además
  está bloqueado mientras el navegador corre.
- **Firefox** sí las guarda en claro, pero aquí no hay Firefox instalado y dependía
  del perfil del usuario. Se mantiene como respaldo (`ui/login.rs`).
- **WebView2 propia: sí.** El motor y el perfil son nuestros
  (`%APPDATA%\NeoTube2\webview2`) y `WebView::cookies()` devuelve la lista completa,
  incluidas las `HttpOnly` como `SAPISID`, que son las que firman.

La ventana se abre ya y funciona; falta que alguien entre con una cuenta.

---

## 2. Defecto conocido que debo corregir yo

**Los iconos de la derecha de los controles del reproductor son decorativos.** En
`screens/watch.rs` se dibujan pantalla completa, mini reproductor, ajustes y
subtítulos, pero el clic solo se interpreta en reproducir, siguiente y barra de
progreso. Eso incumple la regla del encargo («ningún botón decorativo presentado
como función terminada»). Hay dos salidas honestas:

1. Conectarlos: pantalla completa (egui lo soporta), velocidad y subtítulos si el
   motor los expone.
2. Quitarlos hasta que funcionen.

Es lo primero que arreglaría del listado.

---

## 3. Funciones no implementadas

### Reproducción
- **Directos**: `resolve_stream` solo mira `formats` y `adaptiveFormats`. Un directo
  llega como `hlsManifestUrl` y acaba en «no ofreció ninguna pista reproducible».
- **Subtítulos**: no se piden ni se pintan.
- **Cambio de calidad en caliente**: la calidad se elige al resolver; no se puede
  cambiar sin reabrir.
- **Velocidad de reproducción** y **selección de pista de audio** (audios doblados).
- **Modo cine**.
- **Reanudar donde se dejó** un vídeo largo.

### Cuenta y contenido
- **Ver más tarde** (hoy el menú avisa de que requiere sesión).
- **Editar listas**: crear, añadir, reordenar, borrar.
- **Historial de YouTube** (el que ve la web); solo hay historial local.
- **Subidas y Studio**: se abre la web a propósito. No hay API documentada para
  clientes de terceros.
- **Membresías, compras, moderación**: ídem.

### Escritorio
- **SMTC (Windows)** y **MPRIS (Linux)**: los controles multimedia del sistema no
  están conectados.
- **Bandeja del sistema**, **atajos globales**, **notificaciones nativas del SO**.
- **Actualizador** e **instaladores** (MSI/PKG/deb). Hoy se compila y se ejecuta.
- **Abrir enlaces `neotube://`** desde el navegador (existe `NEOTUBE_START` como
  base, sin registrar el esquema).

### Música
- **Radio y mezclas** (`RD…`): la cola se pide por `next`, pero no hay botón que
  arranque una radio desde una canción.
- **Letras**.

---

## 4. Fidelidad visual pendiente

Comparado con las capturas de `plan-y-referencias/imagenes/`:

- **Título de tarjeta a dos líneas**: `grid_card` corta a una en algunos casos. La
  referencia usa dos.
- **Canal**: faltan en el modelo `description`, `links` y `verified`, así que la
  línea «…más», los enlaces y la marca de verificado **no se pintan**. Es
  deliberado: no se inventan. Requiere ampliar `ChannelInfo` y parsearlos.
- **Contador de vídeos y «N vídeos» de las listas**: no vienen en los datos actuales.
- **Comentarios**: el recuento de la cabecera usa el número de comentarios
  cargados, no el total real.
- **Detalle de densidad** en ventanas muy anchas (>2000 px): no se ha comparado.

---

## 5. Rendimiento: sin medir

No hay **ninguna cifra** de RAM, CPU, GPU ni fotogramas perdidos de esta aplicación.
`PLAN.md` lo dice así en vez de inventar presupuestos. Falta:

- Arranque en frío hasta interfaz usable.
- Reposo con la app abierta.
- Música sonando (debería ser el caso más barato: no se decodifica vídeo).
- Vídeo 1080p y 4K (CPU, GPU, memoria gráfica, fotogramas perdidos).
- Navegación de una hora (crecimiento de memoria y tamaño de caché).
- Shorts tras 50 cambios (liberación de recursos).

El presupuesto de miniaturas (48 MB) y la caché con TTL están puestos, pero **no
medidos**.

---

## 6. Riesgos abiertos

| Riesgo | Detalle |
|---|---|
| Versiones de cliente fijadas a mano | `WEB_CLIENT_VERSION` y `REMIX_CLIENT_VERSION` caducan cada pocos días. Lo correcto es leer `INNERTUBE_CLIENT_VERSION` de la página. Si YouTube empieza a contestar 400 a todo, es lo primero que hay que mirar. |
| `ANDROID_VR` como respaldo | Solo sirve una ventana del fichero (1 MiB en audio, ~30 % en vídeo). El respaldo puede devolver URLs que se cortan a mitad. |
| `VISIONOS` sin garantía | Es el cliente que funciona hoy y el que usa yt-dlp. No hay contrato de estabilidad. |
| Parseo por renderers | Se buscan por nombre en vez de por ruta fija, pero un renombrado sigue rompiendo pantallas. `docs/API_PROBE.md` tiene las rutas verificadas. |
| Filtros de búsqueda | Los `params` (`EgIQAQ…`) están copiados y pueden estar obsoletos. |

---

## 7. Infraestructura

- Las dos VMs del servidor (`vm-arch` en 192.168.122.75, `vm-windows` en .17) **están
  arrancadas**. Se arrancan con un contenedor sobre el socket de libvirt porque
  `sudo` de `mrp3po` solo cubre `systemctl/certbot/mysql/ufw`.
- A `vm-windows` se le quitó el ISO de arranque del XML (apuntaba a un fichero que
  ya no existe y no dejaba arrancar del disco).
- El ciclo compilar + capturar dentro de `vm-arch` está en `tools/shot-vm.sh`.
  **Ojo**: hay que leer el PNG de *dentro* de la VM; un `scp` desde el servidor
  copia el `/tmp` del servidor y hace comparar capturas viejas.
- El runner de GitLab con tag `windows` ya no sirve para este proyecto: el CI es
  Rust y corre en el contenedor de `rust:latest`.

---

## 8. Orden que yo seguiría

1. **Entrar con una cuenta** y recorrer la tabla del punto 1. Sin esto, el resto es
   teoría.
2. **Quitar o conectar los iconos decorativos** del reproductor (punto 2).
3. **Medir** arranque, reposo y vídeo 4K (punto 5). Son cuatro números y cambian la
   conversación sobre «consumo bajo».
4. **Directos** (HLS), que hoy fallan con un mensaje que no explica nada.
5. **Ampliar `ChannelInfo`** con descripción, enlaces y verificado, y pintarlos.
6. **Subtítulos y velocidad**, que es lo que más se echa de menos al ver vídeos.
7. **Escritorio**: SMTC/MPRIS y bandeja, que es lo que hace que un cliente de
   escritorio se sienta nativo.
8. Instaladores y actualizador, cuando lo anterior esté estable.
