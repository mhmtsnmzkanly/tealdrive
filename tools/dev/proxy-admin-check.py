#!/usr/bin/env python3
"""Read-only Caddy admin isolation probe. Never emits configuration or credential values."""
import argparse
import json
import os
import pwd
import subprocess
import sys

PROBE = r'''
import json,socket,sys
family=socket.AF_UNIX if sys.argv[1]=='unix' else socket.AF_INET
target=sys.argv[2] if family==socket.AF_UNIX else ('127.0.0.1',int(sys.argv[2]))
with socket.socket(family,socket.SOCK_STREAM) as connection:
 connection.settimeout(2)
 try:
  connection.connect(target)
  connection.sendall(b'GET /config/ HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n')
  first=connection.recv(2048).split(b'\r\n',1)[0].split()
  print(json.dumps({'connected':True,'status':int(first[1]) if len(first)>1 else None}))
 except OSError as error:print(json.dumps({'connected':False,'errno':error.errno}))
'''

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--service-user', default='tealdrive')
    parser.add_argument('--unix-socket', default='/run/caddy-admin/admin.sock')
    args = parser.parse_args()
    if os.getuid() != 0 or os.geteuid() != 0:
        raise SystemExit('Real Root required to probe as the unprivileged service UID')
    service = pwd.getpwnam(args.service_user)
    if service.pw_uid == 0:
        raise SystemExit('Refusing a Root service identity')
    def probe(kind, address, as_service):
        command = [sys.executable, '-c', PROBE, kind, str(address)]
        if as_service:
            command = ['/usr/sbin/runuser', '-u', args.service_user, '--', *command]
        result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=5)
        return json.loads(result.stdout)
    tcp = probe('tcp', 2019, True)
    unix = probe('unix', args.unix_socket, True)
    root = probe('unix', args.unix_socket, False)
    passed = not tcp['connected'] and not unix['connected'] and root.get('status') == 200
    print(json.dumps({'serviceTcpDenied':not tcp['connected'],'serviceUnixDenied':not unix['connected'],'rootUnixConfigStatus':root.get('status'),'passed':passed}))
    if not passed:
        raise SystemExit(1)

if __name__ == '__main__':
    main()
