use crate::errors::payload::ErrorKind;

pub fn map_errno_name(errno: &str) -> ErrorKind {
    match errno {
        "EACCES" | "EPERM" => ErrorKind::PermissionDenied,
        "ENOENT" => ErrorKind::NotFound,
        "ENOTDIR" => ErrorKind::InvalidPath,
        _ => ErrorKind::InternalError,
    }
}
