use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::{ErrorKind, TealDriveError};
use crate::fs::filename::SafeFileName;
use crate::fs::path::{
    resolve_existing_path, resolve_new_target, resolve_root, validate_relative_path,
};
use crate::helper::listing::{helper_error, helper_error_from_io, helper_error_from_tealdrive};
use crate::helper::types::{HelperCommand, HelperRequest, HelperResponse};
use crate::policy::decision::PolicyDecision;
use crate::policy::sensitive::detect_sensitive;
use crate::policy::webroot::check_webroot_archive_extract;

fn archive_hidden_allowed(request: &HelperRequest) -> bool {
    crate::helper::policy::current()
        .and_then(|p| {
            p.config
                .roots
                .iter()
                .find(|r| r.root_id == request.root_id)
                .map(|r| r.hidden_files_allowed)
        })
        .unwrap_or(false)
}
fn add_dir_to_tar<W: Write>(
    tar: &mut tar::Builder<W>,
    directory: &Path,
    prefix: &str,
    root: &AllowedRoot,
    sensitive: &SensitivePolicyConfig,
    allow_hidden: bool,
) -> std::io::Result<()> {
    if prefix.split('/').count() > 128 {
        return Err(std::io::Error::other("archive depth exceeded"));
    }
    tar.append_dir(prefix, directory)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let metadata = fs::symlink_metadata(&path)?;
        let relative = RelativePath::new(
            path.strip_prefix(&root.base_path)
                .map_err(std::io::Error::other)?
                .to_string_lossy(),
        );
        if metadata.is_symlink()
            || (!allow_hidden && name.starts_with('.'))
            || detect_sensitive(&relative, sensitive).is_sensitive
        {
            continue;
        }
        let archive_path = format!("{prefix}/{name}");
        if metadata.is_dir() {
            add_dir_to_tar(tar, &path, &archive_path, root, sensitive, allow_hidden)?;
        } else if metadata.is_file() {
            let mut file = crate::helper::sandbox::open(root, &relative, libc::O_RDONLY, 0)?;
            tar.append_file(archive_path, &mut file)?;
        }
    }
    Ok(())
}

pub const MAX_ARCHIVE_EXTRACT_BYTES: u64 = 500 * 1024 * 1024; // 500 MB maximum decompressed size

