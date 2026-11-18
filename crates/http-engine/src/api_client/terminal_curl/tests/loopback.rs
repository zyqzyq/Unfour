use super::*;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

// Run with the actual remote curl version on PATH (UNFOUR_QA_SH can select
// Git's POSIX shell on Windows). Both request construction and response
// parsing go through the production functions.
#[test]
#[ignore = "requires a POSIX shell, real curl on PATH and loopback networking"]
fn real_curl_chunked_trailers_and_large_resource_size() {
    let shell = std::env::var_os("UNFOUR_QA_SH").unwrap_or_else(|| "sh".into());
    let version = Command::new(&shell)
        .args(["-c", "curl -q --version"])
        .output()
        .unwrap();
    assert!(version.status.success());
    eprintln!(
        "{}",
        String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap()
    );

    let chunked = concat!(
        "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 103 Early Hints\r\nLink: </hint>\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
        "Transfer-Encoding: chunked\r\nTrailer: X-Result\r\n\r\n",
        "6\r\n{\"ok\":\r\n5\r\ntrue}\r\n0\r\nX-Result: ok\r\n\r\n"
    );
    let large = "HTTP/1.1 200 OK\r\nContent-Length: 20971520\r\n\r\n";
    for (method, reply, expected_exit) in
        [("GET", chunked, 0), ("HEAD", large, 0), ("GET", large, 63)]
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut request = input();
        request.url = format!("http://{}/", listener.local_addr().unwrap());
        request.method = method.into();
        let prepared = prepare_terminal_curl(request, &[]).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut lines = BufReader::new(&stream).lines();
            let request_line = lines.next().unwrap().unwrap();
            assert_eq!(request_line.split_whitespace().next(), Some(method));
            while !lines.next().unwrap().unwrap().is_empty() {}
            stream.write_all(reply.as_bytes()).unwrap();
            if method == "GET" && reply == large {
                // curl can close as soon as it sees the advertised length.
                let _ = io::copy(&mut io::repeat(b'x').take(20 * 1024 * 1024), &mut stream);
            }
        });
        let output = Command::new(&shell)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut child = output;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(prepared.request.shell_fixture().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{method}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        server.join().unwrap();
        let result = response::parse(&prepared, &output.stdout, expected_exit);
        if expected_exit == 63 {
            assert_eq!(
                code(result.err().unwrap()),
                "SSH_TERMINAL_HTTP_RESPONSE_TOO_LARGE"
            );
            continue;
        }
        let response = result.unwrap();
        assert_eq!(response.status, 200);
        if method == "HEAD" {
            assert!(response.body.is_empty());
            assert!(response
                .headers
                .iter()
                .any(|field| field.key == "content-length" && field.value == "20971520"));
        } else {
            assert_eq!(response.body, r#"{"ok":true}"#);
            assert_eq!(response.headers.len(), 3);
        }
    }
}
