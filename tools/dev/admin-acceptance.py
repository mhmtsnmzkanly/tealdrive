#!/usr/bin/env python3
"""Explicit root fixture: isolated server, unique real Linux users, restored helper policy.
Never resets existing account data or production service config. Requires installed current binaries.
"""
import sys
import fcntl
import grp
import hashlib
import json
import os
import pathlib
import pwd
import secrets
import shutil
import signal
import socket
import sqlite3
import subprocess
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HELPER_CONFIG = pathlib.Path('/home/tealdrive/policy/helper.toml')
DB = pathlib.Path('/home/tealdrive/auth/accounts.db')

def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)

def atomic(path, data, mode=0o600):
    temporary = path.with_name(path.name + '.acceptance-new')
    with temporary.open('xb') as out:
        os.chmod(temporary, mode)
        out.write(data)
        out.flush()
        os.fsync(out.fileno())
    os.replace(temporary, path)

def main():
    if os.geteuid() != 0 or os.environ.get('TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE') != '1':
        raise RuntimeError('Explicit root admin-acceptance opt-in required')
    phase = os.environ.get('TEALDRIVE_ADMIN_ACCEPTANCE_PHASE', 'access')
    if phase not in ('access', 'approval', 'ui'):
        raise RuntimeError('Unknown acceptance phase')
    lock = os.open('/home/tealdrive/auth/.admin-acceptance.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', 13102))  # Refuse an occupied test port.
    virtual = os.environ.get('TEALDRIVE_VIRTUAL_ACCEPTANCE') == '1'
    native = os.environ.get('TEALDRIVE_NATIVE_QUOTA_ACCEPTANCE') == '1'
    if native and not virtual:
        raise RuntimeError('Native acceptance requires virtual acceptance')
    if virtual and os.environ.get('TEALDRIVE_HOME_ACCEPTANCE') != '1':
        raise RuntimeError('Virtual acceptance requires explicit home template mode')
    with sqlite3.connect(DB) as initial:
        pool_original = initial.execute('SELECT limit_bytes FROM quota_pool WHERE id=1').fetchone()[0]
        cursor_original = initial.execute("SELECT value FROM security_state WHERE key='virtual_uid_cursor'").fetchone()
    virtual_parent = pathlib.Path('/home/tealdrive/virtual-homes')
    parent_created = False
    mounted = False
    suffix = secrets.token_hex(5)
    config_dir = pathlib.Path('/home/tealdrive/policy/.admin-acceptance-' + suffix)
    private_dir = pathlib.Path('/home/tealdrive/auth/.admin-acceptance-' + suffix)
    config_dir.mkdir(mode=0o755)
    private_dir.mkdir(mode=0o700)
    original = HELPER_CONFIG.read_bytes()
    original_mode = HELPER_CONFIG.stat().st_mode & 0o777
    atomic(private_dir / 'helper-original.toml', original)
    users = []
    identities = {}
    process = None
    applied = None
    log = None
    audit = pathlib.Path('/home/tealdrive/logs/server/admin-acceptance-' + suffix + '.jsonl')
    runtime = pathlib.Path('/home/tealdrive/config/.admin-acceptance-' + suffix + '.toml')
    manifest = private_dir / 'manifest.json'
    try:
        if virtual:
            if virtual_parent.exists():
                if virtual_parent.is_symlink() or virtual_parent.stat().st_uid != 0 or any(virtual_parent.iterdir()):
                    raise RuntimeError('Refusing to reuse nonempty/untrusted virtual home parent for fixture')
            else:
                virtual_parent.mkdir(mode=0o711)
                parent_created = True
            if native:
                image = private_dir / 'pool.img'
                with image.open('xb') as out:
                    os.chmod(image, 0o600)
                    out.truncate(128 * 1024 * 1024)
                run(['/usr/sbin/mkfs.ext4', '-q', '-F', '-O', 'quota', '-E', 'quotatype=usrquota', str(image)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                run(['/usr/bin/mount', '-o', 'loop,nodev,nosuid,noexec,usrquota', str(image), str(virtual_parent)])
                mounted = True
                os.chmod(virtual_parent, 0o711)
        for label, role, bootstrap in [('a', 'admin', True), ('u', 'user', True), ('p', 'user', False)]:
            name = 'tdaccept' + label + suffix
            try:
                pwd.getpwnam(name)
                raise RuntimeError('Fixture OS name already occupied')
            except KeyError:
                pass
            run(['/usr/sbin/useradd', '-U', '-m', '-d', '/home/' + name, '-s', '/bin/bash', name], stdout=subprocess.DEVNULL)
            identity = pwd.getpwnam(name)
            identities[name] = identity
            content = pathlib.Path(identity.pw_dir) / 'fixture.txt'
            content.write_text('fixture bytes')
            os.chown(content, identity.pw_uid, identity.pw_gid)
            os.chmod(content, 0o600)
            if os.environ.get('TEALDRIVE_HOME_ACCEPTANCE') == '1':
                for relative in ('.env', '.ssh/fixture-key'):
                    item = pathlib.Path(identity.pw_dir) / relative
                    item.parent.mkdir(exist_ok=True)
                    item.write_text('home fixture bytes')
                    os.chown(item.parent, identity.pw_uid, identity.pw_gid)
                    os.chown(item, identity.pw_uid, identity.pw_gid)
                    os.chmod(item, 0o600)
            password = secrets.token_urlsafe(24)
            linux_password = secrets.token_urlsafe(24)
            run(['/usr/sbin/chpasswd'], input=(name + ':' + linux_password + '\n').encode(), stdout=subprocess.DEVNULL)
            users.append(dict(username=name, role=role, password=password, linuxPassword=linux_password, bootstrap=bootstrap))
        atomic(manifest, json.dumps(dict(users=users, config_dir=str(config_dir), audit_path=str(audit))).encode())
        run([str(ROOT / 'target/debug/examples/admin_acceptance_fixture'), str(manifest)], cwd=ROOT)
        applied = (config_dir / 'helper.toml').read_bytes()
        atomic(HELPER_CONFIG, applied, original_mode)
        log = (private_dir / 'server.log').open('wb')
        process = subprocess.Popen(['/usr/sbin/runuser', '-u', 'tealdrive', '--', '/home/tealdrive/bin/tealdrive-server', '--config', str(config_dir / 'server.toml'), '--runtime-config', str(runtime), '--helper-bin', '/home/tealdrive/bin/tealdrive-helper'], stdout=log, stderr=log, start_new_session=True)
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError('Fixture server exited; inspect private log')
            try:
                with urllib.request.urlopen('http://127.0.0.1:13102/api/health', timeout=1) as response:
                    if response.status == 200:
                        break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError('Fixture server did not become ready')
        env = os.environ.copy()
        env['NODE_PATH'] = str(ROOT / 'tools/dev/node_modules')
        env['TEALDRIVE_ADMIN_FIXTURE_MANIFEST'] = str(manifest)
        env['TEALDRIVE_ADMIN_ACCEPTANCE_PHASE'] = phase
        run(['node', str(ROOT / 'tests/live/admin-management.cjs')], env=env, cwd=ROOT)
        if virtual:
            run(['node', str(ROOT / 'tests/live/virtual-management.cjs')], env=env, cwd=ROOT)
        if os.environ.get('TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE') == '1':
            run([sys.executable, str(ROOT / 'tools/dev/local-wizard-acceptance.py'), str(manifest)], cwd=ROOT)
        audit_text = audit.read_text()
        if any(user[key] in audit_text for user in users for key in ('password', 'linuxPassword')):
            raise RuntimeError('Fixture credentials appeared in audit')
        events = [json.loads(line) for line in audit_text.splitlines()]
        for action in ('AccountSuspended', 'AccountEnabled') + (() if phase == 'access' else ('AccountApproved',)):
            if not any(event.get('action') == action and event.get('target_account_id') and event.get('target_username') in identities and event.get('username') == users[0]['username'] for event in events):
                raise RuntimeError('Missing actor/target account audit event')
        if os.environ.get('TEALDRIVE_POLICY_ACCEPTANCE') == '1':
            for actor in (users[0]['username'], users[1]['username']):
                if not any(event.get('action') == 'PolicyChanged' and event.get('username') == actor and event.get('target_account_id') for event in events):
                    raise RuntimeError('Missing policy actor/target audit event')
        if (config_dir / 'helper.toml').read_bytes() != HELPER_CONFIG.read_bytes():
            raise RuntimeError('Public API modified root helper policy')
        print(json.dumps(dict(actorTargetAudit=True, noCredentialAudit=True, rootPolicyUnmodifiedByApi=True)))

    finally:
        if process and process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
        if log:
            log.close()
        if applied is not None:
            current = HELPER_CONFIG.read_bytes()
            if hashlib.sha256(current).digest() != hashlib.sha256(applied).digest():
                raise RuntimeError('Helper policy changed externally; original private backup retained, refusing overwrite')
            atomic(HELPER_CONFIG, original, original_mode)
        # Remove only this run's fresh usernames/UIDs; never restore the whole DB.
        connection = sqlite3.connect(DB)
        connection.execute('PRAGMA foreign_keys=ON')
        for name, identity in identities.items():
            row = connection.execute('SELECT id,uid,home FROM accounts WHERE username=?', (name,)).fetchone()
            if row:
                if row[1] != identity.pw_uid or row[2] != identity.pw_dir:
                    raise RuntimeError('Fixture DB identity changed; refusing cleanup')
                if virtual:
                    children = connection.execute("SELECT id,username,uid,home FROM accounts WHERE owner_admin_id=? AND kind='virtual'", (row[0],)).fetchall()
                    intents = connection.execute("SELECT key,value FROM security_state WHERE key LIKE ?", ('virtual_create_v1:' + row[0] + ':%',)).fetchall()
                    virtual_uids = {child[2] for child in children}
                    for _, value in intents:
                        intent = json.loads(value)
                        if intent['owner'] != row[0]:
                            raise RuntimeError('Fixture intent owner changed')
                        virtual_uids.add(intent['uid'])
                    for uid in virtual_uids:
                        os_name = 'tdv_' + str(uid)
                        home = virtual_parent / os_name
                        try:
                            os_identity = pwd.getpwnam(os_name)
                        except KeyError:
                            os_identity = None
                        if os_identity is not None:
                            if os_identity.pw_uid != uid or os_identity.pw_gid != uid or os_identity.pw_dir != str(home) or os_identity.pw_shell != '/usr/sbin/nologin':
                                raise RuntimeError('Virtual fixture OS identity changed; refusing cleanup')
                            run(['/usr/sbin/userdel', os_name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                        if home.exists():
                            if home.is_symlink() or home.stat().st_uid not in (0,uid):
                                raise RuntimeError('Virtual fixture home changed; refusing cleanup')
                            shutil.rmtree(home)
                        try:
                            group = grp.getgrnam(os_name)
                        except KeyError:
                            group = None
                        if group:
                            if group.gr_gid != uid or group.gr_mem:
                                raise RuntimeError('Virtual fixture group changed; refusing cleanup')
                            run(['/usr/sbin/groupdel', os_name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    for child in children:
                        connection.execute('DELETE FROM session_grants WHERE account_id=?', (child[0],))
                        for prefix in ('account_policy_v1:', 'virtual_quota_sync_v1:'):
                            connection.execute('DELETE FROM security_state WHERE key=?', (prefix + child[0],))
                        connection.execute('DELETE FROM accounts WHERE id=?', (child[0],))
                    for key, _ in intents:
                        connection.execute('DELETE FROM security_state WHERE key=?', (key,))
                    connection.execute('DELETE FROM admin_budgets WHERE account_id=?', (row[0],))
                connection.execute('DELETE FROM session_grants WHERE account_id=?', (row[0],))
                connection.execute('DELETE FROM security_state WHERE key=?', ('account_policy_v1:' + row[0],))
                connection.execute('DELETE FROM accounts WHERE id=?', (row[0],))
            connection.execute('DELETE FROM enrollment WHERE username=?', (name,))
        if virtual:
            current = connection.execute('SELECT limit_bytes FROM quota_pool WHERE id=1').fetchone()[0]
            if current not in (pool_original, pool_original + 16777216):
                raise RuntimeError('Pool changed externally; refusing rollback')
            if current != pool_original:
                connection.execute('UPDATE quota_pool SET limit_bytes=? WHERE id=1', (pool_original,))
            if cursor_original is None:
                connection.execute("DELETE FROM security_state WHERE key='virtual_uid_cursor'")
            else:
                connection.execute("UPDATE security_state SET value=? WHERE key='virtual_uid_cursor'", (cursor_original[0],))
        connection.commit()
        connection.close()
        if mounted:
            if any(entry.name != 'lost+found' for entry in virtual_parent.iterdir()):
                raise RuntimeError('Unknown data appeared in native fixture; retaining mount and private image')
            run(['/usr/bin/umount', str(virtual_parent)])
        if parent_created:
            virtual_parent.rmdir()
        for name, identity in identities.items():
            current = pwd.getpwnam(name)
            if current.pw_uid != identity.pw_uid or current.pw_dir != '/home/' + name:
                raise RuntimeError('OS fixture identity changed; refusing cleanup')
            run(['/usr/sbin/userdel', '-r', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if HELPER_CONFIG.read_bytes() != original:
            raise RuntimeError('Helper policy restoration failed')
        for path in audit.parent.glob(audit.name + '*'):
            path.unlink()
        runtime.unlink(missing_ok=True)
        shutil.rmtree(config_dir)
        shutil.rmtree(private_dir)
        os.close(lock)
    print(json.dumps(dict(fixtureCleanup=True, helperPolicyRestored=True, productionUntouched=True)))

if __name__ == '__main__':
    main()