pub fn compress_zip_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::CompressZip {
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

    if root.read_only {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Root is read-only.",
            "read_only_root",
        );
    }

    let Some(target_rel) = &request.target_relative_path else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target relative path required.",
            "missing_target_relative_path",
        );
    };

    let validated_target = match validate_relative_path(target_rel) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if validated_target.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Cannot write archive to root directory.",
            "invalid_target",
        );
    }

    if !validated_target.0.to_ascii_lowercase().ends_with(".zip") {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target must end with .zip",
            "invalid_extension",
        );
    }

    if detect_sensitive(&validated_target, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Target path is sensitive.",
            "sensitive_path",
        );
    }

    let Some(sources) = &request.archive_source_paths else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Source paths required.",
            "missing_source_paths",
        );
    };

    if sources.is_empty() {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "At least one source path required.",
            "empty_source_paths",
        );
    }

    let (parent_str, filename_str) = match validated_target.0.rsplit_once('/') {
        Some((p, f)) => (p, f),
        None => ("", validated_target.0.as_str()),
    };

    let filename = match SafeFileName::parse(filename_str) {
        Ok(name) => name,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let parent_rel = RelativePath::new(parent_str);
    let target = match resolve_new_target(root, &parent_rel, &filename.0) {
        Ok(target) => target,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let mut validated_sources = Vec::with_capacity(sources.len());
    for src in sources {
        let val_src = match validate_relative_path(src) {
            Ok(path) => path,
            Err(err) => return helper_error_from_tealdrive(request, err),
        };
        if val_src.0.is_empty() {
            return helper_error(
                request,
                ErrorKind::InvalidPath,
                "Cannot archive root directory.",
                "invalid_source",
            );
        }
        if detect_sensitive(&val_src, sensitive_config).is_sensitive {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Source path is sensitive.",
                "sensitive_path",
            );
        }
        let resolved = match resolve_existing_path(root, &val_src, false) {
            Ok(res) => res,
            Err(err) => return helper_error_from_tealdrive(request, err),
        };
        validated_sources.push((val_src, resolved));
    }

    let out_file = match File::create(&target.resolved_path) {
        Ok(file) => file,
        Err(err) => return helper_error_from_io(request, err),
    };

    let mut zip = zip::ZipWriter::new(crate::helper::quota::BudgetWriter::new(out_file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (rel_path, res_path) in validated_sources {
        let entry_name = match rel_path.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => rel_path.0.as_str(),
        };

        if res_path.resolved_path.is_dir() {
            if let Err(err) = add_dir_to_zip(
                &mut zip,
                &res_path.resolved_path,
                entry_name,
                options,
                sensitive_config,
                &target.root.canonical_base,
                archive_hidden_allowed(request),
            ) {
                let _ = fs::remove_file(&target.resolved_path);
                return helper_error_from_tealdrive(request, err);
            }
        } else {
            if zip.start_file(entry_name, options).is_err() {
                let _ = fs::remove_file(&target.resolved_path);
                return helper_error(
                    request,
                    ErrorKind::InternalError,
                    "Failed to write zip entry.",
                    "zip_error",
                );
            }
            let mut f = match File::open(&res_path.resolved_path) {
                Ok(f) => f,
                Err(err) => {
                    let _ = fs::remove_file(&target.resolved_path);
                    return helper_error_from_io(request, err);
                }
            };
            if let Err(err) = std::io::copy(&mut f, &mut zip) {
                let _ = fs::remove_file(&target.resolved_path);
                return helper_error_from_io(request, err);
            }
        }
    }

    if zip.finish().is_err() {
        let _ = fs::remove_file(&target.resolved_path);
        return helper_error(
            request,
            ErrorKind::InternalError,
            "Failed to finalize zip archive.",
            "zip_finish_error",
        );
    }

    HelperResponse::operation_done(request, "Archive created.")
}

fn add_dir_to_zip<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir_path: &Path,
    prefix_in_zip: &str,
    options: zip::write::SimpleFileOptions,
    sensitive_config: &SensitivePolicyConfig,
    canonical_base: &Path,
    allow_hidden: bool,
) -> Result<(), TealDriveError> {
    if prefix_in_zip.split('/').count() > 128 {
        return Err(TealDriveError::Validation);
    }
    zip.add_directory(prefix_in_zip, options)
        .map_err(|_| TealDriveError::Internal)?;

    let entries = fs::read_dir(dir_path).map_err(|_| TealDriveError::InvalidPath)?;
    for entry in entries {
        let entry = entry.map_err(|_| TealDriveError::InvalidPath)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let in_zip_name = format!("{prefix_in_zip}/{name}");
        let metadata = fs::symlink_metadata(&path).map_err(|_| TealDriveError::Internal)?;
        if metadata.is_symlink() || (!allow_hidden && name.starts_with('.')) {
            continue;
        }

        // Sensitive check
        if let Ok(rel) = path.strip_prefix(canonical_base) {
            let rel_str = rel.to_string_lossy();
            if detect_sensitive(&RelativePath::new(rel_str.as_ref()), sensitive_config).is_sensitive
            {
                continue;
            }
        }

        if metadata.is_dir() {
            add_dir_to_zip(
                zip,
                &path,
                &in_zip_name,
                options,
                sensitive_config,
                canonical_base,
                allow_hidden,
            )?;
        } else if metadata.is_file() {
            zip.start_file(&in_zip_name, options)
                .map_err(|_| TealDriveError::Internal)?;
            let root = AllowedRoot {
                base_path: canonical_base.to_path_buf(),
                ..Default::default()
            };
            let relative = RelativePath::new(
                path.strip_prefix(canonical_base)
                    .map_err(|_| TealDriveError::InvalidPath)?
                    .to_string_lossy(),
            );
            let mut f = crate::helper::sandbox::open(&root, &relative, libc::O_RDONLY, 0)
                .map_err(|_| TealDriveError::Internal)?;
            std::io::copy(&mut f, zip).map_err(|_| TealDriveError::Internal)?;
        }
    }

    Ok(())
}

