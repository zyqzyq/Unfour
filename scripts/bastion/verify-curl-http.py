"""Production curl command + real PTY input/shell + loopback HTTP.

The Python harness captures shell stdout before tty rendering and bypasses the
Rust transport driver. Its timeout/SSH framing need separate native coverage.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--output-dir', type=Path)
parser.add_argument('--curl-bin', type=Path, help='Select an existing curl binary only inside the synthetic remote command')
parser.add_argument('--case', action='append', choices=['small', 'json', 'error', 'chunked'])
args = parser.parse_args()
RUN = args.output_dir or ROOT / 'target/api-terminal-curl/qa' / ('http-pty-' + uuid.uuid4().hex[:10])
RUN.mkdir(parents=True, exist_ok=False)
observed = []
cases = args.case or ['json', 'error', 'chunked']
assert len(cases) == len(set(cases)), 'duplicate case selection'
curl_binary = (args.curl_bin or Path(r'D:\Program Files\Git\mingw64\bin\curl.exe')).resolve()
curl_version = subprocess.check_output([str(curl_binary), '--version']).decode(errors='replace')


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        size = int(self.headers['Content-Length'])
        small = urlsplit(self.path).path == '/small'
        request = json.loads(self.rfile.read(size))
        if small:
            assert size == 130 and request == {'input': 'x' * 118}
        else:
            assert 70000 < size < 80000
            assert request['padding'] == 'x' * 70000
            assert request['input'] == ['北京是中国的首都。', "single ' quote $(false) `false` \n next line"]
        assert self.headers['Authorization'] == 'Bearer UNFOUR_CURL_QA_SECRET'
        assert self.headers['Content-Type'] == 'application/json'
        route = urlsplit(self.path)
        assert parse_qs(route.query)['q'] == ["中文 ' $() &"]
        assert route.path in ['/small', '/json', '/error', '/chunked']
        observed.append({'path': route.path, 'requestBytes': size, 'bodyAndAuthExact': True})
        body = json.dumps({'input': request['input'], 'values': [0.125] * 17000,
                           'echo': 'UNFOUR_CURL_QA_SECRET'}, ensure_ascii=False).encode()
        if small:
            body = b'{"value":"' + b'x' * 85 + b'"}'
            assert len(body) == 97
            time.sleep(0.2)
        self.send_response(500 if route.path == '/error' else 200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('X-Repeat', 'one')
        self.send_header('X-Repeat', 'two')
        if route.path == '/chunked':
            self.send_header('Transfer-Encoding', 'chunked')
        else:
            self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        if route.path == '/chunked':
            for offset in range(0, len(body), 4093):
                chunk = body[offset:offset + 4093]
                self.wfile.write(f'{len(chunk):x}\r\n'.encode() + chunk + b'\r\n')
            self.wfile.write(b'0\r\n\r\n')
        else:
            self.wfile.write(body)


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
results = []
try:
    for case in cases:
        directory = RUN / case
        directory.mkdir()
        with (directory / 'bridge.log').open('w', encoding='utf-8') as log:
            process = subprocess.Popen([str(ROOT / 'target/debug/examples/terminal-curl-qa.exe'),
                f'http://127.0.0.1:{server.server_port}/{case}', str(directory), 'small-json' if case == 'small' else 'json'], cwd=ROOT,
                stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                until = time.monotonic() + 20
                while not (directory / 'command.sh').exists():
                    assert process.poll() is None, 'production preparation failed: see bridge.log'
                    assert time.monotonic() < until, 'production command not written'
                    time.sleep(.05)
                original_command = (directory / 'command.sh').read_bytes()
                command_file = directory / 'command.sh'
                if args.curl_bin:
                    # Preserve production command.sh. Select curl only inside
                    # the synthetic operation, with no installation/PATH edit.
                    parent = curl_binary.parent.as_posix()
                    parent = '/' + parent[0].lower() + parent[2:]
                    quoted = "'" + parent.replace("'", "'\\''") + "'"
                    prefix = 'PATH=' + quoted + ':"$PATH"; export PATH; hash -r;\n'
                    command_file = directory / 'pty-command.sh'
                    command_file.write_bytes(prefix.encode() + original_command)
                check = subprocess.run([sys.executable, str(ROOT / 'scripts/bastion/verify-curl-terminal-pty.py'),
                    '--only-command', '--command-file', str(command_file),
                    '--output-dir', str(directory / 'pty')], cwd=ROOT,
                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=120,
                    creationflags=subprocess.CREATE_NO_WINDOW)
                (directory / 'pty.log').write_bytes(check.stdout)
                assert check.returncode == 0, f'{case} real PTY failed: see pty.log'
                assert process.wait(timeout=30) == 0, f'{case} real response parsing failed: see bridge.log'
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
        result = json.loads((directory / 'result.json').read_text())
        assert result['result'] == 'PASS' and result['bodyIsJson']
        assert not result['historyContainsSyntheticSecret']
        if case == 'small':
            assert result['bodyBytes'] == 97
            assert 190 <= result['curlStartTransferMs'] <= result['curlTotalMs']
        else:
            assert result['bodyBytes'] > 65536
        assert result['status'] == (500 if case == 'error' else 200)
        results.append({'case': case, 'productionCommandSha256':hashlib.sha256(original_command).hexdigest(), **result})
    assert len(observed) == len(cases), 'a request was retried or missed'
    (RUN / 'result.json').write_text(json.dumps({'result': 'PASS', 'cases': results,
        'curlVersion':curl_version, 'curlBinarySha256':hashlib.sha256(curl_binary.read_bytes()).hexdigest(),
        'qaPathPrefixOnly':bool(args.curl_bin),
        'requests': observed, 'remoteHost': 'local Git Bash POSIX PTY; customer bastion unverified',
        'coverage': {'productionHttpPreparationAndParsing': True, 'realPosixTtyInput': True,
            'stdoutCaptureBeforePtyRendering': True, 'productionRustTransportDriver': False,
            'productionTotalDeadline': False, 'customerBastion': False}}, indent=2), encoding='utf-8')
    print(f'PASS: {len(cases)} production curl commands ({", ".join(cases)}), real PTY, 70KB JSON POST, >64KB responses, exact request and history redaction: {RUN}')
finally:
    server.shutdown()
    server.server_close()
