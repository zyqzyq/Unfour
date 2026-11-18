"""Exercise the shipped shell protocol on a real local Git Bash PTY.

Windows only; requires Git's existing winpty/bash and Python stdlib. No network,
packages, credentials or customer data. The outer ConPTY hosts winpty, which
provides the inner MSYS POSIX PTY. Protocol stdout goes to a scratch file so
Windows console rendering cannot consume RS/US framing bytes.
"""
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
GIT = Path(sys.argv[1] if len(sys.argv) > 1 else r'D:\Program Files\Git')
QA = ROOT / 'target/bastion-terminal-task/qa'
RUN = QA / ('pty-' + uuid.uuid4().hex[:10])
RUN.mkdir(parents=True)
K = c.WinDLL('kernel32', use_last_error=True)


class Coord(c.Structure):
    _fields_ = [('x', c.c_short), ('y', c.c_short)]


class Startup(c.Structure):
    _fields_ = [('cb', w.DWORD), ('reserved', w.LPWSTR), ('desktop', w.LPWSTR), ('title', w.LPWSTR),
                ('x', w.DWORD), ('y', w.DWORD), ('xs', w.DWORD), ('ys', w.DWORD),
                ('xc', w.DWORD), ('yc', w.DWORD), ('fill', w.DWORD), ('flags', w.DWORD),
                ('show', w.WORD), ('cbReserved2', w.WORD), ('reserved2', c.c_void_p),
                ('stdin', w.HANDLE), ('stdout', w.HANDLE), ('stderr', w.HANDLE)]


class StartupEx(c.Structure):
    _fields_ = [('info', Startup), ('attributes', c.c_void_p)]


class Process(c.Structure):
    _fields_ = [('process', w.HANDLE), ('thread', w.HANDLE), ('pid', w.DWORD), ('tid', w.DWORD)]


K.CreatePipe.argtypes = [c.POINTER(w.HANDLE), c.POINTER(w.HANDLE), c.c_void_p, w.DWORD]
K.CreatePseudoConsole.argtypes = [Coord, w.HANDLE, w.HANDLE, w.DWORD, c.POINTER(w.HANDLE)]
K.CreatePseudoConsole.restype = c.c_long
K.InitializeProcThreadAttributeList.argtypes = [c.c_void_p, w.DWORD, w.DWORD, c.POINTER(c.c_size_t)]
K.UpdateProcThreadAttribute.argtypes = [c.c_void_p, w.DWORD, c.c_size_t, c.c_void_p, c.c_size_t, c.c_void_p, c.c_void_p]
K.DeleteProcThreadAttributeList.argtypes = [c.c_void_p]
K.CreateProcessW.argtypes = [w.LPCWSTR, w.LPWSTR, c.c_void_p, c.c_void_p, w.BOOL, w.DWORD, c.c_void_p, w.LPCWSTR, c.c_void_p, c.POINTER(Process)]
K.ReadFile.argtypes = [w.HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p]
K.WriteFile.argtypes = [w.HANDLE, c.c_void_p, w.DWORD, c.POINTER(w.DWORD), c.c_void_p]
K.CloseHandle.argtypes = [w.HANDLE]
K.ClosePseudoConsole.argtypes = [w.HANDLE]
K.TerminateProcess.argtypes = [w.HANDLE, w.UINT]
K.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]


def shell_path(path):
    value = path.resolve().as_posix()
    return '/' + value[0].lower() + value[2:]


def quoted(path):
    return "'" + shell_path(path).replace("'", "'\\''") + "'"


