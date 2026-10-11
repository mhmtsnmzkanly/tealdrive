use crate::audit::event::AuditEvent;
use crate::audit::writer::to_jsonl;
use crate::config::AuditConfig;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug)]
struct Writer {
    file: File,
    config: AuditConfig,
    path: PathBuf,
    last_prune: Option<Instant>,
}
pub struct PreparedAudit {
    file: Option<File>,
    config: AuditConfig,
    path: PathBuf,
}
#[derive(Debug)]
pub struct JsonlAuditSink {
    writer: Mutex<Writer>,
    write_failures: std::sync::atomic::AtomicU64,
}
fn open_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("audit target must be a regular file"));
    }
    Ok(file)
}
fn archive(path: &Path, index: usize) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{index}"));
    PathBuf::from(name)
}
fn regular_or_missing(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(true),
        Ok(_) => Err(io::Error::other("unsafe audit archive")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
impl Writer {
    fn prune(&mut self, force: bool) -> io::Result<()> {
        if !force
            && self
                .last_prune
                .is_some_and(|time| time.elapsed() < Duration::from_secs(60))
        {
            return Ok(());
        }
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let prefix = format!(
            "{}.",
            self.path
                .file_name()
                .ok_or_else(|| io::Error::other("invalid audit path"))?
                .to_string_lossy()
        );
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(suffix) = name.strip_prefix(&prefix) else {
                continue;
            };
            let Ok(index) = suffix.parse::<usize>() else {
                continue;
            };
            if index == 0 || index.to_string() != suffix {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.is_file() {
                return Err(io::Error::other("unsafe audit archive"));
            }
            let expired = self.config.retention_days.is_some_and(|days| {
                metadata
                    .modified()
                    .ok()
                    .and_then(|t| SystemTime::now().duration_since(t).ok())
                    .is_some_and(|age| age > Duration::from_secs(days as u64 * 86400))
            });
            if index > self.config.max_archives || expired {
                fs::remove_file(entry.path())?;
            }
        }
        self.last_prune = Some(Instant::now());
        Ok(())
    }
    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let temporary = self
            .path
            .with_file_name(format!(".audit-rotation-{}.tmp", uuid::Uuid::new_v4()));
        let new_file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        let result = (|| -> io::Result<()> {
            for index in (1..=self.config.max_archives).rev() {
                let from = archive(&self.path, index);
                let to = archive(&self.path, index + 1);
                if regular_or_missing(&from)? {
                    regular_or_missing(&to)?;
                    fs::rename(from, to)?;
                }
            }
            let first = archive(&self.path, 1);
            regular_or_missing(&first)?;
            fs::rename(&self.path, &first)?;
            if let Err(error) = fs::rename(&temporary, &self.path) {
                let _ = fs::rename(first, &self.path);
                return Err(error);
            }
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        result?;
        self.file = new_file;
        self.prune(true)
    }
}
impl JsonlAuditSink {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_config(&AuditConfig {
            path: Some(path.as_ref().to_path_buf()),
            ..Default::default()
        })
    }
    pub fn open_default() -> io::Result<Self> {
        Self::open_config(&AuditConfig::default())
    }
    pub fn open_config(config: &AuditConfig) -> io::Result<Self> {
        let path = config.path.clone().unwrap_or_else(|| {
            std::env::var("TEALDRIVE_AUDIT_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::temp_dir().join("tealdrive-audit.jsonl"))
        });
        let file = open_file(&path)?;
        Ok(Self {
            write_failures: std::sync::atomic::AtomicU64::new(0),
            writer: Mutex::new(Writer {
                file,
                config: config.clone(),
                path,
                last_prune: None,
            }),
        })
    }
    pub fn path(&self) -> PathBuf {
        self.writer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .path
            .clone()
    }
    /// Open the new target before publishing configuration or persisting it.
    pub fn prepare(&self, config: &AuditConfig) -> io::Result<PreparedAudit> {
        let current = self.path();
        let path = config.path.clone().unwrap_or_else(|| current.clone());
        let file = if path == current {
            None
        } else {
            Some(open_file(&path)?)
        };
        Ok(PreparedAudit {
            file,
            config: config.clone(),
            path,
        })
    }
    pub fn apply(&self, prepared: PreparedAudit) {
        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(file) = prepared.file {
            writer.file = file;
        }
        writer.config = prepared.config;
        writer.path = prepared.path;
        writer.last_prune = None;
    }
    /// Persist completed shutdown records before exiting.
    pub fn sync(&self) -> io::Result<()> {
        self.writer
            .lock()
            .map_err(|_| io::Error::other("audit lock poisoned"))?
            .file
            .sync_all()
    }
    pub fn write_failures(&self) -> u64 {
        self.write_failures
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn emit(&self, event: &AuditEvent) -> io::Result<()> {
        let result = self.emit_inner(event);
        if let Err(error) = &result {
            let count = self
                .write_failures
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            eprintln!(
                "Audit write failed (count={count}, request_id={}, action={:?}): {error}",
                event.request_id, event.action
            );
        }
        result
    }
    fn emit_inner(&self, event: &AuditEvent) -> io::Result<()> {
        let mut line = to_jsonl(event).map_err(io::Error::other)?;
        line.push('\n');
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("audit lock poisoned"))?;
        writer.prune(false)?;
        let size = writer.file.metadata()?.len();
        if size > 0 && size.saturating_add(line.len() as u64) > writer.config.max_file_bytes {
            writer.rotate()?;
        }
        // A single event is never split or dropped even when larger than the cap.
        writer.file.write_all(line.as_bytes())?;
        writer.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::event::{AuditAction, AuditStatus};
    use crate::protocol::frame::RequestId;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!("tdrv-audit-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        p
    }
    fn event(name: &str) -> AuditEvent {
        AuditEvent::new(
            RequestId::new(),
            name,
            1000,
            1000,
            AuditAction::Login,
            AuditStatus::Success,
        )
    }
    #[test]
    fn failed_write_is_counted_and_can_recover() {
        let dir = fixture();
        let path = dir.join("audit.jsonl");
        let sink = JsonlAuditSink::open(&path).unwrap();
        std::os::unix::fs::symlink(&path, dir.join("audit.jsonl.1")).unwrap();
        assert!(sink.emit(&event("alice")).is_err());
        assert_eq!(sink.write_failures(), 1);
        fs::remove_file(dir.join("audit.jsonl.1")).unwrap();
        sink.emit(&event("alice")).unwrap();
        assert_eq!(sink.write_failures(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn concurrent_rotation_keeps_complete_events() {
        let dir = fixture();
        let path = dir.join("audit.jsonl");
        let sink = std::sync::Arc::new(
            JsonlAuditSink::open_config(&AuditConfig {
                path: Some(path),
                max_file_bytes: 1024,
                max_archives: 100,
                retention_days: None,
            })
            .unwrap(),
        );
        let workers: Vec<_> = (0..8)
            .map(|i| {
                let sink = sink.clone();
                std::thread::spawn(move || {
                    for j in 0..5 {
                        sink.emit(&event(&format!("{i}-{j}"))).unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let mut names = std::collections::HashSet::new();
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.metadata().unwrap().len() <= 1024);
            for line in fs::read_to_string(entry.path()).unwrap().lines() {
                let value: AuditEvent = serde_json::from_str(line).unwrap();
                assert!(names.insert(value.username));
            }
        }
        assert_eq!(names.len(), 40);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn retention_bounds_count_and_age_without_deleting_unrelated_files() {
        let dir = fixture();
        let path = dir.join("audit.jsonl");
        let mut config = AuditConfig {
            path: Some(path.clone()),
            max_file_bytes: 1024,
            max_archives: 2,
            retention_days: None,
        };
        let sink = JsonlAuditSink::open_config(&config).unwrap();
        for i in 0..30 {
            sink.emit(&event(&i.to_string())).unwrap();
        }
        assert!(archive(&path, 1).exists());
        assert!(archive(&path, 2).exists());
        assert!(!archive(&path, 3).exists());
        fs::write(dir.join("audit.jsonl.notes"), b"keep").unwrap();
        File::open(archive(&path, 1))
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(2 * 86400)),
            )
            .unwrap();
        config.retention_days = Some(1);
        sink.apply(sink.prepare(&config).unwrap());
        sink.emit(&event("after-expiry")).unwrap();
        assert_eq!(fs::read(dir.join("audit.jsonl.notes")).unwrap(), b"keep");
        for entry in fs::read_dir(&dir).unwrap() {
            assert!(
                entry.unwrap().metadata().unwrap().modified().unwrap()
                    > SystemTime::now() - Duration::from_secs(86400)
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn same_path_reconfigure_survives_rotation_and_new_path_switches() {
        let dir = fixture();
        let path = dir.join("audit.jsonl");
        let mut config = AuditConfig {
            path: Some(path.clone()),
            max_file_bytes: 1024,
            max_archives: 2,
            retention_days: None,
        };
        let sink = JsonlAuditSink::open_config(&config).unwrap();
        let prepared = sink.prepare(&config).unwrap();
        for i in 0..10 {
            sink.emit(&event(&i.to_string())).unwrap();
        }
        sink.apply(prepared);
        sink.emit(&event("current-fd")).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("current-fd"));
        config.path = Some(dir.join("new.jsonl"));
        sink.apply(sink.prepare(&config).unwrap());
        sink.emit(&event("new-location")).unwrap();
        assert!(fs::read_to_string(config.path.unwrap())
            .unwrap()
            .contains("new-location"));
        assert!(!fs::read_to_string(path).unwrap().contains("new-location"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_location_and_failed_persistence_preserve_current_log() {
        let dir = fixture();
        let path = dir.join("audit.jsonl");
        let sink = std::sync::Arc::new(JsonlAuditSink::open(&path).unwrap());
        let state = crate::config::DynamicConfig::new(crate::config::AppConfig::default(), None)
            .with_audit_sink(sink.clone());
        let patch: crate::config::ConfigPatch = serde_json::from_value(
            serde_json::json!({"audit":{"path":dir.join("missing/log.jsonl")}}),
        )
        .unwrap();
        assert!(state.apply_patch(patch).is_err());
        assert_eq!(sink.path(), path);
        assert!(state.load().audit.path.is_none());
        let patch: crate::config::ConfigPatch =
            serde_json::from_value(serde_json::json!({"audit":{"path":dir.join("new.jsonl")}}))
                .unwrap();
        assert!(state.apply_patch_saved(patch, true).is_err());
        assert_eq!(sink.path(), path);
        sink.emit(&event("original-log")).unwrap();
        assert!(fs::read_to_string(path).unwrap().contains("original-log"));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn disabled_retention_survives_toml_persistence() {
        let mut config = crate::config::AppConfig::default();
        let patch: crate::config::ConfigPatch =
            serde_json::from_str(r#"{"audit":{"retention_days":null}}"#).unwrap();
        patch.apply_to(&mut config);
        assert_eq!(config.audit.retention_days, None);
        let encoded = toml::to_string(&config).unwrap();
        assert!(encoded.contains("retention_days = 0"));
        let decoded: crate::config::AppConfig = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.audit.retention_days, None);
    }
    #[test]
    fn audit_target_symlinks_are_rejected() {
        let dir = fixture();
        fs::write(dir.join("target"), b"untouched").unwrap();
        std::os::unix::fs::symlink(dir.join("target"), dir.join("audit.jsonl")).unwrap();
        assert!(JsonlAuditSink::open(dir.join("audit.jsonl")).is_err());
        assert_eq!(fs::read(dir.join("target")).unwrap(), b"untouched");
        fs::remove_dir_all(dir).unwrap();
    }
}
