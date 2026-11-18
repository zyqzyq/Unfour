"""Verify shell protocol with real POSIX PTY input and exact tty restoration.

Only synthetic commands run by default. --command-file accepts an explicitly
prepared local test command; protocol bytes and decoded output stay in target/.
Remote stdout is captured before terminal output rendering: ConPTY consumes
protocol control characters. This tests the shell and tty input, not the Rust
driver's total deadline or SSH output path; native tests cover those separately.
"""
import argparse
import base64
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import re
import threading
import time
import uuid
from curl_qa_source import curl_probe_command

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--git', default=r'D:\Program Files\Git')
parser.add_argument('--command-file', type=Path)
parser.add_argument('--expected-output-file', type=Path)
parser.add_argument('--expected-exit', type=int, default=0)
parser.add_argument('--only-command', action='store_true')
parser.add_argument('--output-dir', type=Path)
parser.add_argument('--case', action='append', help='Run only the named synthetic case; repeat to select several')
args = parser.parse_args()
GIT = Path(args.git)
RUN = args.output_dir or ROOT / 'target/bastion-api-curl/qa' / ('raw-pty-' + uuid.uuid4().hex[:10])
RUN.mkdir(parents=True, exist_ok=False)
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
        self.history = history
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


protocol_source = ROOT / 'crates/ssh-engine/src/ssh/temporary/terminal_operation/protocol.rs'
source = protocol_source.read_text(encoding='utf-8')
block = source.split('pub(super) const BOOTSTRAP: &str = concat!(', 1)[1].split('\n);', 1)[0]
bootstrap = ''.join(json.loads(value) for value in re.findall(r'^\s*("(?:[^"\\]|\\.)*")\s*,?$', block, re.M))
assert bootstrap.startswith(" sh -c '") and bootstrap.endswith("'\n")
wire_source = (ROOT / 'crates/ssh-engine/src/ssh/terminal_wire.rs').read_text(encoding='utf-8')
input_chunk = int(re.search(r'const INPUT_CHUNK_BYTES: usize = (\d+);', wire_source).group(1))
startup = re.search(r'const STARTUP: &str = "([^"]+)";', source).group(1)
ready_capabilities = re.search(r'const READY_CAPABILITIES: &str = "([^"]+)";', source).group(1)


