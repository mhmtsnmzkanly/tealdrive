"""Remove only a verified fresh fixture's private ledger; never follows links or accepts a chosen root."""
import os
from pathlib import Path
import stat
import uuid

BASE = Path('/home/tealdrive/virtual-homes/.tealdrive-audit')
def cleanup_ledger(account_id, uid):
    uuid.UUID(account_id)
    target = BASE / account_id
    if not target.exists():
        return
    metadata = target.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o777 != 0o700:
        raise RuntimeError('Untrusted fixture ledger directory; refusing cleanup')
    for entry in target.iterdir():
        if entry.suffix != '.json':
            raise RuntimeError('Unexpected fixture ledger entry')
        uuid.UUID(entry.stem)
        metadata = entry.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid not in (0, uid) or metadata.st_nlink != 1 or metadata.st_mode & 0o777 != 0o400:
            raise RuntimeError('Untrusted fixture ledger file; refusing cleanup')
        entry.unlink()
    target.rmdir()
    if BASE.exists() and not any(BASE.iterdir()):
        BASE.rmdir()
