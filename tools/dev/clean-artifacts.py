#!/usr/bin/env python3
"""Clean only known Root-owned TealDrive test artifacts in /tmp.
Dry-run by default. Preserves active paths, mounts, runtime-UID uploads and generic
operational audit. Build caches are separately removed with cargo clean.
"""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import shutil
import stat
import time

PREFIXES = (
    'tealdrive-service-create-', 'tealdrive-service-move-',
    'tealdrive-service-download-', 'tealdrive-service-rename-',
    'tealdrive-service-trash-', 'tealdrive-service-list-',
    'tealdrive-service-meta-', 'tealdrive-service-text-',
    'tealdrive-chmod-helper-', 'tealdrive-chmod-svc-',
    'tealdrive-helper-text-', 'tealdrive-helper-upload-',
    'tealdrive-helper-meta-', 'tealdrive-helper-move-',
    'tealdrive-helper-list-', 'tealdrive-helper-rename-',
    'tealdrive-helper-download-', 'tealdrive-helper-trash-',
    'tealdrive-helper-create-', 'tealdrive-dispatch-list-',
    'tealdrive-upload-test-', 'tealdrive-policy-',
    'tealdrive-archive-helper-', 'tealdrive-ws-',
    'tealdrive-e2e-', 'tealdrive-access-', 'tdrv-audit-', 'tdrv-pipe-test-',
)
UUID = r'[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
FIXTURE = re.compile(r'(?:' + '|'.join(map(re.escape, PREFIXES)) + r')' + UUID + r'$')
UPLOAD = re.compile(r'\.tdrv-upload-' + UUID + r'\.tmp$')
AUDIT_FIXTURE = re.compile(r'tealdrive-audit-' + UUID + r'\.jsonl$')
OUTPUT = re.compile(r'(?:tealdrive|tdrv)-[a-z-]+\.log$')
SCRIPTS = {'tealdrive-split-docs.py', 'tealdrive-permission-functions.js',
           'tealdrive-review-README.toml', 'tealdrive-review-CONFIGURATION.toml'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--min-age-hours', type=float, default=1)
    args = parser.parse_args()
    if os.getuid() != 0 or os.geteuid() != 0 or args.min_age_hours < 0:
        raise SystemExit('Real Root and nonnegative age required')
    active = set()
    for process in Path('/proc').iterdir():
        if not process.name.isdigit():
            continue
        try:
            for link in [process/'cwd', process/'exe', *(process/'fd').iterdir()]:
                try:
                    value=os.readlink(link).removesuffix(' (deleted)')
                except (FileNotFoundError, ProcessLookupError):
                    continue
                if value.startswith('/tmp/'):
                    active.add(value)
        except (FileNotFoundError, ProcessLookupError):
            continue
    mounts = []
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        mounts.append(line.split()[4].replace('\\040', ' '))
    cutoff=time.time()-args.min_age_hours*3600
    count=Counter();skipped=Counter();removed=[]
    for entry in sorted(Path('/tmp').iterdir()):
        name=entry.name
        if not (FIXTURE.fullmatch(name) or UPLOAD.fullmatch(name) or AUDIT_FIXTURE.fullmatch(name)
                or OUTPUT.fullmatch(name) or name in SCRIPTS):
            continue
        m=entry.lstat()
        if m.st_uid!=0 or m.st_mtime>cutoff or stat.S_ISLNK(m.st_mode):
            skipped['ownerAgeOrLink']+=1;continue
        path=str(entry)
        if any(v==path or v.startswith(path+'/') for v in active):
            skipped['active']+=1;continue
        if any(v==path or v.startswith(path+'/') for v in mounts):
            skipped['mounted']+=1;continue
        if stat.S_ISDIR(m.st_mode):
            if not FIXTURE.fullmatch(name):
                skipped['unexpectedType']+=1;continue
            if not shutil.rmtree.avoids_symlink_attacks:
                raise SystemExit('FD-safe recursive removal required')
            kind='fixtureDirectories'
            if args.apply:shutil.rmtree(entry)
        elif stat.S_ISREG(m.st_mode) and m.st_nlink==1:
            kind='temporaryFiles'
            if args.apply:entry.unlink()
        else:
            skipped['unexpectedType']+=1;continue
        count[kind]+=1;removed.append(path)
    print(json.dumps({'applied':args.apply,'selected':dict(count),'skipped':dict(skipped)}))
    return removed


if __name__=='__main__':
    main()