def operation(terminal, case, command, expected, exit_code, mode):
    protocol = case / 'protocol.bin'
    newline = b'\r\n' if mode.endswith('-crlf') else b'\r' if mode.endswith('-cr') else b'\n'
    def send_line(data):
        terminal.write(data.rstrip(b'\n') + newline)
    started_at = time.monotonic()
    # Keep the product's LF bootstrap. Alternate endings below exercise the
    # acknowledged child protocol, not an arbitrary parent's input settings.
    terminal.write((bootstrap.rstrip('\n') + f' > {quoted(protocol)} 2>&1\n').encode())
    wait(lambda: frame(startup) in content(protocol), 'raw bootstrap did not establish PTY')
    session, step = uuid.uuid4().hex, uuid.uuid4().hex
    send_line((session + '\n').encode())
    octal = mode == 'octal-fallback'
    capabilities = 'READY:3:240:octal' if octal else ready_capabilities
    wait(lambda: frame(session + ':' + capabilities) in content(protocol), 'capability handshake missing')
    send_line((step + '\n').encode())
    wait(lambda: frame(step + ':ACCEPT') in content(protocol), 'operation accept missing')
    raw, sent_lines = command.encode(), []
    chunk_size = 48 if octal else input_chunk
    for offset in range(0, len(raw), chunk_size):
        chunk = raw[offset:offset+chunk_size]
        line = b'D' + (b"".join(f"\\0{b:03o}".encode() for b in chunk) if octal else base64.b64encode(chunk))
        sent_lines.append(line)
        assert len(line) + 1 <= 242
        send_line(line + b'\n')
        count = offset // chunk_size + 1
        ack = frame(step + f':INPUT:{count}:{len(line)-1}:' + line[1:].decode())
        wait(lambda: ack in content(protocol), 'exact input acknowledgement missing')
    total = f'{len(sent_lines)}:{sum(len(line)-1 for line in sent_lines)}'
    send_line(f'E:{total}\n'.encode())
    wait(lambda: frame(step + f':INPUT_END:{total}') in content(protocol), 'final input acknowledgement missing')
    wait(lambda: frame(step + ':BEGIN') in content(protocol), 'operation begin missing')
    cancel = mode.startswith('cancel')
    decoded = b''
    if cancel:
        if mode != 'cancel-immediate':
            time.sleep(.5)
        terminal.write(b'\x03')
    else:
        wait(lambda: frame(step + ':END:0') in content(protocol), 'raw operation did not drain/finish', 45)
        encoded = content(protocol).split(frame(step + ':BEGIN'), 1)[1].split(frame(step + ':END:0'), 1)[0]
        decoded = base64.b64decode(encoded.replace(b'\r', b'').replace(b'\n', b''), validate=True)
        status = frame(step + f':STATUS:{exit_code}')
        assert decoded.count(status) == 1, 'trusted command exit missing'
        decoded = decoded.replace(status, b'', 1)
        (case / 'stdout.bin').write_bytes(decoded)
        if expected is not None:
            assert decoded == expected, 'raw output bytes changed'
        send_line(b'Q\n')
    restore_started = time.monotonic()
    try:
        wait(lambda: frame(session + ':RESTORED') in content(protocol), 'tty restoration frame missing', 3)
    except AssertionError:
        if mode != 'cancel-immediate':
            raise
        # BEGIN identifies the execution child, but an immediate interrupt may
        # still be delayed by shell/process startup. Record the limitation; do
        # not send a second interrupt or write a parent-liveness command.
        return None, {'case': mode, 'result': 'OBSERVED_LIMITATION',
            'outcome': 'RESTORE_UNCONFIRMED', 'restoreBudgetSeconds': 3,
            'restorationConfirmed': False, 'parentInputSent': False,
            'automaticRetry': False, 'requiredProductBehavior': 'quarantine',
            'productionQuarantineExercisedByThisHarness': False}
    restore_elapsed = time.monotonic() - restore_started
    history, after, alive = terminal.history, case / 'after', case / 'alive'
    # An intentionally inherited igncr discards CR after restoration. Use LF
    # solely for this parent-liveness check; protocol sends remain unchanged.
    parent_newline = '\n' if mode == 'binary-igncr' else '\r'
    terminal.write((f"stty -g > {quoted(after)}; printf PARENT_ALIVE > {quoted(alive)}; history -w" + parent_newline).encode())
    wait(lambda: content(alive) == b'PARENT_ALIVE' and content(after), 'parent shell did not resume')
    history_bytes, console = content(history), bytes(terminal.output)
    assert session.encode() not in history_bytes and session.encode() not in console
    assert b'UNFOUR_RAW_SECRET_42' not in history_bytes and b'UNFOUR_RAW_SECRET_42' not in console
    assert all(line not in history_bytes and line not in console for line in sent_lines), 'encoded payload echo/history leak'
    if len(command) > 32:
        assert command.encode() not in history_bytes and command.encode() not in console
    return decoded, {'case': mode, 'result': 'PASS', 'realPosixTtyInput': True,
        'bootstrapInputNewline': 'LF', 'protocolInputNewline': newline.hex(), 'everyInputLineAndEndAcknowledged': True,
        'exactInputContentAcknowledged': True, 'payloadEchoOrHistory': False,
        'parentResumed': True, 'stdoutBytes': None if cancel else len(decoded), 'exitCode': exit_code,
        'elapsedMs': round((time.monotonic()-started_at)*1000),
        'restoreElapsedMs': round(restore_elapsed*1000), 'restoreBudgetSeconds': 3}

cases = [] if args.only_command else [
    ('binary', "printf 'UNFOUR_RAW_SECRET_42\\n\\001\\036\\037\\377'; exit 23", b'UNFOUR_RAW_SECRET_42\n\x01\x1e\x1f\xff', 23),
    ('octal-fallback', "printf 'verified fallback'", b'verified fallback', 0),
    ('large', "printf '%0150000d' 0; exit 0", b'0' * 150000, 0),
    ('background', "printf head; (sleep 0.2; printf tail) &", b'headtail', 0),
    ('cancel', "printf WAITING; sleep 30", None, None),
    ('cancel-noflsh', "printf WAITING; sleep 30", None, None),
    ('cancel-intr-x', "printf WAITING; sleep 30", None, None),
    ('cancel-intr-disabled', "printf WAITING; sleep 30", None, None),
    ('cancel-immediate', "printf WAITING; sleep 5", None, None),
    ('binary-after-startup-cr', "printf 'UNFOUR_RAW_SECRET_42\\n\\001\\036\\037\\377'; exit 23", b'UNFOUR_RAW_SECRET_42\n\x01\x1e\x1f\xff', 23),
    ('binary-after-startup-crlf', "printf 'UNFOUR_RAW_SECRET_42\\n\\001\\036\\037\\377'; exit 23", b'UNFOUR_RAW_SECRET_42\n\x01\x1e\x1f\xff', 23),
    ('binary-inlcr', "printf 'UNFOUR_RAW_SECRET_42\\n\\001\\036\\037\\377'; exit 23", b'UNFOUR_RAW_SECRET_42\n\x01\x1e\x1f\xff', 23),
    ('binary-igncr', "printf 'UNFOUR_RAW_SECRET_42\\n\\001\\036\\037\\377'; exit 23", b'UNFOUR_RAW_SECRET_42\n\x01\x1e\x1f\xff', 23),
]
if not args.only_command:
    probe_command = curl_probe_command(ROOT)
    cases.append(('curl-tools', probe_command, b'UNFOUR_CURL_TOOLS', 0))
    transport_text = 'UNFOUR_TRANSPORT_ROUNDTRIP:' + '0123456789abcdef' * 60
    transport_command = f"command printf '%s' '{transport_text}'; command printf '\\n\\000\\001\\036\\037\\177\\377'; exit 23"
    transport_expected = transport_text.encode() + b'\n\x00\x01\x1e\x1f\x7f\xff'
    cases.append(('transport-then-curl', transport_command, transport_expected, 23))
    # Strict CLI parsing must reject unknown options before --version. Old curl
    # config parsing is deliberately NOT equivalent; compat QA covers that bug.
    cases.append(('curl-invalid-option', "command curl -q --unfour-invalid-option --version >/dev/null 2>&1; rc=$?; [ \"$rc\" -ne 0 ]", b'', 0))
