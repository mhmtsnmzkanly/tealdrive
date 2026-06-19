use std::fs;
use std::io::Read;

use crate::config::{AllowedRoot, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::path::resolve_existing_path;
use crate::helper::listing::{
    directory_entry_from_metadata, helper_error, helper_error_from_io, helper_error_from_tealdrive,
};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse, HelperTextFileContent};

pub fn read_text_file_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::ReadTextFile {
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
            "Symlink text preview denied.",
            "symlink_denied",
        );
    }
    if metadata.is_dir() {
        return helper_error(
            request,
            ErrorKind::InvalidTarget,
            "Directories cannot be previewed as text.",
            "directory_preview_unsupported",
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
    if metadata.len() > request.limits.max_text_edit_size as u64 {
        return helper_error(
            request,
            ErrorKind::FileTooLarge,
            "File is too large for text preview.",
            "file_too_large",
        );
    }

    let entry_name = resolution
        .resolved_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".")
        .to_owned();
    let entry = directory_entry_from_metadata(entry_name, &metadata, root, sensitive_config);
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    if let Err(error) =
        fs::File::open(&resolution.resolved_path).and_then(|mut file| file.read_to_end(&mut bytes))
    {
        return helper_error_from_io(request, error);
    }
    if bytes.contains(&0) || too_many_control_chars(&bytes) {
        return helper_error(
            request,
            ErrorKind::UnsupportedBinaryFile,
            "Binary files cannot be previewed as text.",
            "unsupported_binary_file",
        );
    }
    let Ok(content) = String::from_utf8(bytes) else {
        return helper_error(
            request,
            ErrorKind::InvalidTextEncoding,
            "File is not valid UTF-8 text.",
            "invalid_text_encoding",
        );
    };
    let line_count = Some(content.lines().count());

    HelperResponse::text_file_content(
        request,
        HelperTextFileContent {
            name: entry.name.clone(),
            content,
            encoding: "utf-8".to_owned(),
            size: metadata.len(),
            modified: entry.modified,
            owner: entry.owner,
            group: entry.group,
            mode: entry.mode,
            permissions: entry.permissions,
            is_sensitive: entry.is_sensitive,
            is_hidden: entry.is_hidden,
            is_read_only: request.policy_context.read_only_root,
            truncated: false,
            line_count,
            language_hint: language_hint(&entry.name),
        },
    )
}

fn too_many_control_chars(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let control_count = bytes
        .iter()
        .filter(|byte| {
            let byte = **byte;
            byte < 0x20 && !matches!(byte, b'\n' | b'\r' | b'\t')
        })
        .count();
    control_count * 100 > bytes.len() * 10
}

fn language_hint(name: &str) -> Option<String> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let language = match ext.as_str() {
        "rs" => "rust",
        "js" => "javascript",
        "ts" => "typescript",
        "json" => "json",
        "html" => "html",
        "css" => "css",
        "md" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" => "shell",
        "py" => "python",
        "php" => "php",
        "txt" => "text",
        _ => return None,
    };
    Some(language.to_owned())
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
        let path =
            std::env::temp_dir().join(format!("tealdrive-helper-text-{}", uuid::Uuid::new_v4()));
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
    fn helper_returns_text_content_for_utf8_file() {
        let base = temp_root();
        fs::write(base.join("main.rs"), b"fn main() {}\n").expect("file");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("main.rs");
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert!(response.success);
        let HelperResponsePayload::TextFileContent(content) = response.payload else {
            panic!("expected text");
        };
        assert_eq!(content.content, "fn main() {}\n");
        assert_eq!(content.language_hint.as_deref(), Some("rust"));
        assert_eq!(content.line_count, Some(1));
    }

    #[test]
    fn helper_text_uid_zero_rejected() {
        let base = temp_root();
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.uid = 0;
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PermissionDenied));
    }

    #[test]
    fn helper_text_response_does_not_leak_absolute_path() {
        let base = temp_root();
        fs::write(base.join("file.txt"), b"hello").expect("file");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("file.txt");
        let response = read_text_file_with_roots(
            &request,
            &[root(base.clone())],
            &SensitivePolicyConfig::default(),
        );
        let serialized = format!("{response:?}");

        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
    }

    #[test]
    fn helper_text_error_does_not_return_file_content() {
        let base = temp_root();
        fs::write(base.join("binary.bin"), b"SECRET\0VALUE").expect("file");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("binary.bin");
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());
        let serialized = format!("{response:?}");

        assert_eq!(response.error_kind, Some(ErrorKind::UnsupportedBinaryFile));
        assert!(!serialized.contains("SECRET"));
    }

    #[test]
    fn helper_text_rejects_invalid_utf8() {
        let base = temp_root();
        fs::write(base.join("bad.txt"), [0xff, 0xfe, b'a']).expect("file");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("bad.txt");
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::InvalidTextEncoding));
    }

    #[test]
    fn helper_text_rejects_large_file() {
        let base = temp_root();
        fs::write(base.join("large.txt"), b"123456789").expect("file");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("large.txt");
        request.limits.max_text_edit_size = 4;
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::FileTooLarge));
    }

    #[cfg(unix)]
    #[test]
    fn helper_text_rejects_symlink() {
        let base = temp_root();
        fs::write(base.join("target"), b"hello").expect("target");
        symlink(base.join("target"), base.join("link")).expect("symlink");
        let mut request = sample_request(HelperCommand::ReadTextFile);
        request.relative_path = RelativePath::new("link");
        let response =
            read_text_file_with_roots(&request, &[root(base)], &SensitivePolicyConfig::default());

        assert_eq!(response.error_kind, Some(ErrorKind::PolicyDenied));
    }
}
