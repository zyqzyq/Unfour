"""Real PTY, curl and loopback HTTP. Model RTT with independent FIFO delivery clocks.

Use --legacy-bridge for the before measurement. --scripts comes from the Rust
emit_curl_bootstraps test. Only fixed synthetic fixture values are recorded.
"""
import argparse
import base64
import hashlib
import json
import queue
import re
import subprocess
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from curl_qa_pty import Terminal, content, frame, quoted, wait
from curl_qa_source import rust_string_constant

ROOT = Path(__file__).resolve().parents[2]
GIT = Path(r'D:\Program Files\Git')
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--scripts', type=Path, required=True)
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--legacy-bridge', type=Path)
parser.add_argument('--curl', type=Path, default=GIT/'mingw64/bin/curl.exe')
parser.add_argument('--rtt-ms', type=int, default=0)
parser.add_argument('--samples', type=int, default=3)
parser.add_argument('--relay-cap', type=int, help="Inject a post-launch burst cutoff for confirmed-path QA")
parser.add_argument('--no-digest', action="store_true")
parser.add_argument('--matrix', action='store_true')
args = parser.parse_args()
run = args.output_dir.resolve()
run.mkdir(parents=True, exist_ok=False)
observed = []
response = b'{"value":"' + b'x' * 85 + b'"}'
large_response = b'{"value":"' + b'x' * 119123 + b'"}'
active_cancel = threading.Event()
version = subprocess.check_output([str(args.curl), '--version']).decode().splitlines()[0]
legacy = bool(args.legacy_bridge)
bridge = args.legacy_bridge or ROOT/'target/debug/examples/terminal-curl-qa.exe'


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        observed.append((self.command, body, self.headers['Authorization'], self.path))
        time.sleep(.8 if active_cancel.is_set() else .2)
        reply = large_response if len(body) > 70000 else response
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(reply)))
        self.end_headers()
        try:
            self.wfile.write(reply)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            if not active_cancel.is_set():
                raise

    do_GET = do_POST


class Link:
    """Both directions deliver at source arrival + delay, independent of write count."""
    def __init__(self, terminal, capture):
        self.capture = capture
        self.terminal = terminal
        self.half = args.rtt_ms / 2000
        self.outgoing = queue.Queue()
        self.stop = threading.Event()
        self.received = bytearray()
        self.workers = [threading.Thread(target=f, daemon=True) for f in (self.writer, self.reader)]
        for worker in self.workers:
            worker.start()

    def writer(self):
        while True:
            packet = self.outgoing.get()
            if packet is None:
                return
            due, data = packet
            if not self.stop.wait(max(0, due-time.monotonic())):
                self.terminal.write(data)

    def reader(self):
        from collections import deque
        pending = deque()
        offset = 0
        while not self.stop.is_set():
            if self.capture.exists():
                with self.capture.open('rb') as f:
                    f.seek(offset)
                    chunk = f.read()
                if chunk:
                    offset += len(chunk)
                    pending.append((time.monotonic()+self.half, chunk))
            while pending and pending[0][0] <= time.monotonic():
                self.received.extend(pending.popleft()[1])
            self.stop.wait(.002)

    def send(self, data):
        for i in range(0, len(data), 512):
            self.outgoing.put((time.monotonic()+self.half, data[i:i+512]))

    def expect(self, marker, timeout=6):
        end = time.monotonic() + timeout
        while marker not in self.received:
            if time.monotonic() >= end:
                raise AssertionError(f'missing protocol event {marker[:46]!r} in {self.capture}')
            time.sleep(.002)

    def close(self):
        self.stop.set()
        self.outgoing.put(None)
        for worker in self.workers:
            worker.join()