pub fn compress_targz_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::CompressTarGz {
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

    if root.read_only {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Root is read-only.",
            "read_only_root",
        );
    }

    let Some(target_rel) = &request.target_relative_path else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target relative path required.",
            "missing_target_relative_path",
        );
    };

    let validated_target = match validate_relative_path(target_rel) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if validated_target.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Cannot write archive to root directory.",
            "invalid_target",
        );
    }

    let lower_target = validated_target.0.to_ascii_lowercase();
    if !lower_target.ends_with(".tar.gz") && !lower_target.ends_with(".tgz") {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target must end with .tar.gz or .tgz",
            "invalid_extension",
        );
    }

    if detect_sensitive(&validated_target, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Target path is sensitive.",
            "sensitive_path",
        );
    }

    let Some(sources) = &request.archive_source_paths else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Source paths required.",
            "missing_source_paths",
        );
    };

    if sources.is_empty() {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "At least one source path required.",
            "empty_source_paths",
        );
    }

    let (parent_str, filename_str) = match validated_target.0.rsplit_once('/') {
        Some((p, f)) => (p, f),
        None => ("", validated_target.0.as_str()),
    };

    let filename = match SafeFileName::parse(filename_str) {
        Ok(name) => name,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let parent_rel = RelativePath::new(parent_str);
    let target = match resolve_new_target(root, &parent_rel, &filename.0) {
        Ok(target) => target,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let mut validated_sources = Vec::with_capacity(sources.len());
    for src in sources {
        let val_src = match validate_relative_path(src) {
            Ok(path) => path,
            Err(err) => return helper_error_from_tealdrive(request, err),
        };
        if val_src.0.is_empty() {
            return helper_error(
                request,
                ErrorKind::InvalidPath,
                "Cannot archive root directory.",
                "invalid_source",
            );
        }
        if detect_sensitive(&val_src, sensitive_config).is_sensitive {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Source path is sensitive.",
                "sensitive_path",
            );
        }
        let resolved = match resolve_existing_path(root, &val_src, false) {
            Ok(res) => res,
            Err(err) => return helper_error_from_tealdrive(request, err),
        };
        validated_sources.push((val_src, resolved));
    }

    let out_file = match File::create(&target.resolved_path) {
        Ok(file) => file,
        Err(err) => return helper_error_from_io(request, err),
    };

    let enc = flate2::write::GzEncoder::new(
        crate::helper::quota::BudgetWriter::new(out_file),
        flate2::Compression::default(),
    );
    let mut tar = tar::Builder::new(enc);

    for (rel_path, res_path) in validated_sources {
        let entry_name = match rel_path.0.rsplit_once('/') {
            Some((_, name)) => name,
            None => rel_path.0.as_str(),
        };

        if res_path.resolved_path.is_dir() {
            if let Err(err) = add_dir_to_tar(
                &mut tar,
                &res_path.resolved_path,
                entry_name,
                root,
                sensitive_config,
                archive_hidden_allowed(request),
            ) {
                let _ = fs::remove_file(&target.resolved_path);
                return helper_error_from_io(request, err);
            }
        } else {
            let mut f = match File::open(&res_path.resolved_path) {
                Ok(f) => f,
                Err(err) => {
                    let _ = fs::remove_file(&target.resolved_path);
                    return helper_error_from_io(request, err);
                }
            };
            if let Err(err) = tar.append_file(entry_name, &mut f) {
                let _ = fs::remove_file(&target.resolved_path);
                return helper_error_from_io(request, err);
            }
        }
    }

    let finish = tar
        .into_inner()
        .and_then(|encoder| encoder.finish())
        .and_then(|mut output| output.flush());
    if let Err(err) = finish {
        let _ = fs::remove_file(&target.resolved_path);
        return helper_error_from_io(request, err);
    }

    HelperResponse::operation_done(request, "Archive created.")
}

