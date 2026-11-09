#!/bin/bash
# Captura de pantalla de NeoTube dentro de la VM de Arch, lanzado desde el
# servidor.
#
# Uso:  tools/shot-vm.sh <salida.png> [NEOTUBE_START] [segundos]
#
# Dos tropiezos que este script resuelve y conviene no repetir:
#
# 1. El PNG hay que **leerlo de dentro de la VM**. Hacer `scp` desde el
#    servidor copia `/tmp/x.png` *del servidor*, no el de la VM; así se comparan
#    capturas viejas y se llega a la conclusión falsa de que un arreglo no
#    funcionó.
# 2. La clave SSH y el binario viven en la VM, así que el lanzamiento se hace
#    con un script **en fichero** dentro de ella. Una cadena de comillas
#    anidadas con `&` para mandar el proceso al fondo se rompe y produce un PNG
#    de 0 bytes sin error visible.
set -euo pipefail

OUT="${1:-/tmp/neotube.png}"
START="${2:-}"
SECS="${3:-24}"
SERVER="${NEOTUBE_VM_SERVER:-mrp3po@192.168.1.20}"
VM="${NEOTUBE_VM_HOST:-ci@192.168.122.75}"

# Script que se ejecuta **dentro** de la VM.
read -r -d '' REMOTE <<'REMOTE_EOF' || true
#!/bin/bash
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
pgrep Xvfb >/dev/null || (Xvfb :99 -screen 0 1920x1080x24 >/tmp/xvfb.log 2>&1 &)
sleep 3
pkill -f neotube2 >/dev/null 2>&1
sleep 1
cd "$HOME/neotube2.0" || { echo "no existe ~/neotube2.0" >&2; exit 1; }
NEOTUBE_START="$1" ./target/debug/neotube2 > /tmp/app.log 2>&1 &
APP=$!
sleep "$2"
if ! kill -0 "$APP" 2>/dev/null; then
    echo "la aplicación terminó antes de la captura:" >&2
    tail -5 /tmp/app.log >&2
fi
import -window root /tmp/_shot.png
pkill -f neotube2 >/dev/null 2>&1
cat /tmp/_shot.png
REMOTE_EOF

# El script viaja por stdin hasta la VM: una sola conexión, sin comillas
# anidadas. La salida estándar es el PNG y los mensajes van a stderr.
printf '%s' "$REMOTE" | ssh -o BatchMode=yes "$SERVER" \
    "ssh -o StrictHostKeyChecking=no -i ~/.ssh/id_ed25519_nt $VM 'bash -s -- \"$START\" \"$SECS\"'" \
    > "$OUT"

BYTES=$(wc -c < "$OUT")
if [ "$BYTES" -lt 1000 ]; then
    echo "FALLO: la captura salió vacía ($BYTES bytes)" >&2
    exit 1
fi
echo "captura: $OUT ($BYTES bytes)"