class Terminal:
    def __init__(self, history):
        ri, wi, ro, wo = w.HANDLE(), w.HANDLE(), w.HANDLE(), w.HANDLE()
        assert K.CreatePipe(c.byref(ri), c.byref(wi), None, 0)
        assert K.CreatePipe(c.byref(ro), c.byref(wo), None, 0)
        self.console = w.HANDLE()
        assert K.CreatePseudoConsole(Coord(180, 40), ri, wo, 0, c.byref(self.console)) == 0
        K.CloseHandle(ri); K.CloseHandle(wo)
        size = c.c_size_t()
        K.InitializeProcThreadAttributeList(None, 1, 0, c.byref(size))
        self.attrs = c.create_string_buffer(size.value)
        assert K.InitializeProcThreadAttributeList(self.attrs, 1, 0, c.byref(size))
        assert K.UpdateProcThreadAttribute(self.attrs, 0, 0x00020016, self.console, c.sizeof(w.HANDLE), None, None)
        startup = StartupEx(); startup.info.cb = c.sizeof(startup); startup.attributes = c.addressof(self.attrs)
        self.process = Process()
        env = dict(os.environ, PATH=str(GIT / 'usr/bin') + ';' + os.environ['PATH'], TERM='dumb',
                   HISTFILE=shell_path(history), HISTCONTROL='', HISTSIZE='100', PS1='QA_PARENT> ')
        env_block = c.create_unicode_buffer('\0'.join(key + '=' + value for key, value in sorted(env.items())) + '\0\0')
        # The native launcher must replace inherited redirected std handles
        # with this ConPTY's console handles before starting the MSYS process.
        command = c.create_unicode_buffer(
            f'"{Path(os.environ["SystemRoot"]) / "System32/cmd.exe"}" /d /s /c '
            f'""{GIT / "usr/bin/winpty.exe"}" -Xallow-non-tty '
            f'"{GIT / "usr/bin/bash.exe"}" --noprofile --norc -i < CONIN$ > CONOUT$ 2>&1"')
        assert K.CreateProcessW(None, command, None, None, False, 0x00080000 | 0x400,
                                env_block, str(ROOT), c.byref(startup), c.byref(self.process)), c.get_last_error()
        self.output = bytearray(); self.input = wi; self.reader = ro
        def read():
            buf = c.create_string_buffer(4096); count = w.DWORD()
            while K.ReadFile(ro, buf, len(buf), c.byref(count), None) and count.value:
                data = buf.raw[:count.value]; self.output.extend(data)
                if b'\x1b[6n' in data:
                    self.write(b'\x1b[1;1R')
        threading.Thread(target=read, daemon=True).start()

    def write(self, data):
        count = w.DWORD()
        assert K.WriteFile(self.input, data, len(data), c.byref(count), None)
        assert count.value == len(data)

    def close(self):
        K.TerminateProcess(self.process.process, 0)
        K.ClosePseudoConsole(self.console)
        K.DeleteProcThreadAttributeList(self.attrs)
        for handle in [self.input, self.reader, self.process.process, self.process.thread]:
            K.CloseHandle(handle)


def wait(check, description, seconds=10):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if check():
            return
        time.sleep(.02)
    raise AssertionError(description)


def content(path):
    return path.read_bytes() if path.exists() else b''


def frame(value):
    return b'\x1e' + value.encode() + b'\x1f'