def transfer(terminal, case, sid, payload, context=None, probe=False, fault=None, raw=False):
    link = Link(terminal, case/'protocol')
    count = len(observed)
    start = time.monotonic()
    def ms(started):
        return round((time.monotonic()-started)*1000)
    try:
        if legacy:
            source = (ROOT/'crates/ssh-engine/src/ssh/temporary/terminal_operation/protocol.rs').read_text()
            script = rust_string_constant(source, 'BOOTSTRAP')
        else:
            name = 'probe' if probe else 'raw' if raw else 'burst' if context and len(payload) <= 720 else 'confirmed'
            script = (args.scripts/name).read_text().replace('0'*32+'\n', sid+'\n')
        if legacy:
            link.send((script.rstrip('\n') + ' > ' + quoted(link.capture) + ' 2>/dev/null\n').encode())
        else:
            setup, fixed = script.splitlines()
            if name == 'burst':
                previous = hashlib.sha256(fixed.encode()).hexdigest()
                fixed = fixed.replace('a'*64, context)
                setup = setup.replace(previous, hashlib.sha256(fixed.encode()).hexdigest())
            # Only redirect the bootstrap process. The second line is fixed receiver code.
            link.send((setup + ' > ' + quoted(link.capture) + ' 2>/dev/null\n').encode())
            link.expect(frame(sid+':LOAD'))
            burst_code = ('+\n'+fixed+'\n').encode()
            link.send(burst_code[:args.relay_cap])
            link.expect(frame(sid+':PULSE'))
            link.send(b'\n')
            if args.relay_cap and len(burst_code) > args.relay_cap:
                link.expect(frame(sid+':SHORT'))
                data = fixed.encode()
                for i in range(0,len(data),240):
                    part = data[i:i+240]
                    link.send(part+b'\n')
                    link.expect(frame(sid+f':L:{i+len(part)}'))
            if args.no_digest:
                link.expect(frame(sid+':VERIFY'))
                verified = bytes(link.received).split(frame(sid+':CHECK'))[1].split(frame(sid+':VERIFY'))[0]
                assert verified == fixed.encode()
                link.send(f'OK:{sid}\n'.encode())
        nonce = sid
        if legacy:
            link.expect(frame('UNFOUR_RAW_V3'))
            link.send((sid+'\n').encode())
            link.expect(frame(sid+':READY:3:240:b64'))
            nonce = uuid.uuid4().hex
            link.send((nonce+'\n').encode())
            link.expect(frame(nonce+':ACCEPT'))
            burst = False
        elif raw and version.startswith('curl 7.29.'):
            link.expect(frame(sid+':FAIL:124'))
            link.send(b'\x03')
            link.expect(frame(sid+':RESTORED'))
            return {'failure': 'FAIL:124', 'http': 0}, None, b''
        else:
            link.expect((sid+':READY:4:').encode())
            wait(lambda: re.search(rb':READY:4:b64:([0-9a-f]{64}|-):([BD])\x1f', link.received), 'complete READY')
            match = re.search(rb':READY:4:b64:([0-9a-f]{64}|-):([BD])\x1f', link.received)
            context, burst = match[1].decode(), match[2] == b'B'
            assert burst == (name in ('probe', 'burst') and not args.relay_cap and not args.no_digest), (name, context, burst)
        prepared = ms(start)
        lines = [base64.b64encode(payload[i:i+180]) for i in range(0, len(payload), 180)]
        total = f'{len(lines)}:{sum(map(len,lines))}'
        end = ('E:'+total+'\n').encode()
        ack_wait = 0
        input_start = time.monotonic()
        if burst:
            digest = hashlib.sha256(b''.join(lines)).hexdigest()
            end = f'F:{total}:{digest}\n'.encode()
            if fault == 'reorder':
                lines[0], lines[1] = lines[1], lines[0]
            elif fault == 'alter':
                lines[0] = (b'B' if lines[0][0:1] == b'A' else b'A') + lines[0][1:]
            elif fault == 'digest':
                end = f'F:{total}:{"0"*64}\n'.encode()
            elif fault == 'truncate':
                lines[-1] = lines[-1][:-4]
            wire = b''.join(b'B'+line+b'\n' for line in lines) + end
            link.send(wire[:args.relay_cap])
            size = len(wire)
            if fault and fault != 'cancel-started':
                link.expect(frame(sid+':INPUT_INVALID'))
                link.send(b'\x03')
                link.expect(frame(sid+':RESTORED'), 3)
                assert frame(sid+':BEGIN') not in link.received
                return {'failure': fault, 'http': 0}, None, b''
        else:
            for i, line in enumerate(lines, 1):
                short_line = b'D'+line+b'\n'
                assert not args.relay_cap or len(short_line) <= args.relay_cap
                link.send(short_line)
                waiting = time.monotonic()
                link.expect(frame(nonce+f':INPUT:{i}:{len(line)}:'+line.decode()))
                ack_wait += ms(waiting)
            size = sum(len(part)+2 for part in lines)+len(end)
            link.send(end)
        waiting = time.monotonic()
        link.expect(frame(nonce+':INPUT_END:'+total))
        ack_wait += ms(waiting)
        input_ms = ms(input_start)
        execute = time.monotonic()
        if fault == 'cancel-started':
            link.expect(frame(sid+':BEGIN'))
            wait(lambda: len(observed) == count + 1, 'HTTP started before cancellation')
            link.send(b'\x03')
            link.expect(frame(sid+':RESTORED'), 3)
            return {'failure': fault, 'http': 1}, None, b''
        link.expect(frame(nonce+':END:0'))
        encoded = bytes(link.received).split(frame(nonce+':BEGIN'))[1].split(frame(nonce+':END:0'))[0]
        stdout = base64.b64decode(encoded)
        status = frame(nonce+':STATUS:0')
        assert stdout.endswith(status)
        stdout = stdout[:-len(status)]
        complete, execution_ms = ms(start), ms(execute)
        link.send(b'Q\n' if legacy else f'Q:{sid}\n'.encode())
        link.expect(frame(sid+':RESTORED'), 3)
        released = ms(start)
        return dict(prepare=prepared, input=input_ms, ackCount=1 if burst else len(lines)+1,
                    ackWait=ack_wait, executionOutput=execution_ms, complete=complete, release=released,
                    restore=released-complete, frameBytes=size, path='burst' if burst else 'confirmed'), context, stdout
    finally:
        link.close()


