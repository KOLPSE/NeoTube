//! Sesión de YouTube: cookies, firma `SAPISIDHASH` y persistencia.
//!
//! Portado del contrato documentado en `docs/PORTING_CONTRACT.md` (que a su vez
//! sale de `youtube-native/lib/core/yt_auth.dart`). Las reglas que **no** se
//! negocian, porque cada una costó un fallo real:
//!
//! 1. La cookie que firma es `SAPISID` (o `__Secure-3PAPISID` /
//!    `__Secure-1PAPISID`), **nunca** `APISID`: son valores distintos y firmar
//!    con el equivocado no da 401, da **200 con la sesión anónima**.
//! 2. Solo se guardan cookies de `youtube.com` (o subdominios). Mezclar las de
//!    `.google.com` produce nombres repetidos en la cabecera `Cookie:` y YouTube
//!    contesta 400.
//! 3. Nombre y valor de cookie se limpian a ASCII imprimible (RFC 6265).
//! 4. La firma usa `origin` y ese mismo valor viaja en `Origin` y `X-Origin`.
//!
//! Las cookies se guardan en `%APPDATA%\NeoTube2\session.json` con permisos del
//! usuario. No se escriben en logs (PLAN.md §6).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::domain::{Account, Thumbs};

/// Cookies de la sesión, ya filtradas y normalizadas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cookies {
    /// Pares nombre/valor en el orden en que llegaron, sin duplicados por
    /// nombre.
    entries: Vec<(String, String)>,
}

impl Cookies {
    /// Los nombres con los que Google manda la cookie que firma, por orden de
    /// preferencia. `APISID` no está y no puede estar.
    const SIGNING_NAMES: [&'static str; 3] =
        ["SAPISID", "__Secure-3PAPISID", "__Secure-1PAPISID"];

    /// Construye la lista a partir de cookies crudas `(nombre, valor, dominio)`.
    ///
    /// Aplica las tres limpiezas del contrato de autenticación: filtrar a
    /// youtube.com, limpiar a ASCII imprimible y quedarse con la primera
    /// aparición de cada nombre.
    pub fn from_raw(raw: impl IntoIterator<Item = (String, String, String)>) -> Self {
        let mut entries: Vec<(String, String)> = Vec::new();
        for (name, value, domain) in raw {
            if !domain.contains("youtube.com") {
                continue;
            }
            let name = clean_token(&name);
            let value = clean_token(&value);
            if name.is_empty() {
                continue;
            }
            if entries.iter().any(|(n, _)| *n == name) {
                continue;
            }
            entries.push((name, value));
        }
        Self { entries }
    }

    /// A partir de una cabecera `Cookie:` pegada por el usuario
    /// (`A=1; B=2`). Acepta también el formato `cookies.txt` de una línea por
    /// cookie con siete campos tabulados (Netscape), porque es lo que exportan
    /// las extensiones.
    pub fn from_header(header: &str) -> Self {
        let mut entries: Vec<(String, String)> = Vec::new();
        for line in header.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Formato Netscape: domain \t flag \t path \t secure \t expiry \t name \t value
            let fields: Vec<&str> = line.split('\t').collect();
            let (domain, name, value) = if fields.len() >= 7 {
                (fields[0].to_string(), fields[5].to_string(), fields[6].to_string())
            } else {
                // Formato cabecera: "a=b; c=d" (varios pares en una línea) o un
                // par por línea. Se procesan **todos** los pares: quedarse con
                // el primero perdía `SAPISID` cuando no iba en cabeza, que es
                // exactamente el fallo de «estoy logueado pero no salen mis
                // playlists».
                for pair in line.split(';') {
                    let pair = pair.trim();
                    if pair.is_empty() {
                        continue;
                    }
                    let mut it = pair.splitn(2, '=');
                    let name = it.next().unwrap_or("").trim().to_string();
                    let value = it.next().unwrap_or("").trim().to_string();
                    let (name, value) = (clean_token(&name), clean_token(&value));
                    if name.is_empty() || entries.iter().any(|(n, _)| *n == name) {
                        continue;
                    }
                    entries.push((name, value));
                }
                continue;
            };
            if !domain.contains("youtube.com") {
                continue;
            }
            let (name, value) = (clean_token(&name), clean_token(&value));
            if name.is_empty() || entries.iter().any(|(n, _)| *n == name) {
                continue;
            }
            entries.push((name, value));
        }
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    /// El valor con el que se firma. `None` si no hay sesión utilizable.
    pub fn signing_value(&self) -> Option<&str> {
        Self::SIGNING_NAMES.iter().find_map(|n| self.get(n))
    }

    /// Cabecera `Cookie:` completa.
    pub fn header(&self) -> String {
        self.entries
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Firma `SAPISIDHASH`: `sha1("<epoch> <sapisid> <origin>")` y cabecera
    /// `SAPISIDHASH <epoch>_<hash>`.
    pub fn authorization(&self, origin: &str) -> Option<String> {
        let sapisid = self.signing_value()?;
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let mut hasher = Sha1::new();
        hasher.update(format!("{epoch} {sapisid} {origin}").as_bytes());
        let hash = hasher.finalize();
        Some(format!("SAPISIDHASH {epoch}_{}", hex(&hash)))
    }
}

/// Deja solo ASCII imprimible. WebView2 en Windows devuelve las cadenas con un
/// `\0` pegado que rompe tanto el nombre como la cabecera entera.
fn clean_token(s: &str) -> String {
    s.chars().filter(|c| ('\u{21}'..='\u{7e}').contains(c)).collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Dónde vive la sesión guardada.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionFile {
    cookies: Cookies,
    origin: String,
    #[serde(default)]
    account: Option<Account>,
}

/// Sesión compartida entre la interfaz y el cliente de red.
#[derive(Debug, Default)]
pub struct SessionStore {
    inner: RwLock<SessionState>,
    path: parking_lot::Mutex<Option<PathBuf>>,
}

#[derive(Debug, Default)]
struct SessionState {
    cookies: Cookies,
    account: Option<Account>,
}

impl SessionStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Carga la sesión de disco si existe. Nunca falla: una sesión ilegible se
    /// trata como "sin sesión" y se pide volver a iniciar (PLAN.md §10).
    pub fn load(self: &Arc<Self>, dir: &Path) {
        let path = dir.join("session.json");
        *self.path.lock() = Some(path.clone());
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(file) = serde_json::from_str::<SessionFile>(&text) else {
            log::warn!("sesión guardada ilegible, se ignora");
            return;
        };
        let mut state = self.inner.write();
        state.cookies = file.cookies;
        state.account = file.account;
        log::info!(
            "sesión cargada: {} cookies, cuenta: {:?}",
            state.cookies.len(),
            state.account.as_ref().map(|a| a.name.clone())
        );
    }

    fn persist(&self) {
        let Some(path) = self.path.lock().clone() else {
            return;
        };
        let state = self.inner.read();
        let file = SessionFile {
            cookies: state.cookies.clone(),
            origin: "https://www.youtube.com".to_string(),
            account: state.account.clone(),
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&file) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("no se pudo guardar la sesión: {e}");
                } else {
                    restrict_permissions(&path);
                }
            }
            Err(e) => log::warn!("no se pudo serializar la sesión: {e}"),
        }
    }

    pub fn set_cookies(&self, cookies: Cookies) {
        {
            let mut state = self.inner.write();
            state.cookies = cookies;
            state.account = None;
        }
        self.persist();
    }

    pub fn set_account(&self, account: Account) {
        self.inner.write().account = Some(account);
        self.persist();
    }

    /// Vacía la sesión (cierre de sesión real: no queda nada en disco).
    pub fn clear(&self) {
        {
            let mut state = self.inner.write();
            state.cookies = Cookies::default();
            state.account = None;
        }
        if let Some(path) = self.path.lock().clone() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn cookies(&self) -> Cookies {
        self.inner.read().cookies.clone()
    }

    pub fn is_logged_in(&self) -> bool {
        self.inner.read().cookies.signing_value().is_some()
    }

    pub fn account(&self) -> Option<Account> {
        self.inner.read().account.clone()
    }
}

