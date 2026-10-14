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

    if manifest.get('virtualAdmin'):
        run(['/usr/bin/mount','--bind',str(base/'etc'),'/etc'])
        run(['/usr/bin/mount','--bind',str(base/'storage'),'/home/tealdrive/storage'])
        image=base/'storage/virtual-pool.ext4'
        with image.open('xb') as volume:
            image.chmod(0o600);os.posix_fallocate(volume.fileno(),0,128*1024*1024)
        run(['/usr/sbin/mke2fs','-q','-t','ext4','-F','-O','quota','-E','quotatype=usrquota',str(image)])
        run(['/usr/bin/mount','-o','loop,nodev,nosuid,noexec,usrquota',str(image),'/home/tealdrive/virtual-homes'])
        os.chmod('/home/tealdrive/virtual-homes',0o711)
        run(['/home/tealdrive/bin/tealdrive-wizard','pool','set','64MiB'])
        # Root TTY password input with verified echo disabled; no argv/env secret.
        import pty, select, termios, uuid
        pid,terminal=pty.fork()
        if pid==0:
            os.execv('/home/tealdrive/bin/tealdrive-wizard',['tealdrive-wizard','virtual','admin','create','tealdrive','64MiB',str(uuid.uuid4())])
        output=b'';sent=0;deadline=time.monotonic()+30
        try:
            while time.monotonic()<deadline:
                ready,_,_=select.select([terminal],[],[],.2)
                if ready:
                    try:data=os.read(terminal,65536)
                    except OSError:break
                    if not data:break
                    output+=data
                prompt=b'New virtual admin web password: ' if sent==0 else b'Confirm web password: '
                if sent<2 and prompt in output:
                    if termios.tcgetattr(terminal)[3]&termios.ECHO:raise RuntimeError('Secret prompt has echo enabled')
                    os.write(terminal,(base/'password').read_bytes().strip()+b'\n');sent+=1
            _,status=os.waitpid(pid,0)
            if os.waitstatus_to_exitcode(status)!=0 or sent!=2:raise RuntimeError('Virtual admin provisioning failed')
        finally:os.close(terminal)
        account=json.loads(output[output.index(b'{'):].decode())
        run(['/home/tealdrive/bin/tealdrive-wizard','budget','own','set',account['id'],'32MiB'])
        run(['/home/tealdrive/bin/tealdrive-wizard','account','scope',account['id'],'global'])
        (base/'panel-account-id').write_text(account['id'])
        manifest['username']='tealdrive'
        manifest['expectGlobal']=True
        print('PASS: private NSS virtual service-name alias and Root scope assignment')
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
        childenv={**os.environ,'NODE_PATH':str(ROOT/'tools/dev/node_modules'),'TEALDRIVE_TEST_USERNAME':manifest['username'],'TEALDRIVE_TEST_WEB_PASSWORD_FILE':str(base/'password'),'TEALDRIVE_EXPECT_GLOBAL':str(manifest.get('expectGlobal',False)),'TEALDRIVE_PANEL_ACCOUNT_ID_FILE':str(base/'panel-account-id')}
        subprocess.run(['node',str(ROOT/'tests/live/core-transfer.cjs')],env=childenv,check=True)
        if list((base/'runtime/tmp').glob('.tdrv-upload-*')):raise RuntimeError('Staging leak after completed transfer')
        print('PASS: private namespace, real setuid helper/User worker, no production home or auth DB writes')
    finally:
        if server.poll() is None:
            os.killpg(server.pid,signal.SIGTERM)
            try:server.wait(timeout=15)
            except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait()

        log.close()
    if manifest.get('virtualAdmin'):
        account_id=(base/'panel-account-id').read_text().strip()
        db=sqlite3.connect('/home/tealdrive/auth/accounts.db')
        home=Path(db.execute('SELECT home FROM accounts WHERE id=?',(account_id,)).fetchone()[0]);db.close()
        import hashlib
        digest=hashlib.sha256((home/'core-large.bin').read_bytes()).hexdigest()
        rejected=subprocess.run(['/home/tealdrive/bin/tealdrive-wizard','budget','own','set',account_id,'8MiB'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        if rejected.returncode==0:raise RuntimeError('Own quota below actual usage accepted')
        run(['/home/tealdrive/bin/tealdrive-wizard','storage','sync'])
        print('PASS: fixed own quota, 17MiB transfer, below-usage shrink rejected, native sync')
        old_status=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','storage','status']))
        run(['/home/tealdrive/bin/tealdrive-wizard','storage','check'])
        run(['/home/tealdrive/bin/tealdrive-wizard','storage','expand','--pool-size','128MiB'])
        current=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','storage','status']))
        if current['pool']['limitBytes']!=128*1024*1024 or current['imageBytes']<=old_status['imageBytes'] or current['expansionPending']:raise RuntimeError('Expansion did not publish verified net capacity')
        if digest!=hashlib.sha256((home/'core-large.bin').read_bytes()).hexdigest():raise RuntimeError('Expansion changed existing user content')
        rejected=subprocess.run(['/home/tealdrive/bin/tealdrive-wizard','storage','expand','--pool-size','64MiB'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        if rejected.returncode==0:raise RuntimeError('Shrink was accepted')
        run(['/home/tealdrive/bin/tealdrive-wizard','storage','check'])
        print('PASS: verified image/filesystem expansion, net pool publication, hash preservation, shrink rejection')
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
    virtual=os.environ.get('TEALDRIVE_CORE_VIRTUAL_ADMIN')=='1'
    if virtual:
        shutil.copytree('/etc',base/'etc',symlinks=True)
        (base/'storage').mkdir(mode=0o700)
    (base/'manifest.json').write_text(json.dumps({'username':username,'home':identity.pw_dir,'virtualAdmin':virtual}))
    subprocess.run(['/usr/bin/unshare','--mount','--pid','--fork','--mount-proc','python3',str(Path(__file__).resolve()),'--child',str(base)],check=True)