pub fn extract_archive_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.command != HelperCommand::ExtractArchive {
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

    if root.read_only {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Root is read-only.",
            "read_only_root",
        );
    }

    match check_webroot_archive_extract(root.is_web_root) {
        PolicyDecision::Allow | PolicyDecision::WarnAllowed(_) => {}
        PolicyDecision::Deny(_) => {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive extraction denied in webroot.",
                "webroot_archive_denied",
            );
        }
    }

    let validated_archive = match validate_relative_path(&request.relative_path) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if validated_archive.0.is_empty() {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Cannot extract root directory.",
            "invalid_source",
        );
    }

    if detect_sensitive(&validated_archive, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Archive path is sensitive.",
            "sensitive_path",
        );
    }

    let archive_res = match resolve_existing_path(root, &validated_archive, false) {
        Ok(res) => res,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let Some(target_rel) = &request.target_relative_path else {
        return helper_error(
            request,
            ErrorKind::ValidationError,
            "Target relative path required.",
            "missing_target_relative_path",
        );
    };

    let validated_target = match validate_relative_path(target_rel) {
        Ok(path) => path,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    if detect_sensitive(&validated_target, sensitive_config).is_sensitive {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Target path is sensitive.",
            "sensitive_path",
        );
    }

    let resolved_root = match resolve_root(root) {
        Ok(r) => r,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let dest_dir: PathBuf = if validated_target.0.is_empty() {
        resolved_root.canonical_base.clone()
    } else {
        resolved_root.canonical_base.join(&validated_target.0)
    };

    if let Err(err) = fs::create_dir_all(&dest_dir) {
        return helper_error_from_io(request, err);
    }

    let canonical_dest = match dest_dir.canonicalize() {
        Ok(c) => c,
        Err(err) => return helper_error_from_io(request, err),
    };

    if !canonical_dest.starts_with(&resolved_root.canonical_base) {
        return helper_error(
            request,
            ErrorKind::PolicyDenied,
            "Destination directory escapes root.",
            "path_escapes_root",
        );
    }

    let lower_name = validated_archive.0.to_ascii_lowercase();
    if lower_name.ends_with(".zip") {
        extract_zip(
            &archive_res.resolved_path,
            &canonical_dest,
            &resolved_root.canonical_base,
            sensitive_config,
            request,
        )
    } else if lower_name.ends_with(".tar.gz")
        || lower_name.ends_with(".tgz")
        || lower_name.ends_with(".tar")
    {
        extract_tar(
            &archive_res.resolved_path,
            &canonical_dest,
            &resolved_root.canonical_base,
            sensitive_config,
            &lower_name,
            request,
        )
    } else {
        helper_error(
            request,
            ErrorKind::ValidationError,
            "Unsupported archive extension. Supported: .zip, .tar.gz, .tgz, .tar",
            "unsupported_archive_format",
        )
    }
}

fn extract_zip(
    archive_path: &Path,
    dest_dir: &Path,
    canonical_base: &Path,
    sensitive_config: &SensitivePolicyConfig,
    request: &HelperRequest,
) -> HelperResponse {
    let file = match File::open(archive_path) {
        Ok(f) => f,
        Err(err) => return helper_error_from_io(request, err),
    };

    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => {
            return helper_error(
                request,
                ErrorKind::ValidationError,
                "Corrupted archive file.",
                "corrupted_archive",
            )
        }
    };

    let mut total_extracted: u64 = 0;

    for i in 0..zip.len() {
        let mut file = match zip.by_index(i) {
            Ok(f) => f,
            Err(_) => {
                return helper_error(
                    request,
                    ErrorKind::ValidationError,
                    "Failed to read zip entry.",
                    "zip_read_error",
                )
            }
        };

        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            if (mode & 0o170000) == 0o120000 {
                // Reject symlinks inside archives to prevent link traversal attacks
                continue;
            }
        }

        let Some(enclosed) = file.enclosed_name() else {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry path attempts directory traversal.",
                "zip_slip_attempt",
            );
        };

        if !archive_hidden_allowed(request)
            && crate::fs::path::is_hidden_relative(&RelativePath::new(enclosed.to_string_lossy()))
        {
            return HelperResponse::permission_denied(request, "Hidden archive entry denied.");
        }
        let outpath = dest_dir.join(enclosed);
        if !outpath.starts_with(dest_dir) {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry path attempts directory traversal.",
                "zip_slip_attempt",
            );
        }

        if let Ok(rel) = outpath.strip_prefix(canonical_base) {
            let rel_str = rel.to_string_lossy();
            if detect_sensitive(&RelativePath::new(rel_str.as_ref()), sensitive_config).is_sensitive
            {
                return helper_error(
                    request,
                    ErrorKind::PolicyDenied,
                    "Archive entry extracts to sensitive path.",
                    "sensitive_path",
                );
            }
        }

        let mut curr = outpath.parent();
        let mut parent_is_symlink = false;
        while let Some(p) = curr {
            if p == dest_dir {
                break;
            }
            if let Ok(meta) = fs::symlink_metadata(p) {
                if meta.file_type().is_symlink() {
                    parent_is_symlink = true;
                    break;
                }
            }
            curr = p.parent();
        }
        if parent_is_symlink {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry parent directory is a symlink.",
                "symlink_denied",
            );
        }

        if let Ok(meta) = fs::symlink_metadata(&outpath) {
            if meta.file_type().is_symlink() {
                return helper_error(
                    request,
                    ErrorKind::PolicyDenied,
                    "Archive entry target is a symlink.",
                    "symlink_denied",
                );
            }
        }

        if file.is_dir() {
            if let Err(err) = fs::create_dir_all(&outpath) {
                return helper_error_from_io(request, err);
            }
        } else {
            if let Some(parent) = outpath.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    return helper_error_from_io(request, err);
                }
            }
            let mut outfile = match File::create(&outpath) {
                Ok(f) => f,
                Err(err) => return helper_error_from_io(request, err),
            };

            let remaining_budget = MAX_ARCHIVE_EXTRACT_BYTES
                .min(crate::helper::quota::budget().unwrap_or(u64::MAX))
                .saturating_sub(total_extracted);
            let mut limited = (&mut file).take(remaining_budget + 1);
            let written = match std::io::copy(
                &mut limited,
                &mut crate::helper::quota::BudgetWriter::new(&mut outfile),
            ) {
                Ok(n) => n,
                Err(err) => return helper_error_from_io(request, err),
            };
            total_extracted += written;
            if total_extracted
                > MAX_ARCHIVE_EXTRACT_BYTES.min(crate::helper::quota::budget().unwrap_or(u64::MAX))
            {
                return helper_error(
                    request,
                    ErrorKind::FileTooLarge,
                    "Archive decompressed size exceeds maximum extraction limit (500 MB).",
                    "decompression_limit_exceeded",
                );
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                let safe_mode = mode & 0o777;
                let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(safe_mode));
            }
        }
    }

    HelperResponse::operation_done(request, "Archive extracted.")
}