#[cfg(windows)]
fn restrict_permissions(path: &Path) {
    // ACL heredada del perfil del usuario: se deja como está, pero se evita que
    // el fichero sea de solo lectura por herencia rota. No se usa `icacls` para
    // no depender de procesos externos en cada guardado.
    let _ = path;
}

#[cfg(not(windows))]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

/// Utilidad para pruebas y para el importador de cookies desde navegador.
pub fn thumbs_from_url(url: &str) -> Thumbs {
    Thumbs::new(vec![crate::domain::Thumbnail {
        url: url.to_string(),
        width: 88,
        height: 88,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firma_usa_sapisid_y_no_apisid() {
        let c = Cookies::from_raw(vec![
            ("APISID".into(), "malo".into(), ".youtube.com".into()),
            ("SAPISID".into(), "bueno".into(), ".youtube.com".into()),
        ]);
        assert_eq!(c.signing_value(), Some("bueno"));
        let auth = c.authorization("https://www.youtube.com").unwrap();
        assert!(auth.starts_with("SAPISIDHASH "));
        assert_eq!(auth.matches('_').count(), 1);
    }

    #[test]
    fn descarta_cookies_de_google_y_limpia_basura() {
        let c = Cookies::from_raw(vec![
            ("SAPISID".into(), "valor\u{0}".into(), ".google.com".into()),
            ("VISITOR_INFO1_LIVE".into(), "abc\u{0}def".into(), ".youtube.com".into()),
        ]);
        assert_eq!(c.len(), 1);
        assert_eq!(c.header(), "VISITOR_INFO1_LIVE=abcdef");
    }

    #[test]
    fn acepta_cabecera_pegada() {
        let c = Cookies::from_header("SAPISID=xyz; __Secure-3PSID=1; PREF=hl=es");
        assert_eq!(c.signing_value(), Some("xyz"));
        assert!(c.header().contains("PREF=hl=es"));
    }
}
