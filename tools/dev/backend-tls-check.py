#!/usr/bin/env python3
"""Opt-in isolated Caddy rejects a same-name untrusted backend certificate.
No production server/config changes; no credentials are sent.
"""
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

if os.geteuid() != 0 or os.environ.get('TEALDRIVE_BACKEND_TLS_ACCEPTANCE') != '1':
    raise SystemExit('Explicit Root backend TLS acceptance required')
for port in (13107,13108):
    with socket.socket() as check:
        check.bind(('127.0.0.1',port))
with tempfile.TemporaryDirectory(prefix='td-backend-tls-',dir='/root') as directory:
    base=Path(directory);os.chmod(base,0o700)
    key,cert=base/'fake-key.pem',base/'fake-cert.pem'
    subprocess.run(['/usr/bin/openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-subj','/CN=tealdrive-origin.local','-addext','subjectAltName=DNS:tealdrive-origin.local,IP:127.0.0.1','-keyout',str(key),'-out',str(cert)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(cert,key)
    listener=socket.socket();listener.settimeout(.2);listener.bind(('127.0.0.1',13107));listener.listen()
    stopped=threading.Event();received=[]
    def backend():
        while not stopped.is_set():
            try:connection,_=listener.accept()
            except socket.timeout:continue
            except OSError:return
            try:
                with context.wrap_socket(connection,server_side=True) as stream:
                    stream.settimeout(2);received.append(stream.recv(4096))
            except (ssl.SSLError,OSError):connection.close()
    thread=threading.Thread(target=backend);thread.start()
    config=base/'Caddyfile';config.write_text('''{\n admin off\n}\nhttp://127.0.0.1:13108 {\n reverse_proxy https://127.0.0.1:13107 {\n  transport http {\n   tls_trust_pool file /home/tealdrive/policy/origin-cert.pem\n   tls_server_name tealdrive-origin.local\n  }\n }\n}\n''')
    log=(base/'caddy.log').open('w')
    env={**os.environ,'XDG_CONFIG_HOME':str(base/'config'),'XDG_DATA_HOME':str(base/'data')}
    process=subprocess.Popen(['/usr/bin/caddy','run','--config',str(config),'--adapter','caddyfile'],stdout=log,stderr=log,env=env)
    try:
        status=None
        for _ in range(50):
            try:
                urllib.request.urlopen('http://127.0.0.1:13108/',timeout=2)
                raise RuntimeError('Untrusted backend accepted')
            except urllib.error.HTTPError as error:
                status=error.code;break
            except urllib.error.URLError:time.sleep(.1)
        if status!=502 or received:
            raise RuntimeError('TLS rejection did not prevent HTTP delivery')
        print(json.dumps({'untrustedSameNameBackendRejected':True,'proxyStatus':502,'httpRequestsDeliveredToFakeBackend':0}))
    finally:
        process.terminate();process.wait(timeout=10);log.close()
        stopped.set();listener.close();thread.join(timeout=3)
