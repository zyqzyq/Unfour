"""Exercise production curl preparation with real old/modern curl on loopback.

Requires a built terminal-curl-qa bridge and two explicitly supplied binaries.
No binary is downloaded or installed. This checks shell/curl HTTP compatibility,
not the SSH driver, PTY recovery, production total deadline, or customer gateway.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit
from curl_qa_source import curl_probe_command

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--old-curl', type=Path, required=True)
parser.add_argument('--modern-curl', type=Path, required=True)
parser.add_argument('--git', type=Path, default=Path(r'D:\Program Files\Git'))
parser.add_argument('--bridge', type=Path, default=ROOT / 'target/debug/examples/terminal-curl-qa.exe')
parser.add_argument('--output-dir', type=Path)
args = parser.parse_args()
RUN = (args.output_dir or ROOT / 'target/bastion-transport-review' / ('curl-compat-' + uuid.uuid4().hex[:10])).resolve()
RUN.mkdir(parents=True, exist_ok=False)
observed, proxy_requests = [], []


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
        observed.append({'path':urlsplit(self.path).path, 'method':self.command, 'body':body,
                         'authorization':self.headers.get('Authorization'),
                         'contentType':self.headers.get('Content-Type')})
        missing = self.path.startswith('/legacy') and not body
        response = b'{"error":"EOF at line 1 column 0"}' if missing else b'{"ok":true}'
        self.send_response(400 if missing else 200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    do_GET = do_POST


class ProxyHandler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        proxy_requests.append(self.path)
        self.send_response(502)
        self.send_header('Content-Length', '0')
        self.end_headers()

    do_GET = do_POST
    do_CONNECT = do_POST


def shell_path(path):
    value = path.resolve().as_posix()
    return '/' + value[0].lower() + value[2:]


def quote(value):
    return "'" + str(value).replace("'", "'\\''") + "'"


def execute(command, binary, directory, reject=False):
    directory.mkdir(parents=True, exist_ok=False)
    argv_log = directory / 'curl-argv.txt'
    wrapper = directory / 'curl'
    # The shim only records public argv. Request secrets remain in pipe/stdin.
    text = '#!/usr/bin/bash\n'
    text += "printf '%s\\n' CALL \"$@\" >> " + quote(shell_path(argv_log)) + '\n'
    if reject:
        text += 'for arg in "$@"; do [ "$arg" != --max-filesize ] || exit 2; done\n'
    text += 'exec ' + quote(shell_path(binary)) + ' "$@"\n'
    wrapper.write_text(text, encoding='utf-8', newline='\n')
    script = 'PATH=' + quote(shell_path(directory)) + ':"$PATH"; export PATH; hash -r\n' + command
    environment = os.environ.copy()
    for key in ('http_proxy','HTTP_PROXY','https_proxy','HTTPS_PROXY','all_proxy','ALL_PROXY'):
        environment[key] = f'http://127.0.0.1:{proxy.server_port}'
    environment['NO_PROXY'] = environment['no_proxy'] = ''
    process = subprocess.run([str(args.git / 'bin/bash.exe'),'--noprofile','--norc'],
                             input=script.encode(), capture_output=True, cwd=ROOT,
                             env=environment, timeout=35)
    (directory / 'stdout.bin').write_bytes(process.stdout)
    (directory / 'stderr.bin').write_bytes(process.stderr)
    calls = argv_log.read_text(encoding='utf-8').split('CALL\n')[1:]
    argv = [call.splitlines() for call in calls]
    assert all('UNFOUR_CURL_QA_SECRET' not in call and 'synthetic-model' not in call for call in calls)
    return process, argv


def prepare(name, scenario):
    directory = RUN / name
    url = f'http://127.0.0.1:{server.server_port}/{name}'
    process = subprocess.run([str(args.bridge),url,str(directory),scenario,'emit-only'],
                             capture_output=True, timeout=20, cwd=ROOT)
    assert process.returncode == 0, process.stderr.decode(errors='replace')
    return directory, (directory / 'command.sh').read_text(encoding='utf-8'), (directory / 'request-body.bin').read_bytes()


server = ThreadingHTTPServer(('127.0.0.1',0), Handler)
proxy = ThreadingHTTPServer(('127.0.0.1',0), ProxyHandler)
for service in (server, proxy):
    threading.Thread(target=service.serve_forever, daemon=True).start()
results = []
try:
    binaries = [('old',args.old_curl.resolve()),('modern',args.modern_curl.resolve())]
    versions = {label:subprocess.check_output([str(binary),'--version']).decode(errors='replace') for label,binary in binaries}
    assert versions['old'].startswith('curl 7.29.0 '), 'baseline requires actual curl 7.29.0'
    assert not versions['modern'].startswith('curl 7.29.0 '), 'modern comparison must differ'
    # Preserve the original bug as an executable historical regression, not a
    # simulation: the old parser ignores data-raw and --version still exits 0.
    legacy = ('silent\ninclude\nhttp1.1\nproxy = ""\nnoproxy = "*"\n'
              f'url = "http://127.0.0.1:{server.server_port}/legacy"\n'
              'request = "POST"\nheader = "Content-Type: application/json"\n'
              'data-raw = "{}"\nmax-time = 2\n')
    for version_only in (True,False):
        before = len(observed)
        process = subprocess.run([str(args.old_curl),'-q','--config','-',*(['--version'] if version_only else [])],
                                 input=legacy.encode(), capture_output=True, timeout=5)
        assert process.returncode == 0
        if version_only:
            assert len(observed) == before
        else:
            assert len(observed) == before+1 and observed[-1]['body'] == b''
            assert b'400 Bad Request' in process.stdout
        results.append({'case':'old-baseline-' + ('false-capability' if version_only else 'empty-post'),
                        'result':'REPRODUCED_BUG','exit':process.returncode,'httpRequests':len(observed)-before})
    for label,binary in binaries:
        # Point the fixed dummy probe URL at our observer. This is the only
        # change to the extracted production probe and detects accidental I/O.
        probe = curl_probe_command(ROOT).replace('http://127.0.0.1:1/',f'http://127.0.0.1:{server.server_port}/probe')
        before = len(observed)
        process, argv = execute(probe,binary,RUN / (label+'-probe'))
        assert process.returncode == 0 and process.stdout == b'UNFOUR_CURL_TOOLS'
        assert len(observed) == before, 'capability probe performed HTTP'
        assert all('--version' in call for call in argv), 'probe performed a transfer command'
        results.append({'case':label+'-probe','result':'PASS','httpRequests':0,'dummyUrlRedirectedForObservation':True})
        for scenario in ('json','text-crlf','empty','text-at'):
            name = label+'-'+scenario
            directory,command,expected = prepare(name,scenario)
            before = len(observed)
            process,argv = execute(command,binary,directory / 'shell')
            rejected = label == 'old' and scenario == 'text-at'
            if rejected:
                assert process.returncode == 124 and len(observed) == before
            else:
                assert process.returncode == 0, (name,process.returncode,process.stderr)
                assert len(observed) == before+1, 'request was retried, omitted or duplicated'
                assert observed[-1]['method'] == 'POST'
                assert observed[-1]['body'] == expected, 'request bytes changed'
                assert observed[-1]['authorization'] == 'Bearer UNFOUR_CURL_QA_SECRET'
                transfer = [call for call in argv if '--version' not in call]
                assert len(transfer) == 1 and '--config' in transfer[0]
                if scenario != 'text-at':
                    assert sum('--version' in call for call in argv) == 1, 'redundant curl preflight'
                assert ('--http1.1' in transfer[0]) == (label == 'modern')
                timeout = transfer[0][transfer[0].index('--max-time')+1]
                assert timeout == '20', 'production finite timeout missing'
                assert transfer[0][transfer[0].index('--proxy')+1] == ''
                assert transfer[0][transfer[0].index('--noproxy')+1] == '*'
            results.append({'case':name,'result':'EXPECTED_REJECTION' if rejected else 'PASS',
                            'exit':process.returncode,'httpRequests':len(observed)-before,
                            'expectedBytes':len(expected),'expectedSha256':hashlib.sha256(expected).hexdigest(),
                            'secretsAbsentFromArgv':True,'leadingAtHandledAsLiteralOrRejected':scenario=='text-at'})
        directory,command,_ = prepare(label+'-unsupported','json')
        before = len(observed)
        process,_ = execute(command,binary,directory / 'shell',reject=True)
        assert process.returncode == 125 and len(observed) == before
        results.append({'case':label+'-unsupported','result':'EXPECTED_REJECTION','exit':125,
                        'httpRequests':0,'fault':'shim rejects fixed --max-filesize before real curl'})
    assert not proxy_requests, 'environment proxy was used'
    result = {'result':'PASS','versions':versions,'cases':results,'proxyRequests':0,
              'binarySha256':{label:hashlib.sha256(binary.read_bytes()).hexdigest() for label,binary in binaries},
              'productionSourceSha256':{path:hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in (
                  'crates/unfour-core/src/terminal_curl_contract.rs',
                  'crates/http-engine/src/api_client/terminal_curl/request.rs',
                  'crates/ssh-engine/src/ssh/temporary/terminal_operation/probe.rs')},
              'coverage':{'productionHttpPreparation':True,'realCurlBinaries':True,
                          'realPosixShell':True,'productionResponseParser':False,
                          'realPty':False,'productionRustTransportDriver':False,
                          'productionTotalDeadline':False,'customerBastion':False},
              'note':'curl max-time presence/value verified from actual transfer argv; total deadline remains native-test coverage.'}
    (RUN/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(f'PASS: {len(results)} old/modern curl cases; exact bytes, no preflight HTTP, no proxy/retry: {RUN}')
finally:
    for service in (server,proxy):
        service.shutdown()
        service.server_close()
