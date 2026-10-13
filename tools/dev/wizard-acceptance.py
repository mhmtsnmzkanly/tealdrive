#!/usr/bin/env python3
"""Root opt-in CLI acceptance in a private mount namespace; no live data mutation."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[2]
NAMES = ('tealdrive-server', 'tealdrive-helper', 'tealdrive-wizard', 'tealdrive-ctl')

def invoke(binary, *args, ok=True):
    result = subprocess.run([str(binary), *args], capture_output=True, text=True)
    if (result.returncode == 0) != ok:
        raise RuntimeError(f'{args}: rc={result.returncode}: {result.stderr[:2000]}')
    return result

def mount(source, target):
    subprocess.run(['/usr/bin/mount', '--bind', str(source), str(target)], check=True)

def inner(base):
    mount(base / 'home', '/home/tealdrive')
    mount(base / 'systemd', '/etc/systemd/system')
    mount(base / 'pam', '/etc/pam.d')
    mount(base / 'systemctl', '/usr/bin/systemctl')
    wizard = REPO / 'target/debug/tealdrive-wizard'
    bundle = base / 'bundle'
    invoke(wizard, 'install', '--bundle', str(bundle))
    home = Path('/home/tealdrive')
    for name in NAMES:
        mode = (home / 'bin' / name).stat().st_mode & 0o7777
        assert mode == (0o4755 if name == 'tealdrive-helper' else 0o755)
    assert (home / 'auth/accounts.db').stat().st_mode & 0o777 == 0o600
    assert (home / 'policy/helper.key').stat().st_mode & 0o777 == 0o600
    key_before = (home / 'policy/helper.key').read_bytes()
    cfg_before = (home / 'policy/helper.toml').read_bytes()
    invoke(wizard, 'install', '--bundle', str(bundle), ok=False)
    invoke(wizard, 'config', 'validate')
    invoke(wizard, 'diagnose')
    # A real dedicated quota filesystem inside this mount/PID namespace.
    assert invoke(wizard, 'storage', 'status', ok=False).returncode != 0
    marker = home / 'virtual-homes/preserved.txt'
    marker.write_text('do not hide or replace')
    invoke(wizard, 'storage', 'init', '128MiB', ok=False)
    assert marker.read_text() == 'do not hide or replace'
    marker.unlink()
    invoke(wizard, 'storage', 'init', '128MiB')
    native = json.loads(invoke(wizard, 'storage', 'status').stdout)
    assert native['mounted'] and native['kernelUserQuota']
    assert 0 < native['poolCapacityBytes'] < native['totalBytes'] <= 128 * 1024 * 1024
    assert (home / 'storage/virtual-pool.ext4').stat().st_mode & 0o777 == 0o600
    escaped = subprocess.check_output(['/usr/bin/systemd-escape', '--path', '--suffix=mount', '/home/tealdrive/virtual-homes'], text=True).strip()
    assert (Path('/etc/systemd/system') / escaped).is_file()
    assert escaped in Path('/etc/systemd/system/tealdrive.service.d/virtual-storage.conf').read_text()
    invoke(wizard, 'storage', 'init', '128MiB', ok=False)
    invoke(wizard, 'storage', 'mount') # Already mounted is idempotent, no reformat.
    native_cfg = home / 'policy/native-acceptance.toml'
    native_cfg.write_text((home / 'policy/helper.toml').read_text().replace('enforcement = "application"', 'enforcement = "user_quota"'))
    native_cfg.chmod(0o600)
    invoke(wizard, 'config', 'apply', str(native_cfg))
    invoke(wizard, 'pool', 'set', str(native['totalBytes']), ok=False)
    invoke(wizard, 'pool', 'set', '128KiB')
    invoke(wizard, 'pool', 'set', '0')
    subprocess.run(['/usr/bin/umount', '/home/tealdrive/virtual-homes'], check=True)
    invoke(wizard, 'storage', 'status', ok=False)
    invoke(wizard, 'storage', 'mount')
    assert json.loads(invoke(wizard, 'storage', 'status').stdout)['kernelUserQuota']
    subprocess.run(['/usr/bin/umount', '/home/tealdrive/virtual-homes'], check=True)
    native_cfg.write_bytes(cfg_before)
    invoke(wizard, 'config', 'apply', str(native_cfg))
    invoke(wizard, 'account', 'approve', 'root', ok=False)
    invoke(wizard, 'account', 'approve', 'tealdrive', '--allow-nologin', ok=False)
    invoke(wizard, 'update', '--bundle', str(bundle))
    assert (home / 'policy/helper.key').read_bytes() == key_before
    assert (home / 'policy/helper.toml').read_bytes() == cfg_before
    prior = next((home / 'storage').glob('update-backup-*'))
    invoke(wizard, 'rollback', '--bundle', str(prior / 'binaries'))
    invoke(wizard, 'backup', str(home / 'storage/manual-backup'))
    invoke(wizard, 'restore', str(home / 'storage/manual-backup'))
    assert (home / 'policy/helper.key').read_bytes() == key_before
    invoke(wizard, 'repair')
    # A checksum failure must leave the already installed executable untouched.
    server_hash = hashlib.sha256((home / 'bin/tealdrive-server').read_bytes()).hexdigest()
    with (bundle / 'tealdrive-server').open('ab') as stream:
        stream.write(b'changed')
    invoke(wizard, 'update', '--bundle', str(bundle), ok=False)
    assert hashlib.sha256((home / 'bin/tealdrive-server').read_bytes()).hexdigest() == server_hash
    db_before = (home / 'auth/accounts.db').read_bytes()
    invoke(wizard, 'uninstall')
    assert (home / 'auth/accounts.db').read_bytes() == db_before
    assert (home / 'policy/helper.key').read_bytes() == key_before
    assert not (home / 'bin/tealdrive-server').exists()
    print('PASS: isolated install, reinstall rejection, modes, Root/service denial, update, binary rollback, metadata backup/restore, repair, tamper rejection, preserving uninstall, dedicated native quota image and remount')

def main():
    if len(sys.argv) == 3 and sys.argv[1] == '--inner':
        inner(Path(sys.argv[2]))
        return
    if os.getuid() != 0 or os.environ.get('TEALDRIVE_WIZARD_ACCEPTANCE') != '1':
        raise SystemExit('Requires Root and TEALDRIVE_WIZARD_ACCEPTANCE=1')
    # Root-owned parent is intentional: /tmp is untrusted by the production Wizard.
    with tempfile.TemporaryDirectory(prefix='td-wizard-', dir='/root') as directory:
        base = Path(directory)
        for name in ('home', 'proc', 'systemd', 'pam', 'bundle'):
            (base / name).mkdir(mode=0o700)
        hashes = {}
        for name in NAMES:
            dest = base / 'bundle' / name
            shutil.copyfile(REPO / 'target/debug' / name, dest)
            dest.chmod(0o600)
            hashes[name] = hashlib.sha256(dest.read_bytes()).hexdigest()
        (base / 'bundle/manifest.json').write_text(json.dumps({'schema_version': 1, 'version': 'acceptance-v1', 'sha256': hashes}))
        (base / 'systemctl').write_text('#!/bin/sh\nexit 0\n')
        (base / 'systemctl').chmod(0o755)
        subprocess.run(['/usr/bin/unshare', '--mount', '--pid', '--fork', '--mount-proc', '--propagation', 'private', sys.executable, str(Path(__file__).resolve()), '--inner', str(base)], check=True)

if __name__ == '__main__':
    main()
