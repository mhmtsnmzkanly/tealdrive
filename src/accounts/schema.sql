CREATE TABLE accounts (
 id TEXT PRIMARY KEY,
 username TEXT NOT NULL UNIQUE,
 uid INTEGER NOT NULL UNIQUE CHECK(uid BETWEEN 1 AND 4294967295),
 gid INTEGER NOT NULL CHECK(gid BETWEEN 1 AND 4294967295),
 home TEXT NOT NULL CHECK(substr(home,1,1)='/'),
 kind TEXT NOT NULL CHECK(kind IN ('linux','virtual')),
 role TEXT NOT NULL CHECK(role IN ('user','admin')),
 owner_admin_id TEXT REFERENCES accounts(id),
 status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','suspended','revoked','deleted')),
 login_enabled INTEGER NOT NULL DEFAULT 1 CHECK(login_enabled IN (0,1)),
 generation INTEGER NOT NULL DEFAULT 1 CHECK(generation > 0),
 web_password_hash TEXT,
 quota_bytes INTEGER CHECK(quota_bytes >= 0),
 undo_hours INTEGER CHECK(undo_hours >= 1),
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 deleted_at INTEGER,
 CHECK(kind='virtual' OR (owner_admin_id IS NULL AND quota_bytes IS NULL)),
 CHECK(kind='linux' OR (role='admin' AND owner_admin_id IS NULL AND quota_bytes IS NULL) OR (role='user' AND owner_admin_id IS NOT NULL AND quota_bytes IS NOT NULL))
) STRICT;
CREATE INDEX account_owner ON accounts(owner_admin_id);
CREATE TABLE quota_pool (id INTEGER PRIMARY KEY CHECK(id=1), limit_bytes INTEGER NOT NULL CHECK(limit_bytes >= 0)) STRICT;
INSERT INTO quota_pool VALUES(1,0);
CREATE TABLE admin_budgets (account_id TEXT PRIMARY KEY REFERENCES accounts(id), limit_bytes INTEGER NOT NULL CHECK(limit_bytes >= 0)) STRICT;
CREATE TABLE security_state (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE enrollment (
 token_hash TEXT PRIMARY KEY,
 username TEXT NOT NULL,
 uid INTEGER NOT NULL CHECK(uid > 0),
 caller_uid INTEGER NOT NULL,
 expires_at INTEGER NOT NULL,
 consumed INTEGER NOT NULL DEFAULT 0 CHECK(consumed IN (0,1))
) STRICT;
CREATE TABLE login_limits (
 scope TEXT NOT NULL,
 key_hash TEXT NOT NULL,
 failures INTEGER NOT NULL CHECK(failures >= 0),
 lock_until INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(scope,key_hash)
) STRICT;
PRAGMA application_id = 1413763670;
PRAGMA user_version = 1;
