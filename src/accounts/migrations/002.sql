ALTER TABLE accounts ADD COLUMN deny_enrollment INTEGER NOT NULL DEFAULT 0 CHECK(deny_enrollment IN (0,1));
-- Legacy pending proofs did not bind full caller/OS identity. Never carry them forward.
DROP TABLE enrollment;
CREATE TABLE enrollment (
 token_hash TEXT PRIMARY KEY,
 username TEXT NOT NULL,
 uid INTEGER NOT NULL CHECK(uid > 0),
 gid INTEGER NOT NULL CHECK(gid > 0),
 home TEXT NOT NULL,
 caller_uid INTEGER NOT NULL,
 caller_pid INTEGER NOT NULL CHECK(caller_pid > 0),
 caller_start INTEGER NOT NULL CHECK(caller_start > 0),
 caller_boot TEXT NOT NULL,
 expires_at INTEGER NOT NULL,
 consumed INTEGER NOT NULL DEFAULT 0 CHECK(consumed IN (0,1))
) STRICT;
CREATE TABLE session_grants (
 nonce TEXT PRIMARY KEY,
 account_id TEXT NOT NULL REFERENCES accounts(id),
 account_generation INTEGER NOT NULL CHECK(account_generation > 0),
 caller_uid INTEGER NOT NULL,
 caller_pid INTEGER NOT NULL CHECK(caller_pid > 0),
 caller_start INTEGER NOT NULL CHECK(caller_start > 0),
 caller_boot TEXT NOT NULL,
 expires_at INTEGER NOT NULL,
 revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
) STRICT;
CREATE INDEX grant_account ON session_grants(account_id);
UPDATE accounts SET generation=generation+1;
PRAGMA user_version = 2;