fn extract_tar(
    archive_path: &Path,
    dest_dir: &Path,
    canonical_base: &Path,
    sensitive_config: &SensitivePolicyConfig,
    lower_name: &str,
    request: &HelperRequest,
) -> HelperResponse {
    let file = match File::open(archive_path) {
        Ok(f) => f,
        Err(err) => return helper_error_from_io(request, err),
    };

    let tar_reader: Box<dyn Read> =
        if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") {
            Box::new(flate2::read::GzDecoder::new(file))
        } else {
            Box::new(file)
        };

    let mut archive = tar::Archive::new(tar_reader);
    let entries = match archive.entries() {
        Ok(e) => e,
        Err(err) => return helper_error_from_io(request, err),
    };

    let mut total_extracted: u64 = 0;

    for entry in entries {
        if crate::helper::cancellation::check().is_err() {
            return HelperResponse::permission_denied(request, "Operation cancelled.");
        }
        let mut entry = match entry {
            Ok(e) => e,
            Err(_) => {
                return helper_error(
                    request,
                    ErrorKind::ValidationError,
                    "Failed to read tar entry.",
                    "tar_read_error",
                )
            }
        };

        let path = match entry.path() {
            Ok(p) => p.into_owned(),
            Err(err) => return helper_error_from_io(request, err),
        };

        if path.is_absolute() {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry path attempts directory traversal.",
                "tar_slip_attempt",
            );
        }

        for component in path.components() {
            if matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            ) {
                return helper_error(
                    request,
                    ErrorKind::PolicyDenied,
                    "Archive entry path attempts directory traversal.",
                    "tar_slip_attempt",
                );
            }
        }

        if !archive_hidden_allowed(request)
            && crate::fs::path::is_hidden_relative(&RelativePath::new(path.to_string_lossy()))
        {
            return HelperResponse::permission_denied(request, "Hidden archive entry denied.");
        }
        let outpath = dest_dir.join(&path);
        if !outpath.starts_with(dest_dir) {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry path attempts directory traversal.",
                "tar_slip_attempt",
            );
        }

        if let Ok(rel) = outpath.strip_prefix(canonical_base) {
            let rel_str = rel.to_string_lossy();
            if detect_sensitive(&RelativePath::new(rel_str.as_ref()), sensitive_config).is_sensitive
            {
                return helper_error(
                    request,
                    ErrorKind::PolicyDenied,
                    "Archive entry extracts to sensitive path.",
                    "sensitive_path",
                );
            }
        }

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            // Reject symlinks/hardlinks in archive to prevent link traversal attacks
            continue;
        }

        let mut curr = outpath.parent();
        let mut parent_is_symlink = false;
        while let Some(p) = curr {
            if p == dest_dir {
                break;
            }
            if let Ok(meta) = fs::symlink_metadata(p) {
                if meta.file_type().is_symlink() {
                    parent_is_symlink = true;
                    break;
                }
            }
            curr = p.parent();
        }
        if parent_is_symlink {
            return helper_error(
                request,
                ErrorKind::PolicyDenied,
                "Archive entry parent directory is a symlink.",
                "symlink_denied",
            );
        }

        if let Ok(meta) = fs::symlink_metadata(&outpath) {
            if meta.file_type().is_symlink() {
                return helper_error(
                    request,
                    ErrorKind::PolicyDenied,
                    "Archive entry target is a symlink.",
                    "symlink_denied",
                );
            }
        }

        let entry_size = entry.header().size().unwrap_or(0);
        total_extracted += entry_size;
        if total_extracted
            > MAX_ARCHIVE_EXTRACT_BYTES.min(crate::helper::quota::budget().unwrap_or(u64::MAX))
        {
            return helper_error(
                request,
                ErrorKind::FileTooLarge,
                "Archive decompressed size exceeds maximum extraction limit (500 MB).",
                "decompression_limit_exceeded",
            );
        }

        if let Err(err) = entry.unpack(&outpath) {
            return helper_error_from_io(request, err);
        }
    }

    HelperResponse::operation_done(request, "Archive extracted.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RelativePath, RootId};
    use crate::helper::types::{HelperLimits, HelperPolicyContext};
    use crate::protocol::frame::RequestId;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("tealdrive-archive-helper-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("temp root");
        path
    }

    fn sample_request(
        command: HelperCommand,
        archive_rel: &str,
        target_rel: &str,
        sources: Option<Vec<&str>>,
    ) -> HelperRequest {
        HelperRequest {
            request_id: RequestId::new(),
            username: "alice".to_owned(),
            uid: 1000,
            gid: 1000,
            command,
            root_id: RootId::new("home"),
            relative_path: RelativePath::new(archive_rel),
            target_relative_path: Some(RelativePath::new(target_rel)),
            policy_context: HelperPolicyContext {
                read_only_root: false,
                is_web_root: false,
            },
            limits: HelperLimits {
                max_directory_page_size: 100,
                max_text_edit_size: 1024,
                max_chunk_size: 1024,
                max_download_helper_bytes: 1024,
            },
            list_options: None,
            write_options: None,
            chmod_mode: None,
            archive_source_paths: sources.map(|s| s.into_iter().map(RelativePath::new).collect()),
            search_options: None,
        }
    }

    #[test]
    fn compress_and_extract_zip_roundtrip() {
        let base = temp_root();
        fs::write(base.join("hello.txt"), b"hello archive").expect("write");
        fs::create_dir_all(base.join("subdir")).expect("mkdir");
        fs::write(base.join("subdir/nested.txt"), b"nested content").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        // Compress
        let req = sample_request(
            HelperCommand::CompressZip,
            "",
            "bundle.zip",
            Some(vec!["hello.txt", "subdir"]),
        );
        let resp = compress_zip_with_roots(
            &req,
            std::slice::from_ref(&root),
            &SensitivePolicyConfig::default(),
        );
        assert!(resp.success, "compress failed: {:?}", resp.safe_message);
        assert!(base.join("bundle.zip").is_file());

        // Extract
        let ext_req = sample_request(
            HelperCommand::ExtractArchive,
            "bundle.zip",
            "extracted",
            None,
        );
        let ext_resp =
            extract_archive_with_roots(&ext_req, &[root], &SensitivePolicyConfig::default());
        assert!(
            ext_resp.success,
            "extract failed: {:?}",
            ext_resp.safe_message
        );

        assert_eq!(
            fs::read_to_string(base.join("extracted/hello.txt")).expect("read"),
            "hello archive"
        );
        assert_eq!(
            fs::read_to_string(base.join("extracted/subdir/nested.txt")).expect("read"),
            "nested content"
        );
    }

    #[test]
    fn compress_and_extract_targz_roundtrip() {
        let base = temp_root();
        fs::write(base.join("data.txt"), b"tar content").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        // Compress
        let req = sample_request(
            HelperCommand::CompressTarGz,
            "",
            "archive.tar.gz",
            Some(vec!["data.txt"]),
        );
        let resp = compress_targz_with_roots(
            &req,
            std::slice::from_ref(&root),
            &SensitivePolicyConfig::default(),
        );
        assert!(resp.success, "compress failed: {:?}", resp.safe_message);
        assert!(base.join("archive.tar.gz").is_file());

        // Extract
        let ext_req = sample_request(
            HelperCommand::ExtractArchive,
            "archive.tar.gz",
            "tar_out",
            None,
        );
        let ext_resp =
            extract_archive_with_roots(&ext_req, &[root], &SensitivePolicyConfig::default());
        assert!(
            ext_resp.success,
            "extract failed: {:?}",
            ext_resp.safe_message
        );

        assert_eq!(
            fs::read_to_string(base.join("tar_out/data.txt")).expect("read"),
            "tar content"
        );
    }

    #[test]
    fn extract_archive_in_webroot_denied() {
        let base = temp_root();
        fs::write(base.join("bundle.zip"), b"fake zip").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: true,
            quota_bytes: None,
        };

        let req = sample_request(HelperCommand::ExtractArchive, "bundle.zip", "out", None);
        let resp = extract_archive_with_roots(&req, &[root], &SensitivePolicyConfig::default());
        assert!(!resp.success);
        assert_eq!(resp.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn compress_zip_sensitive_source_denied() {
        let base = temp_root();
        fs::write(base.join(".env"), b"SECRET=1").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base,
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        let req = sample_request(
            HelperCommand::CompressZip,
            "",
            "leak.zip",
            Some(vec![".env"]),
        );
        let resp = compress_zip_with_roots(&req, &[root], &SensitivePolicyConfig::default());
        assert!(!resp.success);
        assert_eq!(resp.error_kind, Some(ErrorKind::PolicyDenied));
    }

    #[test]
    fn extract_archive_symlink_parent_rejected() {
        let base = temp_root();
        let outside = temp_root();

        // Create a valid zip with one file
        let file_path = base.join("data.txt");
        fs::write(&file_path, b"test payload").expect("write");

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        let comp_req = sample_request(
            HelperCommand::CompressZip,
            "",
            "test.zip",
            Some(vec!["data.txt"]),
        );
        let comp_resp = compress_zip_with_roots(
            &comp_req,
            std::slice::from_ref(&root),
            &SensitivePolicyConfig::default(),
        );
        assert!(comp_resp.success);

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&outside, base.join("escape_dir")).expect("symlink");

            // Attempting to extract to escape_dir must be rejected
            let ext_req = sample_request(
                HelperCommand::ExtractArchive,
                "test.zip",
                "escape_dir",
                None,
            );
            let ext_resp =
                extract_archive_with_roots(&ext_req, &[root], &SensitivePolicyConfig::default());
            assert!(!ext_resp.success);
            assert_eq!(ext_resp.error_kind, Some(ErrorKind::PolicyDenied));
        }
    }
}
