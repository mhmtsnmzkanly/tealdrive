#!/usr/bin/env python3
"""Root-only, offline legacy file copy to an existing Linux user's NSS home.
Drops all privilege before walking either tree. Existing files are never replaced.
Original source remains intact. Inspect/backup first; stop external writers.
"""
import argparse
import hashlib
import os
from pathlib import Path
import pwd
import shutil
import stat
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--user', required=True)
    parser.add_argument('--source', required=True)
    args = parser.parse_args()
    if os.getuid() != 0 or os.geteuid() != 0:
        raise SystemExit('Real Root session required')
    account = pwd.getpwnam(args.user)
    source, target = Path(args.source), Path(account.pw_dir)
    if account.pw_uid < 1000 or args.user == 'tealdrive':
        raise SystemExit('Regular Linux file owner required')
    for path in (source, target):
        if not path.is_absolute() or '..' in path.parts or path == Path('/'):
            raise SystemExit('Unsafe path')
        for part in (path, *path.parents):
            if part.is_symlink():
                raise SystemExit('Symlink directory rejected')
        if not path.is_dir() or path.stat().st_uid != account.pw_uid:
            raise SystemExit('Both directories must belong to the selected user')
    if source == target or source in target.parents or target in source.parents:
        raise SystemExit('Overlapping trees rejected')
    os.setgroups([])
    os.setresgid(account.pw_gid, account.pw_gid, account.pw_gid)
    os.setresuid(account.pw_uid, account.pw_uid, account.pw_uid)
    count = 0

    def digest(path):
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').digest()

    def copy(src, dst):
        nonlocal count
        info = src.lstat()
        if stat.S_ISDIR(info.st_mode):
            if os.path.lexists(dst):
                if dst.is_symlink() or not dst.is_dir():
                    raise RuntimeError('Directory collision; source retained')
            else:
                dst.mkdir(mode=0o700)
            for entry in sorted(src.iterdir()):
                copy(entry, dst / entry.name)
            # Existing destination directories retain their settings.
        elif stat.S_ISREG(info.st_mode):
            if os.path.lexists(dst):
                if dst.is_symlink() or not dst.is_file() or digest(src) != digest(dst):
                    raise RuntimeError('File collision; source retained')
                return
            fd, temporary = tempfile.mkstemp(prefix='.td-migrate-', dir=dst.parent)
            os.close(fd)
            try:
                shutil.copy2(src, temporary, follow_symlinks=False)
                if digest(src) != digest(Path(temporary)):
                    raise RuntimeError('Source changed during copy')
                # Atomic no-replace publication, even if another writer creates dst.
                os.link(temporary, dst, follow_symlinks=False)
            finally:
                os.unlink(temporary)
            count += 1
        elif stat.S_ISLNK(info.st_mode):
            value = os.readlink(src)
            if os.path.lexists(dst):
                if not dst.is_symlink() or os.readlink(dst) != value:
                    raise RuntimeError('Symlink collision; source retained')
            else:
                os.symlink(value, dst)
                count += 1
        else:
            raise RuntimeError('Special file rejected; source retained')
    copy(source, target)
    print(f'Copied {count} entries without replacing existing files; source retained.')


if __name__ == '__main__':
    main()
