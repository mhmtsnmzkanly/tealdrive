#!/usr/bin/env python3
"""Create a new Root-owned four-binary bundle for TealDrive Wizard.
Build with cargo build --locked --release --bins first. Checksums detect changes;
the operator must separately approve the source/build. Existing output is refused.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--bin-dir', required=True)
parser.add_argument('--output', required=True)
parser.add_argument('--version', required=True)
args = parser.parse_args()
if os.getuid() != 0 or os.geteuid() != 0:
    raise SystemExit('Real Root session required')
if not re.fullmatch(r'[A-Za-z0-9._-]{1,64}', args.version):
    raise SystemExit('Invalid release version')
output = Path(args.output)
if not output.is_absolute() or '..' in output.parts:
    raise SystemExit('Absolute output required')
for parent in output.parents:
    m = parent.lstat()
    if not stat.S_ISDIR(m.st_mode) or m.st_uid != 0 or m.st_mode & 0o022:
        raise SystemExit('Output parent must be Root-owned, unwritable and real')
names = ('tealdrive-server','tealdrive-helper','tealdrive-wizard','tealdrive-ctl')
source = Path(args.bin_dir)
for name in names:
    p = source / name
    m = p.lstat()
    if not stat.S_ISREG(m.st_mode) or m.st_uid != 0 or m.st_mode & 0o022 or m.st_size > 536870912:
        raise SystemExit('Untrusted source binary')
    with p.open('rb') as stream:
        if stream.read(4) != b'\x7fELF':
            raise SystemExit('ELF binary required')
os.umask(0o077)
output.mkdir(mode=0o700)
hashes = {}
for name in names:
    destination = output / name
    shutil.copyfile(source / name, destination)
    destination.chmod(0o755)
    with destination.open('rb') as stream:
        hashes[name] = hashlib.file_digest(stream, 'sha256').hexdigest()
manifest = output / 'manifest.json'
manifest.write_text(json.dumps({'schema_version':1,'version':args.version,'sha256':hashes},indent=2)+'\n')
manifest.chmod(0o600)
print('Created verified-shape release bundle:', output)
