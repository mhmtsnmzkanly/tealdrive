#!/usr/bin/env python3
"""A temporary Root-approved package and server; no permanent theme activation."""
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time
import urllib.request
ROOT=Path(__file__).resolve().parents[2]
manifest=Path(sys.argv[1]);fixture=json.loads(manifest.read_text())
if os.getuid()!=0 or os.environ.get('TEALDRIVE_UI_PACKAGE_ACCEPTANCE')!='1' or not manifest.parent.name.startswith('.admin-acceptance-'):
    raise SystemExit('Explicit Root fixture required')
base=Path(fixture['config_dir']);private=manifest.parent;package=base/'custom-package';package.mkdir(mode=0o755);(package/'assets').mkdir(mode=0o755)
files={'index.html':b'<!doctype html><html><head><title>Fixture theme</title></head><body><p id="custom-theme">Root theme fixture</p><script type="module" src="/static/assets/main.js"></script></body></html>',
       'assets/main.js':b'import {createClient} from "/sdk/v1/tealdrive.js"; globalThis.themeApi=createClient({storage:null,quotaPollMs:0}); globalThis.themeReady=themeApi.handshake();'}
for name,data in files.items():path=package/name;path.write_bytes(data);path.chmod(0o644)
pkg={'manifestVersion':1,'name':'fixture','version':'1.0.0','sdkMajor':1,'entry':'index.html','assets':['assets/main.js'],'sha256':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}
(package/'manifest.json').write_text(json.dumps(pkg));(package/'manifest.json').chmod(0o644)
config=base/'custom-server.toml';audit=Path(fixture['audit_path']+'.custom')
body=(base/'server.toml').read_text().replace('127.0.0.1:13102','127.0.0.1:13106').replace(fixture['audit_path'],str(audit)).replace('[server]\n','[server]\nstatic_dir = '+json.dumps(str(package))+'\n')
config.write_text(body);config.chmod(0o644)
with socket.socket() as probe:probe.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);probe.bind(('127.0.0.1',13106))
process=None;log=(private/'custom-server.log').open('wb')
try:
    subprocess.run(['/home/tealdrive/bin/tealdrive-wizard','ui','check',str(package)],check=True,stdout=subprocess.DEVNULL)
    process=subprocess.Popen(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(config),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],stdout=log,stderr=log,start_new_session=True)
    for _ in range(100):
        if process.poll() is not None:raise RuntimeError('Custom fixture exited')
        try:
            with urllib.request.urlopen('http://127.0.0.1:13106/api/health',timeout=1) as response:
                if response.status==200:break
        except OSError:time.sleep(.1)
    else:raise RuntimeError('Custom fixture not ready')
    env=os.environ.copy();env['NODE_PATH']=str(ROOT/'tools/dev/node_modules');env['TEALDRIVE_UI_PACKAGE_DIR']=str(package)
    subprocess.run(['node',str(ROOT/'tests/live/ui-package.cjs')],env=env,check=True,cwd=ROOT)
finally:
    if process and process.poll() is None:
        os.killpg(process.pid,signal.SIGTERM)
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
    log.close()
    if audit.exists():
        text=audit.read_text()
        if any(user[field] in text for user in fixture['users'] for field in ('password','linuxPassword')):raise RuntimeError('Credential in custom UI audit')
        audit.unlink()
