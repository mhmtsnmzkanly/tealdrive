#!/usr/bin/env python3
"""Explicit root fixture: isolated server, unique real Linux users, restored helper policy.
Never resets existing account data or production service config. Requires installed current binaries.
"""
import sys
sys.dont_write_bytecode=True
from fixture_cleanup import cleanup_ledger
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
import uuid

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
    subid_original={pathlib.Path(path):pathlib.Path(path).read_bytes() if pathlib.Path(path).exists() else None for path in ('/etc/subuid','/etc/subgid')}
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
        if virtual:
            with sqlite3.connect(DB) as initial:
                initial.execute("DELETE FROM security_state WHERE key='virtual_uid_cursor'")
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
        if os.environ.get('TEALDRIVE_UI_PACKAGE_ACCEPTANCE') == '1':
            run([sys.executable, str(ROOT / 'tools/dev/ui-package-acceptance.py'), str(manifest)], env=env, cwd=ROOT)
        if os.environ.get('TEALDRIVE_TLS_ACCEPTANCE') == '1':
            run([sys.executable, str(ROOT / 'tools/dev/tls-acceptance.py'), str(manifest)], env=env, cwd=ROOT)
        if os.environ.get('TEALDRIVE_UI_FILES_ACCEPTANCE') == '1':
            run(['node', str(ROOT / 'tests/live/ui-files.cjs')], env=env, cwd=ROOT)
        if os.environ.get('TEALDRIVE_ROOT_VIRTUAL_ACCEPTANCE') == '1':
            if not virtual:
                raise RuntimeError('Root virtual acceptance requires virtual fixture')
            run([sys.executable, str(ROOT / 'tools/dev/root-virtual-admin-acceptance.py'), str(manifest)], env=env, cwd=ROOT)
        if virtual:
            run(['node', str(ROOT / 'tests/live/virtual-management.cjs')], env=env, cwd=ROOT)
        if virtual and os.environ.get('TEALDRIVE_JOURNAL_ACCEPTANCE') == '1':
            with sqlite3.connect(DB) as ledger_state:
                owner=ledger_state.execute('SELECT id FROM accounts WHERE username=?',(users[0]['username'],)).fetchone()[0]
                children=ledger_state.execute('SELECT id,uid FROM accounts WHERE owner_admin_id=?',(owner,)).fetchall()
            seen=0
            for account_id,uid in children:
                ledger=virtual_parent/'.tealdrive-audit'/account_id
                if not ledger.exists():
                    continue
                if ledger.stat().st_uid!=0 or ledger.stat().st_mode&0o777!=0o700:
                    raise RuntimeError('Private ledger directory permissions')
                for entry in ledger.iterdir():
                    metadata=entry.stat()
                    if metadata.st_uid!=uid or metadata.st_mode&0o777!=0o400 or metadata.st_size<8192:
                        raise RuntimeError('Virtual ledger allocation not charged to UID')
                    attempted=subprocess.run(['/usr/sbin/runuser','-u','tdv_'+str(uid),'--','/usr/bin/cat',str(entry)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                    if attempted.returncode==0:
                        raise RuntimeError('Virtual UID could read private ledger')
                    seen+=1
            if not seen:
                raise RuntimeError('No real virtual quota ledger allocations')
            print(json.dumps(dict(virtualLedgerCharged=True,ownerCannotReadLedger=True,nativeLedger=native)))
            # Age only one owned completed record and verify the real Root GC releases its file.
            cutoff=int(time.time())-504*3600-1
            with sqlite3.connect(DB) as state:
                owned={account_id for account_id,_ in children}
                expired=state.execute("SELECT value FROM security_state WHERE key GLOB 'operation_v1:*' AND json_extract(value,'$.createdAt')<?",(cutoff+1,)).fetchall()
                if any(json.loads(row[0])['accountId'] not in owned for row in expired):
                    raise RuntimeError('Unrelated expired journal exists; refusing global GC acceptance')
                candidate=None
                for account_id,uid in children:
                    rows=state.execute("SELECT key,value FROM security_state WHERE key GLOB ? AND json_extract(value,'$.status')='success' LIMIT 1",('operation_v1:'+account_id+':*',)).fetchall()
                    if rows:
                        candidate=(account_id,uid,*rows[0]);break
                if not candidate:
                    raise RuntimeError('No completed virtual journal for GC test')
                account_id,uid,old_key,value=candidate
                record=json.loads(value);record['createdAt']=cutoff
                new_key='operation_v1:'+account_id+':'+str(cutoff).zfill(20)+':'+record['id']
                state.execute('UPDATE security_state SET key=?,value=? WHERE key=?',(new_key,json.dumps(record),old_key))
                entry=virtual_parent/'.tealdrive-audit'/account_id/(record['id']+'.json')
                if not entry.exists():
                    raise RuntimeError('Expected real ledger before GC')
            cleaned=run(['/home/tealdrive/bin/tealdrive-wizard','gc'],capture_output=True,text=True)
            if json.loads(cleaned.stdout).get('operation_logs_expired')!=1 or entry.exists():
                raise RuntimeError('Protected log retention/real file release failed')
            with sqlite3.connect(DB) as state:
                if state.execute('SELECT 1 FROM security_state WHERE key=?',(new_key,)).fetchone():
                    raise RuntimeError('Expired protected record retained')
            print(json.dumps(dict(logRetention504h=True,physicalLogRelease=True)))
        if os.environ.get('TEALDRIVE_LIFECYCLE_ACCEPTANCE') == '1':
            if not virtual:
                raise RuntimeError('Lifecycle acceptance requires virtual fixture')
            with sqlite3.connect(DB) as state:
                fixture_admin_id = state.execute('SELECT id FROM accounts WHERE username=?', (users[0]['username'],)).fetchone()[0]
                target = state.execute("SELECT id,uid,home FROM accounts WHERE owner_admin_id=? AND status='deleted' ORDER BY username LIMIT 1", (fixture_admin_id,)).fetchone()
                if not target:
                    raise RuntimeError('Missing retained deleted fixture')
                # Refuse a global GC test if it could touch any unrelated old account.
                unexpected = state.execute("SELECT count(*) FROM accounts WHERE kind='virtual' AND status='deleted' AND deleted_at<=? AND owner_admin_id!=?", (int(time.time())-36*3600, fixture_admin_id)).fetchone()[0]
                if unexpected:
                    raise RuntimeError('Unrelated eligible account exists; refusing global GC fixture')
            initial_gc = run(['/home/tealdrive/bin/tealdrive-wizard', 'gc'], capture_output=True, text=True)
            if json.loads(initial_gc.stdout)['virtual_purged'] != 0:
                raise RuntimeError('Retention window violated')
            target_home = pathlib.Path(target[2])
            marker = private_dir / 'outside-marker'
            marker.write_text('outside preserved')
            (target_home / 'outside-link').symlink_to(marker)
            nested = target_home / 'mode-zero'
            nested.mkdir()
            os.chown(nested, target[1], target[1])
            os.chmod(nested, 0)
            with sqlite3.connect(DB) as state:
                state.execute('UPDATE accounts SET deleted_at=? WHERE id=?', (int(time.time())-36*3600-1, target[0]))
            result = run(['/home/tealdrive/bin/tealdrive-wizard', 'gc'], capture_output=True, text=True)
            if json.loads(result.stdout)['virtual_purged'] != 1 or target_home.exists() or marker.read_text() != 'outside preserved':
                raise RuntimeError('Purge scope/retention failure')
            try:
                pwd.getpwuid(target[1])
                raise RuntimeError('Purged UID still exists')
            except KeyError:
                pass
            with sqlite3.connect(DB) as state:
                row = state.execute('SELECT quota_bytes,web_password_hash,status FROM accounts WHERE id=?', (target[0],)).fetchone()
                if row != (0, None, 'deleted') or not state.execute("SELECT 1 FROM security_state WHERE key=?", ('virtual_uid_retired:'+str(target[1]),)).fetchone():
                    raise RuntimeError('Purge metadata/fence failure')
            # Simulate interruption after OS removal but before the committed completion marker.
            with sqlite3.connect(DB) as state:
                state.execute('DELETE FROM security_state WHERE key=?', ('virtual_purged_v1:'+target[0],))
            resumed = run(['/home/tealdrive/bin/tealdrive-wizard', 'gc'], capture_output=True, text=True)
            if json.loads(resumed.stdout)['virtual_purged'] != 1:
                raise RuntimeError('Purge intent failed to recover after OS removal')
            marker.unlink()
            print(json.dumps(dict(purgeResume=True, retention36h=True, uidDroppedPurge=True, hiddenSymlinkOutsidePreserved=True, deletedQuotaReleased=True, retiredUidFence=True)))
        if os.environ.get('TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE') == '1':
            run([sys.executable, str(ROOT / 'tools/dev/local-wizard-acceptance.py'), str(manifest)], cwd=ROOT)
        if os.environ.get('TEALDRIVE_JOURNAL_ACCEPTANCE') == '1':
            run(['node', str(ROOT / 'tests/live/journal-history.cjs')], env=env, cwd=ROOT)
            with sqlite3.connect(DB) as state:
                records=[]
                for user in users:
                    account=state.execute('SELECT id FROM accounts WHERE username=?',(user['username'],)).fetchone()
                    if account:
                        records += [json.loads(row[0]) for row in state.execute('SELECT value FROM security_state WHERE key LIKE ?',('operation_v1:'+account[0]+':%',))]
            if len(records)<5 or not any(r['status']=='success' for r in records) or not any(r['status']=='failed' for r in records):
                raise RuntimeError('Missing protected helper intent/results')
            if os.environ.get('TEALDRIVE_UI_FILES_ACCEPTANCE') == '1':
                chmods=[r for r in records if r['action']=='chmod' and r['status']=='success']
                if not chmods or any(r.get('oldMode') is None or r.get('newMode') is None or r.get('after',{}).get('mode')!=r['newMode'] for r in chmods):
                    raise RuntimeError('Missing actual pinned chmod modes/state')
                snapshots=[r for r in records if r['action'] in ('rename','move','trash') and r['status']=='success']
                if not any(r.get('before') and r.get('after') and r['before']['inode']==r['after']['inode'] for r in snapshots):
                    raise RuntimeError('Missing observed mutation inode continuity')
            encoded=json.dumps(records)
            if any(user[key] in encoded for user in users for key in ('password','linuxPassword')):
                raise RuntimeError('Credentials appeared in protected journal')
            if any(r['status']=='pending' for r in records):
                raise RuntimeError('Completed fixture has unexplained pending journal')
            print(json.dumps(dict(protectedHelperJournal=True,successAndFailure=True,noCredentialJournal=True)))
            # Expire only explicitly named trash fixtures from this fresh Linux account.
            cutoff=int(time.time())-504*3600-2
            targets=[]
            with sqlite3.connect(DB) as state:
                owned={r['accountId'] for r in records}
                expired=state.execute("SELECT value FROM security_state WHERE key GLOB 'operation_v1:*' AND json_extract(value,'$.createdAt')<=?",(cutoff+2,)).fetchall()
                if any(json.loads(row[0])['accountId'] not in owned for row in expired):
                    raise RuntimeError('Unrelated expired journal exists; refusing trash GC fixture')
                for record in records:
                    if record['action']!='trash' or not record['sourcePath'].startswith(('gc-unchanged-','gc-changed-')):
                        continue
                    account=state.execute('SELECT home,uid FROM accounts WHERE id=?',(record['accountId'],)).fetchone()
                    if not account or account[0] not in [str(pathlib.Path('/home')/u['username']) for u in users]:
                        raise RuntimeError('Trash fixture outside fresh homes')
                    target=pathlib.Path(account[0])/record['targetPath']
                    if not target.is_file() or target.is_symlink() or target.stat().st_uid!=account[1]:
                        raise RuntimeError('Untrusted trash fixture')
                    if record['sourcePath'].startswith('gc-changed-'):
                        target.write_bytes(b'changed expiry fixture')
                    old_key='operation_v1:'+record['accountId']+':'+str(record['createdAt']).zfill(20)+':'+record['id']
                    record['createdAt']=cutoff
                    new_key='operation_v1:'+record['accountId']+':'+str(cutoff).zfill(20)+':'+record['id']
                    state.execute('UPDATE security_state SET key=?,value=? WHERE key=?',(new_key,json.dumps(record),old_key))
                    state.execute('UPDATE security_state SET value=? WHERE key=?',(json.dumps(record),'trash_expiry_v1:'+record['accountId']+':'+record['id']))
                    targets.append((record['sourcePath'],target))
            if len(targets)!=2:
                raise RuntimeError('Missing explicit trash expiry fixtures')
            cleaned=json.loads(run(['/home/tealdrive/bin/tealdrive-wizard','gc'],capture_output=True,text=True).stdout)
            if cleaned.get('trash_cleaned')!=1 or cleaned.get('trash_skipped')!=1 or cleaned.get('operation_logs_expired')!=2:
                raise RuntimeError('Expected-state trash GC result: '+json.dumps(cleaned))
            for label,target in targets:
                if label.startswith('gc-unchanged-') and target.exists():
                    raise RuntimeError('Expired unchanged trash remained')
                if label.startswith('gc-changed-') and (not target.is_file() or target.read_bytes()!=b'changed expiry fixture'):
                    raise RuntimeError('Changed trash was deleted or altered')
            print(json.dumps(dict(trashRetention504h=True,userIdentityTrashCleanup=True,changedTrashPreserved=True)))
            # Root-only private worker accepts an explicit test clock; host time is unchanged.
            as_of=int(time.time())+24*3600+5
            with sqlite3.connect(DB) as state:
                actor=state.execute('SELECT id,home,uid,gid,generation FROM accounts WHERE username=?',(users[1]['username'],)).fetchone()
                if not actor or actor[1]!='/home/'+users[1]['username']:
                    raise RuntimeError('Staging fixture outside own fresh account')
                baseline=next(r for r in records if r['accountId']==actor[0] and r['action']=='write')
                staged=[]
                for changed in (False,True):
                    record=dict(baseline)
                    record.update(id=str(uuid.uuid4()),requestId=str(uuid.uuid4()),generation=actor[4],createdAt=int(time.time()),updatedAt=int(time.time()),action='write',sourcePath='gc-staging-destination-'+suffix,targetPath=None,status='unknown',before=None,after=None,oldMode=None,newMode=None,undoStatus=None,stagingGcStatus=None,logReserveBytes=0,logStorage='systemBudget')
                    record['stagingPath']='.tdrv-transaction-'+record['id']
                    target=pathlib.Path(actor[1])/record['stagingPath']
                    fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
                    try:
                        os.write(fd,b'private staging fixture');os.fchown(fd,actor[2],actor[3])
                    finally:
                        os.close(fd)
                    if changed:
                        os.utime(target,(as_of+3600,as_of+3600))
                    data=json.dumps(record)
                    state.execute('INSERT INTO security_state(key,value) VALUES(?,?)',('operation_v1:'+actor[0]+':'+str(record['createdAt']).zfill(20)+':'+record['id'],data))
                    state.execute('INSERT INTO security_state(key,value) VALUES(?,?)',('staging_expiry_v1:'+actor[0]+':'+record['id'],data))
                    staged.append((changed,record['id'],target))
            for changed,operation_id,target in staged:
                outcome=subprocess.run(['/home/tealdrive/bin/tealdrive-wizard','__staging-expiry-worker',actor[0],operation_id,str(as_of)],capture_output=True,text=True)
                if changed:
                    if outcome.returncode==0 or not target.is_file() or target.read_bytes()!=b'private staging fixture':
                        raise RuntimeError('Recently changed staging was removed')
                elif outcome.returncode!=0 or target.exists():
                    raise RuntimeError('Old exact staging token was not cleaned: '+outcome.stderr[:256])
            print(json.dumps(dict(stagingRetention24h=True,exactStagingToken=True,recentStagingPreserved=True,hostClockUnchanged=True)))


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
        with sqlite3.connect(DB) as state:
            controls=[json.loads(row[0]) for row in state.execute("SELECT value FROM security_state WHERE key GLOB 'control_v1:*'")]
            claims={user['username'] for user in users}
            claims.update(row[0] for row in state.execute('SELECT id FROM accounts WHERE username IN (?,?,?)',tuple(user['username'] for user in users)))
            relevant=[row for row in controls if row.get('subjectClaim') in claims or row.get('targetClaim') in claims]
        if not any(row['action']=='Login' and row['status']=='success' for row in relevant) or not any(row['action']=='SetAccountAccess' and row['status']=='success' for row in relevant):
            raise RuntimeError('Missing protected auth/management control facts')
        encoded=json.dumps(controls)
        if any(user[field] in encoded for user in users for field in ('password','linuxPassword')):
            raise RuntimeError('Credentials appeared in protected control audit')
        print(json.dumps(dict(protectedControlAudit=True,authAndManagementFacts=True,noCredentialControlAudit=True)))


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
                        cleanup_ledger(child[0],child[2])
                        connection.execute('DELETE FROM session_grants WHERE account_id=?', (child[0],))
                        for prefix in ('account_policy_v1:', 'virtual_quota_sync_v1:', 'virtual_purged_v1:', 'virtual_purge_intent_v1:'):
                            connection.execute('DELETE FROM security_state WHERE key=?', (prefix + child[0],))
                        connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('operation_v1:'+child[0]+':%',))
                        connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('trash_expiry_v1:'+child[0]+':%',))
                        connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('staging_expiry_v1:'+child[0]+':%',))
                        connection.execute('DELETE FROM accounts WHERE id=?', (child[0],))
                    for uid in virtual_uids:
                        connection.execute('DELETE FROM security_state WHERE key=?', ('virtual_uid_retired:' + str(uid),))
                    for key, _ in intents:
                        connection.execute('DELETE FROM security_state WHERE key=?', (key,))
                    connection.execute('DELETE FROM admin_budgets WHERE account_id=?', (row[0],))
                cleanup_ledger(row[0],identity.pw_uid)
                connection.execute('DELETE FROM session_grants WHERE account_id=?', (row[0],))
                connection.execute('DELETE FROM security_state WHERE key=?', ('account_policy_v1:' + row[0],))
                connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('operation_v1:'+row[0]+':%',))
                connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('trash_expiry_v1:'+row[0]+':%',))
                connection.execute('DELETE FROM security_state WHERE key LIKE ?', ('staging_expiry_v1:'+row[0]+':%',))
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
                connection.execute("INSERT INTO security_state(key,value) VALUES('virtual_uid_cursor',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (cursor_original[0],))
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
        if virtual:
            for path,original_bytes in subid_original.items():
                current=path.read_bytes()
                cleaned=current if b'0:1000000:100000' in (original_bytes or b'').splitlines() else b''.join(line for line in current.splitlines(keepends=True) if line.strip()!=b'0:1000000:100000')
                if cleaned!=(original_bytes or b''):
                    raise RuntimeError('Subordinate ID file changed externally; preserving safe reservation and refusing overwrite')
                if original_bytes is None:
                    path.unlink()
                else:
                    metadata=path.stat()
                    atomic(path,original_bytes,metadata.st_mode & 0o777)
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
