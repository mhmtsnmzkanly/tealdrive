use std::fs;
use std::io::{self, Read};

use crate::config::{AllowedRoot, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::path::resolve_existing_path;
use crate::helper::listing::{helper_error, helper_error_from_io, helper_error_from_tealdrive};
use crate::helper::types::{
    HelperCommand, HelperDownloadChunk, HelperDownloadFile, HelperRequest, HelperResponse,
};
use crate::protocol::frame::TransferId;

pub fn download_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    _sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::DownloadFile {
        return HelperResponse::not_implemented(request);
    }
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|root| root.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };
    let resolution = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(resolution) => resolution,
        Err(error) => return helper_error_from_tealdrive(request, error),
    };
    let metadata = match fs::symlink_metadata(&resolution.resolved_path) {
        Ok(metadata) => metadata,
        Err(error) => return helper_error_from_io(request, error),
    };
    if metadata.file_type().is_symlink() {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Symlink download denied.",
            "symlink_denied",
        );
    }
    if metadata.is_dir() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Directories cannot be downloaded in this phase.",
            "directory_download_unsupported",
        );
    }
    if !metadata.is_file() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Target is not a regular file.",
            "invalid_target",
        );
    }
    if metadata.len() > request.limits.max_download_helper_bytes as u64 {
        return helper_error(
            request,
            ErrorKind::FileTooLarge,
            "File is too large for bounded helper download.",
            "file_too_large",
        );
    }

    match read_download_file(request, &resolution.resolved_path, metadata.len()) {
        Ok(download) => HelperResponse::download_file(request, download),
        Err(error) => helper_error_from_io(request, error),
    }
}

fn read_download_file(
    request: &HelperRequest,
    path: &std::path::Path,
    total_bytes: u64,
) -> io::Result<HelperDownloadFile> {
    let chunk_size = request
        .limits
        .max_chunk_size
        .clamp(1, crate::limits::MAX_CHUNK_SIZE);
    let mut file = fs::File::open(path)?;
    let mut chunks = Vec::new();
    let mut offset = 0_u64;
    let mut chunk_index = 0_u32;
    loop {
        let mut buffer = vec![0_u8; chunk_size];
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        buffer.truncate(read);
        chunks.push(HelperDownloadChunk {
            chunk_index,
            offset,
            chunk_bytes: buffer,
        });
        offset += read as u64;
        chunk_index = chunk_index
            .checked_add(1)
            .ok_or_else(|| io::Error::other("too many chunks"))?;
    }

    Ok(HelperDownloadFile {
        transfer_id: TransferId::new(),
        total_bytes,
        chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RelativePath, RootId};
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperResponsePayload;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tealdrive-helper-download-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn root(path: PathBuf) -> AllowedRoot {
        AllowedRoot {
            root_id: RootId::new("home"),
            base_path: path,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
        }
    }

    #[test]
    fn helper_download_reads_file_chunks() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello world").expect("file");
        let mut request = sample_request(HelperCommand::DownloadFile);
        request.relative_path = RelativePath::new("file.txt");
        request.limits.max_chunk_size = 5;
        let response =
            download_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert!(response.success);
        let HelperResponsePayload::DownloadFile(download) = response.payload else {
            panic!("expected download");
        };
        assert_eq!(download.total_bytes, 11);
        assert_eq!(download.chunks.len(), 3);
        assert_eq!(download.chunks[1].offset, 5);
    }

    #[test]
    fn helper_download_does_not_leak_absolute_path() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::DownloadFile);
        request.relative_path = RelativePath::new("file.txt");
        let response = download_file_with_roots(
            &request,
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn helper_download_uid_zero_rejected() {
        let base = temp_root();
        let mut request = sample_request(HelperCommand::DownloadFile);
        request.uid = 0;
        let response =
            download_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_download_large_file_rejected() {
        let base = temp_root();
        fs::write(base.join("big.bin"), vec![1_u8; 16]).expect("file");
        let mut request = sample_request(HelperCommand::DownloadFile);
        request.relative_path = RelativePath::new("big.bin");
        request.limits.max_download_helper_bytes = 8;
        let response =
            download_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::FileTooLarge));
    }

    #[cfg(unix)]
    #[test]
    fn helper_download_symlink_denied() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let mut request = sample_request(HelperCommand::DownloadFile);
        request.relative_path = RelativePath::new("link");
        let response =
            download_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }
}
