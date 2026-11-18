"""Compare real POSIX PTY input bytes and bounded synthetic relay behavior.

Requires Git for Windows/winpty and a saved pre-change protocol.rs supplied with
--baseline-protocol. The relay's burst capacity and inherited INTR=Ctrl-X are
deliberate fault injections, not evidence of a customer's gateway configuration.
Only the fixed no-network capability probe and a cancellable sleep are executed.
Normal cancellation waits for pipeline startup. V2 emits BEGIN from the execution
child, but immediate cancellation can still exceed the three-second restoration
budget in real shells. Preserve that failure evidence; production must isolate
the uncertain terminal and avoid retries or additional input.
"""
from pathlib import Path
import argparse
import sys
import uuid as id_module

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline-protocol', type=Path, required=True)
parser.add_argument('--git', default=r'D:\Program Files\Git')
parser.add_argument('--output-dir', type=Path)
parser.add_argument('--case', action='append', help='Run only the named case; repeat to select several')
cli = parser.parse_args()
fixture = Path(__file__).with_name('verify-curl-terminal-pty.py')
output_dir = cli.output_dir or ROOT / 'target/bastion-begin-timeout' / ('relay-' + id_module.uuid4().hex[:10])
sys.argv = [str(fixture), '--git', cli.git, '--output-dir', str(output_dir)]
# Reuse the existing ConPTY/winpty launcher without running its test cases.
namespace = {'__file__': str(fixture), '__name__': '__qa_input_relay__'}
exec(compile(fixture.read_text(encoding='utf-8').split('\ncases = ', 1)[0], str(fixture), 'exec'), namespace)
globals().update({key: value for key, value in namespace.items() if not key.startswith('__')})


def extract(source, name):
    block = source.split(f'const {name}: &str = concat!(', 1)[1].split('\n);', 1)[0]
    return ''.join(json.loads(value) for value in re.findall(r'^\s*("(?:[^"\\]|\\.)*")\s*,?$', block, re.M))


old_bytes = cli.baseline_protocol.read_bytes()
current_bytes = protocol_source.read_bytes()
old_bootstrap = extract(old_bytes.decode().replace('\r\n', '\n'), 'BOOTSTRAP')
current_source = current_bytes.decode().replace('\r\n', '\n')
new_bootstrap = extract(current_source, 'BOOTSTRAP')
chunk_bytes = input_chunk
assert chunk_bytes == 180 and 'INPUT_END' in new_bootstrap and 'INPUT_INVALID' in new_bootstrap
assert 'u_lines%8' in old_bootstrap, 'baseline must be the eight-line ACK protocol'
probe_source = protocol_source.parent / 'probe.rs'
probe_bytes = probe_source.read_bytes()
probe_command = curl_probe_command(ROOT)
(RUN / 'baseline-protocol.rs').write_bytes(old_bytes)
(RUN / 'current-protocol.rs').write_bytes(current_bytes)
(RUN / 'probe.rs').write_bytes(probe_bytes)
results = []


def seen(path, marker, seconds=10):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if marker in content(path):
            return True
        time.sleep(.01)
    return marker in content(path)


def translate(data, mode):
    if mode == 'CR':
        return data.replace(b'\n', b'\r')
    if mode == 'CRLF':
        return data.replace(b'\n', b'\r\n')
    return data


def capture_console(terminal, case):
    # Snapshot before file I/O: the reader thread must remain free to resize.
    (case / 'console.bin').write_bytes(bytes(terminal.output))


