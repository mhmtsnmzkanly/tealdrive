#!/usr/bin/env python3
"""Only fresh admin-acceptance fixture users: real UID, PAM, TTY enrollment/revoke."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import sys
import termios
import time

WIZARD = '/home/tealdrive/bin/tealdrive-wizard'

def call(user, *args, ok=True):
    result = subprocess.run(['/usr/sbin/runuser', '-u', user, '--', WIZARD, *args], capture_output=True, text=True)
    if (result.returncode == 0) != ok:
        raise RuntimeError(f'Local command {args} failed: {result.stderr[:400]}')
    return result

def enroll(user):
    master, slave = pty.openpty()
    def attach():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)
    proc = subprocess.Popen(['/usr/sbin/runuser', '-u', user['username'], '--', WIZARD, 'init'], stdin=slave, stdout=slave, stderr=slave, preexec_fn=attach)
    os.close(slave)
    prompts = [b'Linux password: ', b'New web password (12+ characters): ', b'Confirm web password: ']
    values = [user['linuxPassword'], user['password'], user['password']]
    output = b''
    index = 0
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            if select.select([master], [], [], .1)[0]:
                try:
                    block = os.read(master, 65536)
                except OSError:
                    break
                if not block:
                    break
                output += block
                if index < len(prompts) and prompts[index] in output:
                    # Read the controlling terminal's live flags before sending a secret.
                    assert not (termios.tcgetattr(master)[3] & termios.ECHO)
                    os.write(master, values[index].encode() + b'\n')
                    index += 1
            if proc.poll() is not None:
                break
        rc = proc.wait(timeout=5)
        if rc != 0 or index != 3:
            # Never print output: an unexpected TTY echo may contain a secret.
            raise RuntimeError('TTY enrollment failed; inspect behavior without logging credentials')
        assert all(value.encode() not in output for value in values)
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
        os.close(master)

if os.getuid() != 0 or os.environ.get('TEALDRIVE_LOCAL_WIZARD_ACCEPTANCE') != '1':
    raise SystemExit('Explicit Root fixture opt-in required')
manifest = Path(sys.argv[1])
assert manifest.parent.name.startswith('.admin-acceptance-') and manifest.stat().st_mode & 0o077 == 0
users = json.loads(manifest.read_text())['users']
user = users[1]
assert user['username'].startswith('tdacceptu')
call(user['username'], 'account', 'list', ok=False)
call(user['username'], 'service', 'stop', ok=False)
before = json.loads(call(user['username'], 'status').stdout)['AccountUpdated']
call(user['username'], 'policy', 'get')
call(user['username'], 'revoke')
enroll(user)
after = json.loads(call(user['username'], 'status').stdout)['AccountUpdated']
assert after['id'] == before['id'] and after['generation'] > before['generation']
assert after['role'] == before['role'] and after['uid'] == before['uid']
print('PASS: real UID own status/policy, Root management denial, self-revoke, PAM re-enrollment, password echo disabled, role/UID preserved')
