//! Bounded discovery of local login candidates. This does not grant login access.
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct UserCandidate {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered: Option<bool>,
}
#[derive(Debug, Serialize)]
pub struct UserCandidates {
    pub users: Vec<UserCandidate>,
    pub truncated: bool,
}
fn read_public_file(path: &Path) -> io::Result<String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("account source must be regular"));
    }
    let mut content = String::new();
    file.take(1024 * 1024 + 1).read_to_string(&mut content)?;
    if content.len() > 1024 * 1024 {
        return Err(io::Error::other("account source too large"));
    }
    Ok(content)
}
pub fn discover(minimum_uid: u32) -> io::Result<UserCandidates> {
    let passwd = read_public_file(Path::new("/etc/passwd"))?;
    let shells = match read_public_file(Path::new("/etc/shells")) {
        Ok(shells) => shells,
        Err(error) if error.kind() == io::ErrorKind::NotFound => "/bin/sh\n/usr/bin/sh\n/bin/bash\n/usr/bin/bash\n/bin/dash\n/usr/bin/dash\n/bin/zsh\n/usr/bin/zsh\n/bin/fish\n/usr/bin/fish".into(),
        Err(error) => return Err(error),
    };
    Ok(candidates(&passwd, &shells, minimum_uid))
}
fn candidates(passwd: &str, shells: &str, minimum_uid: u32) -> UserCandidates {
    let shells: HashSet<_> = shells
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    let mut users = BTreeMap::new();
    for line in passwd.lines() {
        let fields: Vec<_> = line.split(':').collect();
        if fields.len() != 7 {
            continue;
        }
        let (Ok(uid), Ok(gid)) = (fields[2].parse::<u32>(), fields[3].parse::<u32>()) else {
            continue;
        };
        let username = fields[0];
        let shell = fields[6];
        let policy = crate::config::AccountPolicyConfig {
            allowed_users: vec![username.into()],
            ..Default::default()
        };
        let service_name = crate::auth::account::is_reserved_service_username(username);
        let disabled = matches!(
            Path::new(shell).file_name().and_then(|s| s.to_str()),
            Some("nologin" | "false")
        );
        if uid == 0
            || uid < minimum_uid
            || service_name
            || disabled
            || !shells.contains(shell)
            || !policy.validate()
        {
            continue;
        }
        users.insert(
            username.to_owned(),
            UserCandidate {
                username: username.into(),
                uid,
                gid,
                registered: None,
            },
        );
    }
    let truncated = users.len() > 1024;
    UserCandidates {
        users: users.into_values().take(1024).collect(),
        truncated,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_services_even_with_regular_uids_and_interactive_shells() {
        let passwd = "root:x:0:0::/root:/bin/bash\nalice:x:1000:1000::/home/alice:/bin/bash\nbob:x:1001:1001::/home/bob:/bin/zsh\ncaddy:x:1002:1002::/var/lib/caddy:/bin/bash\nwww-data:x:1003:1003::/var/www:/bin/bash\nsystem:x:999:999::/:/bin/bash\nlocked-shell:x:1004:1004::/:/usr/bin/false\nno-login:x:1005:1005::/:/usr/sbin/nologin\ncustom:x:1006:1006::/:/unknown/shell\nmalformed\n";
        let users = candidates(
            passwd,
            "/bin/bash\n/bin/zsh\n/usr/bin/false\n/usr/sbin/nologin",
            1000,
        );
        assert_eq!(
            users
                .users
                .iter()
                .map(|u| u.username.as_str())
                .collect::<Vec<_>>(),
            vec!["alice", "bob"]
        );
        assert!(!users.truncated);
        assert_eq!(
            candidates(passwd, "/bin/bash\n/bin/zsh", 1001).users[0].username,
            "bob"
        );
    }
    #[test]
    fn discovery_is_sorted_deduplicated_and_bounded() {
        let mut passwd =
            "alice:x:1000:1000::/:/bin/bash\nalice:x:1000:1000::/:/bin/bash\n".to_owned();
        for i in 0..1100 {
            passwd.push_str(&format!("user{i:04}:x:1000:1000::/:/bin/bash\n"));
        }
        let found = candidates(&passwd, "/bin/bash", 1000);
        assert_eq!(found.users.len(), 1024);
        assert!(found.truncated);
        assert_eq!(found.users[0].username, "alice");
    }
}