# Check the bytes at the actual POSIX tty, beyond ConPTY/winpty's key translation.
for label, data in [('LF', b'\n'), ('CR', b'\r'), ('CRLF', b'\r\n')]:
    if cli.case and 'wire-' + label not in cli.case:
        continue
    case = RUN / ('wire-' + label)
    case.mkdir()
    terminal = Terminal(case / 'history')
    ready, captured = case / 'ready', case / 'captured.bin'
    try:
        time.sleep(.4)
        shell = f'export PATH=/mingw64/bin:/usr/bin:/bin; qa_tty=$(stty -g); stty raw -echo; printf READY > {quoted(ready)}; dd bs=1 count={len(data)} of={quoted(captured)} 2>/dev/null; stty "$qa_tty"; printf DONE > {quoted(ready)}\r'
        terminal.write(shell.encode())
        wait(lambda: content(ready) == b'READY', 'wire capture ready')
        terminal.write(data)
        wait(lambda: content(ready) == b'DONE', 'wire capture done', 3)
        result = {'case': 'wire-' + label, 'sentHex': data.hex(), 'posixPtyReceivedHex': content(captured).hex(), 'pass': content(captured) == data}
        results.append(result)
        print(json.dumps(result), flush=True)
    finally:
        capture_console(terminal, case)
        terminal.close()


