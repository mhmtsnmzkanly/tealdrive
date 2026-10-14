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
    for source,target in [('bin','/home/tealdrive/bin'),('policy','/home/tealdrive/policy'),('auth','/home/tealdrive/auth'),('runtime','/home/tealdrive/data/runtime'),('config','/home/tealdrive/config'),('tls','/home/tealdrive/tls'),('home',manifest['home'])]:
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
        # Fresh virtual fixture metadata only; the production DB is a private clone.
        db=sqlite3.connect('/home/tealdrive/auth/accounts.db')
        db.executescript("DELETE FROM session_grants; DELETE FROM admin_budgets; DELETE FROM accounts "+("WHERE kind='virtual'" if os.environ.get('TEALDRIVE_CORE_MIGRATION')=='1' else "")+"; DELETE FROM enrollment; DELETE FROM security_state WHERE key!='dummy_password_hash';")
        db.commit();db.close()
        run(['/home/tealdrive/bin/tealdrive-wizard','pool','set','64MiB'])
        if os.environ.get('TEALDRIVE_CORE_MIGRATION')=='1':
            import hashlib
            source_home=Path(manifest['home']);(source_home/'test').mkdir(mode=0o700);os.chown(source_home/'test',manifest['sourceUid'],manifest['sourceGid'])
            source_file=source_home/'test/hello gpt.md';source_file.write_bytes(b'Preserve original administrator content during virtual migration.\n');source_file.chmod(0o600);os.chown(source_file,manifest['sourceUid'],manifest['sourceGid'])
            db=sqlite3.connect('/home/tealdrive/auth/accounts.db');source_id=db.execute('SELECT id FROM accounts WHERE username=?',(manifest['username'],)).fetchone()[0];db.close()
            run(['/home/tealdrive/bin/tealdrive-wizard','account','admin',source_id])
            result=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','account','migrate-admin',source_id,'tealdrive','64MiB','32MiB']))
            account=result['account'];target_file=Path(account['home'])/'test/hello gpt.md'
            if target_file.read_bytes()!=source_file.read_bytes() or target_file.stat().st_uid!=account['uid'] or source_file.stat().st_uid!=manifest['sourceUid']:raise RuntimeError('Migration did not preserve contents/owners')
            db=sqlite3.connect('/home/tealdrive/auth/accounts.db')
            same=db.execute('SELECT a.web_password_hash=b.web_password_hash FROM accounts a,accounts b WHERE a.id=? AND b.id=?',(source_id,account['id'])).fetchone()[0];db.close()
            if not same:raise RuntimeError('Migration changed password credentials')
            repeated=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','account','migrate-admin',source_id,'tealdrive','64MiB','32MiB']))
            if not repeated['completed'] or repeated['targetAccountId']!=account['id']:raise RuntimeError('Migration was not idempotent')
            print('PASS: offline descriptor migration, existing hash/content preserved, new UID, no overwrite, idempotent retry')
        elif os.environ.get('TEALDRIVE_CORE_BOOTSTRAP')=='1':
            result=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','bootstrap','admin','64MiB','32MiB']))
            state=json.loads(Path(result['credentialFile']).read_text())
            (base/'bootstrap-password').write_text(state['password']);(base/'bootstrap-password').chmod(0o600)
            account=result['account']
            if not account['password_change_required'] or not account['global_admin'] or account['kind']!='virtual':raise RuntimeError('Bootstrap account scope/restriction incorrect')
            repeated=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','bootstrap','admin','64MiB','32MiB']))
            if repeated['account']['id']!=account['id'] or not repeated['existing']:raise RuntimeError('Bootstrap was not idempotent')
        else:
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
    if os.environ.get('TEALDRIVE_CORE_CHANNEL')=='1':
        generated=json.loads(subprocess.check_output(['/home/tealdrive/bin/tealdrive-wizard','channel','keygen','/home/tealdrive/tls/channel.key']))
        (base/'channel-public-key').write_text(generated['publicKey'])
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
        childenv={**os.environ,'NODE_PATH':str(ROOT/'tools/dev/node_modules'),'TEALDRIVE_TEST_USERNAME':manifest['username'],'TEALDRIVE_TEST_WEB_PASSWORD_FILE':str(base/'password'),'TEALDRIVE_EXPECT_GLOBAL':str(manifest.get('expectGlobal',False)),'TEALDRIVE_PANEL_ACCOUNT_ID_FILE':str(base/'panel-account-id'),'TEALDRIVE_CHANNEL_KEY_FILE':str(base/'channel-public-key'),'TEALDRIVE_BOOTSTRAP_PASSWORD_FILE':str(base/'bootstrap-password')}
        subprocess.run(['node',str(ROOT/'tests/live/core-transfer.cjs')],env=childenv,check=True)
        if os.environ.get('TEALDRIVE_CORE_TLS_RENEWAL')=='1':
            caddy_dir=base/'caddy';caddy_dir.mkdir(mode=0o700)
            admin_dir=caddy_dir/'admin';admin_dir.mkdir(mode=0o700)
            caddyfile=caddy_dir/'Caddyfile'
            caddyfile.write_text('{\n admin unix/'+str(admin_dir/'admin.sock')+'\n auto_https off\n}\nhttp://127.0.0.1:13107 {\n reverse_proxy https://127.0.0.1:13106 {\n transport http {\n tls_server_name tealdrive-origin.local\n tls_trust_pool file /home/tealdrive/policy/origin-cert.pem\n }\n }\n}\n')
            caddyfile.chmod(0o600)
            caddylog=(caddy_dir/'caddy.log').open('wb')
            caddy=subprocess.Popen(['/usr/bin/caddy','run','--config',str(caddyfile),'--adapter','caddyfile'],stdout=caddylog,stderr=caddylog)
            try:
                import urllib.request
                for _ in range(100):
                    if caddy.poll() is not None:raise RuntimeError('Private Caddy exited')
                    try:
                        urllib.request.urlopen('http://127.0.0.1:13107/api/health',timeout=2);break
                    except Exception:time.sleep(.1)
                else:raise RuntimeError('Private Caddy not ready')
                old_cert=Path('/home/tealdrive/policy/origin-cert.pem').read_bytes()
                run(['/home/tealdrive/bin/tealdrive-wizard','tls','prepare','tealdrive-origin.local'])
                if old_cert!=Path('/home/tealdrive/policy/origin-cert.pem').read_bytes():raise RuntimeError('Prepare published active certificate')
                original_caddy=caddyfile.read_text()
                caddyfile.write_text(original_caddy.replace('http://127.0.0.1:13107 {','http://127.0.0.1:13107 {\n header X-Unrelated-Change blocked'))
                rejected=subprocess.run(['/home/tealdrive/bin/tealdrive-wizard','tls','activate','tealdrive-origin.local','--caddy',str(caddyfile)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                if rejected.returncode==0 or old_cert!=Path('/home/tealdrive/policy/origin-cert.pem').read_bytes():raise RuntimeError('Renewal published unrelated pending Caddy changes')
                caddyfile.write_text(original_caddy)
                run(['/home/tealdrive/bin/tealdrive-wizard','tls','activate','tealdrive-origin.local','--caddy',str(caddyfile)])
                if old_cert==Path('/home/tealdrive/policy/origin-cert.pem').read_bytes():raise RuntimeError('Renewal did not change active certificate')
                urllib.request.urlopen('http://127.0.0.1:13107/api/health',timeout=3)
                if Path('/home/tealdrive/tls/renewal').exists():raise RuntimeError('Completed renewal retained private staged keys')
                if Path('/home/tealdrive/policy/origin-trust.pem').read_bytes()!=Path('/home/tealdrive/policy/origin-cert.pem').read_bytes():raise RuntimeError('Old Caddy trust not retired')
                run(['/home/tealdrive/bin/tealdrive-wizard','tls','renew','tealdrive-origin.local','--caddy',str(caddyfile)])
                if Path('/home/tealdrive/tls/renewal').exists():raise RuntimeError('Daily check unnecessarily renewed fresh certificate')
                print('PASS: real private Caddy dual trust, live TLS refresh, strict identity/health, old trust removal, renewal threshold')
            finally:
                caddy.terminate()
                try:caddy.wait(timeout=10)
                except subprocess.TimeoutExpired:caddy.kill();caddy.wait()
                caddylog.close()
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
    shutil.copytree('/home/tealdrive/tls',base/'tls')
    os.chown(base/'tls',0,service.pw_gid)
    for key in (base/'tls').glob('*.pem'):
        os.chown(key,0,service.pw_gid)

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
        if os.environ.get('TEALDRIVE_CORE_CHANNEL')=='1':
            body=body.replace('[server]','[server]\nchannel_key_path = "/home/tealdrive/tls/channel.key"')
        path.write_text(body)
    (base/'password').write_bytes(password.read_bytes());(base/'password').chmod(0o600)
    virtual=os.environ.get('TEALDRIVE_CORE_VIRTUAL_ADMIN')=='1'
    if virtual:
        shutil.copytree('/etc',base/'etc',symlinks=True)
        (base/'storage').mkdir(mode=0o700)
    (base/'manifest.json').write_text(json.dumps({'username':username,'home':identity.pw_dir,'sourceUid':identity.pw_uid,'sourceGid':identity.pw_gid,'virtualAdmin':virtual}))
    subprocess.run(['/usr/bin/unshare','--mount','--pid','--fork','--mount-proc','python3',str(Path(__file__).resolve()),'--child',str(base)],check=True)
