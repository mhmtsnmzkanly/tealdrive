//! Do not allocate a virtual POSIX identity that another account may map through user namespaces.
use super::store::StoreError;
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
unsafe extern "C" {
    fn lckpwdf() -> libc::c_int;
    fn ulckpwdf() -> libc::c_int;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    start: u32,
    end: u32,
    root: bool,
}
fn parse(value: &str) -> Result<Vec<Range>, StoreError> {
    let mut ranges = Vec::new();
    for line in value
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
    {
        if ranges.len() >= 16384 {
            return Err(StoreError::InvalidInput);
        }
        let fields: Vec<_> = line.split(':').collect();
        if fields.len() != 3
            || fields[0].is_empty()
            || fields[0].len() > 256
            || fields[0].chars().any(char::is_whitespace)
        {
            return Err(StoreError::InvalidInput);
        }
        let start: u32 = fields[1].parse().map_err(|_| StoreError::InvalidInput)?;
        let count: u32 = fields[2].parse().map_err(|_| StoreError::InvalidInput)?;
        let end = start
            .checked_add(count.checked_sub(1).ok_or(StoreError::InvalidInput)?)
            .ok_or(StoreError::InvalidInput)?;
        if start == 0 || end == u32::MAX {
            return Err(StoreError::InvalidInput);
        }
        ranges.push(Range {
            start,
            end,
            root: fields[0] == "root" || fields[0] == "0",
        });
    }
    Ok(ranges)
}
fn missing(min: u32, max: u32, ranges: &[Range]) -> Vec<(u32, u32)> {
    let mut occupied: Vec<_> = ranges
        .iter()
        .filter(|range| range.end >= min && range.start <= max)
        .map(|r| (r.start.max(min), r.end.min(max)))
        .collect();
    occupied.sort_unstable();
    let mut next = u64::from(min);
    let mut gaps = Vec::new();
    for (start, end) in occupied {
        if u64::from(start) > next {
            gaps.push((next as u32, start - 1));
        }
        next = next.max(u64::from(end) + 1);
    }
    if next <= u64::from(max) {
        gaps.push((next as u32, max));
    }
    gaps
}
pub(crate) fn free_segments(min: u32, max: u32, blocked: &[(u32, u32)]) -> Vec<(u32, u32)> {
    missing(
        min,
        max,
        &blocked
            .iter()
            .map(|(start, end)| Range {
                start: *start,
                end: *end,
                root: false,
            })
            .collect::<Vec<_>>(),
    )
}
fn reserve(path: &Path, min: u32, max: u32) -> Result<Vec<Range>, StoreError> {
    let (mut content, mode, gid) = match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.uid() != 0
                || metadata.nlink() != 1
                || metadata.mode() & 0o022 != 0
            {
                return Err(StoreError::UntrustedStore);
            }
            (
                String::from_utf8(crate::helper::security::trusted_read(path, false)?)
                    .map_err(|_| StoreError::InvalidInput)?,
                metadata.mode() & 0o777,
                metadata.gid(),
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), 0o644, 0),
        Err(error) => return Err(error.into()),
    };
    let ranges = parse(&content)?;
    let gaps = missing(min, max, &ranges);
    if gaps.is_empty() {
        return Ok(ranges);
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    for (start, end) in gaps {
        content.push_str(&format!(
            "0:{start}:{}\n",
            u64::from(end) - u64::from(start) + 1
        ));
    }
    if content.len() > 1048576 {
        return Err(StoreError::InvalidInput);
    }
    let temporary = path.with_file_name(format!(".tealdrive-subids-{}", uuid::Uuid::new_v4()));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temporary)?;
    let mut file = file;
    use std::os::fd::AsRawFd;
    if unsafe { libc::fchown(file.as_raw_fd(), 0, gid) } != 0 {
        let _ = std::fs::remove_file(&temporary);
        return Err(std::io::Error::last_os_error().into());
    }
    let published = (|| -> std::io::Result<()> {
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        std::fs::File::open(
            path.parent()
                .ok_or_else(|| std::io::Error::other("missing parent"))?,
        )?
        .sync_all()
    })();
    if published.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    published?;
    parse(&content)
}
pub(crate) fn prepare(min: u32, max: u32) -> Result<Vec<(u32, u32)>, StoreError> {
    if unsafe { libc::geteuid() } != 0 || min == 0 || max < min || max == u32::MAX {
        return Err(StoreError::Forbidden);
    }
    let switch = String::from_utf8(crate::helper::security::trusted_read(
        Path::new("/etc/nsswitch.conf"),
        false,
    )?)
    .map_err(|_| StoreError::InvalidInput)?;
    if switch
        .lines()
        .filter_map(|line| line.trim().strip_prefix("subid:"))
        .any(|value| value.trim() != "files")
    {
        return Err(StoreError::Forbidden);
    }
    if unsafe { lckpwdf() } != 0 {
        return Err(StoreError::Conflict);
    }
    struct Unlock;
    impl Drop for Unlock {
        fn drop(&mut self) {
            unsafe {
                ulckpwdf();
            }
        }
    }
    let _lock = Unlock;
    let mut blocked = Vec::new();
    for path in ["/etc/subuid", "/etc/subgid"] {
        blocked.extend(
            reserve(Path::new(path), min, max)?
                .into_iter()
                .filter(|range| !range.root)
                .map(|range| (range.start, range.end)),
        );
    }
    blocked.sort_unstable();
    Ok(blocked)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapped_id_ranges_are_not_posix_isolation() {
        let ranges = parse("alice:100000:65536\nroot:200000:100000\nbob:165536:65536\n").unwrap();
        assert!(ranges
            .iter()
            .any(|range| !range.root && range.start <= 200000 && range.end >= 200000));
        assert_eq!(
            missing(200000, 299999, &parse("alice:165536:65536\n").unwrap()),
            vec![(231072, 299999)]
        );
        assert!(missing(200000, 299999, &ranges).is_empty());
        for invalid in [
            "a:0:1",
            "a:1:0",
            "a:4294967295:2",
            "a:1:2:3",
            "a:1:4294967295",
        ] {
            assert!(parse(invalid).is_err());
        }
    }
}