# protocol, newline, relay burst capacity, inherited Ctrl-X, injected action.
cases = [
    ('old-direct-lf', 'old', 'LF', None, False, None),
    ('old-direct-cr', 'old', 'CR', None, False, None),
    ('old-direct-crlf', 'old', 'CRLF', None, False, None),
    ('old-relay-2048', 'old', 'LF', 2048, False, None),
    ('old-relay-2048-intr-x', 'old', 'LF', 2048, True, None),
    ('new-relay-2048', 'new', 'LF', 2048, False, None),
    ('new-relay-256-intr-x', 'new', 'LF', 256, True, None),
    ('new-relay-crlf', 'new', 'CRLF', 256, False, None),
    ('new-cancel-intr-x', 'new', 'LF', 256, True, 'cancel'),
    ('new-cancel-immediate', 'new', 'LF', 256, False, 'cancel-immediate'),
    ('new-truncated-line', 'new', 'LF', 256, True, 'truncate-line'),
    ('new-mutated-line', 'new', 'LF', 256, True, 'mutate-line'),
    ('new-invalid-final-count', 'new', 'LF', 256, True, 'invalid-final'),
]
for label, version, newline, capacity, custom_intr, action in cases:
    if cli.case and label not in cli.case:
        continue
    case = RUN / label
    case.mkdir()
    terminal = Terminal(case / 'history')
    before, after, alive, protocol = [case / name for name in ['before', 'after', 'alive', 'protocol.bin']]
    correlation, operation = uuid.uuid4().hex, uuid.uuid4().hex
    command = 'printf WAITING; sleep 5' if action and action.startswith('cancel') else probe_command
    raw = command.encode()
    chunk_size = 96 if version == 'old' else chunk_bytes
    lines = [('D' + ''.join(f'\\0{byte:03o}' for byte in raw[offset:offset+chunk_size])).encode() + b'\n'
             for offset in range(0, len(raw), chunk_size)]
    if version == 'new':
        lines = [b'D' + base64.b64encode(raw[offset:offset+chunk_size]) + b'\n'
                 for offset in range(0, len(raw), chunk_size)]
    total_chars = sum(len(line)-2 for line in lines)
    declared_chars = total_chars + (1 if action == 'invalid-final' else 0)
    end_line = b'E\n' if version == 'old' else f'E:{len(lines)}:{declared_chars}\n'.encode()
    phase, outcome, dropped, outstanding = 'STARTUP', None, 0, 0
    restored, rescue_restored, decoded_ok, cancel_sent = False, False, False, False
    input_acks = 0
    try:
        time.sleep(.4)
        intr = 'stty intr ^X; ' if custom_intr else ''
        terminal.write((f'export PATH=/mingw64/bin:/usr/bin:/bin; {intr}stty -g > {quoted(before)}; stty -a > {quoted(case / "tty-description")}; printf READY > {quoted(alive)}\r').encode())
        wait(lambda: content(alive) == b'READY', 'setup complete')
        shell = old_bootstrap if version == 'old' else new_bootstrap
        wire = (shell.rstrip('\n') + f' > {quoted(protocol)} 2>&1\n').encode()
        for offset in range(0, len(wire), 512):
            terminal.write(translate(wire[offset:offset+512], newline if version == 'old' else 'LF'))
        if not seen(protocol, frame('UNFOUR_RAW_V1' if version == 'old' else startup)):
            raise RuntimeError('STARTUP_TIMEOUT')
        phase = 'READY'
        terminal.write(translate((correlation + '\n').encode(), newline))
        ready = 'READY' if version == 'old' else ready_capabilities
        if not seen(protocol, frame(correlation + ':' + ready)):
            raise RuntimeError('READY_TIMEOUT')
        phase = 'ACCEPT'
        terminal.write(translate((operation + '\n').encode(), newline))
        if not seen(protocol, frame(operation + ':ACCEPT')):
            raise RuntimeError('ACCEPT_TIMEOUT')
        phase = 'INPUT_ACK'
        for index, original_line in enumerate(lines + [end_line]):
            forwarded = original_line
            if action == 'truncate-line' and index == 0:
                forwarded = original_line[:-5] + b'\n'
            if action == 'mutate-line' and index == 0:
                # Change one encoded byte without changing length or syntax.
                replacement = b'0' if original_line[5:6] != b'0' else b'1'
                forwarded = original_line[:5] + replacement + original_line[6:]
            if capacity is not None:
                allowed = max(0, capacity - outstanding)
                retained = forwarded[:allowed]
                dropped += len(forwarded) - len(retained)
                forwarded = retained
                outstanding += len(forwarded)
            if forwarded:
                terminal.write(translate(forwarded, newline))
            if original_line.startswith(b'D'):
                count = index + 1
                if version == 'new':
                    expected = frame(operation + f':INPUT:{count}:{len(original_line)-2}:' + original_line[1:-1].decode())
                    if action in ('truncate-line', 'mutate-line') and index == 0:
                        actual = frame(operation + f':INPUT:1:{len(forwarded)-2}:' + forwarded[1:-1].decode())
                        assert seen(protocol, actual)
                        assert expected not in content(protocol)
                        raise RuntimeError('INPUT_ACK_MISMATCH')
                    if not seen(protocol, expected):
                        raise RuntimeError('INPUT_ACK_TIMEOUT')
                    input_acks += 1
                    outstanding = 0
                elif count % 8 == 0:
                    if not seen(protocol, frame(operation + f':INPUT:{count}')):
                        raise RuntimeError('INPUT_ACK_TIMEOUT')
                    input_acks += 1
                    outstanding = 0
        if version == 'new':
            phase = 'INPUT_END'
            if action == 'invalid-final':
                assert seen(protocol, frame(operation + ':INPUT_INVALID'))
                raise RuntimeError('INPUT_INVALID')
            if not seen(protocol, frame(operation + f':INPUT_END:{len(lines)}:{total_chars}')):
                raise RuntimeError('INPUT_END_TIMEOUT')
            outstanding = 0
        phase = 'BEGIN'
        if not seen(protocol, frame(operation + ':BEGIN')):
            raise RuntimeError('BEGIN_TIMEOUT')
        if action and action.startswith('cancel'):
            if action != 'cancel-immediate':
                time.sleep(.5)
            terminal.write(b'\x03')
            cancel_sent = True
            restored = seen(protocol, frame(correlation + ':RESTORED'), 3)
            outcome = 'CANCELLED_RESTORED' if restored else 'RESTORE_TIMEOUT'
        else:
            phase = 'END'
            if not seen(protocol, frame(operation + ':END:0'), 5):
                raise RuntimeError('END_TIMEOUT')
            encoded = content(protocol).split(frame(operation + ':BEGIN'), 1)[1].split(frame(operation + ':END:0'), 1)[0]
            decoded = base64.b64decode(encoded.replace(b'\r', b'').replace(b'\n', b''), validate=True)
            decoded_ok = decoded == b'UNFOUR_CURL_TOOLS' + frame(operation + ':STATUS:0')
            terminal.write(translate(b'Q\n', newline))
            restored = seen(protocol, frame(correlation + ':RESTORED'), 3)
            outcome = 'PASS' if decoded_ok and restored else 'RESULT_OR_RESTORE_FAILURE'
    except RuntimeError as failure:
        outcome = str(failure)
        if outcome != 'INPUT_INVALID' and phase not in ('STARTUP', 'READY'):
            terminal.write(b'\x03')
            cancel_sent = True
        restored = seen(protocol, frame(correlation + ':RESTORED'), 3)
        if custom_intr and not restored:
            terminal.write(b'\x18')
            rescue_restored = seen(protocol, frame(correlation + ':RESTORED'), 3)
    finally:
        parent_alive, tty_restored = False, False
        if restored or rescue_restored:
            terminal.write((f'stty -g > {quoted(after)}; printf PARENT_ALIVE > {quoted(alive)}\r').encode())
            parent_alive = seen(alive, b'PARENT_ALIVE', 3)
            tty_restored = content(before) == content(after)
        received_begin = frame(operation + ':BEGIN') in content(protocol)
        if label == 'old-direct-crlf':
            passed = outcome == 'READY_TIMEOUT'
        elif label == 'old-relay-2048':
            passed = outcome == 'BEGIN_TIMEOUT' and dropped > 0 and restored and parent_alive and tty_restored
        elif label == 'old-relay-2048-intr-x':
            passed = outcome == 'BEGIN_TIMEOUT' and not restored and rescue_restored and parent_alive and tty_restored
        elif action and action.startswith('cancel'):
            passed = outcome == 'CANCELLED_RESTORED' and parent_alive and tty_restored
        elif action in ('truncate-line', 'mutate-line', 'invalid-final'):
            passed = outcome == ('INPUT_INVALID' if action == 'invalid-final' else 'INPUT_ACK_MISMATCH') and not received_begin and restored and parent_alive and tty_restored
        else:
            passed = outcome == 'PASS' and dropped == 0 and parent_alive and tty_restored
        result = {'case': label, 'protocol': version, 'newline': newline,
                  'bootstrapNewline': newline if version == 'old' else 'LF',
                  'simulatedRelayBurstCapacity': capacity, 'inheritedIntr': 'Ctrl-X' if custom_intr else 'Ctrl-C',
                  'commandBytes': len(raw), 'payloadDLines': len(lines), 'observedInputAcks': input_acks,
                  'outcome': outcome, 'lastPhase': phase, 'droppedInputBytes': dropped,
                  'beginReceived': received_begin, 'ctrlCSent': cancel_sent,
                  'restoredAfterProductionAction': restored, 'restoredAfterCorrectIntrRescue': rescue_restored,
                  'decodedProbeCorrect': decoded_ok, 'parentAlive': parent_alive, 'exactTtyRestored': tty_restored,
                  'pass': passed}
        results.append(result)
        print(json.dumps(result), flush=True)
        capture_console(terminal, case)
        terminal.close()

(RUN / 'result.json').write_text(json.dumps({'syntheticRelayOnly': True,
    'cancellationScope': 'V2 tests both immediately after execution-child BEGIN and an established command; restoration must finish within three seconds.',
    'coverage': {'realPosixTtyInput': True, 'stdoutCaptureBeforePtyRendering': True, 'productionRustDriver': False, 'productionTotalDeadline': False, 'customerBastion': False},
    'baselineProtocolSha256': hashlib.sha256(old_bytes).hexdigest(),
    'currentProtocolSha256': hashlib.sha256(current_bytes).hexdigest(),
    'probeSha256': hashlib.sha256(probe_bytes).hexdigest(), 'cases': results}, indent=2) + '\n')
print('Evidence:', RUN, flush=True)
raise SystemExit(0 if results and all(case['pass'] for case in results) else 1)
