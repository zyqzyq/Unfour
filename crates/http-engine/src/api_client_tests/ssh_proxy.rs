use super::{execution::request, support::service};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn read_string(stream: &mut TcpStream) -> Vec<u8> {
    let mut length = [0];
    stream.read_exact(&mut length).unwrap();
    let mut bytes = vec![0; length[0] as usize];
    stream.read_exact(&mut bytes).unwrap();
    bytes
}

fn accept_socks(listener: &TcpListener, host: &str, port: u16) -> TcpStream {
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut version = [0];
    stream.read_exact(&mut version).unwrap();
    assert_eq!(version, [5]);
    assert!(read_string(&mut stream).contains(&2));
    stream.write_all(&[5, 2]).unwrap();
    stream.read_exact(&mut version).unwrap();
    assert_eq!(version, [1]);
    assert_eq!(read_string(&mut stream), b"lease");
    assert_eq!(read_string(&mut stream), b"private-token");
    stream.write_all(&[1, 0]).unwrap();
    let mut connect = [0; 4];
    stream.read_exact(&mut connect).unwrap();
    // Remote DNS: the wire contains the destination hostname, including on redirect.
    assert_eq!(connect, [5, 1, 0, 3]);
    assert_eq!(read_string(&mut stream), host.as_bytes());
    let mut network_port = [0; 2];
    stream.read_exact(&mut network_port).unwrap();
    assert_eq!(u16::from_be_bytes(network_port), port);
    stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).unwrap();
    stream
}

#[tokio::test]
async fn ssh_proxy_authenticates_on_both_hops_and_preserves_original_http_hosts() {
    let service = service().await;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for (host, reply) in [
            ("orders.invalid", "HTTP/1.1 302 Found\r\nLocation: http://status.invalid/check\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
            ("status.invalid", "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"),
        ] {
            let mut stream = accept_socks(&listener, host, 80);
            let lines: Vec<String> = BufReader::new(&mut stream).lines()
                .map(Result::unwrap).take_while(|line| !line.is_empty()).collect();
            assert!(lines.iter().any(|line| line.eq_ignore_ascii_case(&format!("host: {host}"))));
            assert!(!lines.iter().any(|line| line.to_lowercase().starts_with("proxy-authorization:")));
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
    let response = service
        .send_via_ssh_proxy(
            request("http://orders.invalid/start", Some(3000)),
            CancellationToken::new(),
            &[],
            address,
            "lease",
            "private-token",
            std::time::Instant::now(),
        )
        .await
        .unwrap();
    assert_eq!(response.body, "OK");
    server.join().unwrap();
}

#[tokio::test]
async fn https_starts_tls_for_the_original_hostname_after_socks_connect() {
    let service = service().await;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let mut stream = accept_socks(&listener, "api.internal.invalid", 443);
        // TLS ClientHello has an SNI extension for the original host. The loopback
        // endpoint and SOCKS peer are transport details, never certificate identities.
        let mut record = [0; 5];
        stream.read_exact(&mut record).unwrap();
        assert_eq!(record[0], 0x16);
        let mut hello = vec![0; u16::from_be_bytes([record[3], record[4]]) as usize];
        stream.read_exact(&mut hello).unwrap();
        let name = b"api.internal.invalid";
        assert!(hello.windows(name.len()).any(|part| part == name));
    });
    let result = service
        .send_via_ssh_proxy(
            request("https://api.internal.invalid/", Some(3000)),
            CancellationToken::new(),
            &[],
            address,
            "lease",
            "private-token",
            std::time::Instant::now(),
        )
        .await;
    assert!(
        result.is_err(),
        "the fake peer supplies no server certificate"
    );
    peer.join().unwrap();
}

#[tokio::test]
async fn exhausted_setup_budget_stops_before_contacting_the_proxy() {
    let service = service().await;
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    proxy.set_nonblocking(true).unwrap();
    let result = service
        .send_via_ssh_proxy(
            request("https://api.internal.invalid/", Some(1)),
            CancellationToken::new(),
            &[],
            proxy.local_addr().unwrap(),
            "lease",
            "private-token",
            std::time::Instant::now() - Duration::from_secs(1),
        )
        .await
        .unwrap_err();
    assert_eq!(result.code(), "SSH_TUNNEL_TIMEOUT");
    assert_eq!(
        proxy.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn disconnected_ssh_proxy_does_not_fallback_even_for_a_loopback_destination() {
    let service = service().await;
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let dead_proxy = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let result = service
        .send_via_ssh_proxy(
            request(
                &format!("http://{}/private", target.local_addr().unwrap()),
                Some(2000),
            ),
            CancellationToken::new(),
            &[],
            dead_proxy,
            "lease",
            "private-token",
            std::time::Instant::now(),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
