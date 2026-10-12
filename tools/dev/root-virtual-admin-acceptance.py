#!/usr/bin/env python3
"""Fresh Root-created virtual admin only; own cleanup, no preexisting user changes."""
import grp
import importlib.util
import json
import os
from pathlib import Path
import pwd
import secrets
import shutil
import sqlite3
import subprocess
import sys
import uuid

DB = '/home/tealdrive/auth/accounts.db'
WIZARD = '/home/tealdrive/bin/tealdrive-wizard'
ROOT = Path(__file__).resolve().parents[2]
if os.getuid() != 0 or os.environ.get('TEALDRIVE_VIRTUAL_ACCEPTANCE') != '1':
    raise SystemExit('Explicit Root virtual fixture required')
spec = importlib.util.spec_from_file_location('tty_fixture', ROOT / 'tools/dev/local-wizard-acceptance.py')
sys.dont_write_bytecode = True
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
name = 'tdtenant-' + secrets.token_hex(5)
request = str(uuid.uuid4())
key = 'virtual_create_v1:__root__:' + request
password = secrets.token_urlsafe(24)
manifest = Path(sys.argv[1]).parent / 'root-virtual-admin.json'
try:
    output = module.prompt_command([WIZARD, 'virtual', 'admin', 'create', name, '1048576', request], [b'New virtual admin web password: ', b'Confirm web password: '], [password, password])
    account = json.loads(output[output.index(b'{'):])
    assert account['role'] == 'admin' and account['kind'] == 'virtual' and account['owner_admin_id'] is None
    assert account['home'] == '/home/tealdrive/virtual-homes/tdv_' + str(account['uid'])
    assert pwd.getpwuid(account['uid']).pw_shell == '/usr/sbin/nologin'
    manifest.write_text(json.dumps(dict(username=name, password=password, account=account)))
    manifest.chmod(0o600)
    env = os.environ.copy()
    env['TEALDRIVE_ROOT_VIRTUAL_MANIFEST'] = str(manifest)
    subprocess.run(['node', str(ROOT / 'tests/live/root-virtual-admin.cjs')], env=env, check=True, cwd=ROOT)
    replay = module.prompt_command([WIZARD, 'virtual', 'admin', 'create', name, '1048576', request], [b'New virtual admin web password: ', b'Confirm web password: '], ['changed-password-ignored', 'changed-password-ignored'])
    assert json.loads(replay[replay.index(b'{'):])['id'] == account['id']
    print('PASS: Root virtual admin provisioning, pool reservation, tenant-only management, own usage budget guard, idempotent retry')
finally:
    with sqlite3.connect(DB) as state:
        row = state.execute('SELECT id,uid,home,kind,role,owner_admin_id FROM accounts WHERE username=?', (name,)).fetchone()
        intent = state.execute('SELECT value FROM security_state WHERE key=?', (key,)).fetchone()
        records = []
        if row:
            assert row[3:] == ('virtual', 'admin', None)
            records = state.execute("SELECT id,uid,home FROM accounts WHERE owner_admin_id=? AND kind='virtual'", (row[0],)).fetchall() + [row[:3]]
        elif intent:
            value = json.loads(intent[0])
            assert value['name'] == name and value['owner'] == '__root__'
            records = [(None, value['uid'], '/home/tealdrive/virtual-homes/tdv_'+str(value['uid']))]
        for account_id, uid, home in records:
            expected = 'tdv_' + str(uid)
            assert home == '/home/tealdrive/virtual-homes/' + expected
            try:
                identity = pwd.getpwnam(expected)
            except KeyError:
                identity = None
            if identity:
                assert identity.pw_uid == uid and identity.pw_gid == uid and identity.pw_dir == home and identity.pw_shell == '/usr/sbin/nologin'
                subprocess.run(['/usr/sbin/userdel', expected], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            path = Path(home)
            if path.exists():
                assert not path.is_symlink() and path.stat().st_uid == uid
                shutil.rmtree(path)
            try:
                group = grp.getgrnam(expected)
            except KeyError:
                group = None
            if group:
                assert group.gr_gid == uid and not group.gr_mem
                subprocess.run(['/usr/sbin/groupdel', expected], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if account_id:
                state.execute('DELETE FROM session_grants WHERE account_id=?', (account_id,))
                state.execute('DELETE FROM admin_budgets WHERE account_id=?', (account_id,))
                for prefix in ('account_policy_v1:', 'virtual_quota_sync_v1:'):
                    state.execute('DELETE FROM security_state WHERE key=?', (prefix+account_id,))
                state.execute("DELETE FROM security_state WHERE key LIKE ?", ('virtual_create_v1:'+account_id+':%',))
                state.execute('DELETE FROM accounts WHERE id=?', (account_id,))
        state.execute('DELETE FROM security_state WHERE key=?', (key,))
    manifest.unlink(missing_ok=True)
