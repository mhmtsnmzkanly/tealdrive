use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::audit::event::AuditEvent;
use crate::audit::writer::to_jsonl;

#[derive(Debug)]
pub struct JsonlAuditSink {
    file: Mutex<File>,
    path: PathBuf,
}

impl JsonlAuditSink {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn open_default() -> io::Result<Self> {
        let path = std::env::var("TEALDRIVE_AUDIT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("tealdrive-audit.jsonl"));
        Self::open(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn emit(&self, event: &AuditEvent) -> io::Result<()> {
        let line = to_jsonl(event).map_err(io::Error::other)?;
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("audit sink lock poisoned"))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }
}
