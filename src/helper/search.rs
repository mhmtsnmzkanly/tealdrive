use std::fs;

use crate::config::{AllowedRoot, RelativePath, SensitivePolicyConfig};
use crate::errors::ErrorKind;
use crate::fs::listing::helper_entry_to_payload;
use crate::fs::path::resolve_existing_path;
use crate::helper::listing::{
    directory_entry_from_metadata, helper_error, helper_error_from_tealdrive,
};
use crate::helper::types::{HelperRequest, HelperResponse};
use crate::protocol::payload::{SearchMatchEntry, SearchResultsPayload};

pub fn search_files_with_roots(
    request: &HelperRequest,
    roots: &[AllowedRoot],
    sensitive_config: &SensitivePolicyConfig,
) -> HelperResponse {
    if request.uid == 0 {
        return HelperResponse::permission_denied(request, "Helper refuses uid 0.");
    }

    let Some(root) = roots.iter().find(|r| r.root_id == request.root_id) else {
        return helper_error(
            request,
            ErrorKind::InvalidPath,
            "Unknown root id.",
            "invalid_root_id",
        );
    };

    let resolution = match resolve_existing_path(root, &request.relative_path, false) {
        Ok(res) => res,
        Err(err) => return helper_error_from_tealdrive(request, err),
    };

    let opts = request.search_options.as_ref();
    let query = opts.map(|o| o.query.trim()).unwrap_or("");
    if query.is_empty() {
        return HelperResponse::search_results(
            request,
            SearchResultsPayload {
                root_id: request.root_id.clone(),
                query: String::new(),
                matches: Vec::new(),
                total_matches: 0,
                truncated: false,
            },
        );
    }

    let query_lower = query.to_ascii_lowercase();
    let ext_filter = opts
        .and_then(|o| o.extension.as_deref())
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase());
    let max_results = opts
        .and_then(|o| o.max_results)
        .unwrap_or(200)
        .clamp(1, 1000);
    let include_hidden =
        opts.map(|o| o.include_hidden).unwrap_or(false) && root.hidden_files_allowed;

    let mut matches = Vec::new();
    let mut truncated = false;

    let mut queue = vec![(
        resolution.resolved_path.clone(),
        request.relative_path.0.clone(),
        0usize,
    )];
    let max_depth = opts.and_then(|o| o.max_depth).unwrap_or(20);

    'outer: while let Some((dir_path, dir_rel, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }

        let read_dir = match fs::read_dir(&dir_path) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in read_dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name = entry.file_name().to_string_lossy().to_string();
            if name.is_empty() {
                continue;
            }

            let is_hidden = name.starts_with('.');
            if is_hidden && !include_hidden {
                continue;
            }
            if name == ".tealdrive-trash" {
                continue;
            }

            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.file_type().is_symlink() {
                continue;
            }

            let item_rel = if dir_rel.is_empty() {
                name.clone()
            } else {
                format!("{dir_rel}/{name}")
            };

            let name_lower = name.to_ascii_lowercase();
            let query_matched = name_lower.contains(&query_lower);

            let ext_matched = if let Some(ref filter_ext) = ext_filter {
                name_lower
                    .rsplit_once('.')
                    .map(|(_, ext)| ext == filter_ext)
                    .unwrap_or(false)
            } else {
                true
            };

            if query_matched && ext_matched {
                let helper_entry =
                    directory_entry_from_metadata(name, &metadata, root, sensitive_config);
                let payload_entry = helper_entry_to_payload(helper_entry);
                matches.push(SearchMatchEntry {
                    relative_path: RelativePath::new(&item_rel),
                    entry: payload_entry,
                });

                if matches.len() >= max_results {
                    truncated = true;
                    break 'outer;
                }
            }

            if metadata.is_dir() && depth < max_depth {
                queue.push((path, item_rel, depth + 1));
            }
        }
    }

    let total = matches.len();
    HelperResponse::search_results(
        request,
        SearchResultsPayload {
            root_id: request.root_id.clone(),
            query: query.to_owned(),
            matches,
            total_matches: total,
            truncated,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RootId;
    use crate::helper::ipc::tests_support::sample_request;
    use crate::helper::types::HelperCommand;
    use std::fs;
    use std::path::PathBuf;

    fn temp_test_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("tdrv-search-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn search_finds_nested_files() {
        let base = temp_test_dir();
        fs::create_dir_all(base.join("docs/sub")).unwrap();
        fs::write(base.join("docs/readme.txt"), "hello").unwrap();
        fs::write(base.join("docs/sub/report.pdf"), "pdf").unwrap();
        fs::write(base.join("docs/sub/notes.txt"), "notes").unwrap();

        let root = AllowedRoot {
            root_id: RootId::new("home"),
            base_path: base.clone(),
            read_only: false,
            uploads_allowed: true,
            hidden_files_allowed: false,
            is_web_root: false,
            quota_bytes: None,
        };

        let mut req = sample_request(HelperCommand::SearchFiles);
        req.root_id = RootId::new("home");
        req.relative_path = RelativePath::new("");
        req.search_options = Some(crate::helper::types::HelperSearchOptions {
            query: "notes".to_owned(),
            extension: None,
            max_results: None,
            include_hidden: false,
            max_depth: None,
        });

        let resp = search_files_with_roots(
            &req,
            std::slice::from_ref(&root),
            &SensitivePolicyConfig::default(),
        );
        assert!(resp.success);
        let crate::helper::types::HelperResponsePayload::SearchResults(results) = resp.payload
        else {
            panic!("expected search results");
        };
        assert_eq!(results.matches.len(), 1);
        assert_eq!(results.matches[0].relative_path.0, "docs/sub/notes.txt");

        let _ = fs::remove_dir_all(base);
    }
}
