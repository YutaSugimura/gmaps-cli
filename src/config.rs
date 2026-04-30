use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationProvider {
    /// Use `default_place` as the center point.
    #[default]
    Default,
    /// Use GPS (CoreLocation); fall back to `default_place` on failure.
    Gps,
    /// Always require an explicit --location flag.
    Manual,
}

impl std::fmt::Display for LocationProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Default => "default",
            Self::Gps => "gps",
            Self::Manual => "manual",
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,

    /// Name of a place registered in places.yaml (None = unset).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub default_place: Option<String>,

    #[serde(default = "default_language")]
    pub language: String,

    #[serde(default = "default_region")]
    pub region: String,

    #[serde(default)]
    pub location_provider: LocationProvider,
}

// Manual Debug impl: derive(Debug) would dump the API key in plaintext as soon
// as anyone formatted a Config with `{:?}`, which is a classic accidental-leak
// vector in CI logs and bug reports. Always show the masked form.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("api_key", &mask_api_key(&self.api_key))
            .field("default_place", &self.default_place)
            .field("language", &self.language)
            .field("region", &self.region)
            .field("location_provider", &self.location_provider)
            .finish()
    }
}

fn default_language() -> String {
    "en".to_string()
}

fn default_region() -> String {
    "US".to_string()
}

pub fn config_dir() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("Could not locate home directory")?;
    Ok(base.home_dir().join(".config").join("gmaps"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.yaml"))
}

/// Load the config file. Returns None if missing, empty, or `api_key` is unset.
pub fn load() -> Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    let config: Config = serde_yaml_ng::from_str(&text)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    if config.api_key.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(config))
}

/// Persist the config file. Directory is set to 0700 and the file to 0600.
pub fn save(config: &Config) -> Result<()> {
    let dir = config_dir()?;
    ensure_private_dir(&dir)?;

    let path = config_path()?;
    let yaml = serde_yaml_ng::to_string(config).context("Failed to serialize config to YAML")?;
    write_private_file(&path, yaml.as_bytes())?;
    Ok(())
}

/// Ensure a directory exists with mode 0700.
///
/// Creates the leaf directory via `mkdir(2)` with mode 0700 set atomically,
/// closing the window where `create_dir_all` + follow-up `chmod` would have
/// left it at the umask-derived mode (typically 0755). Parent directories
/// (e.g. `~/.config`) are created with the platform default so we do not
/// override permissions other tools rely on. When the leaf already exists,
/// fall back to `chmod` to normalize potentially looser permissions.
pub(crate) fn ensure_private_dir(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("Failed to chmod {}", dir.display()))?;
            Ok(())
        }
        Err(e) => {
            Err(anyhow::Error::from(e).context(format!("Failed to create {}", dir.display())))
        }
    }
}

/// Atomically write `contents` to `path` with mode 0600.
///
/// Writes to a sibling temp file created with `O_CREAT|O_EXCL|mode(0o600)`, then
/// renames over the destination. This avoids the brief window where `fs::write`
/// would otherwise leave the file readable under the process's umask before a
/// follow-up `set_permissions(0o600)` call.
pub(crate) fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .with_context(|| format!("{} has no file name", path.display()))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(file_name);
    tmp_name.push(format!(".tmp.{}", std::process::id()));
    let tmp_path = dir.join(tmp_name);

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)
        .with_context(|| format!("Failed to create {}", tmp_path.display()))?;
    let write_result = f.write_all(contents).and_then(|()| f.sync_all());
    drop(f);

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(
            anyhow::Error::from(e).context(format!("Failed to write {}", tmp_path.display()))
        );
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow::Error::from(e).context(format!("Failed to replace {}", path.display())));
    }
    Ok(())
}

/// Convert an API key to a masked display string (e.g., AIza****....****abcd).
///
/// Operates on Unicode scalar values rather than bytes, so a key that
/// somehow contains multi-byte characters does not panic on a non-char
/// boundary slice.
pub fn mask_api_key(key: &str) -> String {
    let char_count = key.chars().count();
    if char_count <= 8 {
        return "****".to_string();
    }
    let head: String = key.chars().take(4).collect();
    let tail: String = key.chars().skip(char_count - 4).collect();
    let middle = "*".repeat(char_count - 8);
    format!("{head}{middle}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_short_key() {
        assert_eq!(mask_api_key(""), "****");
        assert_eq!(mask_api_key("abc"), "****");
        assert_eq!(mask_api_key("12345678"), "****");
    }

    #[test]
    fn mask_long_key() {
        // "AIzaSyABC123xyz" is 15 chars → head 4 + middle 7 + tail 4
        assert_eq!(mask_api_key("AIzaSyABC123xyz"), "AIza*******3xyz");
    }

    #[test]
    fn debug_format_masks_api_key() {
        let cfg = Config {
            api_key: "AIzaSyABC123xyzSECRET".to_string(),
            default_place: Some("home".to_string()),
            language: "en".to_string(),
            region: "US".to_string(),
            location_provider: LocationProvider::Default,
        };
        let dbg = format!("{cfg:?}");
        assert!(
            !dbg.contains("AIzaSyABC123xyzSECRET"),
            "raw key leaked: {dbg}"
        );
        assert!(!dbg.contains("SECRET"), "tail of key leaked: {dbg}");
        assert!(dbg.contains("AIza"), "expected masked prefix: {dbg}");
        assert!(dbg.contains("CRET"), "expected masked suffix: {dbg}");
        assert!(dbg.contains("default_place"));
    }

    #[test]
    fn mask_multibyte_key_does_not_panic() {
        // Each "あ" is 3 bytes; byte-indexed slicing at [..4] would panic.
        // 9 chars → head 4 + middle 1 + tail 4
        assert_eq!(mask_api_key("あいうえおかきくけ"), "あいうえ*かきくけ");
        // Mixed ASCII + multibyte, exactly 8 chars → fully masked.
        assert_eq!(mask_api_key("AIza日本語x"), "****");
    }

    #[test]
    fn ensure_private_dir_creates_0700_and_normalizes_existing() {
        let parent = std::env::temp_dir().join(format!(
            "gmaps-cli-test-dir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&parent).unwrap();
        let dir = parent.join("leaf");

        // Newly created leaf must be exactly 0700.
        ensure_private_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "expected 0700 on create, got {mode:o}");

        // Existing-but-loose leaf must be tightened.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        ensure_private_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "expected 0700 after normalize, got {mode:o}");

        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn write_private_file_creates_0600_and_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "gmaps-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        ensure_private_dir(&dir).unwrap();
        let path = dir.join("secret.yaml");

        // Initial write.
        write_private_file(&path, b"first").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
        assert_eq!(fs::read(&path).unwrap(), b"first");

        // Overwrite via rename: previous inode replaced atomically.
        write_private_file(&path, b"second").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read(&path).unwrap(), b"second");

        // Tmp file must not linger.
        let lingering: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".secret.yaml.tmp.")
            })
            .collect();
        assert!(lingering.is_empty(), "stale temp file left behind");

        fs::remove_dir_all(&dir).ok();
    }
}