server = ThreadingHTTPServer(('127.0.0.1',0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
samples = []
terminal = Terminal(run/'history', GIT, ROOT)
try:
    time.sleep(.5)
    before, after = run/'before', run/'after'
    tool_path = ''
    if args.no_digest:
        shim = run/'sha256sum'
        shim.write_text('#!/bin/sh\nexit 127\n', newline='\n')
        tool_path = quoted(run)+':'
    terminal.write((f'export PATH={tool_path}{quoted(args.curl.resolve().parent)}:/usr/bin:/bin; stty -g > {quoted(before)}\n').encode())
    wait(lambda: content(before), 'POSIX tty')
    def parent_check():
        after.unlink(missing_ok=True)
        terminal.write((f'history -w; stty -g > {quoted(after)}\n').encode())
        wait(lambda: content(before) == content(after), 'parent command and exact stty restore')
    context = None
    cases = [('small-json', False, None)] * args.samples
    if not legacy:
        cases += [('probe', False, None)]
        if not args.relay_cap and not args.no_digest:
            cases += [('small-json', True, None)] * args.samples
    if args.matrix:
        cases += [(case, True, None) for case in ('get','long-header','text-crlf','text-at')]
        cases += [('small-json', True, fault) for fault in ('truncate','alter','reorder','digest')]
        cases += [('json', False, None), ('small-json', True, 'cancel-started')]
    for index, (scenario, use_cache, fault) in enumerate(cases):
        directory = run/f'{index}-{scenario}'
        directory.mkdir()
        count = len(observed)
        if scenario == 'probe':
            marker = bytes(65+(i+i//83)%26 for i in range(720))
            timings, context, echoed = transfer(terminal, directory, uuid.uuid4().hex, marker, probe=True)
            assert echoed == marker and len(observed) == count
            if args.no_digest or args.relay_cap:
                assert timings['path'] == 'confirmed'
                context = None
            else:
                assert timings['frameBytes'] == 1041 and timings['path'] == 'burst'
            continue
        url = f'http://127.0.0.1:{server.server_port}/qa'
        # For before/after comparisons use the same bridge entry and URL shape.
        process = subprocess.Popen([str(bridge), url, str(directory), scenario],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            wait(lambda: (directory/'command.sh').exists(), 'HTTP bridge preparation')
            if legacy:
                payload = (directory/'command.sh').read_bytes()
                sid = uuid.uuid4().hex
            else:
                sid = (directory/'nonce.txt').read_text()
                payload = (directory/'config.txt').read_bytes()
            if fault == 'cancel-started':
                active_cancel.set()
            result, _, stdout = transfer(terminal, directory, sid, payload,
                                         context if use_cache else None, fault=fault, raw=scenario=='text-at')
            if 'failure' in result:
                assert count + result['http'] == len(observed)
                parent_check()
            else:
                assert len(observed) == count+1
                method, body, auth, path = observed[-1]
                assert body == (directory/'request-body.bin').read_bytes()
                assert method == ('GET' if scenario in ('get','long-header') else 'POST')
                assert auth == 'Bearer UNFOUR_CURL_QA_SECRET' + ('x'*1200 if scenario=='long-header' else '')
                reply = large_response if scenario == 'json' else response
                assert '%E4%B8%AD' in path and reply in stdout
                (directory/'response.bin').write_bytes(stdout)
                (directory/'exit-code.txt').write_text('0')
                assert process.wait(timeout=10) == 0, process.stderr.read()
                parsed = json.loads((directory/'result.json').read_text())
                assert parsed['bodyBytes']==len(reply) and parsed['status']==200
                result.update(curlMs=parsed['curlTotalMs'], http=1)
            samples.append(dict(case=scenario, rtt=args.rtt_ms, **result))
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait()
    parent_check()
    assert b'UNFOUR_CURL_QA_SECRET' not in content(run/'history')
    print(json.dumps(samples, indent=2))
    (run/'samples.json').write_text(json.dumps({'curl':version, 'samples':samples}, indent=2))
finally:
    terminal.close()
    server.shutdown()
    server.server_close()

