#!/usr/bin/env python3
"""Opt-in private mount/PID namespace: candidate binaries + cloned auth DB.
Uses an existing approved test account; never writes its production home.
"""
import json, os, pwd, shutil, signal, socket, sqlite3, subprocess, sys, tempfile, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
BINS=('tealdrive-server','tealdrive-helper','tealdrive-wizard','tealdrive-ctl')
if os.getuid()!=0 or os.environ.get('TEALDRIVE_CORE_TRANSFER_ACCEPTANCE')!='1':
    raise SystemExit('Explicit Root acceptance required')
def run(args):
    subprocess.run(args,check=True,stdout=subprocess.DEVNULL)
if len(sys.argv)>1 and sys.argv[1]=='--child':
    base=Path(sys.argv[2]);manifest=json.loads((base/'manifest.json').read_text())
    run(['/usr/bin/mount','--make-rprivate','/'])
    for source,target in [('bin','/home/tealdrive/bin'),('policy','/home/tealdrive/policy'),('auth','/home/tealdrive/auth'),('runtime','/home/tealdrive/data/runtime'),('config','/home/tealdrive/config'),('home',manifest['home'])]:
        run(['/usr/bin/mount','--bind',str(base/source),target])
    env={**os.environ,'TMPDIR':'/home/tealdrive/data/runtime/tmp'}
    log=(base/'server.log').open('wb')
    server=subprocess.Popen(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config','/home/tealdrive/policy/server.toml','--helper-bin','/home/tealdrive/bin/tealdrive-helper'],env=env,stdout=log,stderr=log,start_new_session=True)
    try:
        for _ in range(100):
            if server.poll() is not None:raise RuntimeError('Candidate exited: '+(base/'server.log').read_text()[-2000:])
            try:
                with socket.create_connection(('127.0.0.1',13106),timeout=.2):break
            except OSError:time.sleep(.1)
        else:raise RuntimeError('Candidate listener not ready')
        childenv={**os.environ,'NODE_PATH':str(ROOT/'tools/dev/node_modules'),'TEALDRIVE_TEST_USERNAME':manifest['username'],'TEALDRIVE_TEST_WEB_PASSWORD_FILE':str(base/'password')}
        subprocess.run(['node',str(ROOT/'tests/live/core-transfer.cjs')],env=childenv,check=True)
        if list((base/'runtime/tmp').glob('.tdrv-upload-*')):raise RuntimeError('Staging leak after completed transfer')
        print('PASS: private namespace, real setuid helper/User worker, no production home or auth DB writes')
    finally:
        if server.poll() is None:
            os.killpg(server.pid,signal.SIGTERM)
            try:server.wait(timeout=15)
            except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait()
        log.close()
    sys.exit()
username=os.environ.get('TEALDRIVE_TEST_USERNAME','tealdrive-test')
password=Path(os.environ['TEALDRIVE_TEST_WEB_PASSWORD_FILE'])
if password.stat().st_mode&0o077:raise SystemExit('Private credential file required')
identity=pwd.getpwnam(username);service=pwd.getpwnam('tealdrive')
with socket.socket() as check:
    check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
    check.bind(('127.0.0.1',13106))
with tempfile.TemporaryDirectory(prefix='.core-transfer-',dir='/root') as name:
    base=Path(name);base.chmod(0o700)
    (base/'bin').mkdir()
    for binary in BINS:
        shutil.copy2(ROOT/'target/debug'/binary,base/'bin'/binary)
        (base/'bin'/binary).chmod(0o4755 if binary=='tealdrive-helper' else 0o755)
    shutil.copytree('/home/tealdrive/policy',base/'policy')
    (base/'auth').mkdir(mode=0o700)
    source=sqlite3.connect('file:/home/tealdrive/auth/accounts.db?mode=ro',uri=True)
    destination=sqlite3.connect(base/'auth/accounts.db');source.backup(destination);source.close();destination.close()
    (base/'auth/accounts.db').chmod(0o600)
    for directory in ['runtime','runtime/tmp','config','home']:
        (base/directory).mkdir()
        uid,gid=(identity.pw_uid,identity.pw_gid) if directory=='home' else (service.pw_uid,service.pw_gid)
        os.chown(base/directory,uid,gid);(base/directory).chmod(0o700)
    (base/'config/settings.toml').write_text('');os.chown(base/'config/settings.toml',service.pw_uid,service.pw_gid);(base/'config/settings.toml').chmod(0o600)
    for filename in ['server.toml','helper.toml']:
        path=base/'policy'/filename;body=path.read_text().replace('127.0.0.1:13100','127.0.0.1:13106').replace('https://tdrive.novastrum.dev','https://tealdrive-origin.local:13106').replace('/home/tealdrive/logs/server/audit.jsonl','/home/tealdrive/data/runtime/audit.jsonl').replace('max_in_flight_upload = 1048576','max_in_flight_upload = 1073741824')
        path.write_text(body)
    (base/'password').write_bytes(password.read_bytes());(base/'password').chmod(0o600)
    (base/'manifest.json').write_text(json.dumps({'username':username,'home':identity.pw_dir}))
    subprocess.run(['/usr/bin/unshare','--mount','--pid','--fork','--mount-proc','python3',str(Path(__file__).resolve()),'--child',str(base)],check=True)
