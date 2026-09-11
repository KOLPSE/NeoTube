## Fixes

- **Login vacío en Windows.** Google/WebView2 dejaba la ventana de inicio de sesión en blanco: el plugin cancelaba cada navegación, el User-Agent delataba WebView2 y el perfil vivía junto al `.exe`. Fork local del WebView + perfil en `%APPDATA%\neotube\webview2`.
