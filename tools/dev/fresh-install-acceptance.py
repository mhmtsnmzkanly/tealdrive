#!/usr/bin/env python3
"""Root opt-in: fresh native image + random virtual admin install in private namespaces."""
import hashlib,json,os,pwd,shutil,sqlite3,subprocess,sys,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[2]
if os.getuid()!=0 or os.environ.get('TEALDRIVE_FRESH_ACCEPTANCE')!='1':raise SystemExit('Explicit Root fresh-install acceptance required')
def run(args):
 result=subprocess.run(args,capture_output=True,text=True)
 if result.returncode:raise RuntimeError(Path(args[0]).name+' failed: '+result.stderr[-3000:])
 return result
if len(sys.argv)>1 and sys.argv[1]=='--child':
 base=Path(sys.argv[2]);run(['/usr/bin/mount','--make-rprivate','/'])
 for source,target in [('etc','/etc'),('home','/home/tealdrive'),('local-bin','/usr/local/bin'),('systemctl','/usr/bin/systemctl')]:run(['/usr/bin/mount','--bind',str(base/source),target])
 wizard=root/'target/debug/tealdrive-wizard'
 output=run([str(wizard),'install','--bundle',str(base/'bundle'),'--pool-size','64MiB','--admin-own-quota','32MiB'])
 credential=Path('/home/tealdrive/auth/bootstrap.json');state=json.loads(credential.read_text())
 if credential.stat().st_mode&0o077 or len(state['password'])<32:raise RuntimeError('Unprotected bootstrap credentials')
 if state['password'] in output.stdout or state['password'] in output.stderr:raise RuntimeError('Bootstrap password leaked to output')
 db=sqlite3.connect('/home/tealdrive/auth/accounts.db')
 row=db.execute('SELECT id,username,uid,home,kind,role FROM accounts').fetchall()
 if len(row)!=1 or row[0][1]!='tealdrive' or row[0][4:]!=('virtual','admin'):raise RuntimeError('Unexpected first account')
 account_id,_,uid,home,_,_=row[0]
 if uid==pwd.getpwnam('tealdrive').pw_uid or pwd.getpwuid(uid).pw_shell!='/usr/sbin/nologin':raise RuntimeError('Service identity was adopted/unlocked')
 if Path(home).stat().st_uid!=uid:raise RuntimeError('Home owner mismatch')
 values=dict(db.execute('SELECT key,value FROM security_state'))
 if values['panel_scope:'+account_id]!='global' or values['password_change_required:'+account_id]!='1' or int(values['admin_own_quota_v1:'+account_id])!=32*1024*1024:raise RuntimeError('Initial scope/quota/restriction incorrect')
 if db.execute('SELECT limit_bytes FROM quota_pool').fetchone()[0]!=64*1024*1024:raise RuntimeError('Net pool mismatch')
 status=json.loads(run([str(wizard),'storage','status']).stdout)
 if not status['mounted'] or not status['kernelUserQuota']:raise RuntimeError('Native pool unavailable')
 run([str(wizard),'diagnose'])
 repeated=json.loads(run([str(wizard),'bootstrap','admin','64MiB','32MiB']).stdout)
 if not repeated['existing'] or repeated['account']['id']!=account_id:raise RuntimeError('Bootstrap was not idempotent')
 print('PASS: fresh four-binary install, preallocated quota image/net pool, random private credential, virtual global admin, fixed own quota, first-login restriction, nologin isolation, idempotent bootstrap')
 sys.exit()
with tempfile.TemporaryDirectory(prefix='.td-fresh-',dir='/root') as name:
 base=Path(name);base.chmod(0o700)
 shutil.copytree('/etc',base/'etc',symlinks=True)
 # This fixture represents a fresh host; copied production units are not inputs.
 unit_dir=base/'etc/systemd/system'
 for path in list(unit_dir.glob('tealdrive*'))+list(unit_dir.glob('home-tealdrive-virtual*')):
  if path.is_dir() and not path.is_symlink():shutil.rmtree(path)
  else:path.unlink()
 for path in unit_dir.rglob('*'):
  if path.is_symlink() and 'tealdrive' in path.name:path.unlink()

 for part in ['home','local-bin','bundle']:(base/part).mkdir(mode=0o700)
 (base/'systemctl').write_text('#!/bin/sh\nexit 0\n');(base/'systemctl').chmod(0o755)
 hashes={}
 for binary in ['tealdrive-server','tealdrive-helper','tealdrive-wizard','tealdrive-ctl']:
  destination=base/'bundle'/binary;shutil.copy2(root/'target/debug'/binary,destination);destination.chmod(0o755);hashes[binary]=hashlib.sha256(destination.read_bytes()).hexdigest()
 (base/'bundle/manifest.json').write_text(json.dumps({'schema_version':1,'version':'fresh-native-bootstrap','sha256':hashes}));(base/'bundle/manifest.json').chmod(0o600)
 print(run(['/usr/bin/unshare','--mount','--pid','--fork','--mount-proc','python3',str(Path(__file__).resolve()),'--child',str(base)]).stdout.strip())
 print('PASS: fresh-install namespace exited and private fixtures cleaned')
