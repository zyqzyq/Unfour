"""Exercise the shipped POSIX protocol in a real shell, with stty mocked.

This verifies shell parsing, bytes, exit codes and pipe drain, NOT actual PTY
echo/signal semantics. Windows needs Git Bash; no packages are installed.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / 'crates/ssh-engine/src/task/terminal/protocol.rs').read_text(encoding='utf-8')
strings = re.findall(r'^\s*("(?:[^"\\]|\\.)*")\s*,?$', source.split('\n);', 1)[0], re.M)
bootstrap = ''.join(json.loads(value) for value in strings)
assert bootstrap.startswith(" sh -c '") and bootstrap.endswith("'\n")
bash = sys.argv[1] if len(sys.argv) > 1 else 'bash'

with tempfile.TemporaryDirectory(prefix='unfour-shell-', dir=ROOT / 'target') as temp:
    temp_path = Path(temp)
    (temp_path / 'stty').write_text('#!/bin/sh\n[ "$1" != -g ] || printf "mock-tty-state\\n"\nexit 0\n')
    os.chmod(temp_path / 'stty', 0o700)
    shell_temp = temp_path.as_posix()
    if os.name == 'nt':
        shell_temp = '/' + shell_temp[0].lower() + shell_temp[2:]
    env = dict(os.environ, PATH=shell_temp + ':/usr/bin:/bin', LC_ALL='C.UTF-8')
    subprocess.run([bash, '--noprofile', '--norc', '-n', '-c', bootstrap], env=env, check=True)
    proc = subprocess.Popen([bash, '--noprofile', '--norc', '-c', bootstrap], env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = bytearray()
    def read():
        while True:
            data = proc.stdout.read(1)
            if not data: return
            output.extend(data)
    threading.Thread(target=read, daemon=True).start()
    def send(data):
        proc.stdin.write(data); proc.stdin.flush()
    def await_bytes(data, start=0, timeout=5):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if data in bytes(output[start:]): return
            if proc.poll() is not None: break
            time.sleep(.01)
        raise AssertionError(f'missing {data!r}; output={bytes(output)!r}')
    def frame(value): return b'\x1e' + value.encode() + b'\x1f'
    try:
        await_bytes(frame('UNFOUR_TASK_V2'))
        send(b'qa-session\n'); await_bytes(frame('qa-session:READY'))
        cases = [
            ("printf 'stdout\\n'; printf 'stderr\\n' >&2; exit 23", 23, b'stderr\n'),
            ("printf '%s\\n' 'quoted \" value' '终端'; exit 0", 0, '终端\n'.encode()),
            ("printf '%s' '" + 'long-payload-' * 900 + "'", 0, b'long-payload-' * 900),
            ("printf 'foreground\\n'; (sleep 0.2; printf 'background-tail\\n') &", 0, b'background-tail\n'),
        ]
        for index, (command, rc, expected) in enumerate(cases):
            name = f'qa-step-{index}'
            start = len(output); send((name + '\n').encode()); await_bytes(frame(name + ':ACCEPT'), start)
            raw = command.encode()
            for offset in range(0, len(raw), 48):
                part = ''.join(f'\\0{byte:03o}' for byte in raw[offset:offset+48])
                send(('D' + part + '\n').encode())
                await_bytes(frame(f'{name}:INPUT:{offset // 48 + 1}:{len(part)}:{part}'), start)
            send(f'E:{(len(raw)+47)//48}:{len(raw)*5}\n'.encode()); await_bytes(frame(name + ':END'), start)
            data = bytes(output[start:])
            assert frame(name + f':STATUS:{rc}') in data and expected in data
            assert data.index(expected) < data.index(frame(name + ':END'))
        send(b'Q\n'); await_bytes(frame('qa-session:RESTORED'))
        assert proc.wait(timeout=3) == 0
        print('PASS: real shell syntax, octal/Unicode/long command, exit 0/23, sequential reuse and background output drain; stty mocked, PTY/signals NOT VERIFIED')
    finally:
        if proc.poll() is None: proc.kill()
        proc.wait(timeout=3)

    # A dropped transport must never execute a command whose final E frame
    # has not arrived. All input is synthetic and stdout is captured locally.
    partial = subprocess.Popen([bash, '--noprofile', '--norc', '-c', bootstrap], env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    command = b"printf 'SHOULD_NOT_EXECUTE\\n'"
    incomplete = b'qa-partial\nqa-partial-step\n' + ('D' + ''.join(f'\\0{byte:03o}' for byte in command) + '\n').encode()
    try:
        data, _ = partial.communicate(incomplete, timeout=5)
        assert partial.returncode == 125, (partial.returncode, data)
        assert b'SHOULD_NOT_EXECUTE' not in data and b':BEGIN' not in data, data
        print('PASS: payload EOF without explicit terminator does not execute a partial command')
    finally:
        if partial.poll() is None: partial.kill()
        partial.wait(timeout=3)

    # Even if the commit line arrives, a missing middle data line must not run.
    raw = b':' * 140
    segments = [b'D' + b''.join(f'\\0{b:03o}'.encode() for b in raw[i:i+48]) + b'\n'
                for i in range(0, len(raw), 48)]
    commit = f'E:{len(segments)}:{len(raw)*5}\n'.encode()
    for name, wire in [('missing-middle', segments[0] + segments[2] + commit),
                       ('unterminated-commit', b''.join(segments) + commit[:-1])]:
        result = subprocess.run([bash, '--noprofile', '--norc', '-c', bootstrap], env=env,
                                input=b'session\nstep\n' + wire, capture_output=True, timeout=5)
        assert result.returncode == 125 and frame('step:INPUT_INVALID') in result.stdout, name
        assert frame('step:BEGIN') not in result.stdout and frame('session:RESTORED') in result.stdout, name
        print('PASS:', name, 'rejected before execution')
