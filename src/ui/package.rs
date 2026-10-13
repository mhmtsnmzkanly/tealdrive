//! Root-approved UI snapshot. All declared files are loaded together; no mutable-path serving.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub manifest_version: u8,
    pub name: String,
    pub version: String,
    pub sdk_major: u8,
    pub entry: String,
    pub assets: Vec<String>,
    #[serde(default)]
    pub sha256: BTreeMap<String, String>,
}
pub struct Package {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
}
fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn asset_name(value: &str) -> bool {
    if value.len() > 512
        || value.starts_with('/')
        || value
            .split('/')
            .any(|p| p.is_empty() || p.starts_with('.') || !valid_name(p))
    {
        return false;
    }
    if value
        .split('/')
        .next()
        .is_some_and(|part| part.eq_ignore_ascii_case("sdk"))
    {
        return false;
    }
    matches!(
        Path::new(value).extension().and_then(|v| v.to_str()),
        Some(
            "html"
                | "css"
                | "js"
                | "mjs"
                | "json"
                | "svg"
                | "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
        )
    )
}
fn trusted_asset(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    for parent in path.ancestors().skip(1) {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.nlink() != 1
        || metadata.len() > 8 * 1024 * 1024
    {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}
pub fn load(directory: &Path) -> std::io::Result<Package> {
    use std::io::{Error, ErrorKind};
    let invalid = || {
        Error::new(
            ErrorKind::InvalidData,
            "Invalid or incompatible Root UI package",
        )
    };
    let raw = crate::helper::security::trusted_read(&directory.join("manifest.json"), false)?;
    if raw.len() > 65536 {
        return Err(invalid());
    }
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(|_| invalid())?;
    if manifest.manifest_version != 1
        || manifest.sdk_major != 1
        || !valid_name(&manifest.name)
        || !valid_name(&manifest.version)
        || !asset_name(&manifest.entry)
        || manifest.assets.len() > 1024
    {
        return Err(invalid());
    }
    let mut names = BTreeSet::new();
    names.insert(manifest.entry.clone());
    for asset in &manifest.assets {
        if !asset_name(asset) || asset != &manifest.entry && !names.insert(asset.clone()) {
            return Err(invalid());
        }
    }
    if !manifest.sha256.is_empty()
        && manifest.sha256.keys().collect::<BTreeSet<_>>() != names.iter().collect::<BTreeSet<_>>()
    {
        return Err(invalid());
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for name in names {
        let data = trusted_asset(&directory.join(&name))?;
        if std::fs::symlink_metadata(directory.join(&name))?.nlink() != 1 {
            return Err(invalid());
        }
        total = total.checked_add(data.len()).ok_or_else(invalid)?;
        if total > 16 * 1024 * 1024 {
            return Err(invalid());
        }
        if let Some(expected) = manifest.sha256.get(&name) {
            use sha2::{Digest, Sha256};
            if expected.len() != 64 || *expected != format!("{:x}", Sha256::digest(&data)) {
                return Err(invalid());
            }
        }
        files.insert(name, data);
    }
    if Path::new(&manifest.entry)
        .extension()
        .and_then(|v| v.to_str())
        != Some("html")
    {
        return Err(invalid());
    }
    std::str::from_utf8(files.get(&manifest.entry).ok_or_else(invalid)?).map_err(|_| invalid())?;
    Ok(Package { manifest, files })
}
type Fingerprint = (u64, u64, u32, u32, u64, i64, i64);
type Cache = (
    std::path::PathBuf,
    Option<Fingerprint>,
    Option<Arc<Package>>,
);
pub fn current(directory: &Path) -> Option<Arc<Package>> {
    static CACHE: OnceLock<Mutex<Option<Cache>>> = OnceLock::new();
    let metadata = std::fs::symlink_metadata(directory.join("manifest.json")).ok();
    let fingerprint = metadata.map(|m| {
        (
            m.dev(),
            m.ino(),
            m.uid(),
            m.mode(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
        )
    });
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().ok()?;
    if let Some((path, before, result)) = &*cache {
        if path == directory && *before == fingerprint {
            return result.clone();
        }
    }
    let result = load(directory).ok().map(Arc::new);
    *cache = Some((directory.to_path_buf(), fingerprint, result.clone()));
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_names_reject_traversal_secrets_and_sdk_overrides() {
        for name in [
            "../index.html",
            "/index.html",
            "assets//app.js",
            "assets/.secret.json",
            "sdk/v1/tealdrive.js",
            "helper.key",
            "assets\\app.js",
        ] {
            assert!(!asset_name(name), "{name}");
        }
        assert!(asset_name("assets/main.js"));
        assert!(asset_name("index.html"));
    }
}
