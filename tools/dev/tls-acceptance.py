#!/usr/bin/env python3
"""Uses only the active unique admin fixture; tests direct TLS and an isolated Caddy proxy."""
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import ssl
import subprocess
import sys
import time
import urllib.request

ROOT=Path(__file__).resolve().parents[2]
manifest=Path(sys.argv[1]); fixture=json.loads(manifest.read_text())
if os.getuid()!=0 or os.environ.get('TEALDRIVE_TLS_ACCEPTANCE')!='1' or not manifest.parent.name.startswith('.admin-acceptance-'):
    raise SystemExit('Explicit Root admin fixture required')
base=Path(fixture['config_dir']); private=manifest.parent; auditbase=Path(fixture['audit_path'])
for port in (13103,13104,13105):
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);probe.bind(('127.0.0.1',port))
processes=[]; logs=[]; auditpaths=[]
cert=base/'tls-cert.pem';key=base/'tls-key.pem'
service_gid=__import__('pwd').getpwnam('tealdrive').pw_gid
subprocess.run(['/usr/bin/openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost,IP:127.0.0.1','-keyout',str(key),'-out',str(cert)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
os.chmod(cert,0o644);os.chown(key,0,service_gid);os.chmod(key,0o640)
original=(base/'server.toml').read_text()
def config(name,port,direct):
    path=base/(name+'.toml');audit=Path(str(auditbase)+'.'+name);auditpaths.append(audit)
    body=original.replace('127.0.0.1:13102','127.0.0.1:'+str(port)).replace('http://127.0.0.1:'+str(port),'https://127.0.0.1:'+str(port if direct else 13104)).replace('secure_cookies = false','secure_cookies = true').replace(str(auditbase),str(audit))
    body=body.replace('[server]\n','[server]\nrequire_tls = true\n')
    if direct: body=body.replace('[server]\n','[server]\n'+f'tls_cert_path = {json.dumps(str(cert))}\ntls_key_path = {json.dumps(str(key))}\n')
    else:
        body=body.replace('behind_tls_proxy = false','behind_tls_proxy = true')
        body=body.replace('trusted_proxies = []','trusted_proxies = ["127.0.0.1/32"]')
    path.write_text(body);path.chmod(0o644);return path

def spawn(args,label,env=None):
    log=(private/(label+'.log')).open('wb');logs.append(log)
    process=subprocess.Popen(args,stdout=log,stderr=log,start_new_session=True,env=env);processes.append(process);return process

def stop(process):
    if process.poll() is None:
        os.killpg(process.pid,signal.SIGTERM)
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()

def ready(url,process):
    for _ in range(100):
        if process.poll() is not None:raise RuntimeError('Fixture TLS/proxy exited; private log retained until cleanup')
        try:
            with urllib.request.urlopen(url,context=ssl._create_unverified_context(),timeout=1) as response:
                if response.status==200:return
        except OSError:time.sleep(.1)
    raise RuntimeError('TLS fixture did not become ready')

try:
    directconfig=config('direct',13103,True)
    direct=spawn(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(directconfig),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],'direct-server')
    ready('https://127.0.0.1:13103/api/health',direct)
    try:
        with urllib.request.urlopen('http://127.0.0.1:13103/api/health',timeout=2) as response:
            raise RuntimeError('TLS listener accepted clear HTTP')
    except (OSError,http.client.HTTPException):pass
    env=os.environ.copy();env['NODE_PATH']=str(ROOT/'tools/dev/node_modules');env['TEALDRIVE_TLS_TEST_URL']='https://127.0.0.1:13103';env['TEALDRIVE_TLS_STOP_PID']=str(direct.pid)
    subprocess.run(['node',str(ROOT/'tests/live/tls.cjs')],env=env,check=True,cwd=ROOT)
    stop(direct)
    # Private key world-readability and symlinks are rejected at startup.
    key.chmod(0o644)
    worldread=spawn(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(directconfig),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],'world-readable-key')
    if worldread.wait(timeout=10)==0:raise RuntimeError('World-readable TLS key accepted')
    key.chmod(0o640)
    real=base/'tls-real-key.pem';key.rename(real);key.symlink_to(real)
    linked=spawn(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(directconfig),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],'symlink-key')
    if linked.wait(timeout=10)==0:raise RuntimeError('Symlink TLS key accepted')
    key.unlink();real.rename(key)
    # No provider HTTP fallback for an invalid key, even on the same requested port.
    key.write_text('invalid private key');key.chmod(0o640)
    failed=spawn(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(directconfig),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],'invalid-tls')
    if failed.wait(timeout=10)==0:raise RuntimeError('Invalid key was accepted')
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);probe.bind(('127.0.0.1',13103))
    # Restore fixture certificate for independent proxy, never production Caddy config.
    subprocess.run(['/usr/bin/openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost,IP:127.0.0.1','-keyout',str(key),'-out',str(cert)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    os.chown(key,0,service_gid);key.chmod(0o640)
    proxyconfig=config('proxy',13105,False)
    backend=spawn(['/usr/sbin/runuser','-u','tealdrive','--','/home/tealdrive/bin/tealdrive-server','--config',str(proxyconfig),'--helper-bin','/home/tealdrive/bin/tealdrive-helper'],'proxy-backend')
    for _ in range(100):
        try:
            connection=http.client.HTTPConnection('127.0.0.1',13105,timeout=1);connection.request('GET','/api/health');response=connection.getresponse();assert response.status==426;connection.close();break
        except OSError:time.sleep(.1)
    else:raise RuntimeError('Proxy backend did not become ready')
    untrusted=http.client.HTTPConnection('127.0.0.1',13105,timeout=2,source_address=('127.0.0.2',0));untrusted.request('GET','/api/health',headers={'X-Forwarded-Proto':'https'});assert untrusted.getresponse().status==426;untrusted.close()
    caddyfile=private/'TLS-Caddyfile';caddyfile.write_text('{\n admin off\n auto_https off\n}\nhttps://127.0.0.1:13104 {\n tls '+str(cert)+' '+str(key)+'\n reverse_proxy 127.0.0.1:13105\n}\n')
    caddyenv={'PATH':'/usr/sbin:/usr/bin:/sbin:/bin','XDG_DATA_HOME':str(private/'caddy-data'),'XDG_CONFIG_HOME':str(private/'caddy-config')}
    proxy=spawn(['/usr/bin/caddy','run','--config',str(caddyfile),'--adapter','caddyfile'],'isolated-caddy',caddyenv)
    ready('https://127.0.0.1:13104/api/health',proxy)
    env['TEALDRIVE_TLS_TEST_URL']='https://127.0.0.1:13104';env['TEALDRIVE_TLS_STOP_PID']=str(backend.pid)
    subprocess.run(['node',str(ROOT/'tests/live/tls.cjs')],env=env,check=True,cwd=ROOT)
    print(json.dumps(dict(directTls=True,clearHttpRejected=True,invalidKeyNoFallback=True, unsafeKeyModesAndLinksRejected=True,independentProxy=True,untrustedForwardedProtoRejected=True)))
finally:
    for process in reversed(processes):stop(process)
    for log in logs:log.close()
    for path in auditpaths:
        if path.exists():
            content=path.read_text()
            if any(user[field] in content for user in fixture['users'] for field in ('password','linuxPassword')):raise RuntimeError('Credential in TLS fixture audit')
            path.unlink()