if args.command_file:
    command = args.command_file.read_text(encoding='utf-8')
    expected = args.expected_output_file.read_bytes() if args.expected_output_file else None
    cases.append(('command-file', command, expected, args.expected_exit))
assert cases, '--only-command requires --command-file'
if args.case:
    cases = [case for case in cases if case[0] in args.case]
    assert len(cases) == len(set(args.case)), 'unknown or duplicate selected case'
results = []
for mode, command, expected, exit_code in cases:
    cancel = mode.startswith('cancel')
    case = RUN / mode
    case.mkdir()
    history, protocol = case / 'history', case / 'protocol.bin'
    before, after, alive = case / 'before', case / 'after', case / 'alive'
    terminal = Terminal(history)
    try:
        time.sleep(.5)
        tty_setup = {
            'cancel-noflsh': 'stty noflsh; ',
            'cancel-intr-x': 'stty intr ^X; ',
            'cancel-intr-disabled': 'stty intr undef; ',
            'binary-inlcr': 'stty -icrnl inlcr; ',
            'binary-igncr': 'stty igncr; ',
        }.get(mode, '')
        if mode == 'octal-fallback':
            # A decoder can print the right sample and still report failure.
            wrapper = case / 'base64'
            wrapper.write_text('#!/bin/sh\nif [ "$1" = -d ]; then printf x; exit 1; fi\nexec /usr/bin/base64 "$@"\n', encoding='utf-8', newline='\n')
            tty_setup += f'export PATH={quoted(case)}:"$PATH"; '
        terminal.write((f"export PATH=/mingw64/bin:/usr/bin:/bin; {tty_setup}stty -g > {quoted(before)}; test -t 0 && printf PTY_OK > {quoted(alive)}\r").encode())
        wait(lambda: content(alive) == b'PTY_OK', 'real POSIX tty is required')
        decoded, result = operation(terminal, case, command, expected, exit_code, mode)
        if result['result'] == 'OBSERVED_LIMITATION':
            results.append(result)
            print('OBSERVED_LIMITATION:', mode, 'restore unconfirmed at 3s; no further input sent', flush=True)
            continue
        assert content(before) == content(after), 'tty modes did not restore exactly'
        result['exactTtyRestore'] = True
        if mode == 'transport-then-curl':
            second = case / 'second-operation'
            second.mkdir()
            _, second_result = operation(terminal, second, probe_command, b'UNFOUR_CURL_TOOLS', 0, 'curl-tools-same-terminal')
            assert content(before) == content(second / 'after'), 'second operation changed original tty'
            second_result['exactTtyRestore'] = True
            result['secondOperation'] = second_result
            result['sameParentTerminalReused'] = True
        if mode == 'command-file':
            # Production HTTP preparation/parsing; the Python transport is not
            # the production Rust driver and does not prove its total deadline.
            response_path = args.command_file.parent / 'response.bin'
            response_pending = args.command_file.parent / 'response.pending'
            response_pending.write_bytes(decoded)
            os.replace(response_pending, response_path)
            exit_pending = args.command_file.parent / 'exit-code.pending'
            exit_pending.write_text(str(exit_code), encoding='ascii')
            os.replace(exit_pending, args.command_file.parent / 'exit-code.txt')
        results.append(result)
        print('PASS:', mode, 'real tty input, exact restoration, no payload echo/history, parent resumed', flush=True)
    finally:
        # write_bytes can retain a memoryview during I/O; snapshot first so the
        # reader thread can continue appending without BufferError.
        (case / 'console.bin').write_bytes(bytes(terminal.output))
        terminal.close()
(RUN / 'result.json').write_text(json.dumps({'protocolSourceSha256': hashlib.sha256(protocol_source.read_bytes()).hexdigest(), 'coverage': {'realPosixTtyInput': True, 'stdoutCaptureBeforePtyRendering': True, 'productionRustDriver': False, 'productionTotalDeadline': False, 'customerBastion': False}, 'cases': results}, indent=2) + '\n')
print('Evidence:', RUN)