protocol_source = ROOT / 'crates/ssh-engine/src/task/terminal/protocol.rs'
source = protocol_source.read_text(encoding='utf-8')
strings = re.findall(r'^\s*("(?:[^"\\]|\\.)*")\s*,?$', source.split('\n);', 1)[0], re.M)
bootstrap = ''.join(json.loads(value) for value in strings)
assert bootstrap.startswith(" sh -c '") and bootstrap.endswith("'\n")
results = []
for mode in ['success', 'cancel', 'cancel-noflsh']:
    cancel = mode != 'success'
    case = RUN / mode; case.mkdir()
    history, protocol = case / 'history', case / 'protocol.bin'
    before, after, alive = case / 'before', case / 'after', case / 'alive'
    terminal = Terminal(history)
    try:
        time.sleep(.5)
        tty_setup = 'stty noflsh; ' if mode == 'cancel-noflsh' else ''
        terminal.write((f"export PATH=/usr/bin:/bin; {tty_setup}stty -g > {quoted(before)}; test -t 0 && printf PTY_OK > {quoted(alive)}\r").encode())
        wait(lambda: content(alive) == b'PTY_OK', 'real POSIX tty is required')
        terminal.write((bootstrap.rstrip('\n') + f' > {quoted(protocol)} 2>&1\r').encode())
        wait(lambda: frame('UNFOUR_TASK_V2') in content(protocol), 'bootstrap did not establish a PTY')
        session = 'qa-' + uuid.uuid4().hex
        terminal.write((session + '\r').encode())
        wait(lambda: frame(session + ':READY') in content(protocol), 'random handshake missing')
        json_record = r"""printf '\036{"ok":true}\n'"""
        commands = [("printf 'UNFOUR_PTY_SECRET_42\\n'; printf 'stderr\\n' >&2; exit 23", 23),
                    ("printf 'foreground\\n'; (sleep 0.2; printf 'background-tail\\n') &", 0)]
        commands.extend((f'{json_record}; exit {rc}', rc) for rc in (0, 23))
        if cancel:
            commands = [("stty -a </dev/tty; printf 'WAITING\\n'; sleep 30", None)]
        sent_lines = []
        for command, exit_code in commands:
            step = 'step-' + uuid.uuid4().hex
            terminal.write((step + '\r').encode())
            wait(lambda: frame(step + ':ACCEPT') in content(protocol), 'step accept missing')
            raw = command.encode()
            for offset in range(0, len(raw), 48):
                line = ('D' + ''.join(f'\\0{byte:03o}' for byte in raw[offset:offset+48])).encode()
                sent_lines.append(line)
                terminal.write(line + b'\r')
                ack = frame(f'{step}:INPUT:{offset // 48 + 1}:{len(line)-1}:' + line[1:].decode())
                wait(lambda: ack in content(protocol), 'input confirmation missing')
            terminal.write(f'E:{(len(raw)+47)//48}:{len(raw)*5}\r'.encode())
            wait(lambda: frame(step + ':BEGIN') in content(protocol), 'step begin missing')
            if cancel:
                wait(lambda: b'WAITING' in content(protocol), 'wait command did not start')
                assert b'-noflsh' in content(protocol), 'task must flush queued input on signals'
                # Exercise interruption of the running external command. An
                # earlier printf->exec race is a separate quarantine case.
                time.sleep(.25)
                terminal.write(b'\x03')
            else:
                wait(lambda: frame(step + ':END') in content(protocol), 'step did not drain/finish')
                assert frame(step + f':STATUS:{exit_code}') in content(protocol)
                if command.startswith(json_record):
                    expected = (frame(step + ':BEGIN') + b'\x1e{"ok":true}\n'
                                + frame(step + f':STATUS:{exit_code}') + frame(step + ':END'))
                    assert expected in content(protocol), 'RS-prefixed JSON and correlated result out of order'
                if 'background' in command:
                    data = content(protocol)
                    assert data.index(b'background-tail') < data.index(frame(step + ':END'))
        if not cancel:
            terminal.write(b'Q\r')
        wait(lambda: frame(session + ':RESTORED') in content(protocol), 'tty restoration frame missing', 5)
        terminal.write((f"stty -g > {quoted(after)}; printf PARENT_ALIVE > {quoted(alive)}; history -w\r").encode())
        wait(lambda: content(alive) == b'PARENT_ALIVE' and content(after), 'parent shell did not resume')
        assert content(before) == content(after), 'tty modes did not restore exactly'
        history_bytes, console = content(history), bytes(terminal.output)
        assert session.encode() not in history_bytes and session.encode() not in console
        assert b'UNFOUR_PTY_SECRET_42' not in history_bytes and b'UNFOUR_PTY_SECRET_42' not in console
        assert all(line not in history_bytes and line not in console for line in sent_lines), 'encoded payload echo/history leak'
        results.append({'case': case.name, 'result': 'PASS', 'realPosixTty': True,
                        'exactTtyRestore': True, 'payloadEchoOrHistory': False, 'parentResumed': True})
        print('PASS:', case.name, 'real PTY, exact tty restoration, no payload echo/history, parent shell resumed')
    finally:
        (case / 'console.bin').write_bytes(terminal.output)
        terminal.close()
(RUN / 'result.json').write_text(json.dumps({
    'protocolSourceSha256': hashlib.sha256(protocol_source.read_bytes()).hexdigest(),
    'cases': results,
}, indent=2) + '\n')
print('Evidence:', RUN)
